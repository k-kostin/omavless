// SPDX-License-Identifier: MIT

//! A surviving private transaction is an ambiguity fence, regardless of
//! which internal operation created it. Never infer that it is safe to retry,
//! connect, or stop from the marker's contents alone.

use crate::desired::DesiredPaths;
use std::path::Path;

pub(crate) fn pending(paths: &DesiredPaths) -> bool {
    pending_at(&paths.directory)
}

/// The fixed state directory is also available to startup/cutover callers
/// before they construct a desired-state owner. This remains an existence
/// fence: inaccessible, malformed and unexpected entry types all block.
pub(crate) fn pending_at(directory: &Path) -> bool {
    crate::routing_preset::pending_at(directory)
        || crate::restore_staging_candidate::staging_pending_at(directory)
        || journal_member_pending(directory, "restore-decision.intent")
        || journal_member_pending(directory, "restore-decision.terminal")
        || journal_member_pending(directory, "restore-finalization.pending")
        || journal_member_pending(directory, crate::restore_closure_model::CLOSURE_MEMBER)
        || journal_member_pending(directory, crate::restore_closure_model::NEXT_CLOSURE_MEMBER)
        || crate::restore_disposition_ticket_model::pending_at(directory)
        || crate::restore_disposition_complete_model::pending_at(directory)
        || journal_member_pending(
            directory,
            crate::restore_successor_handoff_model::SUCCESSOR_MEMBER,
        )
}

fn journal_member_pending(directory: &Path, name: &str) -> bool {
    !matches!(
        std::fs::symlink_metadata(directory.join(name)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn orphan_decision_members_remain_a_startup_fence() {
        let home = std::env::var_os("HOME").expect("test needs home");
        let root =
            crate::test_temp::directory_under(Path::new(&home), "orphan-restore-decision").unwrap();
        for name in [
            "restore-decision.intent",
            "restore-decision.terminal",
            "restore-finalization.pending",
            crate::restore_closure_model::CLOSURE_MEMBER,
            crate::restore_closure_model::NEXT_CLOSURE_MEMBER,
            crate::restore_disposition_ticket_model::TICKET_MEMBER,
            crate::restore_disposition_complete_model::COMPLETE_MEMBER,
            crate::restore_successor_handoff_model::SUCCESSOR_MEMBER,
        ] {
            fs::write(root.join(name), b"incomplete").unwrap();
            assert!(pending_at(&root));
            fs::remove_file(root.join(name)).unwrap();
        }
        assert!(!pending_at(&root));
        fs::remove_dir_all(root).unwrap();
    }
}
