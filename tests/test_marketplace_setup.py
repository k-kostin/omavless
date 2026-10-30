# SPDX-License-Identifier: MIT
"""0.9 first-run composition with synthetic boundaries only.

No test invokes pacman, sudo, systemd, a network client, or a private store.
"""
import hashlib
import errno
import json
import os
from pathlib import Path
import pty
import re
import select
import shutil
import subprocess
import tempfile
import time
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "plugin/setup-runtime.sh"
# Explicit next-snapshot selection. This tests the fixed 0.9.9 entry with inert
# providers, not publication or successful downloads of an unavailable release.
VERSION = "0.9.9"
CANDIDATE_VERSION = json.loads((ROOT / "manifest.json").read_text())["version"]
PKGVER = subprocess.check_output(
    ["bash", str(ROOT / "packaging/release/version-mode.sh"), VERSION], text=True
).split()[1]
SOURCE = "b" * 40
RELEASE_TAG = "v0.9.9"


class MarketplaceSetupTests(unittest.TestCase):
    def setUp(self):
        parent = Path.home() / ".cache/ovt"
        parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(prefix="omavless-bootstrap-test-", dir=parent)
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        shutil.copyfile(ROOT / "plugin/setup-predecessor.sh", self.directory / "setup-predecessor.sh")
        self.env = {"PATH": "/usr/bin:/bin", "HOME": str(self.directory),
                    "TEST_DIR": str(self.directory)}

    def run_shell(self, code, input=""):
        prefix = '''
source "$1"
setup_dir="$TEST_DIR"
setup_temp="$TEST_DIR"
native_present() { return 1; }
native_target() { echo legacy; }
native() { echo UNEXPECTED_NATIVE_EFFECT >&2; return 99; }
package_install() { echo UNEXPECTED_PACKAGE_EFFECT >&2; return 99; }
package_installed() { return 1; }
dns_package_registered() { return 1; }
pair_installed() { return 1; }
installed_pair_delivery() { echo current; }
fresh_user_runtime_absent() { return 0; }
pair_selection_status() { return 1; }
user_runtime_stopped() { return 0; }
system_broker_idle() { return 1; }
system_broker_available() { return 1; }
system_broker_stopped() { return 1; }
no_broker_socket() { return 1; }
broker_access_for_user() { return 1; }
no_managed_tun() { return 0; }
enroll_uid() { echo UNEXPECTED_ENROLL_EFFECT >&2; return 99; }
start_broker() { echo UNEXPECTED_BROKER_EFFECT >&2; return 99; }
enable_runtime() { echo UNEXPECTED_ENABLE_EFFECT >&2; return 99; }
curl() { echo UNEXPECTED_NETWORK_EFFECT >&2; return 99; }
'''
        return subprocess.run(["/bin/bash", "-c", prefix + code, "test", str(SCRIPT)],
                              env=self.env, input=input, text=True, capture_output=True,
                              timeout=10, check=False)

    def run_terminal(self, code, input="INSTALL\nSELECT\n"):
        """Real shell/consent/cleanup, fixed synthetic boundaries; no host effects."""
        if os.geteuid() == 0:
            self.skipTest("interactive setup deliberately refuses root")
        master, slave = pty.openpty()
        env = {**self.env, "XDG_RUNTIME_DIR": str(self.directory)}
        process = None
        output = bytearray()
        try:
            process = subprocess.Popen(
                ["/bin/bash", "-c", 'source "$1"\n' + code, "test", str(SCRIPT)],
                env=env, stdin=slave, stdout=slave, stderr=slave,
            )
            os.close(slave)
            slave = -1
            os.write(master, input.encode())
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
                    self.assertLessEqual(len(output), 16384, "bounded synthetic output")
                elif process.poll() is not None:
                    break
            returncode = process.wait(timeout=1)
            return returncode, output.decode().replace("\r\n", "\n")
        finally:
            if process is not None and process.poll() is None:
                # Only this fixture-owned shell; no resource-owning real commands.
                process.kill()
                process.wait(timeout=2)
            os.close(master)
            if slave != -1:
                os.close(slave)

    def terminal_fixture(self):
        store = self.directory / ".config/omavless/profiles.json"
        store.parent.mkdir(parents=True, exist_ok=True)
        store.write_text("synthetic existing profile data")
        return '''
setup_status() { echo needs_activation; }
setup_runtime_directory_safe() { return 0; }
native_target() { if [[ -f "$TEST_DIR/activated" ]]; then echo rust; else echo legacy; fi; }
pair_installed() { return 0; }
user_runtime_stopped() { return 0; }
no_managed_tun() { return 0; }
system_broker_idle() { return 0; }
system_broker_available() { return 0; }
broker_access_for_user() { return 0; }
pair_selection_status() {
  if [[ -f "$TEST_DIR/selected" ]]; then
    echo '{"schemaVersion":1,"scope":"local_pair_only","selected":true}'
  else
    echo '{"schemaVersion":1,"scope":"local_pair_only","selected":false}'
  fi
}
enroll_uid() { echo UNEXPECTED_ENROLL >> "$TEST_DIR/trace"; return 99; }
start_broker() { echo UNEXPECTED_START >> "$TEST_DIR/trace"; return 99; }
package_install() { echo UNEXPECTED_INSTALL >> "$TEST_DIR/trace"; return 99; }
enable_runtime() { echo enable >> "$TEST_DIR/trace"; }
native() {
  echo "$*" >> "$TEST_DIR/trace"
  case "$*" in
    store-compatibility) echo '{"schemaVersion":1,"compatible":true}' ;;
    'dns-pair prepare-template') : ;;
    'dns-pair select') touch "$TEST_DIR/selected" ;;
    'cutover activate') touch "$TEST_DIR/activated" ;;
    *) return 99 ;;
  esac
}
'''

    def pins(self, *, app=None, dns=None, source=SOURCE, version=VERSION):
        entry = lambda digest: {"sha256": digest, "sourceCommit": source}
        for name, digest in (("runtime-release.json", app),
                             ("dns-release.json", dns)):
            data = {"schemaVersion": 1, "version": version,
                    "packages": ({"aarch64": entry(digest),
                                  "x86_64": entry(digest)} if digest else {})}
            (self.directory / name).write_text(json.dumps(data))

    def pending_dns_fixture(self):
        return self.terminal_fixture() + '''
setup_status() {
  if [[ -f "$TEST_DIR/selected" ]]; then echo needs_runtime_start;
  else echo needs_dns_setup; fi
}
native_target() { echo rust; }
system_broker_stopped() { [[ ! -f "$TEST_DIR/broker-started" ]]; }
no_broker_socket() { [[ ! -f "$TEST_DIR/broker-started" ]]; }
system_broker_idle() { [[ -f "$TEST_DIR/broker-started" ]]; }
broker_access_for_user() { [[ -f "$TEST_DIR/broker-started" ]]; }
existing_enrollment_metadata() { echo metadata >> "$TEST_DIR/trace"; }
start_stopped_broker_service() { echo start-existing >> "$TEST_DIR/trace"; touch "$TEST_DIR/broker-started"; }
enroll_uid() { echo first-enroll >> "$TEST_DIR/trace"; }
start_broker() { echo start-first >> "$TEST_DIR/trace"; touch "$TEST_DIR/broker-started"; }
await_broker_ready() { system_broker_idle && broker_access_for_user; }
'''

    def test_pending_dns_existing_enrollment_continues_without_reactivation(self):
        code = self.pending_dns_fixture() + '\nsetup_main resume-dns en\n'
        status, output = self.run_terminal(code, 'START\nSELECT\n')
        self.assertEqual(status, 0, output)
        self.assertEqual((self.directory / 'trace').read_text().splitlines(),
                         ['metadata', 'start-existing', 'dns-pair prepare-template', 'dns-pair select'])
        self.assertIn('Setup is complete', output)
        self.assertIn('"Terminal and system prompts are closed"', output)
        self.assertIn('"Start OmaVLESS"', output)
        self.assertEqual((self.directory / '.config/omavless/profiles.json').read_text(),
                         'synthetic existing profile data')

    def test_pending_dns_first_enrollment_requires_its_separate_consent(self):
        code = self.pending_dns_fixture() + '\nsetup_main resume-dns en\n'
        status, output = self.run_terminal(code, 'FIRST\nDNS\n')
        self.assertEqual(status, 0, output)
        self.assertEqual((self.directory / 'trace').read_text().splitlines(),
                         ['first-enroll', 'start-first', 'dns-pair prepare-template', 'dns-pair select'])

    def test_dns_choices_are_separate_readable_lines_in_both_locales(self):
        for locale, title, first, existing in [
            ('en', 'Choose how to finish setting up the DNS helper:', 'FIRST — first-time setup',
             'START — the DNS helper was already set up successfully for your account'),
            ('ru', 'Выберите, как завершить настройку DNS-помощника:', 'FIRST — первая настройка',
             'START — DNS-помощник на этом компьютере уже успешно настроен для вашей учётной записи'),
        ]:
            with self.subTest(locale=locale):
                code = self.pending_dns_fixture() + f'\nsetup_main resume-dns {locale}\n'
                status, output = self.run_terminal(code, '\n')
                self.assertEqual(status, 1)
                self.assertIn(title, output)
                lines = output.splitlines()
                self.assertTrue(any(first in line and 'START' not in line for line in lines))
                self.assertTrue(any(existing in line and 'FIRST' not in line for line in lines))
                self.assertFalse((self.directory / 'trace').exists())

    def test_shell_reload_needs_exact_consent_and_fresh_stopped_state(self):
        for answer, state, stopped, effects in [
            ('\n', 'needs_dns_setup', True, []),
            ('RELOAD; echo unsafe\n', 'needs_dns_setup', True, []),
            ('RELOAD\n', 'ready', True, []),
            ('RELOAD\n', 'needs_dns_setup', False, []),
            ('RELOAD\n', 'needs_dns_setup', True, ['restart shell']),
        ]:
            with self.subTest(answer=answer, state=state, stopped=stopped):
                trace = self.directory / 'reload-trace'
                if trace.exists():
                    trace.unlink()
                code = f'''
setup_status() {{ echo {state}; }}
user_runtime_stopped() {{ return {0 if stopped else 1}; }}
setup_runtime_directory_safe() {{ return 0; }}
omarchy() {{ echo "$*" >> "$TEST_DIR/reload-trace"; }}
offer_shell_reload
'''
                status, output = self.run_terminal(code, answer)
                self.assertEqual(status, 0, output)
                self.assertEqual(trace.read_text().splitlines() if trace.exists() else [], effects)
                self.assertIn('other shell plugins also reload', output)

    def test_shell_reload_failure_does_not_turn_package_success_into_retry(self):
        code = '''
setup_status() { echo needs_dns_setup; }
user_runtime_stopped() { return 0; }
setup_runtime_directory_safe() { return 0; }
omarchy() { echo "$*" >> "$TEST_DIR/reload-trace"; return 1; }
offer_shell_reload
'''
        status, output = self.run_terminal(code, 'RELOAD\n')
        self.assertEqual(status, 0, output)
        self.assertIn('Packages remain updated; do not repeat UPGRADE.', output)
        self.assertEqual((self.directory / 'reload-trace').read_text().splitlines(), ['restart shell'])

    def test_shell_reload_refuses_an_occupied_or_unsafe_setup_scope(self):
        for kind in ['directory', 'file', 'dangling-symlink', 'unsafe-runtime']:
            with self.subTest(kind=kind):
                lock = self.directory / 'omavless-first-run.lock'
                if kind == 'directory':
                    lock.mkdir()
                elif kind == 'file':
                    lock.write_text('synthetic retained lock')
                elif kind == 'dangling-symlink':
                    lock.symlink_to('missing')
                code = f'''
setup_status() {{ echo needs_dns_setup; }}
user_runtime_stopped() {{ return 0; }}
setup_runtime_directory_safe() {{ return {1 if kind == 'unsafe-runtime' else 0}; }}
omarchy() {{ echo UNEXPECTED_RELOAD >> "$TEST_DIR/reload-trace"; }}
offer_shell_reload
'''
                status, output = self.run_terminal(code, 'RELOAD\n')
                self.assertEqual(status, 0, output)
                self.assertFalse((self.directory / 'reload-trace').exists())
                if kind == 'directory':
                    lock.rmdir()
                elif kind != 'unsafe-runtime':
                    lock.unlink()

    def test_direct_entry_offers_reload_only_after_successful_upgrade(self):
        entry = SCRIPT.read_text().split('if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then\n', 1)[1]
        entry = entry.rsplit('\nfi', 1)[0]
        for action, result, expected in [
            ('upgrade', 0, ['offered']), ('upgrade', 1, []),
            ('install', 0, []), ('resume-dns', 0, []), ('start-broker', 0, []),
        ]:
            with self.subTest(action=action, result=result):
                trace = self.directory / 'entry-trace'
                if trace.exists():
                    trace.unlink()
                code = f'''
setup_main() {{ return {result}; }}
offer_shell_reload() {{ echo offered >> "$TEST_DIR/entry-trace"; }}
say() {{ :; }}
set -- {action} en
''' + entry
                status, output = self.run_terminal(code, '\n')
                self.assertEqual(status, result, output)
                self.assertEqual(trace.read_text().splitlines() if trace.exists() else [], expected)

    def test_pending_dns_failed_existing_metadata_never_infers_first_enrollment(self):
        code = self.pending_dns_fixture() + '''
existing_enrollment_metadata() { echo metadata-refused >> "$TEST_DIR/trace"; return 1; }
setup_main resume-dns en
'''
        status, output = self.run_terminal(code, 'START\n')
        self.assertEqual(status, 1)
        self.assertIn('stage: broker-enrollment-check', output)
        self.assertEqual((self.directory / 'trace').read_text().splitlines(), ['metadata-refused'])
        self.assertFalse((self.directory / 'selected').exists())

    def test_pending_dns_cancel_and_stale_boundary_do_not_submit_effects(self):
        for extra, response in [('', '\n'), ('user_runtime_stopped() { return 1; }', 'FIRST\nDNS\n'),
                                ('no_broker_socket() { return 1; }', 'START\nSELECT\n'),
                                ('native_target() { echo unknown; }', 'FIRST\nDNS\n')]:
            with self.subTest(extra=extra):
                code = self.pending_dns_fixture() + '\n' + extra + '\nsetup_main resume-dns en\n'
                status, _ = self.run_terminal(code, response)
                self.assertEqual(status, 1)
                self.assertFalse((self.directory / 'trace').exists())
                self.assertFalse((self.directory / 'selected').exists())

    def test_stable_package_identity_requires_stable_schema_and_exact_tuple(self):
        for schema in ("2", "3"):
            code = f'''timeout() {{ shift; "$@"; }}
bsdtar() {{ printf '%s\\n' 'schemaVersion={schema}' 'sourceCommit={SOURCE}' 'architecture=aarch64' 'productVersion={VERSION}'; }}
app_archive_identity synthetic-archive {SOURCE} aarch64 {VERSION}
'''
            self.assertEqual(self.run_shell(code).returncode, 0 if schema == "3" else 1)
        for source, arch, version in (("c" * 40, "aarch64", VERSION),
                                      (SOURCE, "x86_64", VERSION),
                                      (SOURCE, "aarch64", "0.9.8-rc.1")):
            code = f'''timeout() {{ shift; "$@"; }}
bsdtar() {{ printf '%s\\n' 'schemaVersion=3' 'sourceCommit={SOURCE}' 'architecture=aarch64' 'productVersion={VERSION}'; }}
app_archive_identity synthetic-archive {source} {arch} {version}
'''
            self.assertNotEqual(self.run_shell(code).returncode, 0)

    def test_selected_delivery_has_fixed_tag_and_product_version(self):
        self.env["release_tag"] = "v0.8.2/caller-selected"
        result = self.run_shell('printf "%s\\n" "$release_tag" "$release_version" "$package_version"')
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, f"{RELEASE_TAG}\n{VERSION}\n{PKGVER}\n")
        changed = self.run_shell('release_tag=caller-selected')
        self.assertNotEqual(changed.returncode, 0)

    def test_frontend_archive_help_matches_selected_delivery(self):
        help_text = (ROOT / "packaging/release/FRONTEND_README.md").read_text()
        tags = set(re.findall(r"v\d+\.\d+\.\d+(?:-fix\.\d+)?", help_text))
        self.assertEqual(tags, {RELEASE_TAG})
        self.assertIn("docs/user/NATIVE_INSTALL.md", help_text)
        self.assertIn("docs/user/NATIVE_INSTALL.md#remove", help_text)
        self.assertIn("only the panel files", help_text)

    def test_selected_candidate_has_matching_bounded_pair_pins(self):
        manifest = json.loads((ROOT / "manifest.json").read_text())
        records = {}
        for filename in ("runtime-release.json", "dns-release.json"):
            metadata = json.loads((ROOT / "plugin" / filename).read_text())
            self.assertEqual(metadata["schemaVersion"], 1)
            self.assertEqual(metadata["version"], manifest["version"])
            # An unpublished native-architecture candidate must not invent
            # the other architecture's pins. release_fields refuses absent
            # architectures; paired app/DNS coverage is checked below.
            self.assertTrue(set(metadata["packages"]) <= {"aarch64", "x86_64"})
            for entry in metadata["packages"].values():
                self.assertEqual(set(entry), {"sha256", "sourceCommit"})
                self.assertRegex(entry["sha256"], r"^[0-9a-f]{64}$")
                self.assertRegex(entry["sourceCommit"], r"^[0-9a-f]{40}$")
            records[filename] = metadata
            shutil.copyfile(ROOT / "plugin" / filename, self.directory / filename)
        self.assertEqual(manifest["version"], CANDIDATE_VERSION)
        self.assertEqual(CANDIDATE_VERSION, VERSION)
        self.assertEqual(set(records["runtime-release.json"]["packages"]),
                         set(records["dns-release.json"]["packages"]))
        for arch, app in records["runtime-release.json"]["packages"].items():
            dns = records["dns-release.json"]["packages"][arch]
            self.assertEqual(app["sourceCommit"], dns["sourceCommit"])
        if not records["runtime-release.json"]["packages"]:
            for arch in ("aarch64", "x86_64"):
                self.assertNotEqual(self.run_shell(f'uname() {{ echo {arch}; }}; release_fields').returncode, 0)
                self.assertEqual(self.run_shell(f'uname() {{ echo {arch}; }}; setup_status').stdout,
                                 "release_unavailable\n")
            return
        for arch in ("aarch64", "x86_64"):
            result = self.run_shell(f'uname() {{ echo {arch}; }}; release_fields')
            if arch not in records["runtime-release.json"]["packages"]:
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.run_shell(f'uname() {{ echo {arch}; }}; setup_status').stdout,
                                 "release_unavailable\n")
                continue
            self.assertEqual(result.returncode, 0)
            app = records["runtime-release.json"]["packages"][arch]
            dns = records["dns-release.json"]["packages"][arch]
            self.assertEqual(result.stdout, f"{VERSION}\t{arch}\t{app['sha256']}\t{dns['sha256']}\t{app['sourceCommit']}\n")
            self.assertEqual(self.run_shell(f'uname() {{ echo {arch}; }}; setup_status').stdout,
                             "needs_package\n")

    def test_unpublished_candidate_refuses_without_both_pins(self):
        self.pins()
        for arch in ("aarch64", "x86_64"):
            self.assertNotEqual(self.run_shell(f'uname() {{ echo {arch}; }}; release_fields').returncode, 0)
            self.assertEqual(self.run_shell(f'uname() {{ echo {arch}; }}; setup_status').stdout,
                             "release_unavailable\n")

    def test_prior_version_metadata_cannot_retarget_selected_099_downloads(self):
        self.pins(app="a" * 64, dns="c" * 64, version="0.9.8")
        for arch in ("aarch64", "x86_64"):
            result = self.run_shell(f'uname() {{ echo {arch}; }}; release_fields')
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
        result = self.run_shell('printf "%s\\n" "$release_tag" "$release_version" "$package_version"')
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, f"{RELEASE_TAG}\n{VERSION}\n{PKGVER}\n")

    def test_selected_candidate_still_requires_same_producer_before_handoff(self):
        fixture = self.directory / "synthetic-candidate"
        (fixture / "plugin").mkdir(parents=True)
        (fixture / "manifest.json").write_text(json.dumps({"version": "0.9.9"}))
        app = {"schemaVersion": 1, "version": "0.9.9", "packages": {
            arch: {"sha256": "a" * 64, "sourceCommit": SOURCE}
            for arch in ("aarch64", "x86_64")}}
        dns = json.loads(json.dumps(app))
        (fixture / "plugin/runtime-release.json").write_text(json.dumps(app))
        path = fixture / "plugin/dns-release.json"
        with patch(f"{__name__}.ROOT", fixture), patch(f"{__name__}.CANDIDATE_VERSION", "0.9.9"):
            # Coherent synthetic metadata earns only parsing/handoff, not real
            # producer provenance, a published download or installed admission.
            path.write_text(json.dumps(dns))
            self.test_selected_candidate_has_matching_bounded_pair_pins()
            for arch in ("aarch64", "x86_64"):
                changed = json.loads(json.dumps(dns))
                changed["packages"][arch]["sourceCommit"] = "d" * 40
                path.write_text(json.dumps(changed))
                with self.subTest(arch=arch), self.assertRaises(AssertionError):
                    self.test_selected_candidate_has_matching_bounded_pair_pins()

    def test_pair_requires_both_exact_pins_and_one_source(self):
        self.pins(app="a" * 64, dns="c" * 64)
        for arch in ("aarch64", "x86_64"):
            result = self.run_shell(f'uname() {{ echo {arch}; }}; release_fields')
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout,
                             f"{VERSION}\t{arch}\t{'a' * 64}\t{'c' * 64}\t{SOURCE}\n")
            self.assertEqual(self.run_shell(f'uname() {{ echo {arch}; }}; setup_status').stdout,
                             "needs_package\n")
        for filename in ("runtime-release.json", "dns-release.json"):
            path = self.directory / filename
            original = path.read_text()
            path.write_text(json.dumps({"schemaVersion": 1, "version": VERSION,
                                        "packages": {}}))
            self.assertNotEqual(self.run_shell("release_fields").returncode, 0)
            path.write_text(original)
        dns = self.directory / "dns-release.json"
        content = json.loads(dns.read_text())
        content["packages"]["x86_64"]["sourceCommit"] = "d" * 40
        dns.write_text(json.dumps(content))
        self.assertNotEqual(self.run_shell('uname() { echo x86_64; }; release_fields').returncode, 0)

    def test_malformed_or_linked_metadata_never_reaches_install(self):
        self.pins(app="a" * 64, dns="c" * 64)
        path = self.directory / "dns-release.json"
        original = path.read_text()
        for value in ("secret://synthetic", "[]", "x" * 8193,
                      json.dumps({"schemaVersion": 1, "version": VERSION,
                                  "packages": {"x86_64": {"sha256": "x",
                                                          "sourceCommit": SOURCE}}}),
                      json.dumps({"schemaVersion": 1, "version": VERSION,
                                  "packages": {"x86_64": {"sha256": "c" * 64,
                                                          "sourceCommit": SOURCE,
                                                          "url": "https://untrusted.invalid"}}})):
            path.write_text(value)
            result = self.run_shell("release_fields")
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout + result.stderr, "")
        path.write_text(original)
        path.rename(self.directory / "dns-real.json")
        path.symlink_to(self.directory / "dns-real.json")
        self.assertNotEqual(self.run_shell("release_fields").returncode, 0)

    def test_existing_package_states_are_bounded_and_read_only(self):
        fixture = '''
native_present() { return 0; }
package_installed() { echo "omavless $package_version-1"; }
pair_installed() { return 0; }
system_broker_available() { return 0; }
native_target() { echo rust; }
pair_selection_status() { echo '{"schemaVersion":1,"scope":"local_pair_only","selected":true}'; }
user_runtime_stopped() { return 1; }
'''
        self.assertEqual(self.run_shell(fixture + "setup_status").stdout, "ready\n")
        self.assertEqual(self.run_shell(fixture + '''
pair_selection_status() { echo '{"schemaVersion":1,"scope":"local_pair_only","selected":false}'; }
user_runtime_stopped() { return 0; }
setup_status
''').stdout, "needs_selection\n")
        self.assertEqual(self.run_shell(fixture + 'system_broker_available() { return 1; }; setup_status').stdout,
                         "needs_broker\n")
        self.assertEqual(self.run_shell(fixture + '''
system_broker_available() { return 1; }
system_broker_stopped() { return 0; }
user_runtime_stopped() { return 0; }
runtime_start_available() { return 0; }
no_broker_socket() { return 0; }
setup_status
''').stdout, "needs_broker_stopped\n")
        self.assertEqual(self.run_shell(fixture + '''
pair_selection_status() { echo '{"schemaVersion":1,"scope":"local_pair_only","selected":false}'; }
user_runtime_stopped() { return 1; }
setup_status
''').stdout, "needs_runtime_stop\n")
        self.assertEqual(self.run_shell(fixture + 'native_target() { echo legacy; }; setup_status').stdout,
                         "needs_activation\n")
        self.assertEqual(self.run_shell(fixture + 'pair_installed() { return 1; }; setup_status').stdout,
                         "needs_companion\n")
        self.assertEqual(self.run_shell(fixture + 'package_installed() { echo "omavless 0.8.2-1"; }; setup_status').stdout,
                         "needs_update\n")
        self.assertEqual(self.run_shell(fixture + 'pair_selection_status() { return 1; }; setup_status').stdout,
                         "needs_attention\n")

    def test_same_version_delivery_state_is_not_hidden_by_ready_or_unpublished_updater(self):
        fixture = '''
native_present() { return 0; }
package_installed() { echo "omavless $package_version-1"; }
native_target() { echo rust; }
pair_installed() { return 0; }
'''
        for delivery, available, expected in [
                ('missing', False, 'needs_companion'),
                ('missing', True, 'needs_pair_install'),
                ('mismatch', True, 'needs_pair_mismatch'),
                ('unknown', True, 'needs_attention')]:
            with self.subTest(delivery=delivery, available=available):
                code = fixture + f'installed_pair_delivery() {{ echo {delivery}; }}; pair_update_available() {{ return {0 if available else 1}; }}; setup_status'
                result = self.run_shell(code)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, expected + '\n')

    def test_stale_pair_freshness_preserves_ready_and_both_start_continuations(self):
        fixture = '''
native_present() { return 0; }
package_installed() { echo "omavless $package_version-1"; }
native_target() { echo rust; }
pair_installed() { return 0; }
installed_pair_delivery() { echo stale; }
pair_selection_status() { echo '{"schemaVersion":1,"scope":"local_pair_only","selected":true}'; }
runtime_start_available() { return 0; }
system_broker_stopped() { return 0; }
no_broker_socket() { return 0; }
'''
        for stopped, broker, state in [(False, True, 'ready'),
                                       (True, True, 'needs_runtime_start'),
                                       (True, False, 'needs_broker_stopped')]:
            for available in (False, True):
                code = fixture + f'''
user_runtime_stopped() {{ return {0 if stopped else 1}; }}
system_broker_available() {{ return {0 if broker else 1}; }}
pair_update_available() {{ return {0 if available else 1}; }}
setup_status
setup_components
'''
                result = self.run_shell(code)
                self.assertEqual(result.returncode, 0, result.stderr)
                delivery = 'stale' if available else 'stale_manual'
                self.assertEqual(result.stdout, f'{state}\n{state}\tpresent\t{delivery}\n')
                self.assertEqual(result.stderr, '')

    def test_target_capability_requires_matching_closed_delivery_metadata(self):
        metadata = self.directory / 'component-updates.json'
        source = 'b' * 40
        identity = {key: 'a' * 64 for key in ['appSha','coreSha','brokerSha','dnsReceiptSha']}
        good = {'schemaVersion': 2, 'sourceCommit': source, 'updatePair': True,
                'identities': {'x86_64': identity, 'aarch64': identity}}
        fixture = f'release_fields() {{ printf "{VERSION}\\tx86_64\\t%s\\t%s\\t%s\\n" "{source}" "{source}" "{source}"; }}; pair_update_available'
        metadata.write_text(json.dumps(good))
        self.assertEqual(self.run_shell(fixture).returncode, 0)
        for wrong in [{**good, 'updatePair': False}, {**good, 'updatePair': 'true'},
                      {**good, 'sourceCommit': 'c' * 40}, {**good, 'command': 'ignored'},
                      {**good, 'schemaVersion': 3}, {**good, 'identities': {}},
                      {**good, 'identities': {'x86_64': identity, 'aarch64': {}}},
                      {**good, 'identities': {'x86_64': {**identity, 'appSha': 'short'}, 'aarch64': identity}}]:
            metadata.write_text(json.dumps(wrong))
            self.assertNotEqual(self.run_shell(fixture).returncode, 0)

    def test_stopped_runtime_is_not_installation_or_optimistic_ready(self):
        fixture = '''
native_present() { return 0; }
package_installed() { echo "omavless $package_version-1"; }
pair_installed() { return 0; }
native_target() { echo rust; }
pair_selection_status() { echo '{"schemaVersion":1,"scope":"local_pair_only","selected":true}'; }
user_runtime_stopped() { return 0; }
runtime_start_available() { return 0; }
system_broker_available() { return 0; }
'''
        self.assertEqual(self.run_shell(fixture + "setup_status").stdout, "needs_runtime_start\n")
        self.assertEqual(self.run_shell(fixture + "setup_components").stdout, "needs_runtime_start\tpresent\n")
        self.assertEqual(self.run_shell(fixture + "runtime_start_available() { return 1; }; setup_status").stdout, "needs_attention\n")
        result = self.run_shell(fixture + '''
system_broker_available() { return 1; }
system_broker_stopped() { return 0; }
no_broker_socket() { return 0; }
runtime_start_available() { echo UNEXPECTED_RUNTIME_START_PROBE >&2; return 0; }
setup_status
''')
        self.assertEqual(result.stdout, "needs_broker_stopped\n")
        self.assertEqual(result.stderr, "")
        result = self.run_shell(fixture + '''
system_broker_available() { return 1; }
system_broker_stopped() { return 0; }
no_broker_socket() { return 0; }
no_managed_tun() { return 1; }
setup_status
''')
        self.assertEqual(result.stdout, "needs_broker\n")
        self.assertEqual(result.stderr, "")

    def test_component_inventory_reports_only_fixed_presence(self):
        for state in ["ready", "needs_runtime_start"]:
            result = self.run_shell(f'setup_status() {{ echo {state}; }}; setup_completion')
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stderr, "")
            self.assertIn('In the panel, confirm "Terminal and system prompts are closed"', result.stdout)
            self.assertIn('This setup did not connect a VPN.', result.stdout)
            self.assertNotIn('VPN is off', result.stdout)
            self.assertNotIn('VPN stays off', result.stdout)
        for state in ["needs_broker", "needs_attention", "needs_selection", "unknown"]:
            result = self.run_shell(f'setup_status() {{ echo {state}; }}; setup_completion')
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertEqual(result.stderr, "")

    def test_component_presence_projection_is_closed(self):
        for state in ("ready", "needs_package", "needs_activation", "needs_companion",
                      "needs_selection", "needs_broker", "needs_broker_stopped", "needs_runtime_stop",
                      "release_unavailable", "needs_attention"):
            for present in (True, False):
                result = self.run_shell(f'''setup_status() {{ echo {state}; }}
pair_installed() {{ return {0 if present else 1}; }}
setup_components''')
                self.assertEqual(result.stdout,
                                 state + "\t" + ("present" if present else "missing") + "\n")
                self.assertEqual(result.stderr, "")

    def test_fresh_install_refuses_existing_or_ambiguous_owner(self):
        self.pins(app="a" * 64, dns="c" * 64)
        for override in (
            "native_present() { return 0; }",
            "package_installed() { echo 'omavless 0.8.2-1'; }",
            "dns_package_registered() { return 0; }",
            "fresh_user_runtime_absent() { return 1; }",
            "no_managed_tun() { return 1; }",
        ):
            with self.subTest(override=override):
                result = self.run_shell(override + "; setup_status")
                self.assertEqual(result.stdout, "needs_attention\n")
                self.assertEqual(result.stderr, "")
                self.assertNotEqual(self.run_shell(override + "; fresh_package_boundary").returncode, 0)

    def test_consent_words_are_exact_and_do_not_accept_passwords(self):
        for answer in ("\n", "yes\n", "install\n", "INSTALL extra\n", ""):
            self.assertNotEqual(self.run_shell("confirm", answer).returncode, 0)
        self.assertEqual(self.run_shell("confirm", "INSTALL\n").returncode, 0)
        for answer in ("\n", "ready\n", "DNS extra\n"):
            self.assertNotEqual(self.run_shell("confirm_dns_enrollment", answer).returncode, 0)
        self.assertEqual(self.run_shell("confirm_dns_enrollment", "DNS\n").returncode, 0)

    def test_previous_stable_update_inventory_never_calls_new_dns_or_effects(self):
        fixture = '''
native_present() { return 0; }
package_installed() { echo 'omavless 0.8.2-1'; }
native_target() { echo rust; }
pair_selection_status() { echo UNEXPECTED_NEW_API >&2; return 99; }
setup_components
'''
        result = self.run_shell(fixture)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "needs_update\tmissing\n")
        self.assertEqual(result.stderr, "")
        for override in (
            "native_target() { echo legacy; }",
            "native_target() { return 1; }",
            "package_installed() { echo 'omavless 0.0.0.dev-1'; }",
            "package_installed() { echo 'omavless 9.9.9-1'; }",
        ):
            result = self.run_shell(fixture.replace("setup_components", override + "; setup_components"))
            self.assertEqual(result.stdout, "needs_attention\tmissing\n")
            self.assertEqual(result.stderr, "")

    def test_first_run_warns_about_firewall_without_changing_it(self):
        source = SCRIPT.read_text()
        for phrase in (
            "Firewall rules will not change",
            "Правила межсетевого экрана не меняются",
            'read "Setup help" before connecting',
            'прочитайте «Помощь» перед подключением',
        ):
            self.assertIn(phrase, source)
        self.assertNotIn("/usr/bin/ufw", source)

    def download_fixture(self, *, app_hash_ok=True, dns_hash_ok=True,
                         dns_package_ok=True, app_identity_ok=True,
                         dns_identity_ok=True, boundary_override=""):
        payload = b"synthetic package -- NOT AN ARCH PACKAGE"
        (self.directory / "payload").write_bytes(payload)
        digest = hashlib.sha256(payload).hexdigest()
        app_hash = digest if app_hash_ok else "0" * 64
        dns_hash = digest if dns_hash_ok else "0" * 64
        return self.run_shell(f'''
release_fields() {{ printf '{VERSION}\\tx86_64\\t{app_hash}\\t{dns_hash}\\t{SOURCE}\\n'; }}
curl() {{
  local output= last=
  while [[ $# -gt 0 ]]; do
    if [[ "$1" == --output ]]; then output=$2; shift 2; continue; fi
    last=$1; shift
  done
  printf '%s\\n' "$last" >> "$TEST_DIR/urls"
  cp "$TEST_DIR/payload" "$output"
}}
package_info() {{
  if [[ "$1" == *'/omavless-dns-'* ]]; then
    echo 'omavless-dns {PKGVER + '-1' if dns_package_ok else '0.8.2-1'}'
  else echo 'omavless {PKGVER}-1'; fi
}}
app_archive_identity() {{ return {0 if app_identity_ok else 1}; }}
dns_archive_identity() {{ return {0 if dns_identity_ok else 1}; }}
package_install() {{ printf '%s\\n' "$1" "$2" > "$TEST_DIR/install-args"; touch "$TEST_DIR/installed"; }}
package_installed() {{ [[ -f "$TEST_DIR/installed" ]] && echo 'omavless {PKGVER}-1'; }}
pair_installed() {{ [[ -f "$TEST_DIR/installed" ]]; }}
{boundary_override}
install_package
''')

    def seed_pair_cache(self):
        payload = b"synthetic package -- NOT AN ARCH PACKAGE"
        digest = hashlib.sha256(payload).hexdigest()
        cache = self.directory / ".cache/omavless/upgrade082" / f"{digest}.{digest}"
        cache.mkdir(parents=True, mode=0o700)
        for path in (self.directory / ".cache", cache.parent.parent, cache.parent, cache):
            path.chmod(0o700)
        pair = [cache / f"{name}-{PKGVER}-1-x86_64.pkg.tar.zst"
                for name in ("omavless", "omavless-dns")]
        for path in pair:
            path.write_bytes(payload)
            path.chmod(0o600)
        return cache, pair

    def test_fresh_install_reuses_only_exact_verified_pair_cache(self):
        cache, pair = self.seed_pair_cache()
        before = [(path.read_bytes(), path.stat().st_mtime_ns) for path in pair]
        result = self.download_fixture()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.directory / "urls").exists())
        self.assertEqual((self.directory / "install-args").read_text().splitlines(),
                         [str(path) for path in pair])
        self.assertEqual(before, [(path.read_bytes(), path.stat().st_mtime_ns) for path in pair])
        self.assertFalse(list(cache.glob(".download.*")))

    def test_cached_pair_never_waives_fresh_owner_boundary(self):
        self.seed_pair_cache()
        for override in (
            "native_present() { return 0; }",
            "package_installed() { echo 'omavless 0.8.2-1'; }",
            "dns_package_registered() { return 0; }",
            "fresh_user_runtime_absent() { return 1; }",
            "no_managed_tun() { return 1; }",
            '''native_present() { [[ -f "$TEST_DIR/appeared" ]]; }
dns_archive_identity() { touch "$TEST_DIR/appeared"; }''',
        ):
            with self.subTest(boundary=override):
                result = self.download_fixture(boundary_override=override)
                self.assertNotEqual(result.returncode, 0)
                for name in ("urls", "installed", "install-args"):
                    self.assertFalse((self.directory / name).exists())

    def test_cached_pair_poison_mode_link_and_changed_hash_refuse_without_network(self):
        _, pair = self.seed_pair_cache()
        payload = pair[0].read_bytes()
        for path in pair:
            for poison in ("bytes", "mode", "symlink", "hardlink"):
                with self.subTest(package=path.name, poison=poison):
                    alias = self.directory / "alias"
                    if poison == "bytes": path.write_bytes(b"poisoned archive")
                    elif poison == "mode": path.chmod(0o644)
                    elif poison == "symlink":
                        path.rename(alias)
                        path.symlink_to(alias)
                    else: os.link(path, alias)
                    result = self.download_fixture()
                    self.assertNotEqual(result.returncode, 0)
                    for name in ("urls", "installed", "install-args"):
                        self.assertFalse((self.directory / name).exists())
                    if poison == "symlink":
                        path.unlink()
                        alias.rename(path)
                    elif poison == "hardlink": alias.unlink()
                    path.write_bytes(payload)
                    path.chmod(0o600)

    def test_cached_pair_unsafe_parent_and_digest_directory_refuse(self):
        cache, _ = self.seed_pair_cache()
        for path in (self.directory / ".cache", cache.parent.parent, cache.parent, cache):
            with self.subTest(parent=path.name):
                path.chmod(0o777)
                result = self.download_fixture()
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((self.directory / "install-args").exists())
                self.assertFalse((self.directory / "urls").exists())
                self.assertEqual(path.stat().st_mode & 0o777, 0o777)
                path.chmod(0o700)

    def test_failed_download_does_not_poison_final_cache_name(self):
        digest = hashlib.sha256(b"synthetic package -- NOT AN ARCH PACKAGE").hexdigest()
        result = self.download_fixture(boundary_override='''
curl() {
  while [[ $# -gt 0 ]]; do
    if [[ "$1" == --output ]]; then printf partial > "$2"; return 1; fi
    shift
  done
  return 99
}
''')
        self.assertNotEqual(result.returncode, 0)
        cache = self.directory / ".cache/omavless/upgrade082" / f"{digest}.{digest}"
        self.assertEqual(list(cache.iterdir()), [])
        self.assertFalse((self.directory / "install-args").exists())
        result = self.download_fixture()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(list(cache.glob(".download.*")))

    def test_cached_pair_receipt_identity_is_required_and_hashes_are_rechecked(self):
        _, pair = self.seed_pair_cache()
        for override in (
            {"app_identity_ok": False}, {"dns_identity_ok": False},
            {"dns_package_ok": False},
            {"boundary_override": '''dns_archive_identity() {
  printf poisoned > "$HOME/.cache/omavless/upgrade082/"*"/omavless-"*".pkg.tar.zst"
}'''},
        ):
            with self.subTest(refusal=override):
                result = self.download_fixture(**override)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((self.directory / "urls").exists())
                self.assertFalse((self.directory / "install-args").exists())

    def test_cache_cannot_bypass_missing_architecture_pins(self):
        self.seed_pair_cache()
        self.pins(app="a" * 64, dns="c" * 64)
        for name in ("runtime-release.json", "dns-release.json"):
            path = self.directory / name
            data = json.loads(path.read_text())
            del data["packages"]["aarch64"]
            path.write_text(json.dumps(data))
        result = self.run_shell('uname() { echo aarch64; }; install_package')
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("UNEXPECTED", result.stdout + result.stderr)

    def test_frozen_stable_pair_pins_select_their_own_cache_key(self):
        # Public immutable fix.3 x86_64 tuple retained in the installed VM
        # reproduction report. This is pin/cache selection, not a download or
        # claim that the synthetic package fixtures are these public bytes.
        app = "bf27584874c7e75d40f856739783f66fdf6af5652e37ba638232a2bc5a3c7750"
        dns = "3309fb54ab92c5f1f30f6ba3c925ddc153619841dc4ae5640f4f279383f6b464"
        source = "8d02f9d7dd1a9063a82e9dcd93cb19cf4c5d8524"
        report = (ROOT / "docs/testing/SETUP_082_UPGRADE_VM_2026-10-13.md").read_text()
        for identity in (app, dns, source): self.assertIn(identity, report)
        self.pins(app=app, dns=dns, source=source, version="0.9.8")
        result = self.run_shell('uname() { echo x86_64; }; release_fields')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")
        # Historical hashes still select their own cache namespace; they do
        # not become a 0.9.9 target by changing displayed version text.
        result = self.run_shell(f'setup_release_cache {app} {dns}')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), str(
            self.directory / ".cache/omavless/upgrade082" / f"{app}.{dns}"))
        self.assertFalse((self.directory / "urls").exists())

    def test_shared_cache_rejects_non_digest_keys_and_linked_cache_parent(self):
        for arguments in ("", "a b", "'../arbitrary' " + "a" * 64,
                          "a" * 64 + " " + "c" * 64 + " extra"):
            with self.subTest(arguments=arguments):
                result = self.run_shell("setup_release_cache " + arguments)
                self.assertEqual(result.returncode, 2)
                self.assertFalse((self.directory / ".cache").exists())
        alias = self.directory / "cache-target"
        alias.mkdir(mode=0o700)
        (self.directory / ".cache").symlink_to(alias)
        result = self.run_shell("setup_release_cache " + "a" * 64 + " " + "c" * 64)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(list(alias.iterdir()), [])

    def test_cached_fresh_install_still_requires_install_then_separate_dns_consent(self):
        cache, pair = self.seed_pair_cache()
        digest = hashlib.sha256(pair[0].read_bytes()).hexdigest()
        fixture = f'''
setup_dir="$TEST_DIR"
setup_runtime_directory_safe() {{ return 0; }}
setup_status() {{ echo needs_package; }}
native_present() {{ return 1; }}
package_installed() {{ [[ -f "$TEST_DIR/installed" ]] && echo 'omavless {PKGVER}-1'; }}
dns_package_registered() {{ return 1; }}
fresh_user_runtime_absent() {{ return 0; }}
no_managed_tun() {{ return 0; }}
release_fields() {{ printf '{VERSION}\tx86_64\t{digest}\t{digest}\t{SOURCE}\n'; }}
package_info() {{ case "$1" in *omavless-dns-*) echo 'omavless-dns {PKGVER}-1';; *) echo 'omavless {PKGVER}-1';; esac; }}
app_archive_identity() {{ return 0; }}
dns_archive_identity() {{ return 0; }}
package_install() {{ printf '%s\\n' "$1" "$2" > "$TEST_DIR/install-args"; touch "$TEST_DIR/installed"; echo packages >> "$TEST_DIR/trace"; }}
pair_installed() {{ [[ -f "$TEST_DIR/installed" ]]; }}
reload_units() {{ echo reload >> "$TEST_DIR/trace"; }}
prepare_private_store() {{ echo store >> "$TEST_DIR/trace"; }}
pair_selection_status() {{ echo '{{"schemaVersion":1,"scope":"local_pair_only","selected":false}}'; }}
system_broker_idle() {{ return 1; }}
user_runtime_stopped() {{ return 0; }}
enroll_uid() {{ echo UNEXPECTED_ENROLL >> "$TEST_DIR/trace"; return 99; }}
curl() {{ echo UNEXPECTED_DOWNLOAD >> "$TEST_DIR/trace"; return 99; }}
native() {{ echo UNEXPECTED_NATIVE >> "$TEST_DIR/trace"; return 99; }}
setup_main install en
'''
        status, _ = self.run_terminal(fixture, "\n")
        self.assertEqual(status, 0)  # Existing no-change cancellation outcome.
        self.assertFalse((self.directory / "trace").exists())
        status, output = self.run_terminal(fixture, "INSTALL\n\n")
        self.assertEqual(status, 1)
        self.assertIn("Type DNS", output)
        self.assertIn("stage: dns-consent", output)
        self.assertEqual((self.directory / "trace").read_text().splitlines(),
                         ["packages", "reload", "store"])
        self.assertEqual((self.directory / "install-args").read_text().splitlines(),
                         [str(path) for path in pair])
        self.assertFalse(list(cache.glob(".download.*")))

    def test_fresh_boundary_refuses_before_download_or_install(self):
        for override in (
            "native_present() { return 0; }",
            "package_installed() { echo 'omavless 0.8.2-1'; }",
            "dns_package_registered() { return 0; }",
            "fresh_user_runtime_absent() { return 1; }",
            "no_managed_tun() { return 1; }",
        ):
            with self.subTest(override=override):
                for name in ("installed", "install-args", "urls"):
                    (self.directory / name).unlink(missing_ok=True)
                result = self.download_fixture(boundary_override=override)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((self.directory / "installed").exists())
                self.assertFalse((self.directory / "install-args").exists())
                self.assertFalse((self.directory / "urls").exists())

    def test_both_downloads_are_pinned_before_single_pacman_transaction(self):
        result = self.download_fixture()
        self.assertEqual(result.returncode, 0, result.stderr)
        urls = (self.directory / "urls").read_text().splitlines()
        self.assertEqual(urls, [
            f"https://github.com/k-kostin/omavless/releases/download/{RELEASE_TAG}/omavless-{PKGVER}-1-x86_64.pkg.tar.zst",
            f"https://github.com/k-kostin/omavless/releases/download/{RELEASE_TAG}/omavless-dns-{PKGVER}-1-x86_64.pkg.tar.zst",
        ])
        args = (self.directory / "install-args").read_text().splitlines()
        self.assertEqual(len(args), 2)
        self.assertEqual([Path(arg).name for arg in args],
                         [f"omavless-{PKGVER}-1-x86_64.pkg.tar.zst",
                          f"omavless-dns-{PKGVER}-1-x86_64.pkg.tar.zst"])
        source = SCRIPT.read_text()
        for bound in ("--disable", "--proto '=https'", "--proto-redir '=https'",
                      "--max-filesize", "--connect-timeout", "--max-time"):
            self.assertIn(bound, source)
        self.assertIn('/usr/bin/sudo /usr/bin/pacman -U -- "$1" "$2"', source)
        self.assertNotIn('omarchy pkg aur add', source)

    def test_any_archive_or_identity_failure_precedes_package_install(self):
        for variant in ({"app_hash_ok": False}, {"dns_hash_ok": False},
                        {"dns_package_ok": False}, {"app_identity_ok": False},
                        {"dns_identity_ok": False}):
            with self.subTest(variant=variant):
                for name in ("installed", "install-args", "urls"):
                    (self.directory / name).unlink(missing_ok=True)
                result = self.download_fixture(**variant)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((self.directory / "install-args").exists())

    def test_enrollment_requires_separate_consent_and_sequential_facts(self):
        fixture = '''
pair_installed() { return 0; }
native_present() { return 0; }
package_installed() { echo "omavless $package_version-1"; }
native_target() { echo rust; }
pair_selection_status() { if [[ -f "$TEST_DIR/selected" ]]; then echo '{"schemaVersion":1,"scope":"local_pair_only","selected":true}'; else echo '{"schemaVersion":1,"scope":"local_pair_only","selected":false}'; fi; }
enroll_uid() { echo enroll >> "$TEST_DIR/trace"; }
start_broker() { echo start >> "$TEST_DIR/trace"; touch "$TEST_DIR/broker"; }
system_broker_idle() { [[ -f "$TEST_DIR/broker" ]]; }
broker_access_for_user() { return 0; }
native() { echo "$*" >> "$TEST_DIR/trace"; [[ "$*" != 'dns-pair select' ]] || touch "$TEST_DIR/selected"; }
enroll_and_select_pair
'''
        result = self.run_shell(fixture, "\n")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.directory / "trace").exists())
        result = self.run_shell(fixture, "DNS\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.directory / "trace").read_text().splitlines(),
                         ["enroll", "start", "dns-pair prepare-template", "dns-pair select"])

    def test_enrollment_and_selection_stop_at_first_unknown_effect(self):
        fixture = '''
pair_installed() { return 0; }
enroll_uid() { echo enroll >> "$TEST_DIR/trace"; return 1; }
start_broker() { echo BAD >> "$TEST_DIR/trace"; }
enroll_and_select_pair
'''
        result = self.run_shell(fixture, "DNS\n")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.directory / "trace").read_text().splitlines(), ["enroll"])
        result = self.run_shell(fixture.replace("pair_installed() { return 0; }",
                                                 "pair_installed() { return 0; }; user_runtime_stopped() { return 1; }"),
                                "DNS\n")
        self.assertNotEqual(result.returncode, 0)

    def test_selection_resume_requires_idle_broker_and_explicit_word(self):
        fixture = '''
pair_installed() { return 0; }
system_broker_idle() { return 0; }
broker_access_for_user() { return 0; }
native_present() { return 0; }
package_installed() { echo "omavless $package_version-1"; }
native_target() { echo rust; }
pair_selection_status() { if [[ -f "$TEST_DIR/selected" ]]; then echo '{"schemaVersion":1,"scope":"local_pair_only","selected":true}'; else echo '{"schemaVersion":1,"scope":"local_pair_only","selected":false}'; fi; }
native() { echo "$*" >> "$TEST_DIR/trace"; [[ "$*" != 'dns-pair select' ]] || touch "$TEST_DIR/selected"; }
finish_pair_selection
'''
        for answer in ("\n", "DNS\n", "select\n"):
            result = self.run_shell(fixture, answer)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((self.directory / "trace").exists())
        self.assertEqual(self.run_shell(fixture, "SELECT\n").returncode, 0)
        self.assertEqual((self.directory / "trace").read_text().splitlines(),
                         ["dns-pair prepare-template", "dns-pair select"])
        (self.directory / "trace").unlink()
        (self.directory / "selected").unlink()
        self.assertNotEqual(self.run_shell(fixture.replace(
            "system_broker_idle() { return 0; }", "system_broker_idle() { return 1; }"),
            "SELECT\n").returncode, 0)
        self.assertFalse((self.directory / "trace").exists())
        self.assertNotEqual(self.run_shell(fixture.replace(
            "broker_access_for_user() { return 0; }",
            "broker_access_for_user() { return 1; }"),
            "SELECT\n").returncode, 0)
        self.assertFalse((self.directory / "trace").exists())

    def test_clean_reinstall_restores_only_enrollment_with_separate_consent(self):
        fixture = '''
pair_installed() { return 0; }
native_target() { echo rust; }
pair_selected() { return 0; }
system_broker_stopped() { return 0; }
no_broker_socket() { return 0; }
system_broker_idle() { return 0; }
broker_access_for_user() { return 0; }
enroll_uid() { echo enroll >> "$TEST_DIR/trace"; }
start_broker() { echo start >> "$TEST_DIR/trace"; }
restore_enrollment
'''
        self.assertNotEqual(self.run_shell(fixture, "\n").returncode, 0)
        self.assertFalse((self.directory / "trace").exists())
        self.assertEqual(self.run_shell(fixture, "DNS\n").returncode, 0)
        self.assertEqual((self.directory / "trace").read_text().splitlines(), ["enroll", "start"])
        (self.directory / "trace").unlink()
        for before, after in (("native_target() { echo rust; }", "native_target() { echo legacy; }"),
                              ("system_broker_stopped() { return 0; }", "system_broker_stopped() { return 1; }"),
                              ("no_broker_socket() { return 0; }", "no_broker_socket() { return 1; }")):
            result = self.run_shell(fixture.replace(before, after), "DNS\n")
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((self.directory / "trace").exists())
        result = self.run_shell(fixture.replace(
            'enroll_uid() { echo enroll >> "$TEST_DIR/trace"; }',
            'enroll_uid() { echo enroll >> "$TEST_DIR/trace"; return 1; }'), "DNS\n")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.directory / "trace").read_text().splitlines(), ["enroll"])

    def test_stopped_existing_broker_starts_without_reenrolling_or_starting_vpn(self):
        fixture = '''
pair_installed() { return 0; }
native_target() { echo rust; }
pair_selected() { return 0; }
user_runtime_stopped() { return 0; }
no_managed_tun() { return 0; }
system_broker_stopped() { return 0; }
no_broker_socket() { return 0; }
existing_enrollment_metadata() { return 0; }
start_stopped_broker_service() { echo start-existing >> "$TEST_DIR/trace"; }
system_broker_idle() { return 0; }
broker_access_for_user() { return 0; }
start_existing_broker
'''
        result = self.run_shell(fixture)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.directory / "trace").read_text().splitlines(), ["start-existing"])
        (self.directory / "trace").unlink()
        for before, after in (
            ("existing_enrollment_metadata() { return 0; }",
             "existing_enrollment_metadata() { return 1; }"),
            ("system_broker_stopped() { return 0; }",
             "system_broker_stopped() { return 1; }"),
            ("no_broker_socket() { return 0; }",
             "no_broker_socket() { return 1; }"),
            ("user_runtime_stopped() { return 0; }",
             "user_runtime_stopped() { return 1; }"),
        ):
            self.assertNotEqual(self.run_shell(fixture.replace(before, after)).returncode, 0)
            self.assertFalse((self.directory / "trace").exists())
        source = SCRIPT.read_text()
        self.assertIn("/usr/bin/sudo /usr/bin/env -i PATH=/usr/bin:/bin LC_ALL=C", source)
        self.assertIn("/usr/bin/stat -c '%F:%u:%a:%h' --", source)
        self.assertIn("/usr/bin/sudo /usr/bin/systemctl --system start omavless-dns-broker.service", source)

    def test_existing_owner_never_reinitializes_or_reenables(self):
        result = self.run_shell('native_target() { echo rust; }; prepare_application')
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout + result.stderr, "")

    def test_clean_store_selects_pair_before_cutover(self):
        fixture = '''
native_target() { if [[ -f "$TEST_DIR/activated" ]]; then echo rust; else echo legacy; fi; }
pair_installed() { return 0; }
pair_selection_status() {
  if [[ -f "$TEST_DIR/selected" ]]; then
    echo '{"schemaVersion":1,"scope":"local_pair_only","selected":true}'
  else
    echo '{"schemaVersion":1,"scope":"local_pair_only","selected":false}'
  fi
}
native() {
  echo "$*" >> "$TEST_DIR/trace"
  case "$*" in
    'setup initialize') mkdir -p "$TEST_DIR/.config/omavless"; touch "$TEST_DIR/.config/omavless/profiles.json" ;;
    store-compatibility) echo '{"schemaVersion":1,"compatible":true}' ;;
    'dns-pair select') touch "$TEST_DIR/selected" ;;
    'cutover activate') [[ -f "$TEST_DIR/selected" ]] || return 1; touch "$TEST_DIR/activated" ;;
  esac
}
enroll_uid() { echo enroll >> "$TEST_DIR/trace"; }
start_broker() { echo start-broker >> "$TEST_DIR/trace"; touch "$TEST_DIR/broker"; }
system_broker_idle() { [[ -f "$TEST_DIR/broker" ]]; }
system_broker_available() { [[ -f "$TEST_DIR/broker" ]]; }
broker_access_for_user() { return 0; }
enable_runtime() { echo enable-runtime >> "$TEST_DIR/trace"; }
prepare_private_store && enroll_and_select_pair && prepare_application
'''
        result = self.run_shell(fixture, "DNS\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.directory / "trace").read_text().splitlines(), [
            "setup initialize", "store-compatibility", "enroll", "start-broker",
            "dns-pair prepare-template", "dns-pair select", "store-compatibility",
            "cutover activate", "enable-runtime",
        ])

    def test_cutover_refuses_unselected_pair(self):
        result = self.run_shell('''
native_target() { echo legacy; }
pair_selection_status() { echo '{"schemaVersion":1,"scope":"local_pair_only","selected":false}'; }
native() { echo "unexpected effect" >&2; return 99; }
prepare_application
''')
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("unexpected effect", result.stderr)

    def test_activation_preserves_existing_store_and_stops_on_failure(self):
        store = self.directory / ".config/omavless/profiles.json"
        store.parent.mkdir(parents=True)
        store.write_text("synthetic unchanged private fixture")
        fixture = '''
native_target() { if [[ -f "$TEST_DIR/activated" ]]; then echo rust; else echo legacy; fi; }
pair_selected() { return 0; }
system_broker_available() { return 0; }
native() {
  echo "$*" >> "$TEST_DIR/trace"
  case "$*" in
    store-compatibility) echo '{"schemaVersion":1,"compatible":true}' ;;
    'cutover activate') touch "$TEST_DIR/activated" ;;
    *) return 1 ;;
  esac
}
enable_runtime() { echo enable >> "$TEST_DIR/trace"; }
prepare_application
'''
        self.assertEqual(self.run_shell(fixture).returncode, 0)
        self.assertEqual((self.directory / "trace").read_text().splitlines(),
                         ["store-compatibility", "cutover activate", "enable"])
        self.assertEqual(store.read_text(), "synthetic unchanged private fixture")
        (self.directory / "activated").unlink()
        (self.directory / "trace").unlink()
        failed = fixture.replace('touch "$TEST_DIR/activated"', 'return 1')
        self.assertNotEqual(self.run_shell(failed).returncode, 0)
        self.assertEqual((self.directory / "trace").read_text().splitlines(),
                         ["store-compatibility", "cutover activate"])
        self.assertEqual(store.read_text(), "synthetic unchanged private fixture")

    def test_invalid_commands_or_headless_install_have_no_effect(self):
        for arguments in ("install", "install ru", "finish-selection", "start-broker", "restore-enrollment", "components en",
                          "status en", "install-core", "download", "install zz"):
            result = self.run_shell("setup_main " + arguments)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout + result.stderr, "")
        source = SCRIPT.read_text()
        self.assertIn('[[ -t 0 && -t 1 && $EUID -ne 0 ]] || return 2', source)
        self.assertIn('[[ ! ${OMAVLESS_HOME+x} ]] || return 2', source)
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)

    def test_stage_reporting_is_closed_and_success_is_silent(self):
        for stage in ("application-activation", "dns-enrollment", "dns-template",
                      "private-settings", "completion"):
            result = self.run_shell(f"setup_step {stage} true")
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout + result.stderr, "")
        for stage in ("secret://synthetic", "application-activation extra", "",
                      "x" * 1000, "bad\nname"):
            result = self.run_shell('''
effect() { touch "$TEST_DIR/unexpected"; }
setup_step "$STAGE" effect
'''.replace('"$STAGE"', "'" + stage + "'"))
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout + result.stderr, "")
            self.assertFalse((self.directory / "unexpected").exists())

    def test_native_failure_reports_one_inner_stage_without_private_output(self):
        for locale, expected in (
            ("en", "Setup stopped at stage: application-activation.\n"),
            ("ru", "Настройка остановлена на этапе: application-activation.\n"),
        ):
            result = self.run_shell(f'''
setup_locale={locale}
native() {{
  echo 'synthetic-private-stdout'
  echo 'synthetic-private-stderr' >&2
  return 42
}}
activate() {{ setup_step application-activation native_quiet cutover activate; }}
setup_step application-preconditions activate
''')
            self.assertEqual(result.returncode, 1)
            self.assertEqual(result.stdout, expected)
            self.assertEqual(result.stderr, "")

    def test_dns_failures_keep_original_order_and_do_not_repeat_enrollment(self):
        fixture = '''
pair_installed() { return 0; }
user_runtime_stopped() { return 0; }
no_managed_tun() { return 0; }
confirm_dns_enrollment() { return 0; }
enroll_uid() { echo enroll >> "$TEST_DIR/trace"; }
start_broker() { echo start >> "$TEST_DIR/trace"; }
system_broker_idle() { return 0; }
broker_access_for_user() { return 0; }
native() { echo "$*" >> "$TEST_DIR/trace"; }
pair_selected() { return 0; }
'''
        cases = (
            ("enroll_uid", "dns-enrollment", ["enroll"]),
            ("start_broker", "dns-broker-start", ["enroll", "start"]),
            ("system_broker_idle", "dns-broker-readiness", ["enroll", "start"]),
            ("broker_access_for_user", "dns-socket-access", ["enroll", "start"]),
        )
        for function, stage, trace in cases:
            with self.subTest(stage=stage):
                (self.directory / "trace").unlink(missing_ok=True)
                if function == "enroll_uid":
                    failing = 'enroll_uid() { echo enroll >> "$TEST_DIR/trace"; return 1; }'
                elif function == "start_broker":
                    failing = 'start_broker() { echo start >> "$TEST_DIR/trace"; return 1; }'
                else:
                    failing = function + "() { return 1; }"
                result = self.run_shell(fixture + failing + "\nenroll_and_select_pair")
                self.assertEqual(result.returncode, 1)
                self.assertEqual(result.stdout, f"Setup stopped at stage: {stage}.\n")
                self.assertEqual(result.stderr, "")
                self.assertEqual((self.directory / "trace").read_text().splitlines(), trace)

    def test_template_failure_does_not_select_or_activate(self):
        result = self.run_shell('''
pair_installed() { return 0; }
system_broker_idle() { return 0; }
broker_access_for_user() { return 0; }
native() {
  echo "$*" >> "$TEST_DIR/trace"
  echo 'synthetic-private-template-error' >&2
  return 1
}
finish_pair_selection
''', "SELECT\n")
        self.assertEqual(result.returncode, 1)
        self.assertIn("Setup stopped at stage: dns-template.\n", result.stdout)
        self.assertNotIn("synthetic-private-template-error", result.stdout + result.stderr)
        self.assertEqual((self.directory / "trace").read_text().splitlines(),
                         ["dns-pair prepare-template"])

    def test_failure_guidance_does_not_claim_pending_authorization(self):
        source = SCRIPT.read_text()
        self.assertIn("Some earlier steps may have succeeded", source)
        self.assertIn("Finish or cancel any open system prompts", source)
        self.assertIn("Do not repeat a step with an unknown result", source)
        self.assertNotIn("Do not repeat an unresolved authorization", source)
        # Reporting must not change either read-only projection to include stages.
        for command in ("status", "components"):
            result = self.run_shell('setup_status() { echo needs_activation; }; setup_main ' + command)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "needs_activation" + ("\tmissing" if command == "components" else "") + "\n")
            self.assertEqual(result.stderr, "")

    def test_terminal_existing_enrollment_refusal_has_exact_stage_and_no_reenroll(self):
        for locale in ("en", "ru"):
            with self.subTest(locale=locale):
                for name in ("trace", "selected", "activated"):
                    (self.directory / name).unlink(missing_ok=True)
                fixture = self.terminal_fixture()
                # Keep every native boundary synthetic, including the failing one.
                fixture = fixture.replace(
                    "'cutover activate') touch \"$TEST_DIR/activated\"",
                    "'cutover activate') echo synthetic-private-error >&2; return 42",
                )
                code, output = self.run_terminal(fixture + f"\nsetup_main install {locale}")
                self.assertEqual(code, 1)
                expected = ("Setup stopped at stage: " if locale == "en"
                            else "Настройка остановлена на этапе: ")
                self.assertEqual(output.count(expected), 1)
                self.assertIn(expected + "application-activation.", output)
                self.assertNotIn("synthetic-private-error", output)
                self.assertNotIn("Do not repeat an unresolved authorization", output)
                self.assertEqual((self.directory / "trace").read_text().splitlines(), [
                    "store-compatibility", "dns-pair prepare-template", "dns-pair select",
                    "store-compatibility", "cutover activate",
                ])
                self.assertEqual((self.directory / ".config/omavless/profiles.json").read_text(),
                                 "synthetic existing profile data")
                self.assertFalse((self.directory / "omavless-first-run.lock").exists())
                self.assertEqual(list(self.directory.glob("omavless-first-run.*")), [])

    def test_terminal_cancelled_selection_never_reenrolls_or_activates(self):
        code, output = self.run_terminal(self.terminal_fixture() + "\nsetup_main install en",
                                         "INSTALL\n\n")
        self.assertEqual(code, 1)
        self.assertIn("Setup stopped at stage: dns-selection-confirmation.", output)
        self.assertEqual((self.directory / "trace").read_text().splitlines(), ["store-compatibility"])
        self.assertFalse((self.directory / "selected").exists())
        self.assertFalse((self.directory / "activated").exists())

    def test_terminal_resume_success_preserves_data_and_never_reenrolls(self):
        for selected in (False, True):
            with self.subTest(already_selected=selected):
                for name in ("trace", "selected", "activated"):
                    (self.directory / name).unlink(missing_ok=True)
                fixture = self.terminal_fixture() + '''
setup_status() {
  if [[ -f "$TEST_DIR/activated" ]]; then echo ready; else echo needs_activation; fi
}
setup_main install en
'''
                if selected:
                    (self.directory / "selected").touch()
                code, output = self.run_terminal(fixture,
                                                 "INSTALL\n" if selected else "INSTALL\nSELECT\n")
                self.assertEqual(code, 0)
                self.assertIn("OmaVLESS is ready.", output)
                self.assertNotIn("Setup stopped", output)
                self.assertNotIn("Setup did not finish", output)
                expected = ["store-compatibility"]
                if not selected:
                    expected += ["dns-pair prepare-template", "dns-pair select"]
                expected += ["store-compatibility", "cutover activate", "enable"]
                self.assertEqual((self.directory / "trace").read_text().splitlines(), expected)
                self.assertEqual((self.directory / ".config/omavless/profiles.json").read_text(),
                                 "synthetic existing profile data")
                self.assertFalse((self.directory / "omavless-first-run.lock").exists())
                self.assertEqual(list(self.directory.glob("omavless-first-run.*")), [])

    def post_dns_terminal_fixture(self):
        # Real production inventory/setup_main; only the external boundaries
        # are synthetic. A successful simple-unit start need not expose its
        # socket on the very first read-only observation.
        return self.terminal_fixture().replace(
            "setup_status() { echo needs_activation; }", ""
        ) + '''
native_present() { return 0; }
package_installed() { echo "omavless $package_version-1"; }
runtime_start_available() { return 1; }
installed_pair_delivery() { echo current; }
user_runtime_stopped() { [[ ! -f "$TEST_DIR/activated" ]]; }
enroll_uid() { echo enroll >> "$TEST_DIR/trace"; touch "$TEST_DIR/enrolled"; }
start_broker() { echo start >> "$TEST_DIR/trace"; touch "$TEST_DIR/broker-started"; }
system_broker_idle() {
  [[ -f "$TEST_DIR/broker-started" ]] || return 1
  local count=0
  if [[ -f "$TEST_DIR/unit-reads" ]]; then read -r count < "$TEST_DIR/unit-reads"; fi
  count=$((count + 1))
  echo "$count" > "$TEST_DIR/unit-reads"
  (( count >= 2 ))
}
broker_access_for_user() {
  [[ -f "$TEST_DIR/broker-started" ]] || return 1
  local count=0
  if [[ -f "$TEST_DIR/socket-reads" ]]; then read -r count < "$TEST_DIR/socket-reads"; fi
  count=$((count + 1))
  echo "$count" > "$TEST_DIR/socket-reads"
  (( count >= 2 ))
}
system_broker_available() { system_broker_idle && broker_access_for_user; }
'''

    def test_terminal_post_dns_delayed_broker_readiness_completes_once(self):
        code, output = self.run_terminal(
            self.post_dns_terminal_fixture() + "\nsetup_main install en", "INSTALL\nDNS\n"
        )
        self.assertEqual(code, 0, output)
        self.assertIn("OmaVLESS is ready.", output)
        self.assertNotIn("Setup stopped", output)
        self.assertEqual((self.directory / "trace").read_text().splitlines(), [
            "store-compatibility", "enroll", "start", "dns-pair prepare-template",
            "dns-pair select", "store-compatibility", "cutover activate", "enable",
        ])
        self.assertGreaterEqual(int((self.directory / "unit-reads").read_text()), 2)
        self.assertGreaterEqual(int((self.directory / "socket-reads").read_text()), 2)
        self.assertEqual((self.directory / ".config/omavless/profiles.json").read_text(),
                         "synthetic existing profile data")
        self.assertFalse((self.directory / "omavless-first-run.lock").exists())
        self.assertEqual(list(self.directory.glob("omavless-first-run.*")), [])

    def test_first_setup_explains_helper_and_other_vpns_before_consent(self):
        for locale, intro, advice in [
            ('en', 'The OmaVLESS DNS helper sets up website address lookup',
             'Before setup, turn off other VPN connections'),
            ('ru', 'DNS-помощник OmaVLESS настраивает поиск адресов сайтов',
             'Перед настройкой отключите другие VPN-подключения'),
        ]:
            with self.subTest(locale=locale):
                code, output = self.run_terminal(
                    self.post_dns_terminal_fixture() + f"\nsetup_main install {locale}", "\n"
                )
                self.assertEqual(code, 0, output)
                install_prompt = 'Type INSTALL' if locale == 'en' else 'Введите INSTALL'
                self.assertLess(output.index(intro), output.index(install_prompt))
                self.assertLess(output.index(advice), output.index(install_prompt))
                self.assertNotIn('Google DNS, Cloudflare', output)
                self.assertFalse((self.directory / "trace").exists())

    def test_terminal_post_dns_socket_timeout_retains_enrollment_without_activation(self):
        code, output = self.run_terminal(self.post_dns_terminal_fixture() + '''
broker_access_for_user() { return 1; }
setup_main install en
''', "INSTALL\nDNS\n")
        self.assertEqual(code, 1)
        self.assertEqual(output.count("Setup stopped at stage:"), 1)
        self.assertIn("Setup stopped at stage: dns-socket-access.", output)
        self.assertNotIn("OmaVLESS is ready.", output)
        self.assertEqual((self.directory / "trace").read_text().splitlines(), [
            "store-compatibility", "enroll", "start",
        ])
        self.assertTrue((self.directory / "enrolled").exists())
        self.assertFalse((self.directory / "selected").exists())
        self.assertFalse((self.directory / "activated").exists())
        self.assertFalse((self.directory / "omavless-first-run.lock").exists())
        self.assertEqual((self.directory / ".config/omavless/profiles.json").read_text(),
                         "synthetic existing profile data")

    def test_terminal_post_dns_start_refusal_never_waits_or_resubmits(self):
        code, output = self.run_terminal(self.post_dns_terminal_fixture() + '''
start_broker() { echo start >> "$TEST_DIR/trace"; return 1; }
setup_main install en
''', "INSTALL\nDNS\n")
        self.assertEqual(code, 1)
        self.assertIn("Setup stopped at stage: dns-broker-start.", output)
        self.assertEqual((self.directory / "trace").read_text().splitlines(), [
            "store-compatibility", "enroll", "start",
        ])
        self.assertFalse((self.directory / "unit-reads").exists())
        self.assertFalse((self.directory / "socket-reads").exists())
        self.assertFalse((self.directory / "activated").exists())

    def test_terminal_post_dns_unit_timeout_never_reaches_socket_or_selection(self):
        code, output = self.run_terminal(self.post_dns_terminal_fixture() + '''
system_broker_idle() { return 1; }
setup_main install en
''', "INSTALL\nDNS\n")
        self.assertEqual(code, 1)
        self.assertIn("Setup stopped at stage: dns-broker-readiness.", output)
        self.assertFalse((self.directory / "socket-reads").exists())
        self.assertEqual((self.directory / "trace").read_text().splitlines(), [
            "store-compatibility", "enroll", "start",
        ])

    def test_terminal_post_dns_template_refusal_is_quiet_and_keeps_stage(self):
        fixture = self.post_dns_terminal_fixture().replace(
            "'dns-pair prepare-template') : ;;",
            "'dns-pair prepare-template') echo synthetic-private-template-error >&2; return 42 ;;",
        )
        code, output = self.run_terminal(fixture + "\nsetup_main install en", "INSTALL\nDNS\n")
        self.assertEqual(code, 1)
        self.assertIn("Setup stopped at stage: dns-template.", output)
        self.assertNotIn("synthetic-private-template-error", output)
        self.assertEqual((self.directory / "trace").read_text().splitlines(), [
            "store-compatibility", "enroll", "start", "dns-pair prepare-template",
        ])
        self.assertFalse((self.directory / "selected").exists())
        self.assertFalse((self.directory / "activated").exists())

    def test_broker_wait_accepts_only_fixed_observation_targets(self):
        for fact in ("unknown", "start", "shell", "idle extra"):
            with self.subTest(fact=fact):
                result = self.run_shell('''
system_broker_idle() { echo unexpected-unit >> "$TEST_DIR/trace"; }
broker_access_for_user() { echo unexpected-socket >> "$TEST_DIR/trace"; }
wait_broker_fact ''' + "'" + fact + "'")
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stdout + result.stderr, "")
                self.assertFalse((self.directory / "trace").exists())
        for command in ("wait_broker_fact", "wait_broker_fact idle extra",
                        "await_broker_ready extra"):
            result = self.run_shell(command)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout + result.stderr, "")

    def test_existing_broker_and_clean_reenrollment_wait_without_repeating_effects(self):
        fixture = '''
pair_installed() { return 0; }
native_target() { echo rust; }
pair_selected() { return 0; }
system_broker_stopped() { return 0; }
no_broker_socket() { return 0; }
existing_enrollment_metadata() { return 0; }
setup_test_unit_reads=0
setup_test_socket_reads=0
system_broker_idle() {
  setup_test_unit_reads=$((setup_test_unit_reads + 1))
  (( setup_test_unit_reads >= 2 ))
}
broker_access_for_user() {
  setup_test_socket_reads=$((setup_test_socket_reads + 1))
  (( setup_test_socket_reads >= 2 ))
}
start_stopped_broker_service() { echo start >> "$TEST_DIR/trace"; }
enroll_uid() { echo enroll >> "$TEST_DIR/trace"; }
start_broker() { echo start >> "$TEST_DIR/trace"; }
'''
        for command, consent, expected in (
            ("start_existing_broker", "", ["start"]),
            ("restore_enrollment", "DNS\n", ["enroll", "start"]),
        ):
            with self.subTest(command=command):
                (self.directory / "trace").unlink(missing_ok=True)
                result = self.run_shell(fixture + command, consent)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertNotIn("Setup stopped", result.stdout)
                self.assertEqual((self.directory / "trace").read_text().splitlines(), expected)

    def test_terminal_preparation_refusals_have_no_native_effects(self):
        cases = (
            ("runtime-directory", "setup_runtime_directory_safe() { return 1; }"),
            ("setup-lock", ""),
            ("temporary-directory", "mktemp() { return 1; }"),
        )
        for stage, override in cases:
            with self.subTest(stage=stage):
                (self.directory / "trace").unlink(missing_ok=True)
                lock = self.directory / "omavless-first-run.lock"
                if stage == "setup-lock":
                    lock.mkdir()
                code, output = self.run_terminal(self.terminal_fixture() + override
                                                 + "\nsetup_main install en", "INSTALL\n")
                self.assertEqual(code, 1)
                self.assertIn(f"Setup stopped at stage: {stage}.", output)
                self.assertFalse((self.directory / "trace").exists())
                if stage == "setup-lock":
                    self.assertTrue(lock.is_dir(), "existing lock must remain untouched")
                    lock.rmdir()
                else:
                    self.assertFalse(lock.exists())

    def test_private_initialization_failure_is_quiet_and_never_validates_or_selects(self):
        result = self.run_shell('''
native_target() { echo legacy; }
native() {
  echo "$*" >> "$TEST_DIR/trace"
  echo synthetic-private-error >&2
  return 1
}
setup_step private-settings prepare_private_store
''')
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "Setup stopped at stage: private-initialization.\n")
        self.assertEqual(result.stderr, "")
        self.assertEqual((self.directory / "trace").read_text().splitlines(), ["setup initialize"])


if __name__ == "__main__":
    unittest.main()
