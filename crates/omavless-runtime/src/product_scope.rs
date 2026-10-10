// SPDX-License-Identifier: MIT
//! Compile-time product scope, never selected through IPC or user settings.
use std::ffi::OsString;

pub const fn backup_only() -> bool {
    cfg!(feature = "product-private-backup")
}

pub const fn restore_enabled() -> bool {
    !backup_only()
}

#[cfg(any(test, feature = "t4-manager-actor-service"))]
pub(crate) fn method_disabled(method: &str) -> bool {
    backup_only()
        && matches!(
            method,
            "backup.restore"
                | "backup.preview"
                | "backup.restore_previewed"
                | "developer.restore_current"
                | "developer.backup_current"
                | "developer.pause_current_intent"
                | "developer.abort_current_intent"
        )
}

pub fn cli_disabled(arguments: &[OsString]) -> bool {
    if !backup_only() {
        return false;
    }
    matches!(
        arguments.first().and_then(|s| s.to_str()),
        Some("restore" | "developer")
    ) || (arguments.first().is_some_and(|s| s == "backup")
        && !arguments.get(1).is_some_and(|s| s == "create"))
        || arguments
            .iter()
            .any(|s| s == "--developer-private-restore" || s == "--developer-private-backup")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn product_scope_keeps_backup_and_dominates_all_restore_selectors() {
        assert!(!method_disabled("backup.create"));
        for method in [
            "backup.restore",
            "backup.preview",
            "backup.restore_previewed",
            "developer.restore_current",
            "developer.backup_current",
            "developer.pause_current_intent",
            "developer.abort_current_intent",
        ] {
            assert_eq!(method_disabled(method), backup_only());
        }
        for parts in [
            vec!["restore", "abort", "--confirm-rollback"],
            vec!["backup", "restore"],
            vec!["backup", "restore-previewed"],
            vec!["backup", "preview"],
            vec!["developer", "pause-current-intent"],
            vec!["tui", "--developer-private-restore"],
            vec!["tui", "--developer-private-backup"],
        ] {
            let args: Vec<_> = parts.into_iter().map(OsString::from).collect();
            assert_eq!(cli_disabled(&args), backup_only());
        }
        for parts in [
            vec!["tui"],
            vec!["backup", "create"],
            vec!["status"],
            vec!["app", "start"],
        ] {
            assert!(!cli_disabled(
                &parts.into_iter().map(OsString::from).collect::<Vec<_>>()
            ));
        }
    }

    #[cfg(feature = "product-private-backup")]
    #[test]
    fn product_scope_helpers_refuse_before_reading_input_or_starting_actors() {
        struct NoRead;
        impl std::io::Read for NoRead {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                panic!("disabled product selector consumed private input");
            }
        }
        assert!(crate::restore_abort_cli::abort_from_private_input(NoRead).is_err());
        assert!(
            crate::developer_current_restore::from_private_input(
                &["developer".into(), "restore-current".into()],
                NoRead
            )
            .is_err()
        );
        assert!(
            crate::private_pair_api::preview_from_private_input(
                &["backup".into(), "preview".into()],
                NoRead
            )
            .is_err()
        );
        assert!(
            crate::private_pair_api::from_private_input(
                &[
                    "backup".into(),
                    "restore".into(),
                    "--confirm-private-pair".into()
                ],
                NoRead
            )
            .is_err()
        );
        for entry in [
            crate::manager_actor_service::actor_entry,
            crate::manager_actor_service::actor_canonical_entry,
            crate::manager_actor_service::actor_mixed_writer_entry,
            crate::manager_actor_service::actor_inspector_entry,
            crate::manager_actor_service::actor_fixed_rollback_entry,
            crate::manager_actor_service::supervisor_entry,
        ] {
            assert!(entry().is_err());
        }
    }
}
