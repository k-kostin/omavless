"""Retention guards only; these do not attest an installed System positive."""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/omavless-runtime/src"


class SystemHistoricalOffBoundary(unittest.TestCase):
    def test_system_entry_is_private_fixed_path_and_returns_only_review(self):
        text = (SRC / "system_historical_off_candidate.rs").read_text()
        self.assertIn("fn current() -> Result<Review, ProductionOwnerError>", text)
        self.assertNotRegex(text, r"pub(?:\([^)]*\))?\s+fn current\s*\(")
        for call in ("RuntimePaths::current()", "DesiredPaths::current()",
                     "CutoverPaths::current(uid)", "MigrationLock::acquire_existing"):
            self.assertIn(call, text)
        entry = text.split("fn current()", 1)[1].split("fn run_under_lease", 1)[0]
        self.assertLess(entry.index("RetainedCurrentOff::capture"), entry.index(".system_off("))
        self.assertNotIn("cleanup_probe_orphans()", entry)
        for source in SRC.rglob("*.rs"):
            self.assertNotIn("system_historical_off::current(", source.read_text())

    def test_system_constructor_has_no_injected_authority(self):
        text = (SRC / "restore_disposition_historical_candidate.rs").read_text()
        constructor = text.split("pub(crate) fn system_off(", 1)[1].split("pub(crate) fn capture(", 1)[0]
        signature = constructor.split("->", 1)[0]
        self.assertIn("ObservationOnlyNativeHost", signature)
        self.assertNotRegex(signature, r"bool|FnMut|CurrentEpochProof|CutoverPaths")
        self.assertIn("CurrentEpochProof::capture(", constructor)
        self.assertIn("host.fresh_observation", constructor)
        self.assertRegex(text, r"#\[cfg\(test\)\]\s*impl<'a> RetainedCurrentOff<'a>\s*\{\s*pub\(crate\) fn research")
        for kind in ("HistoricalProfile", "HistoricalConnection", "DetachedHistoricalBatch"):
            self.assertRegex(text, rf"#\[cfg\(test\)\]\s*pub\(crate\) struct {kind}")

    def test_owner_drop_precedes_final_proof_and_result(self):
        text = (SRC / "system_historical_off_candidate.rs").read_text()
        self.assertNotRegex(text, r"pub(?:\([^)]*\))?\s+fn run_under_lease")
        self.assertRegex(text, r"#\[cfg\(test\)\]\s*pub\(crate\) fn review_for_test")
        body = text.split("fn run_under_lease", 1)[1].split("#[cfg(test)]", 1)[0]
        after_drop = body.split("drop(owner);", 1)[1]
        self.assertLess(after_drop.index("evidence"), after_drop.index("Ok(Review::ReviewedOffStillFenced)"))
        self.assertIn("ProductionNativeOwner::initialize_under_lease", body)
        self.assertNotRegex(body, r"Ok\((?:owner|evidence)\)")
        self.assertNotIn("register", body)

    def test_observation_wrapper_is_sealed_and_ordinary_cleanup_unchanged(self):
        text = (SRC / "native_host.rs").read_text()
        wrapper = text.split("pub(crate) struct ObservationOnlyNativeHost", 1)[1].split("impl NativeLifecycleHost", 1)[0]
        self.assertIn("inner: NativeLifecycleHost", wrapper)
        self.assertNotIn("pub inner", wrapper)
        self.assertNotIn("Deref", wrapper)
        self.assertEqual(text.count("inner.drop_paths = DropPaths::Preserve"), 1)
        self.assertIn("drop_paths: DropPaths::Cleanup", text)
        self.assertNotIn("self.inner.prepare", wrapper)
        self.assertNotIn("self.inner.stop_owned", wrapper)
        self.assertNotIn("self.inner.auxiliary", wrapper)
        for name in ("system_historical_off_candidate.rs", "production_owner.rs"):
            self.assertIn("ObservationOnlyNativeHost::new", (SRC / name).read_text())

    def test_system_vm_driver_is_ignored_fixed_account_and_never_mints_receipt(self):
        source = (SRC / "system_provider_vm_tests.rs").read_text()
        self.assertIn("const UID: u32 = 61080;", source)
        self.assertIn('const HOME: &str = "/home/ov-t4-system";', source)
        self.assertEqual(source.count("#[ignore ="), 2)
        for forbidden in ("epoch_tests::receipt", "epoch_tests::proof", "CurrentEpochProof::synthetic", "Source::Synthetic", "fixture.root =", "Fixture::reopen"):
            self.assertNotIn(forbidden, source)
        self.assertLess(source.index("CurrentEpochProof::capture"), source.index("super::tests::prepared(true)"))
        self.assertIn("receipt.unchanged();", source)
        self.assertIn("current_for_vm_test()", source)
        forwarding = (SRC / "system_historical_off_candidate.rs").read_text()
        self.assertRegex(forwarding, r"#\[cfg\(test\)\]\s*pub\(crate\) fn current_for_vm_test")


if __name__ == "__main__":
    unittest.main()
