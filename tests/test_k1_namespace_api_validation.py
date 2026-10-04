"""Inert review-artifact guards; no external build, download or namespace query."""
import hashlib
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
PATCH = ROOT / "docs/development/patches/nix-readonly-namespace-v2.patch"


class NamespaceApiReview(unittest.TestCase):
    def test_exact_external_patch_and_only_three_syscall_library_files(self):
        raw = PATCH.read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(),
                         "e6229d02e60e20a089a0340f6028f02a436308f5ff031a223c685e7ef31c5c15")
        names = re.findall(r"^diff --git a/(\S+) b/\S+$", raw.decode(), re.M)
        self.assertEqual(names, ["src/sys/mod.rs", "src/sys/nsfs.rs", "src/sys/socket/sockopt.rs"])

    def test_fixed_borrowed_interfaces_and_real_wrapper_test_seams(self):
        text = PATCH.read_text()
        for token in ("pub fn namespace_type(fd: BorrowedFd<'_>)", "pub fn namespace_id(fd: BorrowedFd<'_>)",
                      "impl super::GetSockOpt for NetnsCookie", "namespace_id_with(fd,", "netns_cookie_with(fd.as_fd(),",
                      "FnOnce", "ENOPROTOOPT", "ENOTTY", "EOPNOTSUPP", "socklen_t::MAX", "u64::MAX"):
            self.assertIn(token, text)
        for token in ("libc::setns", "libc::unshare", "libc::setsockopt", "pub unsafe fn", "CanonicalCreator"):
            self.assertNotIn(token, text)

    def test_compile_fixture_is_safe_and_checks_borrow_lifetimes_and_abi(self):
        raw = (ROOT / "docs/development/patches/namespace-api-compile.rs").read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(),
                         "fbc45b549738932817e3ea49317e69f1ca592efdd09ee28eb7eef7f36ae3831f")
        text = raw.decode()
        self.assertIn("#![forbid(unsafe_code)]", text)
        self.assertEqual(text.count("compile_fail,E0505"), 2)
        self.assertIn("0x8008b70d", text)
        self.assertIn('target_arch = "aarch64"', text)
        self.assertIn("libc_main::NS_GET_ID", text)
        self.assertIn("nix::libc::NS_GET_ID", text)

    def test_libc_prerequisite_is_only_constant_and_semver_registration(self):
        for name, pin in (
            ("libc-ns-get-id-main.patch", "df90aa94dcb199ac45a00b15e2247adeea601c4cc5ae8b32dea43a3ea82ca999"),
            ("libc-ns-get-id-02.patch", "da653498d5daa88c459f5a4e7649e9c64e93a9d2b683d3979c5f9ea94cbdd7ab"),
        ):
            raw = (PATCH.parent / name).read_bytes()
            self.assertEqual(hashlib.sha256(raw).hexdigest(), pin)
            added = [line[1:] for line in raw.decode().splitlines()
                     if line.startswith("+") and not line.startswith("+++")]
            self.assertEqual(added, ["NS_GET_ID", "pub const NS_GET_ID: Ioctl = _IOR::<__u64>(NSIO, 13);"])

    def test_contract_keeps_unadopted_constant_and_canonical_launch_prerequisites(self):
        text = (ROOT / "docs/development/K1_NAMESPACE_API_VALIDATION.md").read_text()
        for token in ("e35c00891f52468979f92b795de2dc1f3dd58a87", "b739c733e06a5f184bae7e296919c15d6ccd8ec8",
                      "no `NS_GET_ID`", "not ready for dependency adoption", "trusted system-manager", "ARM64 compilation is not execution"):
            self.assertIn(token, text)


if __name__ == "__main__":
    unittest.main()
