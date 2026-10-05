"""Inert external artifact boundaries; no Cargo or kernel prototype invocation."""
import hashlib
import json
from pathlib import Path
import unittest
import tomllib

ROOT = Path(__file__).resolve().parents[1]
ARTIFACT = ROOT / "tests/k1_namespace_binder"


class BinderArtifact(unittest.TestCase):
    def test_reviewed_external_sources_are_pinned_not_product_dependencies(self):
        pins = json.loads((ARTIFACT / "upstream.json").read_text())
        self.assertEqual(pins["purpose"], "external-review-only-not-product-adoption")
        for library in ("nix", "libc"):
            item = pins[library]
            self.assertEqual(len(item["commit"]), 40)
            self.assertEqual(hashlib.sha256((ROOT / item["patch"]).read_bytes()).hexdigest(), item["sha256"])
        manifest = tomllib.loads((ARTIFACT / "Cargo.toml").read_text())
        self.assertEqual(manifest["workspace"], {})
        self.assertFalse(manifest["package"]["publish"])
        self.assertEqual(manifest["dependencies"]["nix"]["path"], "../nix")
        self.assertEqual(manifest["patch"]["crates-io"]["libc"]["path"], "../libc")
        self.assertNotIn("k1-real-namespace-binder-review", (ROOT / "Cargo.lock").read_text())
        self.assertNotIn("k1_namespace_binder", (ROOT / "Cargo.toml").read_text())

    def test_fixed_real_safe_apis_and_no_effect_or_authority_export(self):
        source = (ARTIFACT / "src/lib.rs").read_text()
        for required in ("#![forbid(unsafe_code)]", "SockProtocol::NetlinkNetFilter",
                         "SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK",
                         'File::open("/proc/thread-self/ns/net")', "NetnsCookie",
                         "namespace_type(file.as_fd())", "namespace_id(file.as_fd())",
                         "creator: Option<OwnedFd>", "PhantomData<Rc<()>>",
                         "ManuallyDrop<Originals>", "pub fn bind_untrusted_anchor",
                         "pub fn verify_local", "fn verify_with"):
            self.assertIn(required, source)
        for forbidden in ("pub fn with_", "pub unsafe", "unsafe {", "from_raw_fd",
                          "setns(", "unshare(", "sendto(", "sendmsg(",
                          "CanonicalCreator", "NamespaceObservation::Canonical", "pub fn into_"):
            self.assertNotIn(forbidden, source)
        self.assertEqual(source.count("compile_fail,E0277"), 2)
        self.assertEqual(source.count("compile_fail,E0616"), 1)

    def test_normal_entry_uses_only_private_real_backend_and_fault_matrix_is_present(self):
        source = (ARTIFACT / "src/lib.rs").read_text()
        self.assertIn("Self::bind_with(anchor, &mut Real)", source)
        self.assertIn("self.verify_queries(&mut Real)", source)
        self.assertNotIn("pub trait Queries", source)
        self.assertNotIn("pub fn bind_with", source)
        constructor = source.split("fn bind_with(", 1)[1].split("fn budget(", 1)[0]
        self.assertLess(constructor.index("let retained_file"), constructor.index("let anchor = owner.sample"))
        faults = (ARTIFACT / "src/fault_tests.rs").read_text()
        for token in ("1..=15", "16..=23", "Cut::Error", "Cut::Panic", "Cut::Late",
                      "assert_eq!(fake.calls, at)", "retained(anchor_fd)", "owner.verify_queries"):
            self.assertIn(token, faults)


if __name__ == "__main__":
    unittest.main()
