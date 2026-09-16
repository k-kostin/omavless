# SPDX-License-Identifier: MIT
"""Offline pairing contracts. Package inspection is separately tested unchanged."""
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
SPEC = importlib.util.spec_from_file_location('frontend_pair', ROOT / 'packaging/release/pair-frontend.py')
PAIR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PAIR)


class FrontendPairTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='omavless-pair-')
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.repo = self.base / 'repo'
        self.repo.mkdir()
        for name, value in {
            'Cargo.toml':'[workspace.package]\nversion = "0.8.0"\n', 'Cargo.lock':'locked',
            'crates/runtime.rs':'accepted runtime', 'templates/default.yaml':'accepted template',
            'LICENSE':'MIT', 'THIRD_PARTY_NOTICES.md':'notices', 'README.md':'intro',
            'manifest.json':'{"version":"0.8.0"}', 'backend.sh':'exit 0', 'install.sh':'exit 0',
            'plugin/Panel.qml':'accepted UI', 'packaging/release/install-frontend.sh':'exit 0',
            'packaging/release/FRONTEND_README.md':'candidate', 'packaging/arch/README.md':'payload docs',
            'packaging/systemd/omavless-runtime.service':'unit',
        }.items():
            self.write(name, value)
        self.git('init', '-q')
        self.git('config', 'user.name', 'Fixture')
        self.git('config', 'user.email', 'fixture@example.invalid')
        self.git('config', 'commit.gpgsign', 'false')
        self.old = self.commit()
        self.output = self.base / 'output'
        self.output.mkdir()
        self.package = self.base / 'accepted.pkg.tar.zst'
        self.package.write_bytes(b'synthetic accepted archive bytes')
        self.sha = hashlib.sha256(self.package.read_bytes()).hexdigest()

    def write(self, name, value):
        file = self.repo / name
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(value)

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.repo), *args], stderr=subprocess.DEVNULL)

    def commit(self):
        self.git('add', '.')
        self.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'fixture')
        return self.git('rev-parse', 'HEAD').decode().strip()

    def pin(self, value):
        self.write('plugin/runtime-release.json', json.dumps(value))
        return self.commit()

    def metadata(self, packages):
        return {'schemaVersion':1, 'version':'0.8.0', 'packages':packages}

    def inspect_pins(self, commit):
        return PAIR.bootstrap_pins(self.repo, commit, '0.8.0', os.uname().machine, self.sha, self.old)

    def test_frontend_only_change_keeps_inputs(self):
        self.write('plugin/Panel.qml', 'new UI')
        self.write('CONTRIBUTING.md', 'developer entry point, not a runtime input')
        self.write('docs/user/INSTALL.md', 'updated instructions')
        new = self.commit()
        self.assertRegex(PAIR.equivalent_inputs(self.repo, self.old, new), r'^[0-9a-f]{64}$')

    def test_runtime_changes_new_files_deletions_and_modes_refuse(self):
        for path in ['Cargo.lock', 'crates/runtime.rs', 'templates/default.yaml',
                     'packaging/arch/README.md', 'packaging/systemd/omavless-runtime.service',
                     'LICENSE', 'THIRD_PARTY_NOTICES.md', '.cargo/config.toml', 'new-build-input']:
            with self.subTest(path=path):
                self.write(path, 'changed')
                with self.assertRaises(ValueError):
                    PAIR.equivalent_inputs(self.repo, self.old, self.commit())
                self.git('revert', '--no-edit', 'HEAD')
        (self.repo / 'crates/runtime.rs').chmod(0o755)
        with self.assertRaises(ValueError):
            PAIR.equivalent_inputs(self.repo, self.old, self.commit())
        self.git('revert', '--no-edit', 'HEAD')
        (self.repo / 'templates/default.yaml').unlink()
        with self.assertRaises(ValueError):
            PAIR.equivalent_inputs(self.repo, self.old, self.commit())

    def test_reverse_ancestry_refuses_even_identical_runtime(self):
        self.write('README.md', 'new')
        new = self.commit()
        with self.assertRaises(subprocess.CalledProcessError):
            PAIR.equivalent_inputs(self.repo, new, self.old)

    def test_invalid_commit_never_reaches_git(self):
        with patch.object(PAIR.release, 'git') as git:
            with self.assertRaises(ValueError):
                PAIR.runtime_tree(self.repo, '--help')
            git.assert_not_called()

    def test_missing_and_empty_pins_are_not_verified(self):
        self.assertEqual(self.inspect_pins(self.old), 'absent')
        self.assertEqual(self.inspect_pins(self.pin(self.metadata({}))), 'empty')

    def test_matching_pin(self):
        head = self.pin(self.metadata({os.uname().machine:{'sha256':self.sha, 'sourceCommit':self.old}}))
        self.assertEqual(self.inspect_pins(head), 'matched')

    def test_malformed_mismatched_or_other_architecture_pins_refuse(self):
        other = 'aarch64' if os.uname().machine == 'x86_64' else 'x86_64'
        for packages in [[], {'unknown':{}}, {other:{'sha256':self.sha, 'sourceCommit':self.old}},
                         {os.uname().machine:{'sha256':'0' * 64, 'sourceCommit':self.old}},
                         {os.uname().machine:{'sha256':self.sha, 'sourceCommit':'0' * 40}},
                         {os.uname().machine:{'sha256':self.sha, 'sourceCommit':self.old, 'url':'untrusted'}}]:
            with self.assertRaises(ValueError):
                self.inspect_pins(self.pin(self.metadata(packages)))

    def test_duplicate_oversized_and_wrong_version_pins_refuse(self):
        for value in ['{"schemaVersion":1,"schemaVersion":1,"version":"0.8.0","packages":{}}',
                      ' ' * 8193, json.dumps(dict(self.metadata({}), version='0.9.0')),
                      json.dumps(dict(self.metadata({}), schemaVersion=True))]:
            self.write('plugin/runtime-release.json', value)
            with self.assertRaises(ValueError):
                self.inspect_pins(self.commit())

    def assembly(self, expected, sha=None, version='0.8.0-1'):
        mark = (self.sha, 0o600, os.getuid())
        info = {'version':version, 'source':self.old, 'binary':'1' * 64}
        with patch.object(PAIR.inspection, 'fingerprint', return_value=mark), \
             patch.object(PAIR.inspection, 'safe_parents'), \
             patch.object(PAIR.inspection, 'inspect_archive', return_value=info):
            return PAIR.assemble(self.repo, self.output, self.package, expected, sha or self.sha)

    def test_assembly_reuses_exact_package_and_records_both_sources(self):
        self.write('README.md', 'updated')
        new = self.commit()
        result = self.assembly(new)
        self.assertEqual(result['runtimeSourceCommit'], self.old)
        self.assertEqual(result['frontendSourceCommit'], new)
        self.assertEqual(result['bootstrapPins'], 'absent')
        self.assertFalse(result['publishedDownloadVerified'])
        self.assertEqual(result['publication'], 'unpublished-candidate')
        self.assertEqual(next(self.output.glob('*.zst')).read_bytes(), self.package.read_bytes())
        self.assertTrue((self.output / 'SHA256SUMS').is_file())

    def test_hash_mismatch_refuses_before_archive_parsing_or_output(self):
        with patch.object(PAIR.inspection, 'fingerprint', return_value=(self.sha, 0o600, os.getuid())), \
             patch.object(PAIR.inspection, 'inspect_archive') as inspect:
            with self.assertRaises(ValueError):
                PAIR.assemble(self.repo, self.output, self.package, self.old, '0' * 64)
            inspect.assert_not_called()
        self.assertEqual(list(self.output.iterdir()), [])

    def test_wrong_version_dirty_source_and_occupied_output_refuse(self):
        with self.assertRaises(ValueError):
            self.assembly(self.old, version='0.9.0-1')
        self.write('README.md', 'dirty')
        with self.assertRaises(ValueError):
            self.assembly(self.old)
        new = self.commit()
        (self.output / 'keep').write_text('preserve')
        with self.assertRaises(ValueError):
            self.assembly(new)
        self.assertEqual((self.output / 'keep').read_text(), 'preserve')

    def test_output_symlink_and_checkout_output_refuse(self):
        original = self.output
        link = self.base / 'output-link'
        link.symlink_to(original, target_is_directory=True)
        self.output = link
        with self.assertRaises(ValueError):
            self.assembly(self.old)
        self.output = self.repo
        with self.assertRaises(ValueError):
            self.assembly(self.old)
        self.assertEqual(list(original.iterdir()), [])

    def test_runtime_change_and_wrong_pin_refuse_before_output(self):
        self.write('templates/default.yaml', 'changed')
        with self.assertRaises(ValueError):
            self.assembly(self.commit())
        self.assertEqual(list(self.output.iterdir()), [])
        self.git('revert', '--no-edit', 'HEAD')
        head = self.pin(self.metadata({os.uname().machine:{'sha256':'0' * 64, 'sourceCommit':self.old}}))
        with self.assertRaises(ValueError):
            self.assembly(head)
        self.assertEqual(list(self.output.iterdir()), [])

    def test_copy_bound_is_enforced(self):
        with patch.object(PAIR.inspection, 'ARCHIVE_LIMIT', 8):
            with self.assertRaises(ValueError):
                self.assembly(self.old)
        self.assertFalse((self.output / 'SHA256SUMS').exists())


if __name__ == '__main__':
    unittest.main()
