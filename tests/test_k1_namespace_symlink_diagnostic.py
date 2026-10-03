"""Bounded metadata-only provenance; no private paths in public output."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('symlink_diag', ROOT /
    'crates/omavless-netguard/tests/support/namespace_filter_symlink_diagnostic.py')
diag = importlib.util.module_from_spec(spec)
spec.loader.exec_module(diag)


class SymlinkDiagnosticTests(unittest.TestCase):
    def test_exact_private_provenance_without_content_read_or_public_paths(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            root = base / 'activation'
            root.mkdir()
            outside = base / 'PRIVATE_PATH'
            outside.write_text('PRIVATE_CONTENT')
            (root / 'link').symlink_to(outside)
            inventory = diag.Inventory([root])
            with patch.object(Path, 'read_bytes', side_effect=AssertionError('content read')), \
                 patch.object(Path, 'read_text', side_effect=AssertionError('content read')):
                public, private = diag.diagnose(inventory)
            self.assertEqual(public['category'], 'escape_observed')
            self.assertEqual(public['root_ordinal'], 0)
            self.assertEqual(private['escape']['source'], str(root / 'link'))
            self.assertEqual(private['escape']['direct_target'], str(outside))
            self.assertEqual(private['escape']['resolved_target'], str(outside))
            self.assertIn(str(base), private['observations'])
            self.assertNotIn(temp, json.dumps(public))
            self.assertNotIn('PRIVATE_', json.dumps(public))
            self.assertNotIn('PRIVATE_CONTENT', json.dumps(private))
            self.assertFalse(public['normal_authority'])

    def test_relative_chain_and_parent_components_are_resolved(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            root = base / 'activation'
            root.mkdir()
            (base / 'target').touch()
            (base / 'middle').symlink_to('target')
            (root / 'link').symlink_to('../middle')
            public, private = diag.diagnose(diag.Inventory([root]))
            self.assertEqual(public['category'], 'escape_observed')
            self.assertEqual(private['escape']['direct_target'], '../middle')
            self.assertEqual(private['escape']['resolved_target'], str(base / 'target'))
            self.assertIn(str(base / 'middle'), private['observations'])

    def test_dangling_and_cycle_refuse(self):
        for target, reason in [('absent', 'dangling'), ('link', 'link_bound')]:
            with self.subTest(target=target), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                (root / 'link').symlink_to(target)
                public, _ = diag.diagnose(diag.Inventory([root]))
                self.assertEqual(public['category'], 'refused')
                self.assertEqual(public['reason'], reason)

    def test_replacement_detected_before_acceptance(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'target').touch()
            (root / 'other').touch()
            (root / 'link').symlink_to(root / 'target')
            inventory = diag.Inventory([root])
            stable = inventory.stable
            def replace():
                (root / 'link').unlink()
                (root / 'link').symlink_to(root / 'other')
                stable()
            inventory.stable = replace
            public, _ = diag.diagnose(inventory)
            self.assertEqual(public['reason'], 'changed')
            self.assertNotIn('target_class', public)

    def test_entry_depth_and_path_bounds(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for n in range(8):
                (root / str(n)).touch()
            public, _ = diag.diagnose(diag.Inventory([root], limit=5))
            self.assertEqual(public['reason'], 'entry_bound')
            public, _ = diag.diagnose(diag.Inventory([root], depth_limit=1))
            self.assertEqual(public['reason'], 'depth_bound')
            with self.assertRaises(diag.Refused):
                diag.Inventory([root]).observe(Path('/' + 'x' * 4097))

    def test_no_escape_and_missing_root_are_not_filter_acceptance(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'regular').write_text('PRIVATE_CONTENT')
            public, _ = diag.diagnose(diag.Inventory([root, root / 'absent']))
            self.assertEqual(public['category'], 'no_escape_observed')
            self.assertFalse(public['runner_invoked'])
            self.assertFalse(public['activation_contents_read'])

    def test_target_classes_are_fixed_and_do_not_claim_authority(self):
        for path, category in [('/usr/bin/example', 'system_usr'),
                               ('/etc/example', 'system_etc'), ('/run/example', 'system_run'),
                               ('/home/kdk_vm/PRIVATE', 'guest_home'),
                               ('/home/other/PRIVATE', 'other')]:
            self.assertEqual(diag.target_class(Path(path)), category)

    def test_os_error_details_are_private(self):
        inventory = diag.Inventory([])
        inventory.run = lambda: (_ for _ in ()).throw(PermissionError('PRIVATE_SENTINEL'))
        public, private = diag.diagnose(inventory)
        self.assertEqual(public['reason'], 'os_error')
        self.assertNotIn('PRIVATE_SENTINEL', json.dumps([public, private]))

    def test_refusal_cannot_be_retried_on_same_inventory(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'link').symlink_to('missing')
            inventory = diag.Inventory([root])
            first, _ = diag.diagnose(inventory)
            self.assertEqual(first['reason'], 'dangling')
            (root / 'missing').touch()
            second, _ = diag.diagnose(inventory)
            self.assertEqual(second['reason'], 'already_used')

    def test_unsupported_activation_type_refuses(self):
        import os
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            os.mkfifo(root / 'fifo')
            public, _ = diag.diagnose(diag.Inventory([root]))
            self.assertEqual(public['reason'], 'unsupported_type')


if __name__ == '__main__':
    unittest.main()
