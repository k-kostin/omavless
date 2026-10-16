# SPDX-License-Identifier: MIT
"""Real committed-tree export; inert API fixtures, no install/VM/publication."""
import importlib.util
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

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
                   ('Reviewed removal entry ' + EXPORT.digest((self.repo / 'tools/uninstall.sh').read_bytes())).encode())
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
        return subprocess.check_output(['git', '-C', str(self.repo), *args], stderr=subprocess.DEVNULL)

    def commit(self):
        self.git('add', '.')
        self.git('commit', '-qm', 'fixture')
        return self.git('rev-parse', 'HEAD').decode().strip()

    def export(self):
        return EXPORT.export(self.repo, self.sha, self.output)

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

    def test_product_version_disagreement_fails(self):
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
        with patch.object(EXPORT, 'api', side_effect=OSError('API unavailable')):
            with self.assertRaises(OSError):
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


if __name__ == '__main__':
    unittest.main()
