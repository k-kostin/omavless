# SPDX-License-Identifier: MIT
"""Real committed-tree export; inert API fixtures, no install/VM/publication."""
import importlib.util
import contextlib
import http.client
import io
import json
import os
from pathlib import Path
import re
import runpy
import shlex
import subprocess
import sys
import tempfile
import unittest
import urllib.error
from unittest.mock import MagicMock, patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('distribution', ROOT / 'packaging/release/export-distribution.py')
EXPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(EXPORT)
BOOT_SPEC = importlib.util.spec_from_file_location('distribution_bootstrap',
    ROOT / 'packaging/release/distribution/source-bootstrap.py')
BOOT = importlib.util.module_from_spec(BOOT_SPEC)
BOOT_SPEC.loader.exec_module(BOOT)


class DistributionTests(unittest.TestCase):
    def setUp(self):
        cache = Path.home() / '.cache'
        cache.mkdir(exist_ok=True)
        self.tmp = tempfile.TemporaryDirectory(prefix='ov-dist-', dir=cache)
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name).resolve()
        self.repo = self.base / 'source'
        self.repo.mkdir()
        for name in EXPORT.TOOLS:
            self.write(name, (ROOT / name).read_bytes())
        for name in ('AGENTS.md', 'CONTRIBUTING.md', 'compatibility.md', 'test.yml',
                     'source-bootstrap.py', 'source-validator-pins.json'):
            self.write(EXPORT.TEMPLATES + name, (ROOT / EXPORT.TEMPLATES / name).read_bytes())
        for name in EXPORT.FILES | EXPORT.ASSETS:
            self.write(name, b'fixture\n')
        for name in ('plugin/Panel.qml', 'plugin/SetupPage.qml'):
            self.write(name, ('Qt.openUrlExternally("' + EXPORT.WEB +
                              'main/docs/user/NATIVE_INSTALL.md")\n').encode())
        self.write('manifest.json', b'{"version":"0.9.8"}')
        guide = '\n'.join('## ' + title for title in ('Install', 'Updates', 'Connect and disconnect', 'Help'))
        self.write('docs/user/NATIVE_INSTALL.md', guide.encode())
        self.write('docs/user/USER_AGENT_GUIDE.md',
                   b'# Assistance\n[Guide](NATIVE_INSTALL.md#install)\n'
                   b'## Report a bug or contribute a fix\n')
        self.write('README.md', b'# Product\n[Guide](docs/user/NATIVE_INSTALL.md#install)\n')
        self.write('docs/user/INSTALL.md', b'old instructions\n')
        self.write('docs/development/private-session.md', b'DEVELOPMENT-ONLY\n')
        self.write('templates/default.yaml', b'mode: rule\n')
        self.write('crates/runtime.rs', b'compiled fixture\n')
        self.write('Cargo.toml', b'[workspace.package]\nversion = "0.9.8"\n')
        self.write('docs/user/CLEAN_REINSTALL_098.md',
                   (ROOT / 'docs/user/CLEAN_REINSTALL_098.md').read_bytes().replace(
                       b'2ecda6be26438e3f6ffa46cc66c7f1c3b55ae1db6d176cf8471b4963724532f1',
                       EXPORT.digest((self.repo / 'tools/uninstall.sh').read_bytes()).encode()))
        self.write('packaging/release/client-features.sh', b'features\n')
        self.write('.github/workflows/test.yml', (ROOT / '.github/workflows/test.yml').read_bytes())
        self.git('init', '-q')
        self.git('config', 'user.name', 'Fixture')
        self.git('config', 'user.email', 'fixture@example.invalid')
        self.git('config', 'commit.gpgsign', 'false')
        self.producer = self.commit()
        pin = {'sourceCommit': self.producer, 'sha256': 'a' * 64}
        for name in ('runtime', 'dns'):
            self.write('plugin/' + name + '-release.json', EXPORT.canonical({
                'schemaVersion': 1, 'version': '0.9.8',
                'packages': {arch: pin for arch in ('aarch64', 'x86_64')}}))
        identity = {field: 'b' * 64 for field in ('appSha', 'coreSha', 'brokerSha', 'dnsReceiptSha')}
        self.write('plugin/component-updates.json', EXPORT.canonical({
            'schemaVersion': 2, 'sourceCommit': self.producer, 'updatePair': True,
            'identities': {arch: identity for arch in ('aarch64', 'x86_64')}}))
        self.sha = self.commit()
        self.output = self.base / 'output'
        self.output.mkdir()

    def write(self, name, data):
        path = self.repo / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)

    def git(self, *args):
        # Keep temporary repository writes synchronous without host config edits.
        return subprocess.check_output(['git', '-c', 'maintenance.auto=false',
            '-c', 'gc.auto=0', '-c', 'maintenance.autoDetach=false',
            '-c', 'gc.autoDetach=false', '-C', str(self.repo), *args], stderr=subprocess.DEVNULL)

    def commit(self):
        self.git('add', '.')
        self.git('commit', '-qm', 'fixture')
        return self.git('rev-parse', 'HEAD').decode().strip()

    def export(self):
        return EXPORT.export(self.repo, self.sha, self.output)

    def localized_guide_fixture(self, version='0.9.8'):
        # Synthetic prose/review only; never approves the real Russian guide.
        english = b'## Install\n## Updates\n## Help\n```sh\nprintf fixture\n```\n'
        russian = (b'<a id="install"></a>\n<a id="updates"></a>\n<a id="help"></a>\n'
                   b'```sh\nprintf fixture\n```\n')
        value = {'schemaVersion': 1, 'version': version,
                 'english': {'path': 'docs/user/NATIVE_INSTALL.md', 'sha256': EXPORT.digest(english)},
                 'russian': {'path': 'docs/user/NATIVE_INSTALL_RU.md', 'sha256': EXPORT.digest(russian),
                             'humanReview': None}}
        data = b'// BEGIN GUIDE MANIFEST\nvar guideManifest = ' + EXPORT.canonical(value).rstrip() + b';\n// END GUIDE MANIFEST\n'
        self.write('plugin/GuideLinks.js', data)
        self.write(value['english']['path'], english)
        self.write(value['russian']['path'], russian)
        return value

    def test_localized_guide_source_and_export_structure_are_separate_checks(self):
        value = self.localized_guide_fixture()
        entries = {name: ((self.repo / name).read_bytes(), 0o644) for name in
                   ('plugin/GuideLinks.js', value['english']['path'], value['russian']['path'])}
        self.assertEqual(EXPORT.guide_manifest(entries, '0.9.8'), value)
        # Allowed link rewrites may change bytes after source identity checking.
        entries[value['english']['path']] = (entries[value['english']['path']][0] + b'\n[Ref](https://example.invalid/source)\n', 0o644)
        EXPORT.guide_structure(entries, value)
        with self.assertRaisesRegex(ValueError, 'stale or missing guide source'):
            EXPORT.guide_manifest(entries, '0.9.8')

    def test_current_complete_guide_pair_uses_the_actual_exporter_boundary(self):
        entries = {name: ((ROOT / name).read_bytes(), 0o644) for name in
                   ('plugin/GuideLinks.js', 'docs/user/NATIVE_INSTALL.md', 'docs/user/NATIVE_INSTALL_RU.md')}
        version = json.loads((ROOT / 'manifest.json').read_bytes())['version']
        value = EXPORT.guide_manifest(entries, version)
        self.assertEqual(value['english']['path'], 'docs/user/NATIVE_INSTALL.md')
        # Covers the H1 title as well as explicit section destinations.
        self.assertIn('omavless-user-guide', EXPORT.anchors(entries[value['russian']['path']][0]))

    def test_semantic_english_drift_invalidates_localized_source_before_export(self):
        value = self.localized_guide_fixture()
        self.write(value['english']['path'], (self.repo / value['english']['path']).read_bytes() + b'\nMeaning changed.\n')
        self.sha = self.commit()
        with self.assertRaisesRegex(ValueError, 'stale or missing guide source'):
            self.export()
        self.assertEqual(list(self.output.iterdir()), [])

    def test_reviewed_russian_missing_destination_or_anchor_refuses_before_export(self):
        value = self.localized_guide_fixture()
        value['russian']['humanReview'] = {'kind': 'owner-human-meaning', 'version': value['version'],
            'englishSha256': value['english']['sha256'], 'russianSha256': value['russian']['sha256'],
            'reviewedOn': '2026-10-30'}
        data = b'// BEGIN GUIDE MANIFEST\nvar guideManifest = ' + EXPORT.canonical(value).rstrip() + b';\n// END GUIDE MANIFEST\n'
        self.write('plugin/GuideLinks.js', data)
        entries = {name: ((self.repo / name).read_bytes(), 0o644) for name in
                   ('plugin/GuideLinks.js', value['english']['path'], value['russian']['path'])}
        del entries[value['russian']['path']]
        with self.assertRaisesRegex(ValueError, 'stale or missing guide source'):
            EXPORT.guide_manifest(entries, '0.9.8')
        entries[value['russian']['path']] = (b'```sh\nprintf fixture\n```\n', 0o644)
        value['russian']['sha256'] = EXPORT.digest(entries[value['russian']['path']][0])
        value['russian']['humanReview']['russianSha256'] = value['russian']['sha256']
        entries['plugin/GuideLinks.js'] = (b'// BEGIN GUIDE MANIFEST\nvar guideManifest = '
            + EXPORT.canonical(value).rstrip() + b';\n// END GUIDE MANIFEST\n', 0o644)
        with self.assertRaisesRegex(ValueError, 'missing translated guide anchor'):
            EXPORT.guide_manifest(entries, '0.9.8')

    def test_real_export_is_reproducible_user_tree_not_working_files(self):
        self.write('README.md', b'UNCOMMITTED-MUST-NOT-SHIP')
        self.write('plugin/untracked-secret.txt', b'UNTRACKED-MUST-NOT-SHIP')
        record = self.export()
        self.assertNotIn(b'UNCOMMITTED', (self.output / 'README.md').read_bytes())
        self.assertFalse((self.output / 'plugin/untracked-secret.txt').exists())
        self.assertFalse((self.output / 'crates').exists())
        self.assertFalse((self.output / 'docs/development').exists())
        self.assertFalse((self.output / 'packaging').exists())
        self.assertIn('help the user', (self.output / 'AGENTS.md').read_text())
        self.assertNotIn('old instructions', (self.output / 'docs/user/INSTALL.md').read_text())
        self.assertIn(EXPORT.WEB + self.sha, (self.output / 'CONTRIBUTING.md').read_text())
        self.assertIn('docs/user/USER_AGENT_GUIDE.md#report-a-bug-or-contribute-a-fix',
                      (self.output / 'CONTRIBUTING.md').read_text())
        self.assertEqual(record['state'], 'draft-unpublished')
        self.assertIsNone(EXPORT.verify(self.repo, self.output))
        another = self.base / 'another'
        another.mkdir()
        self.assertEqual(record, EXPORT.export(self.repo, self.sha, another))
        self.assertEqual((self.output / EXPORT.RECORD).read_bytes(), (another / EXPORT.RECORD).read_bytes())

    def test_unsafe_output_refuses_before_copy(self):
        occupied = self.base / 'occupied'
        occupied.mkdir()
        (occupied / 'keep').write_text('keep')
        redirect = self.base / 'redirect'
        redirect.symlink_to(self.output, target_is_directory=True)
        for target in (occupied, redirect, self.repo, self.repo / 'inside'):
            with self.subTest(target=target), self.assertRaises((ValueError, FileNotFoundError)):
                EXPORT.export(self.repo, self.sha, target)
        self.assertEqual((occupied / 'keep').read_text(), 'keep')
        self.assertEqual(list(self.output.iterdir()), [])

    def test_writable_parent_refuses_before_payload_creation(self):
        unsafe = self.base / 'writable'
        unsafe.mkdir(mode=0o777)
        unsafe.chmod(0o777)
        target = unsafe / 'output'
        target.mkdir(mode=0o700)
        with self.assertRaises(ValueError):
            EXPORT.export(self.repo, self.sha, target)
        self.assertEqual(list(target.iterdir()), [])

    def test_missing_or_nonregular_helper_refuses(self):
        helper = self.repo / 'tools/uninstall.sh'
        helper.unlink()
        self.sha = self.commit()
        with self.assertRaises(KeyError):
            self.export()
        helper.symlink_to('../README.md')
        self.sha = self.commit()
        with self.assertRaises(ValueError):
            self.export()
        self.assertEqual(list(self.output.iterdir()), [])

    def test_modified_bytes_mode_missing_and_extra_members_fail(self):
        self.export()
        target = self.output / 'README.md'
        original = target.read_bytes()
        target.write_bytes(original + b'changed')
        with self.assertRaises(ValueError):
            EXPORT.verify(self.repo, self.output)
        target.write_bytes(original)
        target.chmod(0o755)
        with self.assertRaises(ValueError):
            EXPORT.verify(self.repo, self.output)
        target.chmod(0o644)
        extra = self.output / 'unexpected.txt'
        extra.write_text('extra')
        with self.assertRaises(ValueError):
            EXPORT.verify(self.repo, self.output)
        extra.unlink()
        target.unlink()
        with self.assertRaises(ValueError):
            EXPORT.verify(self.repo, self.output)

    def test_forged_record_and_redirected_payload_fail(self):
        self.export()
        path = self.output / EXPORT.RECORD
        original = path.read_bytes()
        value = json.loads(original)
        value['sourceCommit'] = self.producer
        path.write_bytes(EXPORT.canonical(value))
        with self.assertRaises((ValueError, KeyError)):
            EXPORT.verify(self.repo, self.output)
        path.write_bytes(original)
        helper = self.output / 'tools/uninstall.sh'
        helper.unlink()
        helper.symlink_to(self.repo / 'tools/uninstall.sh')
        with self.assertRaises(ValueError):
            EXPORT.verify(self.repo, self.output)

    def test_help_anchor_and_missing_link_refuse(self):
        for link in ('docs/user/NATIVE_INSTALL.md#absent', 'missing.md'):
            self.write('README.md', ('[Help](' + link + ')').encode())
            self.sha = self.commit()
            with self.assertRaises(ValueError):
                self.export()
            self.assertEqual(list(self.output.iterdir()), [])

    def test_contribution_router_requires_shipped_assistance_anchor(self):
        self.write('docs/user/USER_AGENT_GUIDE.md',
                   b'# Assistance\n[Guide](NATIVE_INSTALL.md#install)\n')
        self.sha = self.commit()
        with self.assertRaisesRegex(ValueError, 'missing guide anchor'):
            self.export()
        self.assertEqual(list(self.output.iterdir()), [])

    def test_changed_removal_guide_pin_fails(self):
        self.write('docs/user/CLEAN_REINSTALL_098.md', b'wrong helper digest')
        self.sha = self.commit()
        with self.assertRaisesRegex(ValueError, 'guide/helper'):
            self.export()

    def published_removal_inputs(self):
        guide = (ROOT / 'docs/user/CLEAN_REINSTALL_098.md').read_bytes()
        snapshot = 'tests/fixtures/public-uninstaller/0250efeb87ff5f6c331a2d067c181a29084b37a2.sh.txt'
        self.write('docs/user/CLEAN_REINSTALL_098.md', guide)
        self.write(snapshot, (ROOT / snapshot).read_bytes())
        self.sha = self.commit()
        return guide, snapshot

    def test_published_guide_rebinds_only_generated_draft_to_its_shipped_helper(self):
        original, _ = self.published_removal_inputs()
        record = self.export()
        generated = (self.output / 'docs/user/CLEAN_REINSTALL_098.md').read_bytes()
        helper = (self.output / 'tools/uninstall.sh').read_bytes()
        expected_url = ('https://raw.githubusercontent.com/k-kostin/omavless/'
                        + self.sha + '/tools/uninstall.sh').encode()
        self.assertIn(expected_url, generated)
        self.assertIn(EXPORT.digest(helper).encode(), generated)
        self.assertNotIn(b'0250efeb87ff5f6c331a2d067c181a29084b37a2/tools', generated)
        self.assertEqual((self.repo / 'docs/user/CLEAN_REINSTALL_098.md').read_bytes(), original)
        self.assertFalse(any(p.startswith('tests/') for p in record['files']))
        self.assertEqual(record['state'], 'draft-unpublished')
        self.assertIsNone(EXPORT.verify(self.repo, self.output))
        another = self.base / 'another'
        another.mkdir()
        self.assertEqual(record, EXPORT.export(self.repo, self.sha, another))

    def test_generated_removal_recipe_admits_only_matching_local_or_download_bytes(self):
        _, snapshot = self.published_removal_inputs()
        self.export()
        guide = (self.output / 'docs/user/CLEAN_REINSTALL_098.md').read_text()
        blocks = re.findall(r'```bash\n(.*?)\n```', guide, re.S)
        self.assertEqual(len(blocks), 3)
        selected = (self.output / 'tools/uninstall.sh').read_bytes()
        public = (self.repo / snapshot).read_bytes()
        expected_url = 'https://raw.githubusercontent.com/k-kostin/omavless/' + self.sha + '/tools/uninstall.sh'
        for kind in ('local', 'download', 'old-local', 'modified-local', 'bad-download'):
            with self.subTest(kind=kind):
                home = self.base / kind
                home.mkdir()
                (home / 'approved-download').write_bytes(b'wrong' if kind == 'bad-download' else selected)
                local = home / '.config/omarchy/plugins/kdk.omavless/tools/uninstall.sh'
                if kind not in ('download', 'bad-download'):
                    local.parent.mkdir(parents=True)
                    local.write_bytes(public if kind == 'old-local' else b'modified' if kind == 'modified-local' else selected)
                # Inert leaves: the helper body is NEVER executed, and curl
                # must witness exactly the generated immutable URL before copy.
                script = '''set -euo pipefail
bash() { printf 'HELPER_ADMITTED\\n'; }
curl() {
  [[ "${@: -3:1}" == "$EXPECTED_URL" ]] || return 91
  printf 'DOWNLOAD_ADMITTED\\n'
  command cp -- "$HOME/approved-download" "${@: -1}"
}
''' + blocks[0]
                result = subprocess.run(['bash', '--noprofile', '--norc'], input=script,
                                        text=True, capture_output=True, timeout=5,
                                        env={**os.environ, 'HOME': str(home), 'EXPECTED_URL': expected_url})
                admitted = kind in ('local', 'download')
                self.assertEqual(result.returncode == 0, admitted, result.stderr)
                self.assertEqual('HELPER_ADMITTED' in result.stdout, admitted)
                self.assertEqual('DOWNLOAD_ADMITTED' in result.stdout, kind in ('download', 'bad-download'))

    def test_unknown_public_guide_or_snapshot_refuses_before_payload_writes(self):
        original, snapshot = self.published_removal_inputs()
        cases = {
            'unknown_source': original.replace(b'0250efeb87ff5f6c331a2d067c181a29084b37a2', b'0' * 40),
            'mutable_source': original.replace(b'0250efeb87ff5f6c331a2d067c181a29084b37a2', b'main'),
            'wrong_checksum': original.replace(b'2ecda6be26438e3f6ffa46cc66c7f1c3b55ae1db6d176cf8471b4963724532f1', b'0' * 64),
            'extra_pin': original + b'\n2ecda6be26438e3f6ffa46cc66c7f1c3b55ae1db6d176cf8471b4963724532f1\n',
            'extra_url': original + b'\nhttps://raw.githubusercontent.com/k-kostin/omavless/0250efeb87ff5f6c331a2d067c181a29084b37a2/tools/uninstall.sh\n',
            'changed_recipe': original.replace(b'sha256sum --check -', b'true'),
        }
        for label, guide in cases.items():
            with self.subTest(label=label):
                self.write('docs/user/CLEAN_REINSTALL_098.md', guide)
                self.sha = self.commit()
                with self.assertRaises(ValueError):
                    self.export()
                self.assertEqual(list(self.output.iterdir()), [])
        self.write('docs/user/CLEAN_REINSTALL_098.md', original)
        self.write(snapshot, b'modified published snapshot')
        self.sha = self.commit()
        with self.assertRaises(ValueError):
            self.export()
        self.assertEqual(list(self.output.iterdir()), [])

    def test_missing_redirected_or_wrong_mode_public_snapshot_refuses(self):
        _, snapshot = self.published_removal_inputs()
        path = self.repo / snapshot
        payload = path.read_bytes()
        path.unlink()
        self.sha = self.commit()
        with self.assertRaises(KeyError):
            self.export()
        self.assertEqual(list(self.output.iterdir()), [])
        path.symlink_to(self.repo / 'README.md')
        self.sha = self.commit()
        with self.assertRaises(ValueError):
            self.export()
        self.assertEqual(list(self.output.iterdir()), [])
        path.unlink()
        path.write_bytes(payload)
        path.chmod(0o755)
        self.sha = self.commit()
        with self.assertRaises(ValueError):
            self.export()
        self.assertEqual(list(self.output.iterdir()), [])

    def test_generated_removal_guide_or_helper_tampering_refuses(self):
        self.published_removal_inputs()
        self.export()
        for name in ('docs/user/CLEAN_REINSTALL_098.md', 'tools/uninstall.sh'):
            target = self.output / name
            original = target.read_bytes()
            target.write_bytes(original + b'\nchanged\n')
            with self.assertRaises(ValueError):
                EXPORT.verify(self.repo, self.output)
            target.write_bytes(original)

    def test_product_version_disagreement_fails(self):
        self.localized_guide_fixture(version='0.9.9')
        self.write('manifest.json', b'{"version":"0.9.9"}')
        self.sha = self.commit()
        with self.assertRaisesRegex(ValueError, 'product version'):
            self.export()

    def test_unexpected_empty_directory_also_fails(self):
        self.export()
        (self.output / 'scratch').mkdir()
        with self.assertRaises(ValueError):
            EXPORT.verify(self.repo, self.output)

    def test_excluded_source_link_becomes_immutable(self):
        self.write('README.md', b'[Source](docs/development/private-session.md)')
        self.sha = self.commit()
        self.export()
        self.assertIn(EXPORT.WEB + self.sha + '/docs/development/private-session.md',
                      (self.output / 'README.md').read_text())

    def test_old_native_pair_cannot_cover_changed_crate_even_test_only(self):
        self.write('crates/runtime.rs', b'compiled fixture\n#[cfg(test)] mod extra {}\n')
        self.sha = self.commit()
        with self.assertRaisesRegex(ValueError, 'runtime inputs changed'):
            self.export()
        self.assertEqual(list(self.output.iterdir()), [])

    def test_mixed_native_tuple_refuses(self):
        value = json.loads((self.repo / 'plugin/dns-release.json').read_bytes())
        value['packages']['aarch64']['sourceCommit'] = '0' * 40
        self.write('plugin/dns-release.json', EXPORT.canonical(value))
        self.sha = self.commit()
        with self.assertRaises(ValueError):
            self.export()

    def api_fixture(self, run_change=None, step_change=None):
        run = {'id': 123, 'workflow_id': 99, 'run_attempt': 1, 'head_sha': self.sha,
               'head_repository': {'full_name': EXPORT.REPO}, 'path': '.github/workflows/test.yml',
               'status': 'completed', 'conclusion': 'success', 'event': 'push'}
        run.update(run_change or {})
        job = {'name': 'test', 'conclusion': 'success', 'steps': [
            {'name': name, 'conclusion': (step_change or {}).get(name, 'success')}
            for name in ('Run isolated checks', 'Run Rust workspace and parity checks')]}
        def response(path):
            if path == 'actions/workflows/test.yml':
                return {'id': 99}
            if path.startswith('actions/workflows/test.yml/runs?'):
                return {'workflow_runs': [run]}
            if path.startswith('actions/runs/'):
                return {'total_count': 1, 'jobs': [job]}
            if path.startswith('contents/.github/workflows/test.yml?'):
                return {'sha': EXPORT.SOURCE_WORKFLOW_BLOB}
            raise AssertionError('unexpected API path')
        return response

    def test_ci_uses_exact_completed_head_repository_workflow_and_steps(self):
        self.export()
        with patch.object(EXPORT, 'api', side_effect=self.api_fixture()):
            self.assertEqual(EXPORT.verify(self.repo, self.output, True), 123)
        for change in ({'head_sha': self.producer}, {'head_repository': {'full_name': 'other/repo'}},
                       {'workflow_id': 98}, {'path': '.github/workflows/other.yml'},
                       {'status': 'in_progress'}, {'conclusion': 'cancelled'}, {'event': 'pull_request'}):
            with self.subTest(change=change), patch.object(EXPORT, 'api', side_effect=self.api_fixture(change)):
                with self.assertRaises(ValueError):
                    EXPORT.verify(self.repo, self.output, True)
        with patch.object(EXPORT, 'api', side_effect=self.api_fixture(step_change={'Run Rust workspace and parity checks': 'skipped'})):
            with self.assertRaises(ValueError):
                EXPORT.verify(self.repo, self.output, True)
        with patch.object(EXPORT, 'api', side_effect=OSError('API unavailable')) as api:
            with self.assertRaises(OSError):
                EXPORT.verify(self.repo, self.output, True)
            api.assert_called()

    def test_reviewed_workflow_and_validator_pins_match_actual_source_bytes(self):
        workflow = ROOT / '.github/workflows/test.yml'
        oid = subprocess.check_output(['git', 'hash-object', str(workflow)]).decode().strip()
        self.assertEqual(EXPORT.SOURCE_WORKFLOW_BLOB, oid)
        pins = json.loads((ROOT / EXPORT.TEMPLATES / 'source-validator-pins.json').read_text())
        self.assertEqual(pins, {name: EXPORT.digest((ROOT / name).read_bytes()) for name in EXPORT.TOOLS})

    def test_remote_workflow_mismatch_and_partial_jobs_are_not_source_ci(self):
        self.export()
        for fault in ('remote_blob', 'job_count', 'missing_job', 'wrong_job', 'missing_isolated'):
            response = self.api_fixture()
            def api(path):
                value = response(path)
                if fault == 'remote_blob' and path.startswith('contents/'):
                    return {'sha': '0' * 40}
                if path.startswith('actions/runs/'):
                    if fault == 'job_count':
                        value['total_count'] = 2
                    elif fault == 'missing_job':
                        value['jobs'] = []
                    elif fault == 'wrong_job':
                        value['jobs'][0]['name'] = 'other'
                    elif fault == 'missing_isolated':
                        value['jobs'][0]['steps'] = value['jobs'][0]['steps'][1:]
                return value
            with self.subTest(fault=fault), patch.object(EXPORT, 'api', side_effect=api):
                with self.assertRaises(ValueError):
                    EXPORT.verify(self.repo, self.output, True)

    def test_unknown_source_workflow_fails_even_if_it_contains_old_names(self):
        self.write('.github/workflows/test.yml', (ROOT / '.github/workflows/test.yml').read_bytes() + b'\n# changed\n')
        self.sha = self.commit()
        self.export()
        with patch.object(EXPORT, 'api', side_effect=AssertionError('must refuse before API')):
            with self.assertRaises(ValueError):
                EXPORT.verify(self.repo, self.output, True)

    def test_bootstrap_refuses_candidate_exit_zero_validator_before_execution(self):
        self.export()
        # The candidate record can name another source; it cannot authorize
        # different executable bytes through the separately pinned bootstrap.
        self.write(EXPORT.TOOLS[0], b'import sys\nsys.exit(0)\n')
        argv = ['bootstrap', str(self.repo), str(self.output)]
        with patch.object(BOOT.sys, 'argv', argv), patch.object(BOOT.os, 'execv') as execute:
            with self.assertRaisesRegex(ValueError, 'reviewed bytes'):
                BOOT.main()
            execute.assert_not_called()

    def test_bootstrap_executes_only_reviewed_files_in_isolated_mode(self):
        self.export()
        argv = ['bootstrap', str(self.repo), str(self.output)]
        with patch.object(BOOT.sys, 'argv', argv), patch.object(BOOT.os, 'execv') as execute:
            BOOT.main()
            self.assertEqual(execute.call_count, 1)
            self.assertEqual(execute.call_args.args[1][1], '-I')
            self.assertEqual(execute.call_args.args[1][2], '-B')
            self.assertEqual(execute.call_args.args[1][-1], '--source-ci')

    def test_bootstrap_refuses_preexisting_source_bytecode_before_execution(self):
        self.export()
        cache = self.repo / 'packaging/release/__pycache__'
        cache.mkdir()
        (cache / 'pair-frontend.synthetic.pyc').write_bytes(b'untrusted')
        argv = ['bootstrap', str(self.repo), str(self.output)]
        with patch.object(BOOT.sys, 'argv', argv), patch.object(BOOT.os, 'execv') as execute:
            with self.assertRaisesRegex(ValueError, 'bytecode cache'):
                BOOT.main()
            execute.assert_not_called()

    def test_legacy_anchor_stays_reachable_without_copying_old_instructions(self):
        self.write('docs/user/INSTALL.md', b'# Older install\n## Legacy details\nOLD-INSTRUCTIONS\n')
        self.write('README.md', b'[Older help](docs/user/INSTALL.md#legacy-details)')
        self.sha = self.commit()
        self.export()
        self.assertIn(EXPORT.WEB + self.sha + '/docs/user/INSTALL.md#legacy-details',
                      (self.output / 'README.md').read_text())
        page = (self.output / 'docs/user/INSTALL.md').read_text()
        self.assertIn('<a id="legacy-details"></a>', page)
        self.assertNotIn('OLD-INSTRUCTIONS', page)

    def test_absolute_current_help_and_immutable_source_anchors_are_checked(self):
        for link in (EXPORT.WEB + 'main/docs/user/NATIVE_INSTALL.md#missing',
                     'docs/development/private-session.md#missing'):
            self.write('README.md', ('[Help](' + link + ')').encode())
            self.sha = self.commit()
            with self.assertRaises(ValueError):
                self.export()

    def test_initial_record_reader_cannot_import_untrusted_checkout_json(self):
        self.export()
        workflow = (ROOT / EXPORT.TEMPLATES / 'test.yml').read_text()
        readers = [line.strip() for line in workflow.splitlines() if line.strip().startswith('source_sha=$(')]
        self.assertEqual(len(readers), 1)
        command = shlex.split(readers[0][len('source_sha=$('):-1])
        self.assertEqual(command[:2], ['python3', '-I'])
        (self.output / 'json.py').write_text("from pathlib import Path\nPath('CANARY').write_text('imported')\n")
        canary = self.output / 'CANARY'
        # The previous non-isolated interpreter really imports this private
        # canary. Keep it distinct from the corrected production command.
        before = subprocess.run([sys.executable, *command[2:]], cwd=self.output,
                                capture_output=True, timeout=5)
        self.assertNotEqual(before.returncode, 0)
        self.assertTrue(canary.exists())
        canary.unlink()
        after = subprocess.run([sys.executable, *command[1:]], cwd=self.output,
                               capture_output=True, timeout=5)
        self.assertEqual(after.returncode, 0)
        self.assertEqual(after.stdout.decode().strip(), self.sha)
        self.assertFalse(canary.exists())


