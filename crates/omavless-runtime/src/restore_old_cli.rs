// SPDX-License-Identifier: MIT
//! Fixed OLD-only consumer, separate from the historical still-fenced Abort.

use crate::RuntimePaths;
use crate::cutover::CutoverPaths;
use crate::desired::DesiredPaths;
use crate::native_coordinator::FreshRecovery;
use crate::native_host::NativeHostPaths;
use nix::unistd::Uid;
use serde::{Deserialize, Deserializer};
use std::ffi::OsString;
use std::io::Read;
use std::path::{Component, Path};
use zeroize::Zeroizing;

const MAX_INPUT: usize = 32 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Disabled,
    InvalidInput,
    RecoveryRefused,
    AvailabilityUnverified,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Disabled => "Restore recovery is unavailable in this Backup-only build",
            Self::InvalidInput => "Invalid private OLD recovery input",
            Self::RecoveryRefused => "OLD recovery refused; preserve its artifacts and do not repeat automatically",
            Self::AvailabilityUnverified => "OLD configuration recovered; runtime availability not verified. Do not repeat recovery",
        })
    }
}

// Deliberately no Debug, Clone or serialization of reusable private data.
struct PrivateText(Zeroizing<String>);
impl<'de> Deserialize<'de> for PrivateText {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|value| Self(Zeroizing::new(value)))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: u8,
    archive: PrivateText,
    passphrase: PrivateText,
}
fn parse(input: impl Read) -> Result<Request, Error> {
    let mut bytes = Zeroizing::new(Vec::new());
    input
        .take((MAX_INPUT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::InvalidInput)?;
    if bytes.len() > MAX_INPUT {
        return Err(Error::InvalidInput);
    }
    let request: Request = serde_json::from_slice(&bytes).map_err(|_| Error::InvalidInput)?;
    let archive = request.archive.0.as_str();
    if request.schema != 1
        || archive.len() > 4096
        || archive.contains('\0')
        || !Path::new(archive).is_absolute()
        || Path::new(archive)
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        || !(12..=1024).contains(&request.passphrase.0.len())
    {
        return Err(Error::InvalidInput);
    }
    Ok(request)
}

pub fn arguments_admitted(arguments: &[OsString]) -> bool {
    arguments == ["restore", "recover-old", "--confirm-rollback"]
}

pub fn recover_from_private_input(input: impl Read) -> Result<(), Error> {
    // This dominates parsing, reservation, manager queries and every effect,
    // including product+development feature union builds and direct helpers.
    if !crate::product_scope::restore_enabled() {
        return Err(Error::Disabled);
    }
    let request = parse(input)?;
    let uid = Uid::current();
    if uid.is_root() || uid != Uid::effective() {
        return Err(Error::RecoveryRefused);
    }
    let runtime = RuntimePaths::current().map_err(|_| Error::RecoveryRefused)?;
    let paths = CutoverPaths::current(uid.as_raw()).map_err(|_| Error::RecoveryRefused)?;
    let desired = DesiredPaths::current().map_err(|_| Error::RecoveryRefused)?;
    let host = NativeHostPaths::current(&runtime.directory).map_err(|_| Error::RecoveryRefused)?;
    let mut recovery = FreshRecovery::reserve().map_err(|_| Error::RecoveryRefused)?;
    recovery
        .recover_old_to_completion(
            Path::new(request.archive.0.as_str()),
            request.passphrase.0.as_bytes(),
            paths,
            desired,
            host,
            uid.as_raw(),
        )
        .map_err(|_| Error::RecoveryRefused)?;
    let released = recovery
        .dispose_and_release_old()
        .map_err(|_| Error::RecoveryRefused)?;
    // No Start while either old lease remains held. No repeated Stop/Abort or
    // automatic connect when this independently admitted Start is unverified.
    crate::runtime_relaunch::start_recovered_off().map_err(|_| Error::AvailabilityUnverified)?;
    released
        .verify_current()
        .map_err(|_| Error::AvailabilityUnverified)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_recovery_command_is_exact_and_product_gate_dominates() {
        let admitted: Vec<_> = ["restore", "recover-old", "--confirm-rollback"]
            .map(OsString::from)
            .to_vec();
        assert!(arguments_admitted(&admitted));
        assert_eq!(
            crate::product_scope::cli_disabled(&admitted),
            crate::product_scope::backup_only()
        );
        for parts in [
            vec!["restore", "recover-old"],
            vec!["restore", "recover-old", "--yes"],
            vec!["restore", "recover-old", "--confirm-rollback", "secret"],
            vec!["restore", "abort", "--confirm-rollback"],
        ] {
            assert!(!arguments_admitted(
                &parts.into_iter().map(OsString::from).collect::<Vec<_>>()
            ));
        }
    }
    #[test]
    fn old_recovery_private_input_is_bounded_and_closed() {
        let valid =
            br#"{"schema":1,"archive":"/private/archive.ovb","passphrase":"synthetic passphrase"}"#;
        assert!(parse(valid.as_slice()).is_ok());
        for input in [b"{}".as_slice(), b"[]", b"null", b"{}{}",
            br#"{"schema":1,"archive":"relative","passphrase":"synthetic passphrase"}"#,
            br#"{"schema":1,"archive":"/a/../b","passphrase":"synthetic passphrase"}"#,
            br#"{"schema":1,"archive":"/a","passphrase":"short"}"#,
            br#"{"schema":1,"schema":1,"archive":"/a","passphrase":"synthetic passphrase"}"#,
            br#"{"schema":1,"archive":"/a","passphrase":"synthetic passphrase","unit":"other.service"}"#] {
            assert!(parse(input).is_err());
        }
        assert!(parse(vec![b' '; MAX_INPUT + 1].as_slice()).is_err());
    }
    #[cfg(feature = "product-private-backup")]
    #[test]
    fn old_recovery_direct_helper_denies_before_stdin_or_reservation() {
        struct NoRead;
        impl Read for NoRead {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                panic!("disabled recovery consumed input");
            }
        }
        assert_eq!(recover_from_private_input(NoRead), Err(Error::Disabled));
    }
}
