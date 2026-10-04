"""Pure finite follow-up controls; no actual system catalog or executable run."""
from pathlib import Path
from unittest.mock import patch
import unittest

from tests import test_t4_startup_inventory as base
from tests.first_abort_cli import startup_followup as followup


class StartupFollowupTests(base.StartupInventoryTests):
    def setUp(self):
        super().setUp()
        selected = patch.object(base, 'inventory', followup)
        selected.start()
        self.addCleanup(selected.stop)

    def package_fixture(self):
        root = self.root / 'packages'
        root.mkdir()
        directory = root / 'example-1.2-3'
        directory.mkdir()
        (directory / 'desc').write_text('%NAME%\nexample\n\n%VERSION%\n1.2-3\n')
        (directory / 'files').write_text('%FILES%\nusr/bin/fixture\n')
        return root, directory

    def test_exact_package_name_version_original_sources_and_drift(self):
        root, directory = self.package_fixture()
        obj = self.object()
        with patch.object(followup, 'PACKAGE_ROOT', root), \
             patch.object(followup, 'PACKAGES', ('example',)):
            obj.package_inputs()
            self.assertEqual(obj.package_selection, {'example': str(directory)})
            obj.capture()
            (root / 'extra').mkdir()
            with self.assertRaises(followup.Refused):
                obj.recheck()
        self.assertTrue(obj.sealed)

    def test_package_ambiguity_missing_wrong_name_and_duplicate_field_refuse(self):
        root, directory = self.package_fixture()
        for name, content in (
            ('missing', None), ('wrong', '%NAME%\nother\n\n%VERSION%\n1\n'),
            ('duplicate', '%NAME%\nexample\n\n%NAME%\nexample\n%VERSION%\n1\n'),
            ('version', '%NAME%\nexample\n\n%VERSION%\n9-9\n'),
        ):
            if content is not None:
                (directory / 'desc').write_text(content)
            obj = self.object()
            with patch.object(followup, 'PACKAGE_ROOT', root), \
                 patch.object(followup, 'PACKAGES', ('absent' if name == 'missing' else 'example',)), \
                 self.assertRaises(followup.Refused):
                obj.package_inputs()
            self.assertTrue(obj.sealed)
        (root / 'example-2-1').mkdir()
        with patch.object(followup, 'PACKAGE_ROOT', root), \
             patch.object(followup, 'PACKAGES', ('example',)), self.assertRaises(followup.Refused):
            self.object().package_inputs()

    def test_fixed_capture_has_no_general_system_tree_or_execution(self):
        self.assertNotEqual(followup.OUTPUT.name, 'ov-t4-user-startup-inventory-v1')
        self.assertLess(len(followup.ROOTS), 128)
        for root in followup.SYSTEM_ROOTS:
            self.assertNotIn(Path(root), followup.ROOTS)
        source = Path(followup.__file__).read_text()
        for forbidden in ('subprocess', 'os.system(', 'exec(', 'eval(', 'ldd'):
            self.assertNotIn(forbidden, source)

    def test_exact_elf_membership_refuses_unlisted_and_non_elf(self):
        root, directory = self.package_fixture()
        elf = self.root / 'elf'
        elf.write_bytes(b'\x7fELFsynthetic-not-executed')
        elf.chmod(0o755)
        (directory / 'files').write_text('%FILES%\n' + str(elf)[1:] + '\n')
        obj = self.object()
        with patch.object(followup, 'PACKAGE_ROOT', root), \
             patch.object(followup, 'PACKAGES', ('example',)), \
             patch.object(followup, 'EXECUTABLES', (str(elf),)):
            obj.package_inputs()
            obj.capture()
            self.assertEqual(obj.package_bindings(), {str(elf): 'example'})
        for payload, listed in ((b'not-elf', True), (b'\x7fELFsynthetic', False)):
            elf.write_bytes(payload)
            (directory / 'files').write_text('%FILES%\n' + (str(elf)[1:] if listed else 'usr/bin/other') + '\n')
            obj = self.object()
            with patch.object(followup, 'PACKAGE_ROOT', root), \
                 patch.object(followup, 'PACKAGES', ('example',)), \
                 patch.object(followup, 'EXECUTABLES', (str(elf),)):
                obj.package_inputs()
                obj.capture()
                with self.assertRaises(followup.Refused):
                    obj.package_bindings()
            self.assertTrue(obj.sealed)


if __name__ == '__main__':
    unittest.main()
