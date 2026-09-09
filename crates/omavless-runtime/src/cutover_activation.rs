// SPDX-License-Identifier: MIT

//! Explicit, disconnected-only package activation. No IPC counterpart, caller
//! paths, shell input, live adoption, startup conversion or recovery bypass.

use crate::cutover_transaction::{CutoverTransactionError, CutoverTransactionOutcome};
use crate::frontend_bridge::FixedFrontendBridge;
use crate::production_cutover::ProductionCutoverHost;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::MetadataExt;

const BINARY: &str = "/usr/bin/omavless";
const UNIT: &str = "/usr/lib/systemd/user/omavless-runtime.service";
const UNIT_BYTES: &[u8] = include_bytes!("../../../packaging/systemd/omavless-runtime.service");

pub fn is_activation(arguments: &[OsString]) -> bool {
    arguments == ["cutover", "activate"]
}

pub(crate) fn check_service_installation(text: &str, native: bool) -> Result<(), ()> {
    let value = |key: &str| -> Result<&str, ()> {
        let mut values = text
            .lines()
            .filter_map(|line| line.split_once('='))
            .filter(|(name, _)| *name == key)
            .map(|(_, value)| value);
        let first = values.next().ok_or(())?;
        if values.next().is_some() {
            return Err(());
        }
        Ok(first)
    };
    if value("UnitFileState")? != "disabled"
        || value("NeedDaemonReload")? != "no"
        || !value("DropInPaths")?.is_empty()
        || (native && value("FragmentPath")? != UNIT)
    {
        return Err(());
    }
    Ok(())
}

fn packaged_identity() -> Result<(), ()> {
    // Overrides cannot make the coordinator and systemd daemon read different
    // homes or state/runtime roots. The installed user-manager environment must
    // be accepted separately; this command takes no environment mutation action.
    if std::env::var_os("OMAVLESS_HOME").is_some() {
        return Err(());
    }
    let binary = fs::symlink_metadata(BINARY).map_err(|_| ())?;
    let running = fs::metadata("/proc/self/exe").map_err(|_| ())?;
    if !binary.is_file()
        || binary.uid() != 0
        || binary.mode() & 0o022 != 0
        || binary.dev() != running.dev()
        || binary.ino() != running.ino()
    {
        return Err(());
    }
    let unit = fs::symlink_metadata(UNIT).map_err(|_| ())?;
    if !unit.is_file()
        || unit.uid() != 0
        || unit.mode() & 0o022 != 0
        || unit.len() != UNIT_BYTES.len() as u64
        || fs::read(UNIT).map_err(|_| ())? != UNIT_BYTES
    {
        return Err(());
    }
    Ok(())
}

/// Invoke only after exact CLI argument validation. Failure before the accepted
/// transaction has no service/ownership effects. Crash recovery remains the
/// durable preparing marker's explicit manual-recovery contract.
pub fn activate() -> Result<CutoverTransactionOutcome, CutoverTransactionError> {
    let rejected = CutoverTransactionError::PreconditionsFailed;
    packaged_identity().map_err(|_| rejected)?;
    let bridge = FixedFrontendBridge::current().map_err(|_| rejected)?;
    let mut host = ProductionCutoverHost::current(bridge).map_err(|_| rejected)?;
    host.activate_disconnected()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_command_and_fixed_installation_facts() {
        assert!(is_activation(&["cutover".into(), "activate".into()]));
        for args in [
            vec![],
            vec!["cutover"],
            vec!["cutover", "activate", "--force"],
            vec!["cutover", "rollback"],
        ] {
            assert!(!is_activation(
                &args.into_iter().map(OsString::from).collect::<Vec<_>>()
            ));
        }
        let valid = format!(
            "UnitFileState=disabled\nNeedDaemonReload=no\nDropInPaths=\nFragmentPath={UNIT}\n"
        );
        assert!(check_service_installation(&valid, true).is_ok());
        for invalid in [
            valid.replace("disabled", "enabled"),
            valid.replace("disabled", "static"),
            valid.replace("Reload=no", "Reload=yes"),
            valid.replace("DropInPaths=", "DropInPaths=/tmp/override"),
            valid.replace(UNIT, "/tmp/unit"),
            format!("{valid}UnitFileState=disabled\n"),
        ] {
            assert!(check_service_installation(&invalid, true).is_err());
        }
    }
}
