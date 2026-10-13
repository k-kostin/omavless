# SPDX-License-Identifier: MIT
"""SOURCE-only public predecessor qualification; fixtures stay under HOME.

Synthetic bytes exercise the private validator seam. They are not public ELF
provenance or installed upgrade acceptance. No fixture executable is executed.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / "plugin/setup-predecessor.sh"
SOURCE = "22e23e64c49b8110088b6be3f063b5e641ba0853"
PINS = {
    "x86_64": "3ff795316aa331463aff4e26571546433bdfd9317f348712d135be2f0a8d28a1",
    "aarch64": "62874def77df9e4f099baf74c57420a7ab1dab7b83935676828f6d445a5f357f",
}


class SetupPredecessorTests(unittest.TestCase):
    def setUp(self):
        parent = Path.home() / ".cache/ovt"
        parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        self.assertFalse(parent.is_symlink())
        self.assertEqual(parent.stat().st_uid, os.getuid())
        self.assertEqual(parent.stat().st_mode & 0o022, 0)
        temporary = tempfile.TemporaryDirectory(prefix="pred-", dir=parent)
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.binary = self.directory / "omavless"
        self.receipt = self.directory / "build-identity.txt"
        self.binary.write_bytes(b"#!/bin/bash\nprintf EXECUTED > \"$TEST_DIR/executed\"\n")
        self.binary.chmod(0o755)
        self.digest = hashlib.sha256(self.binary.read_bytes()).hexdigest()
        self.identity = self.receipt_text()
        self.receipt.write_text(self.identity)
        self.receipt.chmod(0o644)
        self.env = {"PATH": "/usr/bin:/bin", "HOME": str(self.directory),
                    "TEST_DIR": str(self.directory), "TEST_SHA": self.digest,
                    "TEST_UID": str(os.getuid())}

    def receipt_text(self, arch="x86_64", digest=None):
        return (f"schemaVersion=3\nsourceCommit={SOURCE}\n"
                f"binarySha256={digest or self.digest}\narchitecture={arch}\n"
                "provenance=caller-supplied-prebuilt\nproductVersion=0.8.2\n")

    def run_shell(self, code):
        return subprocess.run(["/bin/bash", "-c", 'set -euo pipefail; source "$1"; ' + code,
                               "test", str(HELPER)], env=self.env, text=True,
                              capture_output=True, timeout=12, check=False)

    def inspect(self, prefix="", arch="x86_64"):
        return self.run_shell(prefix + f'''_omavless_predecessor_inspect \
"$TEST_DIR/build-identity.txt" "$TEST_DIR/omavless" {arch} "$TEST_SHA" "$TEST_UID"''')

    def assert_report(self, result, status, reason):
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stderr, "")
        self.assertLess(len(result.stdout), 180)
        self.assertEqual(json.loads(result.stdout), {
            "schemaVersion": 1, "scope": "installed_predecessor_bytes",
            "status": status, "reason": reason})
        self.assertNotIn(str(self.directory), result.stdout)
        self.assertNotIn("synthetic-secret", result.stdout)
        self.assertFalse((self.directory / "executed").exists())

    def test_recorded_public_pins_and_actual_producer_format(self):
        # Retained public evidence is present in shallow CI checkouts too.
        # The writer separately inspected the immutable tag/producer through
        # git show before selecting these pins; this gate does not fetch them.
        report = (ROOT / "docs/testing/NATIVE_082_ARTIFACTS_2026-09-21.md").read_text()
        producer = (ROOT / "packaging/arch/build-local-package.sh").read_text()
        helper = HELPER.read_text()
        for pin in PINS.values():
            self.assertIn(pin, report)
            self.assertIn(pin, helper)
        self.assertIn(SOURCE, report)
        self.assertIn("identity_schema=3", producer)
        self.assertIn("provenance=caller-supplied-prebuilt", producer)
        self.assertIn("printf 'productVersion=%s", producer)

    def test_supported_architecture_synthetic_receipts_and_unchanged_files(self):
        for arch in PINS:
            self.receipt.write_text(self.receipt_text(arch))
            before = {p.name: (p.read_bytes(), p.stat().st_mode, p.stat().st_mtime_ns)
                      for p in self.directory.iterdir()}
            self.assert_report(self.inspect(arch=arch), "qualified", "public_0_8_2")
            self.assertEqual(before, {p.name: (p.read_bytes(), p.stat().st_mode,
                                              p.stat().st_mtime_ns)
                                      for p in self.directory.iterdir()})

    def test_strict_receipt_rejects_wrong_duplicate_unknown_and_oversized_bytes(self):
        variants = [self.identity.replace("schemaVersion=3", "schemaVersion=2"),
                    self.identity.replace(SOURCE, "b" * 40),
                    self.identity.replace("0.8.2", "0.8.2-rc.1"),
                    self.identity.replace("x86_64", "aarch64"),
                    self.identity.replace(self.digest, "b" * 64),
                    self.identity.replace("caller-supplied-prebuilt", "development"),
                    self.identity + "schemaVersion=3\n",
                    self.identity + "synthetic-secret=private.invalid\n",
                    self.identity.replace("sourceCommit", "source\x00Commit"),
                    self.identity.rstrip(),
                    "synthetic-secret" * 400]
        for index, value in enumerate(variants):
            with self.subTest(index=index):
                self.receipt.write_text(value)
                self.assert_report(self.inspect(), "mismatch",
                                   "installed_files_unsafe" if len(value) > 4096
                                   else "receipt_mismatch")

    def test_missing_is_unavailable_and_nonregular_or_symlink_is_unsafe(self):
        self.receipt.unlink()
        self.assert_report(self.inspect(), "unavailable", "installed_files_unavailable")
        self.receipt.symlink_to(self.binary)
        self.assert_report(self.inspect(), "mismatch", "installed_files_unsafe")
        self.receipt.unlink()
        self.receipt.mkdir()
        self.assert_report(self.inspect(), "mismatch", "installed_files_unsafe")

    def test_binary_bytes_are_checked_even_with_valid_receipt(self):
        self.binary.write_bytes(b"synthetic-secret modified bytes")
        self.assert_report(self.inspect(), "mismatch", "binary_mismatch")

    def test_modes_ownership_links_and_parent_symlinks_are_checked(self):
        for path, mode in ((self.receipt, 0o666), (self.binary, 0o777),
                           (self.binary, 0o644)):
            original = path.stat().st_mode
            path.chmod(mode)
            self.assert_report(self.inspect(), "mismatch", "installed_files_unsafe")
            path.chmod(original)
        self.assert_report(self.inspect('TEST_UID=4294967294; '), "mismatch", "installed_files_unsafe")
        alias = self.directory / "hardlink"
        os.link(self.binary, alias)
        self.assert_report(self.inspect(), "mismatch", "installed_files_unsafe")
        alias.unlink()
        self.directory.chmod(0o777)
        self.assert_report(self.inspect(), "mismatch", "installed_files_unsafe")
        self.directory.chmod(0o700)
        alias.symlink_to(self.directory, target_is_directory=True)
        result = self.run_shell('''_omavless_predecessor_inspect "$TEST_DIR/hardlink/build-identity.txt" \
"$TEST_DIR/hardlink/omavless" x86_64 "$TEST_SHA" "$TEST_UID"''')
        self.assert_report(result, "mismatch", "installed_files_unsafe")

    def test_hash_failure_is_unavailable_and_never_executes_native(self):
        self.assert_report(self.inspect('/usr/bin/timeout() { return 124; }; '),
                           "unavailable", "inspection_unavailable")
        self.assert_report(self.inspect('/usr/bin/stat() { return 1; }; '),
                           "unavailable", "inspection_unavailable")

    def test_changed_name_at_final_recheck_refuses(self):
        code = '''original=$(declare -f _omavless_predecessor_file_identity)
eval "${original/_omavless_predecessor_file_identity/original_identity}"
_omavless_predecessor_file_identity() {
  if [[ -e "$TEST_DIR/hashed" && $1 == "$TEST_DIR/build-identity.txt" ]]; then
    printf '%s' synthetic-secret > "$1"
  fi
  original_identity "$@"
}
/usr/bin/timeout() { shift; "$@"; : > "$TEST_DIR/hashed"; }
'''
        self.assert_report(self.inspect(code), "mismatch", "installed_files_changed")

    def test_public_entry_has_fixed_paths_pins_owner_and_no_environment_injection(self):
        self.env.update({"OMAVLESS_HOME": str(self.directory),
                         "OMAVLESS_PREDECESSOR_SHA": self.digest,
                         "OMAVLESS_PREDECESSOR_BINARY": str(self.binary)})
        for arch, pin in PINS.items():
            result = self.run_shell(f'''/usr/bin/uname() {{ printf '%s\\n' {arch}; }}
_omavless_predecessor_inspect() {{ printf '%s\\n' "$@"; }}
omavless_predecessor_status''')
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stderr, "")
            self.assertEqual(result.stdout.splitlines(), [
                "/usr/share/doc/omavless/build-identity.txt", "/usr/bin/omavless", arch, pin, "0"])
        self.assert_report(self.run_shell("omavless_predecessor_status injected"),
                           "unavailable", "invalid_request")
        self.assert_report(self.run_shell('/usr/bin/uname() { echo riscv64; }; omavless_predecessor_status'),
                           "unavailable", "unsupported_architecture")
        self.assert_report(self.run_shell('/usr/bin/uname() { return 1; }; omavless_predecessor_status'),
                           "unavailable", "inspection_unavailable")

    def test_sourcing_has_no_effect_or_shell_policy_change(self):
        result = subprocess.run([
            "/bin/bash", "-c", 'before=$(set +o); mask=$(umask); source "$1"; '
            '[[ $(set +o) == "$before" && $(umask) == "$mask" ]] || exit 1; printf ready',
            "test", str(HELPER)], env=self.env, text=True, capture_output=True,
            timeout=12, check=False)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (0, "ready", ""))
        text = HELPER.read_text()
        for forbidden in ("sudo ", "pacman ", "systemctl ", "curl ", "native(",
                          "mkdir ", "chmod ", "chown ", "rm "):
            self.assertNotIn(forbidden, text)


if __name__ == "__main__":
    unittest.main()
