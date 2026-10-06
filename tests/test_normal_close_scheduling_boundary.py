"""Source-retention guards; behavioral evidence belongs to Rust owner tests."""
from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/omavless-runtime/src"


class NormalSchedulingBoundary(unittest.TestCase):
    def test_qualified_package_is_same_session_private_and_nondefault_not_boolean_promotion(self):
        session=(SRC/"conditional_close_candidate.rs").read_text()
        self.assertRegex(session,r'#\[cfg\(feature = "developer-conditional-close"\)\]\s*pub\(crate\) fn qualified_pair_permit')
        self.assertIn("qualification_attempted = true",session)
        self.assertIn("self.check_qualified_pair()?",session.split("fn effect_lease",1)[1].split("#[cfg(test)]",1)[0])
        qualification=(SRC/"conditional_release_pair.rs").read_text()
        self.assertNotIn("CandidateEffectPermit",qualification)
        self.assertIn("image.image_identity != self.core.identity",qualification)
        self.assertIn("image.source_identity != self.core.identity",qualification)
        self.assertIn("SELECTION_BYTES",qualification)
        self.assertIn("self.original.refuse()",qualification)
        self.assertIn("return Ok(None)",qualification)
        receipt=(SRC/"managed_close_receipt.rs").read_text()
        self.assertIn('"omavless-managed-dns-close-pair-v1"',receipt)
        self.assertIn("deny_unknown_fields",receipt)
        self.assertIn("r.conditional_close_abi != 1",receipt)
        self.assertIn("r.patch_sha256.close != CLOSE_PATCH",receipt)

    def test_shared_transition_is_normal_compiled_private_and_permit_bound(self):
        text = (SRC / "native_coordinator/connection_close.rs").read_text()
        name = "schedule_permitted_connection_close"
        body = text.split(f"    fn {name}(", 1)[1].split("    pub(crate) fn poll_connection_close", 1)[0]
        self.assertNotRegex(text, rf"#\[cfg\(test\)\]\s*(?://[^\n]*\n\s*)*fn {name}")
        self.assertNotRegex(text, rf"pub(?:\([^)]*\))?\s+fn {name}")
        self.assertIn("permit: crate::conditional_close_candidate::CandidateEffectPermit", body)
        self.assertIn("lease: &MigrationLock", body)
        self.assertLess(body.index("lease.authorizes("), body.index(".into_session()"))
        self.assertLess(body.index("session.authorize_effect("), body.index(".start(session, selected.target, permit)"))
        self.assertIn("EffectProof(snapshot.context)", body)
        self.assertIn("snapshot.expiry", body)

    def test_fixture_permit_constructor_and_admission_remain_test_only(self):
        text = (SRC / "conditional_close_candidate.rs").read_text()
        self.assertRegex(text, r"#\[cfg\(test\)\]\s*impl CandidateEffectPermit")
        self.assertIn("struct CandidateEffectPermit {\n    _private: (),", text)
        host = (SRC / "native_host.rs").read_text()
        self.assertRegex(host, r"#\[cfg\(test\)\]\s*pub\(crate\) fn fixture_permit")
        self.assertNotRegex(host, r"#\[cfg\(test\)\]\s*pub\(crate\) fn into_session")
        close = (SRC / "native_coordinator/connection_close.rs").read_text()
        admission = close.split("    fn confirm_connection_close_reserved(", 1)[1].split("    fn schedule_permitted_connection_close", 1)[0]
        self.assertRegex(admission, r"#\[cfg\(test\)\]\s*if let Some\(permit\)")
        self.assertIn("ExternalCloseOutcome::MissingAttestation", admission)
        self.assertEqual(admission.count(".scheduler"), 0)
        self.assertIn("schedule_permitted_connection_close(snapshot, selected, token, permit, &_lease)", re.sub(r"\s+", " ", admission))

    def test_developer_pair_is_a_distinct_nondefault_private_constructor(self):
        manifest = tomllib.loads((SRC.parent / "Cargo.toml").read_text())
        self.assertEqual(manifest["features"]["developer-conditional-close"],
                         ["omavless-tui?/developer-conditional-close"])
        self.assertNotIn("developer-conditional-close", manifest["features"]["default"])
        text = (SRC / "conditional_close_candidate.rs").read_text()
        self.assertRegex(text, r'#\[cfg\(feature = "developer-conditional-close"\)\]\s*pub\(crate\) fn developer_pair_permit')
        package = (SRC / "conditional_package_evidence.rs").read_text()
        self.assertIn('mod developer_pair;', package)
        developer = (SRC / "conditional_developer_pair.rs").read_text()
        self.assertIn('"omavless-developer-conditional-pair-v1"', developer)
        self.assertIn('|| r.production_adoption', developer)
        close = (SRC / "native_coordinator/connection_close.rs").read_text()
        admission = close.split("    fn confirm_connection_close_reserved(", 1)[1].split("    fn schedule_permitted_connection_close", 1)[0]
        self.assertIn(".proves_live_for_scheduling()", admission)
        self.assertNotIn(".proves_live()", admission)

    def test_developer_client_is_weakly_propagated_and_explicitly_dual_gated(self):
        manifest = tomllib.loads((SRC.parent / "Cargo.toml").read_text())
        feature = manifest["features"]["developer-conditional-close"]
        # Exact weak propagation must not pull the optional TUI into a headless
        # build or place developer methods into a default build.
        self.assertEqual(feature, ["omavless-tui?/developer-conditional-close"])
        self.assertNotIn("developer-conditional-close", manifest["features"]["default"])
        tui = SRC.parent.parent / "omavless-tui"
        self.assertEqual(tomllib.loads((tui / "Cargo.toml").read_text())
                         ["features"]["developer-conditional-close"], [])
        main = (SRC / "main.rs").read_text()
        self.assertRegex(main, r'#\[cfg\(all\(feature = "tui", feature = "developer-conditional-close"\)\)\]\s*if arguments == \["tui", "--developer-conditional-close"\]')
        self.assertRegex((tui / "src/lib.rs").read_text(),
                         r'#\[cfg\(feature = "developer-conditional-close"\)\]\s*pub mod developer_close;')


if __name__ == "__main__":
    unittest.main()
