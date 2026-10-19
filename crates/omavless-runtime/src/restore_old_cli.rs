// SPDX-License-Identifier: MIT
//! Fixed recovery consumers, separate from the historical still-fenced Abort.

use crate::RuntimePaths;
use crate::cutover::CutoverPaths;
use crate::desired::DesiredPaths;
use crate::native_coordinator::FreshRecovery;
use crate::native_host::NativeHostPaths;
use crate::restore_recovery_preview::{Choice, Preview};
use nix::unistd::Uid;
use serde::{Deserialize, Deserializer};
use std::ffi::OsString;
use std::io::Read;
use std::path::Path;
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
            Self::RecoveryRefused => {
                "Recovery refused; preserve its artifacts and do not repeat automatically"
            }
            Self::AvailabilityUnverified => {
                "Configuration recovered; runtime availability not verified. Do not repeat recovery"
            }
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
    // Use the producer's spelling/bounds contract too: Path::components alone
    // normalizes interior '/./', which the archive producer deliberately refuses.
    if !crate::developer_current_restore::valid_private(
        request.schema,
        archive,
        &request.passphrase.0,
    ) {
        return Err(Error::InvalidInput);
    }
    Ok(request)
}

pub fn arguments_admitted(arguments: &[OsString]) -> bool {
    choice(arguments).is_some()
}

fn choice(arguments: &[OsString]) -> Option<Choice> {
    if arguments == ["restore", "recover-old", "--confirm-rollback"] {
        Some(Choice::Old)
    } else if arguments == ["restore", "recover-mixed", "--confirm-rollback"] {
        Some(Choice::Mixed)
    } else if arguments == ["restore", "recover-committed", "--confirm-completion"] {
        Some(Choice::Committed)
    } else if arguments == ["restore", "recover-final-old", "--confirm-disposition"] {
        Some(Choice::FinalOld)
    } else if arguments == ["restore", "recover-final-new", "--confirm-disposition"] {
        Some(Choice::FinalNew)
    } else if arguments == ["restore", "recover-ticket-new", "--confirm-disposition"] {
        Some(Choice::NewTicket)
    } else if arguments == ["restore", "recover-complete-new", "--confirm-disposition"] {
        Some(Choice::NewComplete)
    } else {
        None
    }
}

pub fn recover_selected_from_private_input(
    arguments: &[OsString],
    input: impl Read,
) -> Result<(), Error> {
    if !crate::product_scope::restore_enabled() {
        return Err(Error::Disabled);
    }
    recover(input, choice(arguments).ok_or(Error::InvalidInput)?)
}

fn phase(marker: &'static [u8]) {
    // Closed public categories only. Diagnostic I/O must not change recovery
    // authority or unwind an uncertain operation.
    use std::io::Write;
    let _ = std::io::stderr().write_all(marker);
}

pub fn recover_from_private_input(input: impl Read) -> Result<(), Error> {
    recover(input, Choice::Old)
}

fn recover(input: impl Read, choice: Choice) -> Result<(), Error> {
    // This dominates parsing, reservation, manager queries and every effect,
    // including product+development feature union builds and direct helpers.
    if !crate::product_scope::restore_enabled() {
        return Err(Error::Disabled);
    }
    let request = parse(input)?;
    execute_private(
        request.archive.0.as_str(),
        request.passphrase.0.as_str(),
        choice,
        None,
    )
}

/// Exactly one consumer Confirm. The retained preview is comparison DATA;
/// the fresh engine reauthenticates ciphertext and graph under its OWN lease.
pub(crate) fn recover_previewed(
    archive: &str,
    passphrase: &str,
    preview: Preview,
) -> Result<(), Error> {
    if !crate::product_scope::restore_enabled() {
        return Err(Error::Disabled);
    }
    if !crate::developer_current_restore::valid_private(1, archive, passphrase) {
        return Err(Error::InvalidInput);
    }
    execute_private(archive, passphrase, preview.choice(), Some(preview))
}

