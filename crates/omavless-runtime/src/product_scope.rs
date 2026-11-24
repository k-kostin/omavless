// SPDX-License-Identifier: MIT
//! Compile-time product scope, never selected through IPC or user settings.
use std::ffi::OsString;

pub const fn backup_only() -> bool {
    cfg!(feature = "product-private-backup")
}

pub const fn restore_enabled() -> bool {
    !backup_only()
}

pub const fn developer_operations_enabled() -> bool {
    !cfg!(any(
        feature = "product-private-backup",
        feature = "product-private-restore"
    ))
}

#[cfg(any(test, feature = "t4-manager-actor-service"))]
pub(crate) fn method_disabled(method: &str) -> bool {
    (!developer_operations_enabled() && method.starts_with("developer."))
        || backup_only()
            && matches!(
                method,
                "stats.get"
                    | "stats.configure"
                    | "stats.reset"
                    | "backup.restore"
                    | "backup.preview"
                    | "backup.restore_previewed"
                    | "developer.restore_current"
                    | "developer.backup_current"
                    | "developer.pause_current_intent"
                    | "developer.abort_current_intent"
                    | "developer.cut_current_first_rename"
                    | "developer.cut_current_terminal"
                    | "developer.abort_current_after_retirement_cut"
                    | "developer.cut_current_committed_retirement"
                    | "developer.cut_current_committed_ticket"
                    | "developer.cut_current_committed_complete"
            )
}

pub fn cli_disabled(arguments: &[OsString]) -> bool {
    if !developer_operations_enabled()
        && (arguments.first().is_some_and(|s| s == "developer")
            || arguments.iter().filter_map(|s| s.to_str()).any(|s| {
                s.starts_with("--developer-")
                    || s.starts_with("--acceptance-")
                    || s == "--product-image-witness"
            }))
    {
        return true;
    }
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
    #[cfg(feature = "product-private-restore")]
    #[test]
    fn product_restore_rejects_developer_methods_and_selectors_before_input() {
        struct NoRead;
        impl std::io::Read for NoRead {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                panic!("product developer command consumed private input");
            }
        }
        for method in [
            "developer.restore_current",
            "developer.backup_current",
            "developer.pause_current_intent",
            "developer.abort_current_intent",
            "developer.cut_current_first_rename",
            "developer.cut_current_terminal",
            "developer.abort_current_after_retirement_cut",
            "developer.cut_current_committed_retirement",
            "developer.cut_current_committed_ticket",
            "developer.cut_current_committed_complete",
        ] {
            assert!(method_disabled(method), "{method}");
        }
        for parts in [
            vec!["developer", "restore-current"],
            vec!["tui", "--developer-private-restore"],
            vec!["daemon", "--product-image-witness"],
        ] {
            assert!(cli_disabled(
                &parts.into_iter().map(OsString::from).collect::<Vec<_>>()
            ));
        }
        for method in [
            "backup.create",
            "backup.preview",
            "backup.restore_previewed",
        ] {
            assert_eq!(
                method_disabled(method),
                backup_only() && method != "backup.create"
            );
        }
        assert!(
            crate::developer_current_restore::from_private_input(
                &["developer".into(), "restore-current".into()],
                NoRead,
            )
            .is_err()
        );
    }

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
            "developer.cut_current_first_rename",
            "developer.cut_current_terminal",
            "developer.abort_current_after_retirement_cut",
            "developer.cut_current_committed_retirement",
            "developer.cut_current_committed_ticket",
            "developer.cut_current_committed_complete",
        ] {
            assert_eq!(
                method_disabled(method),
                backup_only()
                    || (!developer_operations_enabled() && method.starts_with("developer."))
            );
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
            let developer =
                parts[0] == "developer" || parts.iter().any(|s| s.starts_with("--developer-"));
            let args: Vec<_> = parts.into_iter().map(OsString::from).collect();
            assert_eq!(
                cli_disabled(&args),
                backup_only() || (developer && !developer_operations_enabled())
            );
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
        for (command, confirmation) in [
            (
                "cut-current-first-rename",
                "--confirm-private-first-rename-cut",
            ),
            ("cut-current-terminal", "--confirm-private-terminal-cut"),
        ] {
            assert!(
                crate::developer_current_restore::from_private_input(
                    &["developer".into(), command.into(), confirmation.into()],
                    NoRead
                )
                .is_err()
            );
        }
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
