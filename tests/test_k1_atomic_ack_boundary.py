"""Source-only integration boundaries; never open sockets or execute a VM gate."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/omavless-netguard/src"


class AtomicAckBoundaryTests(unittest.TestCase):
    def test_shared_module_is_normal_compiled_but_adapters_remain_test_only(self):
        observer = (SRC / "kernel_observer.rs").read_text()
        self.assertIn('#[path = "kernel_atomic_batch.rs"]\nmod atomic_batch;', observer)
        self.assertNotIn('#[cfg(test)]\n#[path = "kernel_atomic_batch.rs"]', observer)
        for module in ["conditional_delete", "creator_lifecycle", "prefix_ack_loss"]:
            self.assertIn(
                f'#[cfg(test)]\n#[path = "kernel_{module}.rs"]\nmod {module};', observer)
        for file in ["kernel_creator_lifecycle.rs", "kernel_conditional_delete.rs"]:
            adapter = (SRC / file).read_text()
            self.assertIn("use super::atomic_batch::", adapter)
            self.assertNotIn("fn receive_inner(", adapter)
            self.assertEqual(adapter.count("sendto("), 1)

    def test_fixed_private_input_and_untrusted_output_have_no_effect_plumbing(self):
        source = (SRC / "kernel_atomic_batch.rs").read_text()
        self.assertIn("pub(super) struct AtomicBatch(Vec<Vec<u8>>);", source)
        self.assertIn("fn new(requests: AtomicBatch, port: u32)", source)
        self.assertNotIn("DerefMut", source)
        self.assertNotIn("pub(super) requests:", source)
        self.assertIn("pub(super) enum UntrustedStatus", source)
        for forbidden in [
            "sendto(", "recvmsg", "socket(", "unsafe", "EffectIdentity",
            "EffectPort", "NamespaceObservation", "Command::", "std::fs",
        ]:
            self.assertNotIn(forbidden, source)
        self.assertRegex(source, r"#\[cfg\(test\)\]\s+pub\(super\) fn drift_batch")
        self.assertRegex(source, r"#\[cfg\(test\)\]\s+pub\(super\) fn lease_generation_cut_batch")
        # Two normal fixed encoders and two literal test-only negative fixtures;
        # neither negative constructor accepts a table, rule or raw message.
        self.assertEqual(len(re.findall(r"Ok\(AtomicBatch\(", source)), 4)
        self.assertIn('fn lease_generation_cut_batch(generation: u32, first: u32)', source)
        self.assertIn('b"k1_retained_lease_cut\\0"', source)

    def test_observer_loss_prefixes_and_distinct_legacy_contract_are_preserved(self):
        source = (SRC / "kernel_creator_lifecycle_tests.rs").read_text()
        self.assertIn("K1_PREFIX_ACK_LOSS", source)
        self.assertIn("real_prefix_ack_observer_loss_in_disposable_vm", source)
        self.assertIn("retries=0", source)
        prefix = (SRC / "kernel_prefix_ack_loss.rs").read_text()
        self.assertIn("impl super::atomic_batch::AtomicReplies", prefix)
        self.assertIn("self.poison();", prefix)
        legacy = (SRC / "full_vpn_reply.rs").read_text()
        self.assertIn("pub struct FullVpnTranscript", legacy)
        self.assertIn("Legacy inactive contract", legacy)
        self.assertIn("GETGEN", legacy)


if __name__ == "__main__":
    unittest.main()