fn execute_private(
    archive: &str,
    passphrase: &str,
    choice: Choice,
    preview: Option<Preview>,
) -> Result<(), Error> {
    let diagnostic = preview.is_none();
    let phase = |label: &'static [u8]| {
        if diagnostic {
            phase(label);
        }
    };
    phase(b"RESTORE_RECOVERY_INPUT\n");
    let uid = Uid::current();
    if uid.is_root() || uid != Uid::effective() {
        return Err(Error::RecoveryRefused);
    }
    let runtime = RuntimePaths::current().map_err(|_| Error::RecoveryRefused)?;
    phase(b"RESTORE_RECOVERY_RUNTIME_PATHS\n");
    let paths = CutoverPaths::current(uid.as_raw()).map_err(|_| Error::RecoveryRefused)?;
    let desired = DesiredPaths::current().map_err(|_| Error::RecoveryRefused)?;
    phase(b"RESTORE_RECOVERY_CONFIG_PATHS\n");
    let host = NativeHostPaths::current(&runtime.directory).map_err(|_| Error::RecoveryRefused)?;
    phase(b"RESTORE_RECOVERY_HOST_PATHS\n");
    let mut recovery = match preview {
        Some(expected) => FreshRecovery::reserve_previewed(expected),
        None => FreshRecovery::reserve(),
    }
    .map_err(|_| Error::RecoveryRefused)?;
    phase(b"RESTORE_RECOVERY_RESERVED\n");
    let result = match choice {
        Choice::NewComplete => recovery.recover_new_complete_output(
            Path::new(archive),
            passphrase.as_bytes(),
            paths,
            desired,
            host,
            uid.as_raw(),
        ),
        Choice::NewTicket => recovery.recover_new_ticket_output(
            Path::new(archive),
            passphrase.as_bytes(),
            paths,
            desired,
            host,
            uid.as_raw(),
        ),
        Choice::FinalNew => recovery.recover_final_new_output(
            Path::new(archive),
            passphrase.as_bytes(),
            paths,
            desired,
            host,
            uid.as_raw(),
        ),
        Choice::FinalOld => recovery.recover_final_old_output(
            Path::new(archive),
            passphrase.as_bytes(),
            paths,
            desired,
            host,
            uid.as_raw(),
        ),
        Choice::Old => recovery.recover_old_to_completion(
            Path::new(archive),
            passphrase.as_bytes(),
            paths,
            desired,
            host,
            uid.as_raw(),
        ),
        Choice::Mixed => recovery.recover_mixed_to_completion(
            Path::new(archive),
            passphrase.as_bytes(),
            paths,
            desired,
            host,
            uid.as_raw(),
        ),
        Choice::Committed => recovery.recover_committed_to_completion(
            Path::new(archive),
            passphrase.as_bytes(),
            paths,
            desired,
            host,
            uid.as_raw(),
        ),
    };
    result.map_err(|error| {
        use crate::native_coordinator::NativeFirstError;
        phase(match error {
            NativeFirstError::Prepare => b"RESTORE_RECOVERY_AUTHENTICATION_REFUSED\n",
            NativeFirstError::CiphertextMismatch => b"RESTORE_RECOVERY_ARCHIVE_MISMATCH\n",
            NativeFirstError::Admission => b"RESTORE_RECOVERY_ADMISSION_REFUSED\n",
            NativeFirstError::StillFenced if matches!(choice, Choice::NewComplete) => {
                b"RESTORE_RECOVERY_UNAVAILABLE\n"
            }
            NativeFirstError::StillFenced => b"RESTORE_RECOVERY_FENCED\n",
        });
        Error::RecoveryRefused
    })?;
    phase(b"RESTORE_RECOVERY_COMPLETED\n");
    match choice {
        Choice::NewComplete => {
            let released = recovery
                .dispose_and_release_new_complete()
                .map_err(|_| Error::RecoveryRefused)?;
            start_and_verify(|| released.verify_current())
        }
        Choice::NewTicket => {
            let released = recovery
                .dispose_and_release_new_ticket()
                .map_err(|_| Error::RecoveryRefused)?;
            start_and_verify(|| released.verify_current())
        }
        Choice::FinalNew => {
            let released = recovery
                .dispose_and_release_final_new()
                .map_err(|_| Error::RecoveryRefused)?;
            start_and_verify(|| released.verify_current())
        }
        Choice::FinalOld => {
            let released = recovery
                .dispose_and_release_final_old()
                .map_err(|_| Error::RecoveryRefused)?;
            start_and_verify(|| released.verify_current())
        }
        Choice::Old | Choice::Mixed => {
            let released = recovery
                .dispose_and_release_old()
                .map_err(|_| Error::RecoveryRefused)?;
            start_and_verify(|| released.verify_current())
        }
        Choice::Committed => {
            let released = recovery
                .dispose_and_release_new()
                .map_err(|_| Error::RecoveryRefused)?;
            start_and_verify(|| released.verify_current())
        }
    }
}

