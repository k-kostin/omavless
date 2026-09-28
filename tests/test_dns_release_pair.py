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
        return subprocess.check_output(['git', '-C', str(self.repo), *args], stderr=subprocess.DEVNULL)

    def commit(self):
        self.git('add', '.')
        self.git('commit', '-qm', 'fixture')
        return self.git('rev-parse', 'HEAD').decode().strip()

    def assembly(self, source=None, app_sha=None, dns_sha=None):
        app_mark = (self.app_sha, 0o600, os.getuid())
        dns_mark = (self.dns_sha, 0o600, os.getuid())

        def fingerprint(path, uid):
            return app_mark if path == self.app else dns_mark

        with patch.object(PAIR.inspection, 'fingerprint', side_effect=fingerprint), \
             patch.object(PAIR.inspection, 'safe_parents'), \
             patch.object(PAIR.inspection, 'inspect_archive', return_value={
                 'version': '0.9.0rc1-1', 'source': self.runtime,
                 'binary': '1' * 64}), \
             patch.object(PAIR, 'inspect_dns_archive', return_value={
                 'version': '0.9.0rc1-1', 'source': self.runtime,
                 'coreSha256': '2' * 64, 'brokerSha256': '3' * 64,
                 'receiptSha256': '4' * 64}), \
             patch.object(PAIR, 'member', return_value=(
                 b'pkgname = omavless\npkgver = 0.9.0rc1-1\narch = x86_64\n'
                 b'depend = omavless-dns=0.9.0rc1-1\n')):
            return PAIR.assemble(self.repo, self.output, self.app, self.dns,
                                 source or self.runtime, app_sha or self.app_sha,
                                 dns_sha or self.dns_sha)

    def test_exact_members_include_no_unreviewed_path(self):
        members = PAIR.expected_members()
        self.assertIn('.INSTALL', members)
        self.assertIn('usr/lib/omavless-dns/mihomo', members)
        self.assertNotIn('usr/lib/omavless-dns-experimental/mihomo', members)
        self.assertIn('usr/lib/omavless-dns/', members)

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
