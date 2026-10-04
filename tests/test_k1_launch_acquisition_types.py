"""Compile actual inactive acquisition source; never execute produced code.

Only its two imported trait/error names are inert harness stubs. The tested
owner fields, constructor visibility, callback lifetimes and markers are the
unchanged real module. Run with the Rust gate, not the Python-only gate.
"""
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "crates/omavless-netguard/src/launch_acquisition.rs"
PRELUDE = r'''
#![allow(dead_code)]
#![forbid(unsafe_code)]
mod effect_port {
    #[derive(Debug)] pub enum EffectError { UnavailableOrUncertain }
}
mod authority_composition { pub(crate) trait CanonicalCreator {} }
#[path = SOURCE_LITERAL] mod launch_acquisition;
use launch_acquisition::{AcquiredCreator, ConfigurationEvidence};
struct Candidate;
impl authority_composition::CanonicalCreator for Candidate {}
'''


class AcquisitionTypes(unittest.TestCase):
    def compile(self, body):
        with tempfile.TemporaryDirectory(prefix="k1-launch-types-") as temporary:
            path = Path(temporary)
            source = path / "check.rs"
            # Rust string escaping for a fixed repository path, not caller data.
            literal = '"' + str(SOURCE).replace('\\', '\\\\').replace('"', '\\"') + '"'
            source.write_text(PRELUDE.replace("SOURCE_LITERAL", literal) + body)
            return subprocess.run(
                ["rustc", "--edition=2024", "--crate-type=lib", "--emit=metadata",
                 str(source), "-o", str(path / "check.rmeta")],
                capture_output=True, text=True, timeout=30, check=False,
            )

    def rejected(self, body, diagnostic):
        result = self.compile(body)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(diagnostic, result.stderr)

    def test_positive_actual_module_and_owned_callback_compile(self):
        result = self.compile("fn check(x: &mut AcquiredCreator<Candidate>) { let _ = x.with_lease(|_| Ok(7)); }")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_actual_owner_is_not_send(self):
        self.rejected("fn check(x: AcquiredCreator<Candidate>) { fn send<T: Send>(_: T) {} send(x); }", "E0277")

    def test_actual_owner_is_not_sync(self):
        self.rejected("fn check(x: &AcquiredCreator<Candidate>) { fn sync<T: Sync>(_: &T) {} sync(x); }", "E0277")

    def test_actual_owner_is_not_copy(self):
        self.rejected("fn check(x: AcquiredCreator<Candidate>) { let y = x; drop(x); drop(y); }", "E0382")

    def test_callback_cannot_extract_creator_borrow(self):
        self.rejected("fn check(x: &mut AcquiredCreator<Candidate>) -> &mut Candidate { x.with_lease(|c| Ok(c)).unwrap() }",
                      "lifetime may not live long enough")

    def test_normal_build_has_no_synthetic_constructor(self):
        self.rejected("fn check() { let _ = AcquiredCreator::synthetic(Candidate); }", "E0599")

    def test_configuration_evidence_cannot_become_acquired_owner(self):
        self.rejected("fn check(e: ConfigurationEvidence) -> AcquiredCreator<Candidate> { e }", "E0308")


if __name__ == "__main__":
    unittest.main()
