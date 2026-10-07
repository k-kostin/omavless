"""Synthetic filesystem only; never invoke the privileged capture entry point."""
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from tests.first_abort_cli import startup_inventory as inventory


class StartupInventoryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='.t4-startup-source-', dir=Path.home())
        self.root = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)
        self.objects = []
        self.addCleanup(self.close_owned)

    def close_owned(self):
        for obj in self.objects:
            descriptors = {fd for fd, _ in obj.directories.values()}
            descriptors.update(row[2] for row in obj.nodes.values() if row[2] is not None)
            for fd in descriptors:
                os.close(fd)

    def object(self):
        obj = inventory.Inventory((self.root,), (self.root,), os.getuid())
        self.objects.append(obj)
        return obj

    def test_original_descriptors_and_no_semantic_admission(self):
        (self.root / 'unit').write_bytes(b'[Unit]\n')
        obj = self.object()
        result = obj.capture()
        original = obj.directories[self.root][0]
        obj.add_directory(self.root)
        self.assertEqual(original, obj.directories[self.root][0])
        self.assertIs(result['semantic_admission'], False)
        obj.recheck()

    def test_same_bytes_replacement_and_in_place_changes_seal(self):
        for replace in (False, True):
            with self.subTest(replace=replace):
                path = self.root / ('replace' if replace else 'edit')
                path.write_bytes(b'original')
                obj = self.object()
                obj.capture()
                if replace:
                    replacement = self.root / 'new'
                    replacement.write_bytes(b'original')
                    replacement.replace(path)
                else:
                    path.write_bytes(b'changed!')
                with self.assertRaises(inventory.Refused):
                    obj.recheck()
                with patch.object(inventory.os, 'stat') as read:
                    with self.assertRaises(inventory.Refused):
                        obj.recheck()
                read.assert_not_called()

    def test_fifo_refused_without_opening_it(self):
        path = self.root / 'fifo'
        os.mkfifo(path, 0o600)
        obj = self.object()
        original = inventory.os.open
        opened = []
        def tracked(name, *args, **kwargs):
            opened.append(name)
            return original(name, *args, **kwargs)
        with patch.object(inventory.os, 'open', side_effect=tracked):
            with self.assertRaises(inventory.Refused):
                obj.capture()
        self.assertNotIn('fifo', opened)

    def test_link_escape_and_cycle_refuse(self):
        for target in ('/etc/passwd', 'link'):
            path = self.root / 'link'
            path.symlink_to(target)
            obj = self.object()
            with self.assertRaises(inventory.Refused):
                obj.capture()
            path.unlink()

    def test_missing_root_appearing_refuses(self):
        path = self.root / 'missing'
        obj = inventory.Inventory((path,), (self.root,), os.getuid())
        self.objects.append(obj)
        obj.capture()
        path.write_bytes(b'appeared')
        with self.assertRaises(inventory.Refused):
            obj.recheck()

    def test_bounds_and_short_read_are_terminal(self):
        path = self.root / 'unit'
        path.write_bytes(b'content')
        for limit in ('MAX_FILE', 'MAX_TOTAL', 'MAX_FILES'):
            obj = self.object()
            with patch.object(inventory, limit, 0), self.assertRaises(inventory.Refused):
                obj.capture()
            with self.assertRaises(inventory.Refused):
                obj.capture()
        obj = self.object()
        with patch.object(inventory.os, 'pread', return_value=b''), self.assertRaises(inventory.Refused):
            obj.capture()
        self.assertTrue(obj.sealed)

    def test_deadline_and_directory_entry_bound(self):
        obj = self.object()
        obj.deadline = 0
        with self.assertRaises(inventory.Refused):
            obj.capture()
        for index in range(3):
            (self.root / str(index)).write_bytes(b'')
        obj = self.object()
        with patch.object(inventory, 'MAX_FILES', 2), self.assertRaises(inventory.Refused):
            obj.capture()

    def test_exclusive_output_never_overwrites(self):
        fd = os.open(self.root, os.O_RDONLY | os.O_DIRECTORY)
        try:
            inventory.exclusive(fd, 'receipt', b'first')
            with self.assertRaises(FileExistsError):
                inventory.exclusive(fd, 'receipt', b'second')
            self.assertEqual((self.root / 'receipt').read_bytes(), b'first')
        finally:
            os.close(fd)

    def test_replaced_directory_and_unsafe_mode_refuse(self):
        directory = self.root / 'units'
        directory.mkdir()
        (directory / 'unit').write_bytes(b'source')
        obj = self.object()
        obj.capture()
        directory.rename(self.root / 'original')
        directory.mkdir()
        (directory / 'unit').write_bytes(b'source')
        with self.assertRaises(inventory.Refused):
            obj.recheck()
        (directory / 'unit').chmod(0o666)
        with self.assertRaises(inventory.Refused):
            self.object().capture()

    def test_publication_fsync_uncertainty_propagates_and_retains(self):
        fd = os.open(self.root, os.O_RDONLY | os.O_DIRECTORY)
        try:
            with patch.object(inventory.os, 'fsync', side_effect=OSError('synthetic')) as sync:
                with self.assertRaises(OSError):
                    inventory.exclusive(fd, 'partial', b'preserved')
            self.assertEqual(sync.call_count, 1)
            self.assertEqual((self.root / 'partial').read_bytes(), b'preserved')
        finally:
            os.close(fd)


if __name__ == '__main__':
    unittest.main()
