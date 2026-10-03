"""Source retention only, not execution, installed admission or policy proof."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/omavless-runtime/src"


class FirstRestoreOwnerExecution(unittest.TestCase):
    def test_private_entry_and_no_recovery_or_registration(self):
        text = (SRC / "native_coordinator/restore_first_execution.rs").read_text()
        self.assertIn("fn execute_first_restore(", text)
        self.assertNotRegex(text, r"pub(?:\([^)]*\))?\s+fn execute_first_restore")
        self.assertNotIn("recover_staged_pair", text)
        self.assertNotIn("AbortedStillFenced", text)
        self.assertIn("execute_staged_pair(", text)
        self.assertIn("session.owner.transaction.block();", text)
        self.assertIn("session.owner.invalidate_connection_close();", text)

    def test_created_stage_retains_original_descriptors(self):
        text = (SRC / "restore_staging_candidate.rs").read_text()
        self.assertIn("pub(crate) struct CreatedStage", text)
        self.assertIn("members: Vec<(&'static str, File, Metadata)>", text)
        self.assertIn("created.members.push((READY_MEMBER, file, metadata))", text)
        self.assertIn("created.recheck(state_directory, uid)?;", text)
        self.assertIn("Ok(created)", text)
        self.assertIn(".map(drop)", text)

    def test_generic_pending_guard_stays_conservative(self):
        text = (SRC / "pending_private_transaction.rs").read_text()
        self.assertNotIn("CreatedStage", text)
        self.assertNotIn("first_execution", text)
        for name in ("staging_pending_at", "restore-decision.intent", "restore-decision.terminal"):
            self.assertIn(name, text)

    def test_every_generic_fence_is_accounted_for_by_typed_gate(self):
        generic = (SRC / "pending_private_transaction.rs").read_text().split(
            "pub(crate) fn pending_at(directory: &Path) -> bool {", 1)[1].split("\n}", 1)[0]
        # Any additional crate predicate/constant or literal in the generic
        # guard requires explicit review of the private exception as well.
        expected_calls = {
            "crate::routing_preset::pending_at",
            "crate::restore_staging_candidate::staging_pending_at",
            "crate::restore_closure_model::CLOSURE_MEMBER",
            "crate::restore_closure_model::NEXT_CLOSURE_MEMBER",
            "crate::restore_disposition_ticket_model::pending_at",
            "crate::restore_disposition_complete_model::pending_at",
            "crate::restore_successor_handoff_model::SUCCESSOR_MEMBER",
        }
        self.assertEqual(set(re.findall(r"crate::[A-Za-z_]+::[A-Za-z_]+", generic)), expected_calls)
        self.assertEqual(set(re.findall(r'"([^"]+)"', generic)), {
            "restore-decision.intent", "restore-decision.terminal", "restore-finalization.pending"})
        typed = (SRC / "native_coordinator/restore_first_execution.rs").read_text()
        for token in ("CLOSURE_MEMBER", "NEXT_CLOSURE_MEMBER", "TICKET_MEMBER", "COMPLETE_MEMBER",
                      "SUCCESSOR_MEMBER", "restore-finalization.pending", "routing_preset::pending",
                      "stage.recheck", "restore-decision.intent", "restore-decision.terminal"):
            self.assertIn(token, re.sub(r"\s+", "", typed))

    def test_behavioral_counterexamples_retained(self):
        text = (SRC / "native_coordinator/restore_first_execution_tests.rs").read_text()
        for name in (
            "first_restore_owner_executes_actual_pair_under_one_lease_and_retains_fence",
            "first_restore_owner_original_stage_swap_in_last_callback_refuses_before_intent",
            "first_restore_owner_late_execution_gates_refuse_without_automatic_rollback",
            "first_restore_owner_wrong_archive_and_existing_fence_have_no_stage_effect",
        ):
            self.assertIn(name, text)
        self.assertIn("late boundary reached", text)

    def test_abort_writer_retains_actual_created_descriptor(self):
        text = (SRC / "restore_executor_candidate.rs").read_text()
        self.assertIn("write_record_owned(paths, uid, name, bytes).map(drop)", text)
        self.assertIn("struct CreatedAbort(CreatedRecord)", text)
        writer = text.split("fn write_record_owned(", 1)[1].split("struct Bound", 1)[0]
        for token in ("OFlag::O_EXCL", "directory_before: before", "file,", "member,",
                      "created.recheck(paths, uid)?", "Ok(created)"):
            self.assertIn(token, writer)
        self.assertNotRegex(text, r"impl\s+Clone\s+for\s+Created(?:Abort|Record)")

    def test_abort_owned_lower_has_no_normal_owner_connection(self):
        for name in ("production_owner.rs", "native_dispatch.rs", "lib.rs",
                     "native_coordinator/restore_first_execution.rs"):
            text = (SRC / name).read_text()
            self.assertNotIn("abort_staged_pair_owned", text)
        text = (SRC / "restore_executor_candidate.rs").read_text()
        body = text.split("fn abort_owned_with_hook<", 1)[1].split("impl CreatedRecord", 1)[0]
        self.assertIn("chain.active().phase() != DecisionPhase::Intent", body)
        self.assertIn("write_abort_owned(paths, uid, &terminal)?", body)
        self.assertIn("created.recheck(paths, uid).is_ok()", body)
        self.assertIn("Ok(created)", body)

    def test_abort_created_identity_counterexamples_retained(self):
        text = (SRC / "restore_executor_candidate.rs").read_text()
        for name in ("created_abort_retains_exclusive_inode_and_refuses_replay_or_other_phase",
                     "created_abort_original_descriptor_refuses_same_byte_substitution",
                     "explicit_abort_owned_restores_mixed_pair_and_refuses_both_terminals",
                     "explicit_abort_owned_never_adopts_terminal_replaced_in_post_write_gate"):
            self.assertIn(name, text)


if __name__ == "__main__":
    unittest.main()
