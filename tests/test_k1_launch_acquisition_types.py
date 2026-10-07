"""Compile actual inactive acquisition source; never execute produced code.

Only its imported interface names are inert harness stubs. The tested
owner fields, constructor visibility, callback lifetimes and markers are the
unchanged real module. Run with the Rust gate, not the Python-only gate.
"""
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "crates/omavless-netguard/src/launch_acquisition.rs"
# Exact production macro: diagnostics compile out in this default-feature
# standalone crate, just as in the ordinary library. Do not stub owner logic.
SERVICE_CUT = '''macro_rules! service_cut {
    ($cut:ident) => {
        #[cfg(feature = "netguard-service-diagnostics")]
        crate::service_diagnostic::mark(crate::service_diagnostic::Stage::$cut);
    };
}
'''
PRELUDE = r'''
#![allow(dead_code)]
#![forbid(unsafe_code)]
SERVICE_CUT_LITERAL
mod effect_port {
    #[derive(Debug)] pub enum EffectError { UnavailableOrUncertain }
    pub struct EffectIdentity;
    pub struct EffectSnapshot;
    pub trait EffectPort {
        fn observe(&mut self) -> Result<EffectSnapshot, EffectError> { unimplemented!() }
        fn create_if_absent(&mut self, _: crate::policy::Policy) -> Result<EffectIdentity, EffectError> { unimplemented!() }
        fn replace_owned(&mut self, _: EffectIdentity, _: crate::policy::Policy) -> Result<EffectIdentity, EffectError> { unimplemented!() }
        fn delete_owned(&mut self, _: EffectIdentity) -> Result<(), EffectError> { unimplemented!() }
    }
}
mod policy { pub struct Policy; }
mod receipt { pub struct HostEpoch; }
mod authority_composition {
    pub enum Boundary { Admission, AfterObserve, AfterCreate, AfterReplace, AfterDelete }
    pub(crate) trait CanonicalCreator: crate::effect_port::EffectPort {
        fn retained_epoch(&mut self, _: Boundary) -> Result<crate::receipt::HostEpoch, crate::effect_port::EffectError> { unimplemented!() }
    }
}
#[path = SOURCE_LITERAL] mod launch_acquisition;
use launch_acquisition::{AcquiredCreator, ConfigurationEvidence};
struct Candidate;
impl authority_composition::CanonicalCreator for Candidate {}
impl effect_port::EffectPort for Candidate {}
'''


class AcquisitionTypes(unittest.TestCase):
    def compile(self, body):
        library = (SOURCE.parent / "lib.rs").read_text()
        self.assertEqual(library.count(SERVICE_CUT), 1)
        with tempfile.TemporaryDirectory(prefix="k1-launch-types-") as temporary:
            path = Path(temporary)
            source = path / "check.rs"
            # Rust string escaping for a fixed repository path, not caller data.
            literal = '"' + str(SOURCE).replace('\\', '\\\\').replace('"', '\\"') + '"'
            source.write_text(PRELUDE.replace("SERVICE_CUT_LITERAL", SERVICE_CUT)
                              .replace("SOURCE_LITERAL", literal) + body)
            return subprocess.run(
                ["rustc", "--edition=2024", "--crate-type=lib", "--emit=metadata",
                 str(source), "-o", str(path / "check.rmeta")],
                capture_output=True, text=True, timeout=30, check=False,
            )

    def rejected(self, body, diagnostic):
        result = self.compile(body)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("cannot find macro", result.stderr)
        self.assertIn(diagnostic, result.stderr)

    def test_positive_actual_module_fixed_operations_compile(self):
        result = self.compile("fn check(x: &mut AcquiredCreator<Candidate>) { let _ = x.retained_epoch(authority_composition::Boundary::Admission); let _ = x.observe(); let _ = x.create_if_absent(policy::Policy); let _ = x.replace_owned(effect_port::EffectIdentity, policy::Policy); let _ = x.delete_owned(effect_port::EffectIdentity); }")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_actual_owner_is_not_send(self):
        self.rejected("fn check(x: AcquiredCreator<Candidate>) { fn send<T: Send>(_: T) {} send(x); }", "E0277")

    def test_actual_owner_is_not_sync(self):
        self.rejected("fn check(x: &AcquiredCreator<Candidate>) { fn sync<T: Sync>(_: &T) {} sync(x); }", "E0277")

    def test_actual_owner_is_not_copy(self):
        self.rejected("fn check(x: AcquiredCreator<Candidate>) { let y = x; drop(x); drop(y); }", "E0382")

    def test_callback_is_private_and_cannot_extract_creator_borrow(self):
        self.rejected("fn check(x: &mut AcquiredCreator<Candidate>) -> &mut Candidate { x.with_lease(|c| Ok(c)).unwrap() }",
                      "E0624")

    def test_callback_cannot_replace_the_paired_creator(self):
        self.rejected("fn check(x: &mut AcquiredCreator<Candidate>) { let _ = x.with_lease(|c| { let _old = std::mem::replace(c, Candidate); Ok(()) }); }", "E0624")

    def test_original_creator_field_cannot_be_replaced(self):
        self.rejected("fn check(x: &mut AcquiredCreator<Candidate>) { let _old = std::mem::replace(&mut x.retained.creator, Candidate); }", "E0616")

    def test_normal_build_has_no_synthetic_constructor(self):
        self.rejected("fn check() { let _ = AcquiredCreator::synthetic(Candidate); }", "E0599")

    def test_configuration_evidence_cannot_become_acquired_owner(self):
        self.rejected("fn check(e: ConfigurationEvidence) -> AcquiredCreator<Candidate> { e }", "E0308")


if __name__ == "__main__":
    unittest.main()
