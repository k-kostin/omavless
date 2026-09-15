# SPDX-License-Identifier: MIT
"""First-run shell composition. All mutating boundaries are synthetic functions.

No production test override/environment hook, GUI, sudo, private store or network.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "plugin/setup-runtime.sh"


class MarketplaceSetupTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="omavless-bootstrap-test-")
        self.addCleanup(self.tmp.cleanup)
        self.directory = Path(self.tmp.name)
        self.env = {"PATH": "/usr/bin:/bin", "HOME": str(self.directory),
                    "TEST_DIR": str(self.directory)}

    def run_shell(self, code, input=""):
        prefix = '''
source "$1"
setup_dir="$TEST_DIR"
setup_temp="$TEST_DIR"
native_present() { return 1; }
native() { printf 'UNEXPECTED_NATIVE_EFFECT' >&2; return 99; }
package_install() { printf 'UNEXPECTED_PACKAGE_EFFECT' >&2; return 99; }
install_core_dependency() { printf 'UNEXPECTED_CORE_EFFECT' >&2; return 99; }
enable_runtime() { printf 'UNEXPECTED_ENABLE_EFFECT' >&2; return 99; }
curl() { printf 'UNEXPECTED_NETWORK_EFFECT' >&2; return 99; }
'''
        return subprocess.run(["/bin/bash", "-c", prefix + code, "test", str(SCRIPT)],
                              env=self.env, input=input, text=True, capture_output=True,
                              timeout=10, check=False)

    def pins(self, **updates):
        entry = {"sha256": "a" * 64, "sourceCommit": "b" * 40}
        data = {"schemaVersion": 1, "version": "0.8.0",
                "packages": {"aarch64": entry, "x86_64": entry}}
        data.update(updates)
        (self.directory / "runtime-release.json").write_text(json.dumps(data))

    def test_status_existing_owner_is_read_only(self):
        for target, expected in [("rust", "ready"), ("legacy", "needs_activation"),
                                 ("private-unknown", "needs_attention")]:
            result = self.run_shell(f'native_present() {{ return 0; }}\nnative_target() {{ echo {target}; }}\nsetup_status')
            self.assertEqual(result.stdout, expected + "\n")
            self.assertEqual(result.stderr, "")

    def test_failed_native_read_is_not_a_fresh_install(self):
        result = self.run_shell('native_present() { return 0; }; native_target() { return 1; }; setup_status')
        self.assertEqual(result.stdout, "needs_attention\n")

    def test_pinned_missing_runtime_is_installable_on_both_architectures(self):
        self.pins()
        for arch in ("aarch64", "x86_64"):
            result = self.run_shell(f'native_present() {{ return 1; }}; uname() {{ echo {arch}; }}; setup_status')
            self.assertEqual(result.stdout, "needs_package\n")

    def test_missing_unpublished_and_unsupported_packages_fail_closed(self):
        for arch in ("aarch64", "x86_64", "i686"):
            self.pins(packages={})
            result = self.run_shell(f'native_present() {{ return 1; }}; uname() {{ echo {arch}; }}; setup_status')
            self.assertEqual(result.stdout, "release_unavailable\n")

    def test_malformed_untrusted_metadata_is_not_echoed(self):
        path = self.directory / "runtime-release.json"
        for value in ["secret://synthetic", "[]", "x" * 8193,
                      '{"schemaVersion":1,"version":"0.8.0","packages":{"aarch64":{"sha256":"x","sourceCommit":"y"}}}']:
            path.write_text(value)
            result = self.run_shell('uname() { echo aarch64; }; release_fields')
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout + result.stderr, "")

    def test_symlink_metadata_refused(self):
        self.pins()
        path = self.directory / "runtime-release.json"
        path.rename(self.directory / "real.json")
        path.symlink_to(self.directory / "real.json")
        self.assertNotEqual(self.run_shell('release_fields').returncode, 0)

    def test_unknown_metadata_keys_or_version_refused(self):
        for updates in [{"version": "latest"}, {"schemaVersion": 2}, {"url": "https://example.invalid"}]:
            self.pins(**updates)
            self.assertNotEqual(self.run_shell('release_fields').returncode, 0)

    def test_headless_install_and_unknown_arguments_have_no_effect(self):
        for arguments in ["install", "install ru", "status en", "download", "install zz", "status extra more"]:
            result = self.run_shell("setup_main " + arguments)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout + result.stderr, "")

    def test_cancel_requires_exact_consent(self):
        for answer in ("\n", "yes\n", "install\n", "INSTALL extra\n", ""):
            self.assertNotEqual(self.run_shell("confirm", answer).returncode, 0)
        self.assertEqual(self.run_shell("confirm", "INSTALL\n").returncode, 0)

    def test_existing_native_owner_does_not_restart_or_enable(self):
        result = self.run_shell('native_target() { echo rust; }; prepare_application')
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout + result.stderr, "")

    def activation_fixture(self, existing=False, fail=""):
        config = self.directory / ".config/omavless/profiles.json"
        if existing:
            config.parent.mkdir(parents=True, exist_ok=True)
            config.write_text("synthetic unchanged private fixture")
        return self.run_shell('''
native_target() { if [[ -f "$TEST_DIR/activated" ]]; then echo rust; else echo legacy; fi; }
native() {
  printf '%s\\n' "$*" >> "$TEST_DIR/trace"
  [[ "$*" != "''' + fail + '''" ]] || return 1
  case "$*" in
    'setup initialize') return 0 ;;
    store-compatibility) printf '{"schemaVersion":1,"compatible":true}';;
    'cutover activate') touch "$TEST_DIR/activated" ;;
    *) return 99 ;;
  esac
}
enable_runtime() { echo enable >> "$TEST_DIR/trace"; }
prepare_application
''')

    def test_new_setup_uses_only_canonical_initialization_and_activation(self):
        result = self.activation_fixture()
        self.assertEqual(result.returncode, 0)
        self.assertEqual((self.directory / "trace").read_text().splitlines(),
                         ["setup initialize", "store-compatibility", "cutover activate", "enable"])

    def test_existing_store_never_initialized_or_rewritten(self):
        result = self.activation_fixture(existing=True)
        self.assertEqual(result.returncode, 0)
        self.assertNotIn("setup initialize", (self.directory / "trace").read_text())
        self.assertEqual((self.directory / ".config/omavless/profiles.json").read_text(),
                         "synthetic unchanged private fixture")

    def test_failed_activation_does_not_enable_or_retry(self):
        result = self.activation_fixture(fail="cutover activate")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.directory / "trace").read_text().splitlines(),
                         ["setup initialize", "store-compatibility", "cutover activate"])

    def test_unknown_owner_never_activates(self):
        result = self.run_shell('native_target() { echo invalid; }; prepare_application')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout + result.stderr, "")

    def download_fixture(self, *, hash_ok=True, core=True, consent="", core_success=True):
        payload = b"synthetic package -- NOT AN ARCH PACKAGE"
        (self.directory / "payload").write_bytes(payload)
        checksum = hashlib.sha256(payload).hexdigest() if hash_ok else "0" * 64
        return self.run_shell('''
release_fields() { printf '0.8.0\\taarch64\\t''' + checksum + '''\\tbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\n'; }
curl() {
  printf '%s\\n' "$@" > "$TEST_DIR/curl-args"
  cp "$TEST_DIR/payload" "$setup_temp/omavless-0.8.0-1-aarch64.pkg.tar.zst"
}
package_info() { echo info >> "$TEST_DIR/trace"; echo 'omavless 0.8.0-1'; }
package_install() { echo install >> "$TEST_DIR/trace"; }
package_installed() { echo 'omavless 0.8.0-1'; }
core_dependency_present() { ''' + ('return 0' if core else '[[ -f "$TEST_DIR/core" ]]') + '''; }
install_core_dependency() { echo core >> "$TEST_DIR/trace"; ''' + ('touch "$TEST_DIR/core"' if core_success else 'return 1') + '''; }
install_package
''', consent)

    def test_hash_verified_before_any_package_or_core_effect(self):
        result = self.download_fixture(hash_ok=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.directory / "trace").exists())

    def test_pinned_download_uses_bounded_https_and_normal_package_install(self):
        result = self.download_fixture()
        self.assertEqual(result.returncode, 0)
        self.assertEqual((self.directory / "trace").read_text().splitlines(), ["info", "install"])
        args = (self.directory / "curl-args").read_text().splitlines()
        self.assertEqual(args[-1], "https://github.com/k-kostin/omavless/releases/download/v0.8.0/omavless-0.8.0-1-aarch64.pkg.tar.zst")
        for flag in ("--disable", "--proto", "--proto-redir", "--max-filesize", "--max-time"):
            self.assertIn(flag, args)

    def test_missing_core_requires_separate_consent(self):
        result = self.download_fixture(core=False, consent="\n")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.directory / "trace").read_text().splitlines(), ["info"])

    def test_accepted_core_install_precedes_runtime_package(self):
        result = self.download_fixture(core=False, consent="CORE\n")
        self.assertEqual(result.returncode, 0)
        self.assertEqual((self.directory / "trace").read_text().splitlines(), ["info", "core", "install"])

    def test_failed_core_install_never_installs_runtime(self):
        result = self.download_fixture(core=False, consent="CORE\n", core_success=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.directory / "trace").read_text().splitlines(), ["info", "core"])


if __name__ == "__main__":
    unittest.main()
