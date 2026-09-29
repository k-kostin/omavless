# SPDX-License-Identifier: MIT
"""0.9 first-run composition with synthetic boundaries only.

No test invokes pacman, sudo, systemd, a network client, or a private store.
"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "plugin/setup-runtime.sh"
VERSION = "0.9.0-rc.2"
PKGVER = "0.9.0rc2"
SOURCE = "b" * 40


class MarketplaceSetupTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="omavless-bootstrap-test-")
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
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

    def pins(self, *, app=None, dns=None, source=SOURCE, version=VERSION):
        entry = lambda digest: {"sha256": digest, "sourceCommit": source}
        for name, digest in (("runtime-release.json", app),
                             ("dns-release.json", dns)):
            data = {"schemaVersion": 1, "version": version,
                    "packages": ({"aarch64": entry(digest),
                                  "x86_64": entry(digest)} if digest else {})}
            (self.directory / name).write_text(json.dumps(data))

    def test_committed_rc_pair_pins_match_on_both_architectures(self):
        manifest = json.loads((ROOT / "manifest.json").read_text())
        records = {}
        for filename in ("runtime-release.json", "dns-release.json"):
            metadata = json.loads((ROOT / "plugin" / filename).read_text())
            self.assertEqual(metadata["schemaVersion"], 1)
            self.assertEqual(metadata["version"], manifest["version"])
            self.assertEqual(set(metadata["packages"]), {"aarch64", "x86_64"})
            records[filename] = metadata
            shutil.copyfile(ROOT / "plugin" / filename, self.directory / filename)
        self.assertEqual(manifest["version"], VERSION)
        app = records["runtime-release.json"]["packages"]
        dns = records["dns-release.json"]["packages"]
        for arch in ("aarch64", "x86_64"):
            self.assertEqual(app[arch]["sourceCommit"], dns[arch]["sourceCommit"])
            self.assertEqual(app[arch]["sourceCommit"],
                             "b739ac6a279981ffde3586d5e70e44a6be43ba70")
            self.assertNotEqual(app[arch]["sha256"], dns[arch]["sha256"])
            for filename in ("runtime-release.json", "dns-release.json"):
                shutil.copyfile(ROOT / "plugin" / filename, self.directory / filename)
            result = self.run_shell(f'uname() {{ echo {arch}; }}; release_fields')
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout,
                             f'{VERSION}\t{arch}\t{app[arch]["sha256"]}\t'
                             f'{dns[arch]["sha256"]}\t{app[arch]["sourceCommit"]}\n')
            self.assertEqual(self.run_shell(f'uname() {{ echo {arch}; }}; setup_status').stdout,
                             "needs_package\n")
        self.pins()
        for arch in ("aarch64", "x86_64"):
            result = self.run_shell(f'uname() {{ echo {arch}; }}; release_fields')
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout + result.stderr, "")
            self.assertEqual(self.run_shell(f'uname() {{ echo {arch}; }}; setup_status').stdout,
                             "release_unavailable\n")

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
package_installed() { echo 'omavless 0.9.0rc2-1'; }
pair_installed() { return 0; }
system_broker_available() { return 0; }
native_target() { echo rust; }
pair_selection_status() { echo '{"schemaVersion":1,"scope":"local_pair_only","selected":true}'; }
'''
        self.assertEqual(self.run_shell(fixture + "setup_status").stdout, "ready\n")
        self.assertEqual(self.run_shell(fixture + '''
pair_selection_status() { echo '{"schemaVersion":1,"scope":"local_pair_only","selected":false}'; }
setup_status
''').stdout, "needs_selection\n")
        self.assertEqual(self.run_shell(fixture + 'system_broker_available() { return 1; }; setup_status').stdout,
                         "needs_broker\n")
        self.assertEqual(self.run_shell(fixture + '''
system_broker_available() { return 1; }
system_broker_stopped() { return 0; }
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
                         "needs_attention\n")
        self.assertEqual(self.run_shell(fixture + 'pair_selection_status() { return 1; }; setup_status').stdout,
                         "needs_attention\n")

    def test_component_inventory_reports_only_fixed_presence(self):
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

    def test_first_run_warns_about_firewall_without_changing_it(self):
        source = SCRIPT.read_text()
        for phrase in (
            "firewall denies incoming traffic by default",
            "межсетевой экран по умолчанию блокирует входящий трафик",
            "Setup never changes firewall rules",
            "Установка не меняет правила экрана",
            "omavless runtime test checks HTTPS on the current route",
            "omavless runtime test проверяет HTTPS на текущем маршруте",
            "It is not a VPN leak test",
            "Это не проверка утечки VPN",
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
    echo 'omavless-dns {'0.9.0rc2-1' if dns_package_ok else '0.8.2-1'}'
  else echo 'omavless 0.9.0rc2-1'; fi
}}
app_archive_identity() {{ return {0 if app_identity_ok else 1}; }}
dns_archive_identity() {{ return {0 if dns_identity_ok else 1}; }}
package_install() {{ printf '%s\\n' "$1" "$2" > "$TEST_DIR/install-args"; touch "$TEST_DIR/installed"; }}
package_installed() {{ [[ -f "$TEST_DIR/installed" ]] && echo 'omavless 0.9.0rc2-1'; }}
pair_installed() {{ [[ -f "$TEST_DIR/installed" ]]; }}
{boundary_override}
install_package
''')

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
            f"https://github.com/k-kostin/omavless/releases/download/v{VERSION}/omavless-{PKGVER}-1-x86_64.pkg.tar.zst",
            f"https://github.com/k-kostin/omavless/releases/download/v{VERSION}/omavless-dns-{PKGVER}-1-x86_64.pkg.tar.zst",
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
package_installed() { echo 'omavless 0.9.0rc2-1'; }
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
package_installed() { echo 'omavless 0.9.0rc2-1'; }
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


if __name__ == "__main__":
    unittest.main()
