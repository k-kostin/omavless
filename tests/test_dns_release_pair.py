# SPDX-License-Identifier: MIT
"""Offline DNS release-triple contracts; no package, service or network effects."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('dns_release_pair', ROOT / 'packaging/release/pair-dns-frontend.py')
PAIR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PAIR)


class DnsReleasePairTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='omavless-dns-release-')
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.repo = self.base / 'repo'
        self.repo.mkdir()
        for name, value in {
            'Cargo.toml': '[workspace.package]\nversion = "0.9.0-rc.1"\n',
            'Cargo.lock': 'locked', 'crates/runtime.rs': 'runtime',
            'LICENSE': 'MIT', 'THIRD_PARTY_NOTICES.md': 'notices',
            'manifest.json': '{"version":"0.9.0-rc.1"}',
            'backend.sh': 'exit 0', 'install.sh': 'exit 0',
            'plugin/Panel.qml': 'Panel', 'plugin/runtime-release.json': self.pins(),
            'plugin/dns-release.json': self.pins(),
            'packaging/release/install-frontend.sh': 'exit 0',
            'packaging/release/FRONTEND_README.md': 'guide',
        }.items():
            self.write(name, value)
        self.git('init', '-q')
        self.git('config', 'user.name', 'Fixture')
        self.git('config', 'user.email', 'fixture@example.invalid')
        self.git('config', 'commit.gpgsign', 'false')
        self.runtime = self.commit()
        self.output = self.base / 'output'
        self.output.mkdir()
        self.app = self.base / 'app.pkg.tar.zst'
        self.dns = self.base / 'dns.pkg.tar.zst'
        self.app.write_bytes(b'reviewed app package')
        self.dns.write_bytes(b'reviewed dns package')
        self.app_sha = hashlib.sha256(self.app.read_bytes()).hexdigest()
        self.dns_sha = hashlib.sha256(self.dns.read_bytes()).hexdigest()

    @staticmethod
    def pins(packages=None):
        return json.dumps({'schemaVersion': 1, 'version': '0.9.0-rc.1',
                           'packages': packages if packages is not None else {}})

    def write(self, name, value):
        path = self.repo / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value)

    def git(self, *args):
        # Keep Git from detaching auto-maintenance while this disposable repo is
        # being removed by TemporaryDirectory cleanup.
        return subprocess.check_output(
            ['git', '-c', 'gc.auto=0', '-C', str(self.repo), *args],
            stderr=subprocess.DEVNULL)

    def commit(self):
        self.git('add', '.')
        self.git('commit', '-qm', 'fixture')
        return self.git('rev-parse', 'HEAD').decode().strip()

    def assembly(self, source=None, app_sha=None, dns_sha=None, arch=None,
                 binary_machine=None, stable=False):
        app_mark = (self.app_sha, 0o600, os.getuid())
        dns_mark = (self.dns_sha, 0o600, os.getuid())
        architecture = arch or os.uname().machine
        machine = binary_machine if binary_machine is not None else (
            62 if architecture == 'x86_64' else 183)
        version = PAIR.release.arch_version(PAIR.release.version(self.repo, stable=stable))
        header = bytearray(20)
        header[:6] = b'\x7fELF\x02\x01'
        header[18:20] = machine.to_bytes(2, 'little')

        def fingerprint(path, uid):
            return app_mark if path == self.app else dns_mark

        def archive_member(_path, name, _limit=8192, digest=False):
            if name == 'usr/bin/omavless' and digest:
                return '1' * 64, bytes(header)
            return (f'pkgname = omavless\npkgver = {version}-1\n'
                    f'arch = {architecture}\ndepend = omavless-dns={version}-1\n').encode()

        with patch.object(PAIR.inspection, 'fingerprint', side_effect=fingerprint), \
             patch.object(PAIR.inspection, 'safe_parents'), \
             patch.object(PAIR.inspection, 'inspect_archive', return_value={
                 'version': version + '-1', 'source': self.runtime,
                 'binary': '1' * 64}), \
             patch.object(PAIR, 'inspect_dns_archive', return_value={
                 'version': version + '-1', 'source': self.runtime,
                 'coreSha256': '2' * 64, 'brokerSha256': '3' * 64,
                 'receiptSha256': '4' * 64}), \
             patch.object(PAIR, 'member', side_effect=archive_member):
            return PAIR.assemble(self.repo, self.output, self.app, self.dns,
                                 source or self.runtime, app_sha or self.app_sha,
                                 dns_sha or self.dns_sha, arch, stable)

    def test_stable_pair_requires_explicit_mode_and_remains_unpublished(self):
        with self.assertRaises(ValueError):
            self.assembly(stable=True)  # An RC cannot masquerade as stable.
        self.write('Cargo.toml', '[workspace.package]\nversion = "0.9.8"\n')
        self.write('manifest.json', '{"version":"0.9.8"}')
        for path in ('plugin/runtime-release.json', 'plugin/dns-release.json'):
            self.write(path, json.dumps({'schemaVersion': 1, 'version': '0.9.8', 'packages': {}}))
        self.runtime = self.commit()
        with self.assertRaises(ValueError):
            self.assembly()  # Stable is never inferred by the default RC mode.
        self.assertEqual(list(self.output.iterdir()), [])
        value = self.assembly(stable=True)
        self.assertEqual(value['version'], '0.9.8')
        self.assertEqual(value['publication'], 'unpublished-candidate')
        self.assertFalse(value['publishedDownloadVerified'])
        self.assertEqual(value['bootstrapPins'], 'empty')
        self.assertTrue((self.output / f'omavless-0.9.8-1-{os.uname().machine}.pkg.tar.zst').is_file())

    def test_exact_members_include_no_unreviewed_path(self):
        members = PAIR.expected_members()
        self.assertIn('.INSTALL', members)
        self.assertIn('usr/lib/omavless-dns/mihomo', members)
        self.assertNotIn('usr/lib/omavless-dns-experimental/mihomo', members)
        self.assertIn('usr/lib/omavless-dns/', members)

    def test_beta_pair_keeps_version_source_and_unpublished_boundaries(self):
        self.write('Cargo.toml', '[workspace.package]\nversion = "0.9.5-beta.1"\n')
        self.write('manifest.json', '{"version":"0.9.5-beta.1"}')
        for path in ('plugin/runtime-release.json', 'plugin/dns-release.json'):
            self.write(path, json.dumps({'schemaVersion': 1, 'version': '0.9.5-beta.1', 'packages': {}}))
        self.runtime = self.commit()
        result = self.assembly()
        self.assertEqual(result['version'], '0.9.5-beta.1')
        self.assertEqual(result['bootstrapPins'], 'empty')
        self.assertEqual(result['runtimeSourceCommit'], self.runtime)
        self.assertFalse(result['publishedDownloadVerified'])
        self.assertEqual(result['publication'], 'unpublished-candidate')
        for name in ('omavless', 'omavless-dns'):
            self.assertTrue((self.output / f'{name}-0.9.5beta1-1-{os.uname().machine}.pkg.tar.zst').is_file())

    def test_package_metadata_rejects_duplicate_identity(self):
        with self.assertRaises(ValueError):
            PAIR.package_values(b'pkgname = omavless\npkgname = evil\npkgver = 1\narch = x86_64\n')

    def test_dns_inspector_rejects_extra_member_before_extracting(self):
        listed = ('\n'.join(sorted(PAIR.expected_members() | {'etc/unreviewed'})) + '\n').encode()
        with patch.object(PAIR.inspection, 'capture', return_value=listed), \
             patch.object(PAIR, 'member') as extract:
            with self.assertRaises(ValueError):
                PAIR.inspect_dns_archive(Path('/reviewed/omavless-dns.pkg.tar.zst'),
                                         '0.9.0rc1', os.uname().machine, self.runtime)
            extract.assert_not_called()

    def test_empty_pair_pins_and_reuse(self):
        result = self.assembly()
        self.assertEqual(result['bootstrapPins'], 'empty')
        self.assertEqual(result['runtimeSourceCommit'], self.runtime)
        self.assertFalse(result['publishedDownloadVerified'])
        self.assertEqual(result['publication'], 'unpublished-candidate')
        self.assertEqual(len(list(self.output.glob('*.pkg.tar.zst'))), 2)
        self.assertTrue((self.output / 'SHA256SUMS').is_file())

    def test_foreign_architecture_is_checked_offline_not_executed(self):
        other = 'aarch64' if os.uname().machine == 'x86_64' else 'x86_64'
        result = self.assembly(arch=other)
        self.assertEqual(result['architecture'], other)
        self.assertEqual(len(list(self.output.glob('*-' + other + '.pkg.tar.zst'))), 2)

    def test_mismatched_application_elf_architecture_refuses_before_output(self):
        wrong = 183 if os.uname().machine == 'x86_64' else 62
        with self.assertRaises(ValueError):
            self.assembly(binary_machine=wrong)
        self.assertEqual(list(self.output.iterdir()), [])

    def test_both_matching_pins(self):
        arch = os.uname().machine
        self.write('plugin/runtime-release.json', self.pins({arch: {
            'sha256': self.app_sha, 'sourceCommit': self.runtime}}))
        self.write('plugin/dns-release.json', self.pins({arch: {
            'sha256': self.dns_sha, 'sourceCommit': self.runtime}}))
        head = self.commit()
        self.assertEqual(self.assembly(head)['bootstrapPins'], 'matched')

    def test_partial_and_wrong_pins_refuse_before_output(self):
        arch = os.uname().machine
        self.write('plugin/runtime-release.json', self.pins({arch: {
            'sha256': self.app_sha, 'sourceCommit': self.runtime}}))
        with self.assertRaises(ValueError):
            self.assembly(self.commit())
        self.assertEqual(list(self.output.iterdir()), [])
        self.write('plugin/dns-release.json', self.pins({arch: {
            'sha256': '0' * 64, 'sourceCommit': self.runtime}}))
        with self.assertRaises(ValueError):
            self.assembly(self.commit())
        self.assertEqual(list(self.output.iterdir()), [])

    def test_duplicate_dns_pin_field_refuses(self):
        self.write('plugin/dns-release.json',
                   '{"schemaVersion":1,"schemaVersion":1,"version":"0.9.0-rc.1","packages":{}}')
        with self.assertRaises(ValueError):
            self.assembly(self.commit())
        self.assertEqual(list(self.output.iterdir()), [])

    def test_runtime_drift_refuses_before_output(self):
        self.write('Cargo.lock', 'changed')
        with self.assertRaises(ValueError):
            self.assembly(self.commit())
        self.assertEqual(list(self.output.iterdir()), [])

    def test_bad_hash_refuses_before_archive_inspection(self):
        with self.assertRaises(ValueError):
            self.assembly(app_sha='0' * 64)
        self.assertEqual(list(self.output.iterdir()), [])

    def test_dirty_source_and_occupied_output_refuse(self):
        self.write('plugin/Panel.qml', 'dirty')
        with self.assertRaises(ValueError):
            self.assembly()
        self.commit()
        (self.output / 'keep').write_text('preserve')
        with self.assertRaises(ValueError):
            self.assembly(self.git('rev-parse', 'HEAD').decode().strip())
        self.assertEqual((self.output / 'keep').read_text(), 'preserve')


if __name__ == '__main__':
    unittest.main()
