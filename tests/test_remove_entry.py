# SPDX-License-Identifier: MIT
"""Real no-argument entry filesystem boundary; no removal effects."""
import json
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class RemovalEntryTests(unittest.TestCase):
    def setUp(self):
        parent = Path(os.environ.get('OMAVLESS_TEST_ROOT', str(Path.home() / '.cache')))
        parent.mkdir(parents=True, exist_ok=True)
        self.tmp = tempfile.TemporaryDirectory(prefix='ov-remove-entry-', dir=parent)
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        (self.root / 'plugin').mkdir()
        (self.root / 'tools').mkdir()
        self.entry = self.root / 'plugin/remove-entry.sh'
        self.helper = self.root / 'tools/uninstall.sh'
        shutil.copy2(ROOT / 'plugin/remove-entry.sh', self.entry)
        shutil.copy2(ROOT / 'tools/uninstall.sh', self.helper)
        (self.root / 'manifest.json').write_text(json.dumps({'schemaVersion': 1, 'id': 'kdk.omavless'}))
        self.env = {'PATH': '/usr/bin:/bin', 'HOME': str(self.root / 'not-the-account-home'), 'LC_ALL': 'C'}

    def checked(self):
        return subprocess.run(['/bin/bash', '-c', 'source "$1"; checked_removal_helper',
                               '_', str(self.entry)], env=self.env, text=True,
                              capture_output=True, timeout=5)

    def test_reviewed_local_helper_is_resolved_without_running_it(self):
        result = self.checked()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, str(self.helper) + '\n')
        self.assertFalse((self.root / 'not-the-account-home').exists())

    def test_pin_matches_the_actual_shipped_standalone_helper(self):
        digest = hashlib.sha256((ROOT / 'tools/uninstall.sh').read_bytes()).hexdigest()
        self.assertIn('readonly OV_REMOVAL_HELPER_SHA=' + digest, self.entry.read_text())

    def test_modified_helper_is_refused_without_running_it(self):
        self.helper.write_bytes(self.helper.read_bytes() + b'\n# modified\n')
        self.assertNotEqual(self.checked().returncode, 0)
        self.assertFalse((self.root / 'not-the-account-home').exists())

    def test_missing_helper_is_refused(self):
        self.helper.unlink()
        self.assertNotEqual(self.checked().returncode, 0)

    def test_redirected_helper_is_refused_even_with_matching_bytes(self):
        retained = self.root / 'retained-helper'
        self.helper.rename(retained)
        self.helper.symlink_to(retained)
        self.assertNotEqual(self.checked().returncode, 0)

    def test_redirected_tools_parent_is_refused(self):
        tools = self.root / 'tools'
        tools.rename(self.root / 'retained-tools')
        tools.symlink_to(self.root / 'retained-tools', target_is_directory=True)
        self.assertNotEqual(self.checked().returncode, 0)

    def test_helper_hardlink_is_refused(self):
        os.link(self.helper, self.root / 'other-link')
        self.assertNotEqual(self.checked().returncode, 0)

    def test_writable_member_or_parent_is_refused(self):
        for path in (self.entry, self.helper, self.root / 'manifest.json', self.root / 'tools', self.root):
            original = path.stat().st_mode & 0o777
            path.chmod(original | 0o022)
            self.assertNotEqual(self.checked().returncode, 0, str(path))
            path.chmod(original)

    def test_wrong_manifest_is_refused(self):
        (self.root / 'manifest.json').write_text('{"schemaVersion":1,"id":"another.plugin"}')
        self.assertNotEqual(self.checked().returncode, 0)

    def test_arguments_are_refused_before_helper_resolution(self):
        self.helper.unlink()
        result = subprocess.run(['/bin/bash', str(self.entry), 'REMOVE'], env=self.env,
                                text=True, capture_output=True, timeout=5)
        self.assertEqual(result.returncode, 2)
        self.assertIn('without arguments', result.stderr)
        self.assertFalse((self.root / 'not-the-account-home').exists())

    def test_actual_exec_reaches_only_helpers_pre_effect_account_refusal(self):
        # The real helper rejects root or a noncanonical HOME before any mkdir,
        # private-state read, authorization, native command or confirmation.
        result = subprocess.run(['/bin/bash', str(self.entry)], env=self.env,
                                text=True, capture_output=True, timeout=5)
        self.assertEqual(result.returncode, 1)
        self.assertTrue('Run from your normal desktop account' in result.stderr
                        or 'Run as your desktop user, not with sudo' in result.stderr)
        self.assertFalse((self.root / 'not-the-account-home').exists())


if __name__ == '__main__':
    unittest.main()