fn start_and_verify<E>(verify: impl FnOnce() -> Result<(), E>) -> Result<(), Error> {
    // Each typed release must succeed before Start. Never retry recovery on
    // an independently unverified availability result.
    crate::runtime_relaunch::start_recovered_off().map_err(|_| Error::AvailabilityUnverified)?;
    verify().map_err(|_| Error::AvailabilityUnverified)
}

#[cfg(test)]
mod tests {
    #[test]
    fn new_complete_selector_is_closed_and_product_denial_precedes_input() {
        use super::*;
        let args = ["restore", "recover-complete-new", "--confirm-disposition"].map(OsString::from);
        assert!(matches!(choice(&args), Some(Choice::NewComplete)));
        for bad in [
            vec!["restore", "recover-complete-old", "--confirm-disposition"],
            vec!["restore", "recover-complete-new", "--confirm-completion"],
            vec![
                "restore",
                "recover-complete-new",
                "--confirm-disposition",
                "extra",
            ],
        ] {
            assert!(!arguments_admitted(
                &bad.into_iter().map(OsString::from).collect::<Vec<_>>()
            ));
        }
        if !crate::product_scope::restore_enabled() {
            struct NoRead;
            impl std::io::Read for NoRead {
                fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                    panic!("must not read");
                }
            }
            assert_eq!(
                recover_selected_from_private_input(&args, NoRead),
                Err(Error::Disabled)
            );
        }
    }
    #[test]
    fn new_ticket_selector_is_exact_and_disabled_builds_do_not_read_private_input() {
        use super::*;
        let valid = ["restore", "recover-ticket-new", "--confirm-disposition"].map(OsString::from);
        assert!(matches!(choice(&valid), Some(Choice::NewTicket)));
        for bad in [
            vec!["restore", "recover-ticket-new", "--confirm-completion"],
            vec!["restore", "recover-ticket-new", "--confirm-rollback"],
            vec!["restore", "recover-ticket-old", "--confirm-disposition"],
            vec![
                "restore",
                "recover-ticket-new",
                "--confirm-disposition",
                "extra",
            ],
        ] {
            assert!(!arguments_admitted(
                &bad.into_iter().map(OsString::from).collect::<Vec<_>>()
            ));
        }
        if !crate::product_scope::restore_enabled() {
            struct NoRead;
            impl std::io::Read for NoRead {
                fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                    panic!("must not read");
                }
            }
            assert_eq!(
                recover_selected_from_private_input(&valid, NoRead),
                Err(Error::Disabled)
            );
        }
    }
    use super::*;
    #[test]
    fn final_new_is_a_distinct_closed_disposition_selector() {
        let valid = ["restore", "recover-final-new", "--confirm-disposition"].map(OsString::from);
        assert!(arguments_admitted(&valid));
        assert_eq!(
            crate::product_scope::cli_disabled(&valid),
            crate::product_scope::backup_only()
        );
        for args in [
            vec!["restore", "recover-final-new", "--confirm-completion"],
            vec!["restore", "recover-final-new", "--confirm-rollback"],
            vec![
                "restore",
                "recover-final-new",
                "--confirm-disposition",
                "step",
            ],
        ] {
            assert!(!arguments_admitted(
                &args.into_iter().map(OsString::from).collect::<Vec<_>>()
            ));
        }
    }
    #[cfg(feature = "product-private-backup")]
    #[test]
    fn final_new_product_union_denies_before_reading_private_input() {
        struct NoRead;
        impl Read for NoRead {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                panic!("disabled Source selector read input");
            }
        }
        assert_eq!(
            recover_selected_from_private_input(
                &[
                    "restore".into(),
                    "recover-final-new".into(),
                    "--confirm-disposition".into()
                ],
                NoRead
            ),
            Err(Error::Disabled)
        );
    }
    #[test]
    fn final_old_is_a_distinct_closed_disposition_selector() {
        let valid = ["restore", "recover-final-old", "--confirm-disposition"].map(OsString::from);
        assert!(arguments_admitted(&valid));
        assert_eq!(
            crate::product_scope::cli_disabled(&valid),
            crate::product_scope::backup_only()
        );
        for args in [
            vec!["restore", "recover-final-old", "--confirm-rollback"],
            vec!["restore", "recover-final-old", "--confirm-completion"],
            vec![
                "restore",
                "recover-final-old",
                "--confirm-disposition",
                "step",
            ],
            vec!["restore", "recover-final-both", "--confirm-disposition"],
        ] {
            assert!(!arguments_admitted(
                &args.into_iter().map(OsString::from).collect::<Vec<_>>()
            ));
        }
    }
    #[cfg(feature = "product-private-backup")]
    #[test]
    fn final_old_product_union_denies_before_private_input() {
        struct NoRead;
        impl Read for NoRead {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                panic!("disabled selector read input");
            }
        }
        assert_eq!(
            recover_selected_from_private_input(
                &[
                    "restore".into(),
                    "recover-final-old".into(),
                    "--confirm-disposition".into()
                ],
                NoRead
            ),
            Err(Error::Disabled)
        );
    }
    #[test]
    fn recovery_choices_require_their_exact_confirmation() {
        for command in ["recover-old", "recover-mixed", "recover-committed"] {
            for confirmation in ["--confirm-rollback", "--confirm-completion"] {
                let args = ["restore", command, confirmation].map(OsString::from);
                assert_eq!(
                    arguments_admitted(&args),
                    (command == "recover-committed") == (confirmation == "--confirm-completion")
                );
                assert_eq!(
                    crate::product_scope::cli_disabled(&args),
                    crate::product_scope::backup_only()
                );
            }
        }
    }
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

    #[test]
    fn old_recovery_uses_producer_path_and_secret_bounds_without_echoing_input() {
        for archive in [
            "/", "/a/", "/a//b", "/a/./b", "/a/.", "/a/../b", "/a\nb", "/a\0b",
        ] {
            let input = serde_json::to_vec(&serde_json::json!({
                "schema": 1, "archive": archive, "passphrase": "synthetic secret"
            }))
            .unwrap();
            let error = parse(input.as_slice())
                .err()
                .expect("invalid producer path");
            assert_eq!(error.to_string(), "Invalid private OLD recovery input");
        }
        for (length, accepted) in [(11, false), (12, true), (1024, true), (1025, false)] {
            let input = serde_json::to_vec(&serde_json::json!({
                "schema": 1, "archive": "/private/archive.ovb", "passphrase": "s".repeat(length)
            }))
            .unwrap();
            assert_eq!(parse(input.as_slice()).is_ok(), accepted);
        }
        assert_eq!(parse([0xff].as_slice()).err(), Some(Error::InvalidInput));
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
