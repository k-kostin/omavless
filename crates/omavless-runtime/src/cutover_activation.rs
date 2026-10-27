// SPDX-License-Identifier: MIT

//! Explicit, disconnected-only package activation. No IPC counterpart, caller
//! paths, shell input, live adoption, startup conversion or recovery bypass.

use crate::cutover_transaction::{CutoverTransactionError, CutoverTransactionOutcome};
use crate::frontend_bridge::FixedFrontendBridge;
use crate::production_cutover::ProductionCutoverHost;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::MetadataExt;

pub(crate) mod environment;

const BINARY: &str = "/usr/bin/omavless";
const UNIT: &str = "/usr/lib/systemd/user/omavless-runtime.service";
const UNIT_BYTES: &[u8] = include_bytes!("../../../packaging/systemd/omavless-runtime.service");
const PRODUCT_UNIT_BYTES: &[u8] =
    include_bytes!("../../../packaging/systemd/omavless-runtime-product.service");

const ACCEPTANCE_BEFORE_UNIT_BYTES: &[u8] =
    include_bytes!("../../../packaging/systemd/omavless-runtime-acceptance-before-attempt.service");
const ACCEPTANCE_FINAL_UNIT_BYTES: &[u8] =
    include_bytes!("../../../packaging/systemd/omavless-runtime-acceptance-final-proof.service");

const ACCEPTANCE_BETWEEN_UNIT_BYTES: &[u8] = include_bytes!(
    "../../../packaging/systemd/omavless-runtime-acceptance-between-members.service"
);

fn packaged_unit_matches(bytes: &[u8]) -> bool {
    bytes == UNIT_BYTES
        || (cfg!(feature = "product-image-witness") && bytes == PRODUCT_UNIT_BYTES)
        || (cfg!(feature = "acceptance-connection-close")
            && !crate::product_scope::backup_only()
            && (bytes == ACCEPTANCE_BEFORE_UNIT_BYTES
                || bytes == ACCEPTANCE_FINAL_UNIT_BYTES
                || bytes == ACCEPTANCE_BETWEEN_UNIT_BYTES))
}

pub fn is_activation(arguments: &[OsString]) -> bool {
    arguments == ["cutover", "activate"]
}

fn value<'a>(text: &'a str, key: &str) -> Result<&'a str, ()> {
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
}

/// Absence is affirmative systemd evidence, never a failed/empty query.
/// Callers may use this exception only for the fixed legacy unit.
pub(crate) fn legacy_unit_absent(text: &str) -> Result<bool, ()> {
    match value(text, "LoadState")? {
        "loaded" => Ok(false),
        "not-found" => {
            for (key, expected) in [
                ("UnitFileState", ""),
                ("FragmentPath", ""),
                ("DropInPaths", ""),
                ("NeedDaemonReload", "no"),
                ("ActiveState", "inactive"),
                ("MainPID", "0"),
                ("ExecMainStatus", "0"),
                ("Result", "success"),
            ] {
                if value(text, key)? != expected {
                    return Err(());
                }
            }
            Ok(true)
        }
        _ => Err(()),
    }
}

pub(crate) fn check_service_installation(text: &str, native: bool) -> Result<(), ()> {
    if !native && legacy_unit_absent(text)? {
        return Ok(());
    }
    if value(text, "LoadState")? != "loaded"
        || value(text, "UnitFileState")? != "disabled"
        || value(text, "NeedDaemonReload")? != "no"
        || !value(text, "DropInPaths")?.is_empty()
        || (native && value(text, "FragmentPath")? != UNIT)
    {
        return Err(());
    }
    Ok(())
}

pub(crate) fn packaged_identity() -> Result<(), ()> {
    // Test-only home overrides are never an installed activation input.
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
        || unit.len()
            > UNIT_BYTES
                .len()
                .max(PRODUCT_UNIT_BYTES.len())
                .max(ACCEPTANCE_BEFORE_UNIT_BYTES.len())
                .max(ACCEPTANCE_FINAL_UNIT_BYTES.len())
                .max(ACCEPTANCE_BETWEEN_UNIT_BYTES.len()) as u64
        || !packaged_unit_matches(&fs::read(UNIT).map_err(|_| ())?)
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
    environment::with_current(|| {
        let bridge = FixedFrontendBridge::current().map_err(|_| rejected)?;
        let mut host = ProductionCutoverHost::current(bridge).map_err(|_| rejected)?;
        host.activate_disconnected()
    })
    .map_err(|_| rejected)?
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(crate) const ABSENT: &str = "LoadState=not-found\nUnitFileState=\nFragmentPath=\nDropInPaths=\nNeedDaemonReload=no\nActiveState=inactive\nMainPID=0\nExecMainStatus=0\nResult=success\n";

    #[test]
    fn fixed_packaged_units_are_compile_appropriate_and_byte_exact() {
        assert!(packaged_unit_matches(UNIT_BYTES));
        assert_eq!(
            packaged_unit_matches(PRODUCT_UNIT_BYTES),
            cfg!(feature = "product-image-witness")
        );
        for acceptance in [
            ACCEPTANCE_BEFORE_UNIT_BYTES,
            ACCEPTANCE_FINAL_UNIT_BYTES,
            ACCEPTANCE_BETWEEN_UNIT_BYTES,
        ] {
            assert_eq!(
                packaged_unit_matches(acceptance),
                cfg!(feature = "acceptance-connection-close")
                    && !crate::product_scope::backup_only()
            );
        }
        for source in [
            UNIT_BYTES,
            PRODUCT_UNIT_BYTES,
            ACCEPTANCE_BEFORE_UNIT_BYTES,
            ACCEPTANCE_FINAL_UNIT_BYTES,
            ACCEPTANCE_BETWEEN_UNIT_BYTES,
        ] {
            let text = std::str::from_utf8(source).unwrap();
            for changed in [
                text.replace("daemon", "daem0n"),
                text.replace("daemon", "daemon --force"),
                text.replace("Restart=no", "Restart=on-failure"),
                text.replace("Restart=on-failure", "Restart=always"),
                text.replace("NoNewPrivileges=no", "NoNewPrivileges=yes"),
                format!("{text}\n# override\n"),
            ] {
                if changed.as_bytes() != source {
                    assert!(!packaged_unit_matches(changed.as_bytes()));
                }
            }
            assert!(!packaged_unit_matches(&source[..source.len() - 1]));
        }
        assert!(!packaged_unit_matches(&[]));
    }

    #[test]
    fn only_complete_inactive_absent_legacy_facts_are_accepted() {
        assert_eq!(legacy_unit_absent(ABSENT), Ok(true));
        assert!(check_service_installation(ABSENT, false).is_ok());
        assert!(check_service_installation(ABSENT, true).is_err());
        for line in ABSENT.lines() {
            let key = line.split_once('=').unwrap().0;
            for invalid in [
                ABSENT.replace(&format!("{line}\n"), ""),
                format!("{ABSENT}{line}\n"),
                ABSENT.replace(line, &format!("{key}=unexpected-private-value")),
            ] {
                assert!(
                    check_service_installation(&invalid, false).is_err(),
                    "{key}"
                );
            }
        }
        for invalid in [
            "",
            "LoadState=masked\n",
            "LoadState=error\n",
            "LoadState=not-found\nActiveState=inactive\n",
        ] {
            assert!(check_service_installation(invalid, false).is_err());
        }
    }
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
            "LoadState=loaded\nUnitFileState=disabled\nNeedDaemonReload=no\nDropInPaths=\nFragmentPath={UNIT}\n"
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
