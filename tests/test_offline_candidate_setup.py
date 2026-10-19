# SPDX-License-Identifier: MIT
"""Real shell/data/filesystem controls; every OS/package/native leaf is inert."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
CHANNEL = b'{"schemaVersion":1,"kind":"unpublished-offline-candidate","version":"0.9.9"}\n'


class OfflineCandidateSetupTests(unittest.TestCase):
    def setUp(self):
        parent = Path.home() / '.cache/ovt'
        parent.mkdir(parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(prefix='offline-candidate-', dir=parent)
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.plugin = self.directory / 'plugin'
        self.plugin.mkdir(mode=0o700)
        for name in ('setup-channel.sh', 'setup-entry.sh', 'setup-runtime.sh',
                     'setup-offline-candidate.sh', 'setup-predecessor.sh', 'setup-channel.json'):
            shutil.copyfile(ROOT / 'plugin' / name, self.plugin / name)
        (self.directory / 'manifest.json').write_text(json.dumps({
            'schemaVersion': 1, 'id': 'kdk.omavless', 'version': '0.9.9'}))
        self.input = self.directory / 'archives'
        self.input.mkdir(mode=0o700)
        self.names = ('omavless-0.9.9-1-x86_64.pkg.tar.zst',
                      'omavless-dns-0.9.9-1-x86_64.pkg.tar.zst')
        self.hashes = []
        for name, data in zip(self.names, (b'inert application archive', b'inert DNS archive')):
            path = self.input / name
            path.write_bytes(data)
            path.chmod(0o600)
            self.hashes.append(hashlib.sha256(data).hexdigest())
        for filename, digest in zip(('runtime-release.json', 'dns-release.json'), self.hashes):
            (self.plugin / filename).write_text(json.dumps({'schemaVersion': 1,
                'version': '0.9.9', 'packages': {'x86_64': {
                    'sourceCommit': 'b' * 40, 'sha256': digest}}}))
        self.env = {'PATH': '/usr/bin:/bin', 'HOME': str(self.directory),
                    'TMPDIR': str(self.directory), 'LC_ALL': 'C',
                    'TEST_DIRECTORY': str(self.directory)}

    def shell(self, code, input='', public=False):
        entry = 'setup-runtime.sh' if public else 'setup-offline-candidate.sh'
        prefix = 'source "$1/' + entry + '"\n' + '''
native() { echo UNEXPECTED_NATIVE >&2; return 99; }
package_install() { echo UNEXPECTED_PACKAGE >&2; return 99; }
native_present() { return 1; }
native_target() { echo legacy; }
package_installed() { return 1; }
dns_package_registered() { return 1; }
fresh_user_runtime_absent() { return 0; }
no_managed_tun() { return 0; }
pair_installed() { return 1; }
curl() { echo UNEXPECTED_NETWORK >&2; return 99; }
'''
        return subprocess.run(['/bin/bash', '-c', prefix + code, 'fixture', str(self.plugin)],
                              env=self.env, input=input, text=True, capture_output=True,
                              timeout=15, check=False)

    def acquire(self, input=None, suffix=''):
        app, dns = self.hashes
        code = f'''
cache=$(setup_release_cache {app} {dns}) || exit 1
obtain_install_archives {self.names[0]} {app} {self.names[1]} {dns} "$cache"
''' + suffix
        return self.shell(code, str(self.input) + '\n' if input is None else input)

    def cache(self):
        return self.directory / '.cache/omavless/upgrade082' / '.'.join(self.hashes)

    def test_explicit_channel_and_real_pair_acquisition(self):
        self.assertEqual((ROOT / 'plugin/setup-channel.json').read_bytes(), CHANNEL)
        result = self.acquire()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('Nothing will be downloaded', result.stdout)
        self.assertNotIn('UNEXPECTED', result.stdout + result.stderr)
        for name in self.names:
            target = self.cache() / name
            self.assertEqual(target.read_bytes(), (self.input / name).read_bytes())
            self.assertEqual(target.stat().st_mode & 0o777, 0o600)
            self.assertEqual(target.stat().st_nlink, 1)
        self.assertEqual(sorted(p.name for p in self.cache().iterdir()), sorted(self.names))
        # Already verified pair needs no new prompt, input or network.
        again = self.acquire(input='')
        self.assertEqual(again.returncode, 0, again.stderr)
        self.assertEqual(again.stdout, '')

    def test_both_hashes_precede_any_copy_or_parser(self):
        (self.input / self.names[1]).write_bytes(b'wrong DNS bytes')
        result = self.acquire()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(list(self.cache().iterdir()), [])

    def test_shared_install_parses_only_authenticated_pair_and_submits_one_transaction(self):
        code = '''
package_info() {
  printf 'parse:%s\\n' "${1##*/}" >> "$TEST_DIRECTORY/trace"
  case "${1##*/}" in
    omavless-dns-*) echo 'omavless-dns 0.9.9-1' ;;
    omavless-*) echo 'omavless 0.9.9-1' ;;
    *) return 99 ;;
  esac
}
app_archive_identity() { echo app-identity >> "$TEST_DIRECTORY/trace"; }
dns_archive_identity() { echo dns-identity >> "$TEST_DIRECTORY/trace"; }
package_install() { echo single-transaction >> "$TEST_DIRECTORY/trace"; touch "$TEST_DIRECTORY/installed"; }
package_installed() { [[ -f "$TEST_DIRECTORY/installed" ]] && echo 'omavless 0.9.9-1'; }
pair_installed() { [[ -f "$TEST_DIRECTORY/installed" ]]; }
install_package
'''
        result = self.shell(code, str(self.input) + '\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.directory / 'trace').read_text().splitlines(), [
            'parse:' + self.names[0], 'parse:' + self.names[1],
            'app-identity', 'dns-identity', 'single-transaction'])
        self.assertNotIn('UNEXPECTED', result.stdout + result.stderr)

    def test_public_pair_acquisition_keeps_original_order_and_arguments(self):
        result = self.shell('upgrade_download() { printf "%s\\n" "$*"; }; '
            'obtain_install_archives app APP_SHA dns DNS_SHA CACHE', public=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, 'app APP_SHA CACHE\ndns DNS_SHA CACHE\n')

    def test_no_overwrite_or_network_fallback_for_poisoned_cache(self):
        cache = self.cache()
        cache.mkdir(mode=0o700, parents=True)
        target = cache / self.names[0]
        target.write_bytes(b'keep poisoned evidence')
        target.chmod(0o600)
        result = self.acquire()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(target.read_bytes(), b'keep poisoned evidence')
        self.assertNotIn('UNEXPECTED', result.stdout + result.stderr)

    def test_unsafe_source_file_refuses(self):
        target = self.input / self.names[0]
        for kind in ('symlink', 'hardlink', 'public-mode'):
            with self.subTest(kind=kind):
                if kind == 'symlink':
                    retained = self.directory / 'retained'
                    target.rename(retained)
                    target.symlink_to(retained)
                elif kind == 'hardlink':
                    target.unlink()
                    (self.directory / 'retained').rename(target)
                    os.link(target, self.directory / 'second-link')
                else:
                    (self.directory / 'second-link').unlink()
                    target.chmod(0o644)
                result = self.acquire()
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(list(self.cache().iterdir()), [])

    def test_input_directory_and_cancellation_refusals(self):
        for value in ('', '\n', 'relative\n', '/tmp\n', str(self.input) + '/../archives\n',
                      '/' + 'a' * 4097 + '\n', str(self.input) + '\t\n'):
            with self.subTest(value_length=len(value)):
                result = self.acquire(input=value)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(list(self.cache().iterdir()), [])
        self.input.chmod(0o777)
        self.assertNotEqual(self.acquire().returncode, 0)

    def test_present_invalid_channel_refuses_before_input_copy_or_effect(self):
        path = self.plugin / 'setup-channel.json'
        for data in (CHANNEL.replace(b'0.9.9', b'0.9.8'), CHANNEL + b'\n',
                     CHANNEL.replace(b'1,', b'1,"schemaVersion":1,'), b'{}\n'):
            with self.subTest(data=data):
                path.write_bytes(data)
                result = self.acquire()
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(list(self.cache().iterdir()), [])
                self.assertEqual(result.stdout, '')
        path.write_bytes(CHANNEL)
        path.chmod(0o666)
        self.assertNotEqual(self.acquire().returncode, 0)

    def test_public_entry_still_refuses_real_nonpublic_pins(self):
        result = self.shell('release_fields', public=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, '')
        self.assertEqual(result.stderr, '')
        self.assertEqual(self.shell('candidate_main status').stdout, 'needs_package\n')
        self.assertEqual(self.shell('candidate_main components').stdout, 'needs_package\tmissing\n')

    def test_unsupported_actions_refuse_before_any_effect(self):
        for action in ('upgrade', 'update-pair', 'restore-enrollment', '--install', 'arbitrary'):
            result = self.shell(f'candidate_main {action}')
            self.assertEqual(result.returncode, 2, result.stderr)
            self.assertEqual(result.stdout + result.stderr, '')
        self.assertNotEqual(self.shell('upgrade_download unused unused unused').returncode, 0)

    def test_candidate_inventory_does_not_offer_public_predecessor_update(self):
        result = self.shell('package_installed() { echo "omavless 0.8.2-1"; }; '
                            'native_present() { return 0; }; native_target() { echo rust; }; '
                            'candidate_main components')
        self.assertEqual(result.stdout, 'needs_attention\tmissing\n')
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_dispatcher_uses_only_fixed_entries_and_refuses_unknown_marker(self):
        for name, label in (('setup-runtime.sh', 'PUBLIC'),
                            ('setup-offline-candidate.sh', 'OFFLINE')):
            (self.plugin / name).write_text(f'printf "{label}:%s\\n" "$*"\n')
        def dispatch():
            return subprocess.run(['/bin/bash', str(self.plugin / 'setup-entry.sh'), 'components'],
                env=self.env, text=True, capture_output=True, timeout=5, check=False)
        self.assertEqual(dispatch().stdout, 'OFFLINE:components\n')
        (self.plugin / 'setup-channel.json').unlink()
        self.assertEqual(dispatch().stdout, 'PUBLIC:components\n')
        (self.plugin / 'setup-channel.json').write_bytes(b'unknown\n')
        result = dispatch()
        self.assertEqual(result.returncode, 2)
        self.assertNotIn('PUBLIC', result.stdout)
        self.assertNotIn('OFFLINE', result.stdout)


if __name__ == '__main__':
    unittest.main()