class ApiPrivacyTests(unittest.TestCase):
    """Synthetic credentials only; every transport boundary is inert."""
    TOKEN = 'SYNTHETIC_CANARY_0123456789'

    def setUp(self):
        environment = patch.dict(os.environ, {}, clear=True)
        environment.start()
        self.addCleanup(environment.stop)

    def response(self, data=b'{"ok":true}'):
        response = MagicMock()
        response.__enter__.return_value = response
        response.read.return_value = data
        opener = MagicMock()
        opener.open.return_value = response
        return opener

    def test_contaminated_auth_refuses_before_request_or_transport(self):
        for value in ('launcher\n' + self.TOKEN, self.TOKEN + '\r\n continuation',
                      self.TOKEN + '\n', ' ' + self.TOKEN, self.TOKEN + '\t',
                      self.TOKEN + '\x00', self.TOKEN + '\x7f', self.TOKEN + '\u00e9',
                      self.TOKEN + '\x85', 'x' * 4097):
            with self.subTest(kind=repr(value[-3:])), patch.object(EXPORT.os, 'environ', {'GH_TOKEN': value}), \
                    patch.object(EXPORT.urllib.request, 'Request') as request, \
                    patch.object(EXPORT.urllib.request, 'build_opener') as transport:
                with self.assertRaisesRegex(ValueError, 'authentication') as error:
                    EXPORT.api('actions/workflows/test.yml')
                self.assertNotIn(self.TOKEN, str(error.exception))
                request.assert_not_called()
                transport.assert_not_called()

    def test_valid_auth_bytes_and_anonymous_behavior_are_preserved(self):
        for token in (self.TOKEN, 'synthetic-._~+/token==', '', None):
            env = {} if token is None else {'GH_TOKEN': token}
            opener = self.response()
            with self.subTest(auth=bool(token)), patch.dict(os.environ, env, clear=True), \
                    patch.object(EXPORT.urllib.request, 'build_opener', return_value=opener):
                self.assertEqual(EXPORT.api('actions/workflows/test.yml'), {'ok': True})
            request = opener.open.call_args.args[0]
            self.assertEqual(request.get_header('Authorization'), 'Bearer ' + token if token else None)
            self.assertEqual(opener.open.call_args.kwargs, {'timeout': 30})
            self.assertEqual(request.full_url, 'https://api.github.com/repos/k-kostin/omavless/actions/workflows/test.yml')

    def test_transport_exceptions_never_export_auth_or_remote_reason(self):
        for failure in (ValueError('Bearer ' + self.TOKEN), OSError(self.TOKEN),
                        urllib.error.URLError(self.TOKEN),
                        urllib.error.HTTPError('https://example.invalid', 401, self.TOKEN, {}, None),
                        http.client.BadStatusLine(self.TOKEN)):
            if isinstance(failure, urllib.error.HTTPError):
                self.addCleanup(failure.close)
            opener = self.response()
            opener.open.side_effect = failure
            with self.subTest(error=type(failure).__name__), \
                    patch.dict(os.environ, {'GH_TOKEN': self.TOKEN}), \
                    patch.object(EXPORT.urllib.request, 'build_opener', return_value=opener):
                with self.assertRaisesRegex(ValueError, 'CI API request failed') as error:
                    EXPORT.api('actions/workflows/test.yml')
                self.assertNotIn(self.TOKEN, str(error.exception))
                self.assertTrue(error.exception.__suppress_context__)
                self.assertIsNone(error.exception.__cause__)
                self.assertEqual(opener.open.call_count, 1)

    def test_read_decode_size_and_duplicate_key_failures_stay_private(self):
        for data in (b'not-json-' + self.TOKEN.encode(), b'{"a":1,"a":2}', b'x' * (1024 * 1024 + 1)):
            opener = self.response(data)
            with patch.dict(os.environ, {'GH_TOKEN': self.TOKEN}), \
                    patch.object(EXPORT.urllib.request, 'build_opener', return_value=opener):
                with self.assertRaisesRegex(ValueError, 'CI API request failed') as error:
                    EXPORT.api('actions/workflows/test.yml')
                self.assertNotIn(self.TOKEN, str(error.exception))
        opener = self.response()
        opener.open.return_value.read.side_effect = OSError(self.TOKEN)
        with patch.object(EXPORT.urllib.request, 'build_opener', return_value=opener), \
                self.assertRaisesRegex(ValueError, 'CI API request failed'):
            EXPORT.api('actions/workflows/test.yml')

    def test_redirect_handler_still_refuses_without_following(self):
        opener = self.response()
        with patch.object(EXPORT.urllib.request, 'build_opener', return_value=opener) as build:
            EXPORT.api('actions/workflows/test.yml')
        handler = build.call_args.args[0]()
        with self.assertRaises(ValueError):
            handler.redirect_request(None, None, 302, self.TOKEN, {}, 'https://example.invalid')

    def test_cli_refusal_never_formats_exception_payload(self):
        for command in (['export', '/synthetic-output', 'a' * 40], ['verify', '/synthetic-output', '--source-ci']):
            target = 'export' if command[0] == 'export' else 'verify'
            for failure in (ValueError(self.TOKEN), OSError(self.TOKEN), TypeError(self.TOKEN)):
                stderr = io.StringIO()
                with patch.object(sys, 'argv', ['exporter', *command]), \
                        patch.object(EXPORT, target, side_effect=failure), \
                        contextlib.redirect_stderr(stderr), self.assertRaises(SystemExit) as result:
                    EXPORT.main()
                self.assertEqual(result.exception.code, 2)
                self.assertIn('Distribution refused', stderr.getvalue())
                self.assertNotIn(self.TOKEN, stderr.getvalue())

    def test_bootstrap_top_level_refusal_never_formats_filesystem_error(self):
        script = ROOT / 'packaging/release/distribution/source-bootstrap.py'
        with patch.object(sys, 'argv', [str(script), '/synthetic-source', '/synthetic-distribution']), \
                patch.object(Path, 'lstat', side_effect=OSError(self.TOKEN)), \
                self.assertRaises(SystemExit) as result:
            runpy.run_path(str(script), run_name='__main__')
        self.assertIn('Source validator refused', str(result.exception.code))
        self.assertNotIn(self.TOKEN, str(result.exception.code))


if __name__ == '__main__':
    unittest.main()
