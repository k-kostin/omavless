# SPDX-License-Identifier: MIT
"""Public upgrade shell composition: real PTY and fixed synthetic boundaries.

The source script is copied into each fixture. No test calls a package manager,
sudo, systemd, network client or installed OmaVLESS executable.
"""
import errno
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pty
import select
import shutil
import subprocess
import tempfile
import termios
import time
import unittest


ROOT = Path(__file__).resolve().parents[1]
VERSION = json.loads((ROOT / "manifest.json").read_text())["version"]
SOURCE = "b" * 40


class UpgradeFrontendTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="ovt-upgrade-", dir=os.environ.get("TMPDIR"))
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.script = self.directory / "setup-runtime.sh"
        shutil.copyfile(ROOT / "plugin/setup-runtime.sh", self.script)
        self.env = {"PATH": "/usr/bin:/bin", "HOME": str(self.directory),
                    "TEST_DIR": str(self.directory), "XDG_RUNTIME_DIR": str(self.directory)}
        # Advisory predecessor inspection is independently tested elsewhere.
        # Keep it synthetic here: qualification does not execute the old app.
        (self.directory / "setup-predecessor.sh").write_text('''
omavless_predecessor_status() { printf '%s\\n' '{"schemaVersion":1,"status":"qualified","reason":"public_0_8_2"}'; }
_omavless_predecessor_parents_safe() { return 0; }
''')
        self.helper = self.directory / "fixture-helper"
        self.helper.write_text('''#!/bin/bash
set -eu
[[ "$*" == 'setup upgrade082' ]] || exit 90
IFS= read -r metadata
printf '%s\\n' "$metadata" > "$TEST_DIR/metadata"
printf 'Type UPGRADE to continue: '
IFS= read -r answer < /dev/tty
[[ "$answer" == UPGRADE ]] || exit 1
echo helper >> "$TEST_DIR/trace"
touch "$TEST_DIR/upgraded"
''')
        self.helper_hash = hashlib.sha256(self.helper.read_bytes()).hexdigest()
        expected_app = hashlib.sha256(b"synthetic app archive").hexdigest()
        expected_dns = hashlib.sha256(b"synthetic dns archive").hexdigest()
        self.cache = self.directory / ".cache/omavless/upgrade082" / (expected_app + "." + expected_dns)
        self.cache.mkdir(parents=True, mode=0o700)
        for path in (self.directory / ".cache", self.cache.parent.parent, self.cache.parent, self.cache):
            path.chmod(0o700)
        self.app = self.cache / f"omavless-{VERSION}-1-x86_64.pkg.tar.zst"
        self.dns = self.cache / f"omavless-dns-{VERSION}-1-x86_64.pkg.tar.zst"
        self.app.write_bytes(b"synthetic app archive")
        self.dns.write_bytes(b"synthetic dns archive")
        self.app.chmod(0o600)
        self.dns.chmod(0o600)
        self.hashes = [hashlib.sha256(path.read_bytes()).hexdigest() for path in (self.app, self.dns)]
        for name, digest in zip(("runtime-release.json", "dns-release.json"), self.hashes):
            (self.directory / name).write_text(json.dumps({
                "schemaVersion": 1, "version": VERSION, "packages": {
                    "x86_64": {"sha256": digest, "sourceCommit": SOURCE}}}))
        self.store = self.directory / ".config/omavless/profiles.json"
        self.store.parent.mkdir(parents=True)
        self.store.write_bytes(b"synthetic private-store sentinel")

    def fixture(self):
        return f'''
uname() {{ echo x86_64; }}
timeout() {{ shift; "$@"; }}
setup_runtime_directory_safe() {{ return 0; }}
setup_status() {{ if [[ -f "$TEST_DIR/upgraded" ]]; then echo needs_dns_setup; else echo needs_update; fi; }}
package_info() {{ case "$1" in *omavless-dns-*) echo 'omavless-dns {VERSION}-1';; *) echo 'omavless {VERSION}-1';; esac; }}
app_archive_identity() {{ [[ "$2" == '{SOURCE}' && "$3" == x86_64 && "$4" == '{VERSION}' ]]; }}
dns_archive_identity() {{ [[ "$2" == '{SOURCE}' && "$3" == x86_64 ]]; }}
bsdtar() {{
  [[ "$1" == -xOf && "$2" == '{self.app}' ]] || return 99
  case "$3" in
    usr/share/doc/omavless/build-identity.txt) printf '%s\\n' 'binarySha256={self.helper_hash}';;
    usr/bin/omavless) /usr/bin/cat "$TEST_DIR/fixture-helper";;
    *) return 99;;
  esac
}}
reload_units() {{ echo reload >> "$TEST_DIR/trace"; }}
curl() {{ echo UNEXPECTED_NETWORK_EFFECT >> "$TEST_DIR/trace"; return 99; }}
native() {{ echo UNEXPECTED_NATIVE_EFFECT >> "$TEST_DIR/trace"; return 99; }}
package_install() {{ echo UNEXPECTED_PACKAGE_EFFECT >> "$TEST_DIR/trace"; return 99; }}
prepare_private_store() {{ echo UNEXPECTED_STORE_EFFECT >> "$TEST_DIR/trace"; return 99; }}
prepare_application() {{ echo UNEXPECTED_ACTIVATION_EFFECT >> "$TEST_DIR/trace"; return 99; }}
enroll_uid() {{ echo UNEXPECTED_ENROLL_EFFECT >> "$TEST_DIR/trace"; return 99; }}
start_broker() {{ echo UNEXPECTED_BROKER_EFFECT >> "$TEST_DIR/trace"; return 99; }}
setup_completion() {{ echo UNEXPECTED_COMPLETION_EFFECT >> "$TEST_DIR/trace"; return 99; }}
'''

    def run_script(self, extra="", response="UPGRADE\n", tty=True, action="upgrade", locale="en"):
        code = 'source "$1"\n' + self.fixture() + extra + f'\nsetup_main {action} {locale}\n'
        if not tty:
            result = subprocess.run(["/bin/bash", "-c", code, "test", str(self.script)],
                                    env=self.env, input=response, text=True, capture_output=True, timeout=5)
            return result.returncode, result.stdout + result.stderr
        if os.geteuid() == 0:
            self.skipTest("interactive setup deliberately refuses root")
        master, slave = pty.openpty()
        process = None
        output = bytearray()

        def controlling_terminal():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        try:
            process = subprocess.Popen(["/bin/bash", "-c", code, "test", str(self.script)],
                                       env=self.env, stdin=slave, stdout=slave, stderr=slave,
                                       preexec_fn=controlling_terminal)
            os.close(slave)
            slave = -1
            os.write(master, response.encode())
            deadline = time.monotonic() + 8
            while time.monotonic() < deadline:
                readable, _, _ = select.select([master], [], [], 0.05)
                if readable:
                    try:
                        chunk = os.read(master, 4096)
                    except OSError as error:
                        if error.errno == errno.EIO:
                            break
                        raise
                    if not chunk:
                        break
                    output.extend(chunk)
                    self.assertLessEqual(len(output), 16384)
                elif process.poll() is not None:
                    break
            return process.wait(timeout=1), output.decode().replace("\r\n", "\n")
        finally:
            if process is not None and process.poll() is None:
                process.kill()
                process.wait(timeout=2)
            os.close(master)
            if slave != -1:
                os.close(slave)

    def trace(self):
        path = self.directory / "trace"
        return path.read_text().splitlines() if path.exists() else []

    def pair_fixture(self, stopped=True):
        return f'''
user_runtime_stopped() {{ return {0 if stopped else 1}; }}
pair_update_available() {{ return 0; }}
installed_pair_delivery() {{ echo stale; }}
setup_status() {{ if [[ -f "$TEST_DIR/upgraded" || -f "$TEST_DIR/prepared" ]]; then echo needs_broker_stopped; else echo ready; fi; }}
'''

    def pair_helper(self, preparation_fails=False):
        self.helper.write_text('''#!/bin/bash
set -eu
[[ "$*" == 'setup update-pair' ]] || exit 90
printf '%s\\n' "$OMAVLESS_LOCALE" > "$TEST_DIR/helper-locale"
IFS= read -r metadata
printf '%s\\n' "$metadata" > "$TEST_DIR/metadata"
printf 'Type PREPARE: '
IFS= read -r answer < /dev/tty
[[ "$answer" == PREPARE ]] || exit 1
echo preparation >> "$TEST_DIR/trace"
''' + ('exit 1\n' if preparation_fails else '''touch "$TEST_DIR/prepared"
printf 'Type UPGRADE: '
IFS= read -r answer < /dev/tty
[[ "$answer" == UPGRADE ]] || exit 1
echo package >> "$TEST_DIR/trace"
touch "$TEST_DIR/upgraded"
'''))
        self.helper_hash = hashlib.sha256(self.helper.read_bytes()).hexdigest()

    def test_running_owner_gets_return_instructions_without_download_or_authorization(self):
        self.pair_helper()
        status, output = self.run_script(extra=self.pair_fixture(False), action="update-pair", response="\n")
        self.assertEqual(status, 0, output)
        self.assertEqual(self.trace(), [])
        self.assertFalse((self.directory / "metadata").exists())
        self.assertFalse((self.directory / "prepared").exists())
        self.assertIn("omarchy plugin enable kdk.omavless", output)
        self.assertFalse((self.directory / "omavless-first-run.lock").exists())

    def test_stopped_pair_uses_real_controller_helper_frame_and_separate_consents(self):
        self.pair_helper()
        status, output = self.run_script(extra=self.pair_fixture(), action="update-pair", response="PREPARE\nUPGRADE\n")
        self.assertEqual(status, 0, output)
        self.assertEqual(self.trace(), ["preparation", "package", "reload"])
        self.assertEqual(self.store.read_bytes(), b"synthetic private-store sentinel")
        self.assertIn("Start DNS helper", output)
        self.assertIn('"Terminal and system prompts are closed"', output)
        self.assertNotIn('omarchy plugin enable kdk.omavless', output)
        self.assertFalse((self.directory / "omavless-first-run.lock").exists())

    def test_controller_language_reaches_the_pinned_helper_and_return_labels(self):
        self.pair_helper()
        self.env['OMAVLESS_LOCALE'] = 'unrelated-ambient-locale'
        status, output = self.run_script(extra=self.pair_fixture(), action='update-pair',
                                        response='PREPARE\nUPGRADE\n', locale='ru')
        self.assertEqual(status, 0, output)
        self.assertEqual((self.directory / 'helper-locale').read_text(), 'ru\n')
        self.assertIn('Терминал и запросы системы закрыты', output)
        self.assertIn('Запустить DNS-помощник', output)
        self.assertNotIn('omarchy plugin enable kdk.omavless', output)
        self.assertEqual(self.trace(), ['preparation', 'package', 'reload'])

    def test_cancel_after_preparation_leaves_no_package_action_or_implicit_start(self):
        self.pair_helper()
        status, output = self.run_script(extra=self.pair_fixture(), action="update-pair", response="PREPARE\n\n")
        self.assertEqual(status, 1, output)
        self.assertEqual(self.trace(), ["preparation"])
        self.assertTrue((self.directory / "prepared").exists())
        self.assertFalse((self.directory / "upgraded").exists())
        self.assertEqual(self.store.read_bytes(), b"synthetic private-store sentinel")
        self.assertFalse((self.directory / "omavless-first-run.lock").exists())

    def test_preparation_failure_never_submits_packages_or_other_effects(self):
        self.pair_helper(preparation_fails=True)
        status, output = self.run_script(extra=self.pair_fixture(), action="update-pair", response="PREPARE\nUPGRADE\n")
        self.assertEqual(status, 1, output)
        self.assertEqual(self.trace(), ["preparation"])
        self.assertFalse((self.directory / "prepared").exists())
        self.assertFalse((self.directory / "upgraded").exists())
        self.assertFalse((self.directory / "omavless-first-run.lock").exists())

    def test_real_pty_upgrade_fixed_pair_metadata_and_separate_dns_next_step(self):
        status, output = self.run_script()
        self.assertEqual(status, 0, output)
        self.assertEqual(self.trace(), ["helper", "reload"])
        self.assertIn("Set up DNS helper", output)
        self.assertEqual(json.loads((self.directory / "metadata").read_text()), {
            "schemaVersion": 1, "appSHA": self.hashes[0], "dnsSHA": self.hashes[1],
            "sourceCommit": SOURCE, "version": VERSION, "arch": "x86_64"})
        self.assertEqual(self.store.read_bytes(), b"synthetic private-store sentinel")
        self.assertFalse((self.directory / "omavless-first-run.lock").exists())
        self.assertFalse(list(self.directory.glob("omavless-first-run.*")))

    def test_upgrade_requires_tty_and_native_consent(self):
        status, _ = self.run_script(tty=False)
        self.assertEqual(status, 2)
        self.assertEqual(self.trace(), [])
        status, _ = self.run_script(response="\n")
        self.assertEqual(status, 1)
        self.assertEqual(self.trace(), [])
        self.assertFalse((self.directory / "upgraded").exists())

    def test_interrupted_download_does_not_poison_the_final_cache_or_retry(self):
        for path, name in ((self.app, "download-app"), (self.dns, "download-dns")):
            shutil.copyfile(path, self.directory / name)
            path.unlink()
        partial = '''
curl() {
  while [[ $# -gt 0 ]]; do
    if [[ "$1" == --output ]]; then printf partial > "$2"; return 1; fi
    shift
  done
  return 99
}
'''
        status, _ = self.run_script(extra=partial)
        self.assertEqual(status, 1)
        self.assertFalse(self.app.exists())
        self.assertFalse(self.dns.exists())
        self.assertFalse(list(self.cache.glob(".download.*")))
        self.assertFalse((self.directory / "omavless-first-run.lock").exists())
        complete = '''
curl() {
  local output='' url="${@: -1}"
  while [[ $# -gt 0 ]]; do
    if [[ "$1" == --output ]]; then output="$2"; break; fi
    shift
  done
  case "$url" in
    *'/omavless-dns-'*) /usr/bin/cp "$TEST_DIR/download-dns" "$output";;
    *'/omavless-'*) /usr/bin/cp "$TEST_DIR/download-app" "$output";;
    *) return 99;;
  esac
}
'''
        status, output = self.run_script(extra=complete)
        self.assertEqual(status, 0, output)
        self.assertEqual(self.trace(), ["helper", "reload"])
        self.assertFalse(list(self.cache.glob(".download.*")))

    def test_previous_delivery_cache_is_not_reused_or_overwritten(self):
        obsolete = self.cache.parent / ("a" * 64 + "." + "c" * 64)
        obsolete.mkdir(mode=0o700)
        old_app = obsolete / self.app.name
        old_app.write_bytes(b"different previously verified delivery")
        old_app.chmod(0o600)
        status, output = self.run_script()
        self.assertEqual(status, 0, output)
        self.assertEqual(old_app.read_bytes(), b"different previously verified delivery")

    def test_upgrade_downloads_only_fixed_urls_and_verifies_both_archives(self):
        for path, name in ((self.app, "download-app"), (self.dns, "download-dns")):
            shutil.copyfile(path, self.directory / name)
            path.unlink()
        extra = f'''
curl() {{
  local output='' url="${{@: -1}}"
  while [[ $# -gt 0 ]]; do
    if [[ "$1" == --output ]]; then output="$2"; break; fi
    shift
  done
  case "$url" in
    'https://github.com/k-kostin/omavless/releases/download/v0.9.8-fix.7/omavless-{VERSION}-1-x86_64.pkg.tar.zst')
      /usr/bin/cp "$TEST_DIR/download-app" "$output"; echo download-app >> "$TEST_DIR/trace";;
    'https://github.com/k-kostin/omavless/releases/download/v0.9.8-fix.7/omavless-dns-{VERSION}-1-x86_64.pkg.tar.zst')
      /usr/bin/cp "$TEST_DIR/download-dns" "$output"; echo download-dns >> "$TEST_DIR/trace";;
    *) return 99;;
  esac
}}
'''
        status, output = self.run_script(extra=extra)
        self.assertEqual(status, 0, output)
        self.assertEqual(self.trace(), ["download-app", "download-dns", "helper", "reload"])
        self.assertEqual(self.app.stat().st_mode & 0o777, 0o600)
        self.assertEqual(self.dns.stat().st_mode & 0o777, 0o600)

    def test_upgrade_retains_lock_refusal_and_failed_helper_boundary(self):
        lock = self.directory / "omavless-first-run.lock"
        lock.mkdir()
        status, output = self.run_script()
        self.assertEqual(status, 1)
        self.assertIn("stage: setup-lock", output)
        self.assertTrue(lock.is_dir())
        self.assertFalse((self.directory / "metadata").exists())
        self.assertEqual(self.trace(), [])
        lock.rmdir()
        self.helper.write_text(self.helper.read_text().replace('echo helper >>', 'exit 1\necho helper >>'))
        self.helper_hash = hashlib.sha256(self.helper.read_bytes()).hexdigest()
        status, output = self.run_script()
        self.assertEqual(status, 1)
        self.assertIn("stage: upgrade-operation", output)
        self.assertEqual(self.trace(), [])
        self.assertFalse((self.directory / "upgraded").exists())

    def test_upgrade_rejects_wrong_or_stale_state_before_helper(self):
        for state in ("ready", "needs_package", "needs_attention", "needs_dns_setup"):
            with self.subTest(state=state):
                status, _ = self.run_script(extra=f'\nsetup_status() {{ echo {state}; }}\n')
                self.assertEqual(status, 1)
                self.assertFalse((self.directory / "metadata").exists())
                self.assertEqual(self.trace(), [])
        status, output = self.run_script(extra='''
setup_status() { if [[ -d "$TEST_DIR/omavless-first-run.lock" ]]; then echo needs_attention; else echo needs_update; fi; }
''')
        self.assertEqual(status, 1)
        self.assertIn("stage: setup-state", output)
        self.assertEqual(self.trace(), [])

    def test_upgrade_refuses_tampered_pair_before_extracting_helper(self):
        for target in (self.app, self.dns):
            original = target.read_bytes()
            target.write_bytes(original + b"tampered")
            with self.subTest(package=target.name):
                status, _ = self.run_script()
                self.assertEqual(status, 1)
                self.assertFalse((self.directory / "metadata").exists())
                self.assertEqual(self.trace(), [])
            target.write_bytes(original)

    def test_upgrade_refuses_wrong_receipt_binary_and_archive_metadata(self):
        for extra in (
            '\napp_archive_identity() { return 1; }\n',
            '\ndns_archive_identity() { return 1; }\n',
            '\npackage_info() { echo foreign-package; }\n',
            '\nbsdtar() { echo unverified-helper; }\n',
        ):
            with self.subTest(boundary=extra):
                status, _ = self.run_script(extra=extra)
                self.assertEqual(status, 1)
                self.assertFalse((self.directory / "metadata").exists())
                self.assertEqual(self.trace(), [])
        self.helper.write_text(self.helper.read_text() + "# changed after receipt\n")
        status, _ = self.run_script()
        self.assertEqual(status, 1)
        self.assertFalse((self.directory / "metadata").exists())

    def test_upgrade_never_adopts_unsafe_cache_or_linked_archive(self):
        self.cache.chmod(0o755)
        status, _ = self.run_script()
        self.assertEqual(status, 1)
        self.assertEqual(self.trace(), [])
        self.cache.chmod(0o700)
        actual = self.cache / "actual-app"
        self.app.rename(actual)
        self.app.symlink_to(actual)
        status, _ = self.run_script()
        self.assertEqual(status, 1)
        self.assertEqual(self.trace(), [])


if __name__ == "__main__":
    unittest.main()
