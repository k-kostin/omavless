"""Pure local old-policy counterexample and fixed-pair original-FD controls."""
import hashlib
import importlib.util
import os
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SUPPORT = ROOT / 'crates/omavless-netguard/tests/support'


def load(name):
    spec = importlib.util.spec_from_file_location(name, SUPPORT / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class Inventory(unittest.TestCase):
    def test_old_policy_refuses_new_exact_pair_is_cataloged_not_hidden(self):
        old = load('generator_filter_guest_guard')
        new = load('retained_activation_guest_guard')
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            catalog, stage = base / 'catalog', base / 'stage'
            catalog.mkdir()
            stage.mkdir()
            fragment, link = stage / 'fixture.service', catalog / 'own.service'
            raw = b'[Service]\nExecStart=/usr/bin/false\n'
            fragment.write_bytes(raw)
            link.symlink_to(fragment)
            retained = object.__new__(new.RetainedActivation)
            retained.fragment = fragment.lstat()
            retained.recheck = lambda: raw
            with patch.object(old, 'ACTIVATION_ROOTS', (str(catalog),)), \
                 patch.object(old, 'TARGET_ROOTS', ()):
                with self.assertRaises(old.Refused):
                    old.inventory()
            with patch.object(new, 'ACTIVATION_ROOTS', (str(catalog),)), \
                 patch.object(new, 'TARGET_ROOTS', ()), \
                 patch.object(new, 'RETAINED_LINK', link), \
                 patch.object(new, 'RETAINED_FRAGMENT', fragment), \
                 patch.object(new, 'RETAINED_ACTIVATION', retained):
                result = new.inventory()
                self.assertEqual(set(result), {str(catalog), str(link), str(fragment)})
                self.assertEqual(result[str(link)][3:], ['link', str(fragment)])
                self.assertEqual(result[str(fragment)][3:], ['file', hashlib.sha256(raw).hexdigest()])
                # Same target through a second link is not admitted.
                (catalog / 'alias.service').symlink_to(fragment)
                with self.assertRaises(new.Refused):
                    new.inventory()
                self.assertTrue(new.ACTIVATION_REFUSED)
                (catalog / 'alias.service').unlink()
                with self.assertRaises(new.Refused):
                    new.inventory()

    def fixture(self):
        module = load('retained_activation_guest_guard')
        raw = b'fixed public synthetic unit'
        def meta(ino, mode, size=0):
            return SimpleNamespace(st_dev=1, st_ino=ino, st_mode=mode, st_uid=0,
                st_gid=0, st_nlink=1, st_size=size, st_mtime_ns=2, st_ctime_ns=3)
        paths = [Path('/'), Path('/run'), module.RETAINED_FRAGMENT.parent,
                 Path('/run/systemd'), module.RETAINED_LINK.parent]
        metadata = {10+i: meta(10+i, 0o40700) for i in range(5)}
        metadata[20] = meta(20, 0o120777)
        metadata[21] = meta(21, 0o100600, len(raw))
        original = {fd: SimpleNamespace(**vars(value)) for fd, value in metadata.items()}
        obj = object.__new__(module.RetainedActivation)
        obj.parents = [(path, 10+i, original[10+i]) for i, path in enumerate(paths)]
        obj.link_fd, obj.link = 20, original[20]
        obj.fragment_fd, obj.fragment = 21, original[21]
        def named(name, **kw):
            return metadata[20 if kw['dir_fd'] == 14 else 21]
        return module, obj, raw, paths, metadata, named

    def test_original_fd_hash_identity_and_literal_link_are_mandatory(self):
        for mutation in ('none', 'hash', 'link-target', 'fragment-inode', 'fd-owner',
                         'fragment-mode', 'fragment-size', 'fragment-mtime', 'link-inode',
                         'parent-inode'):
            module, obj, raw, paths, metadata, named = self.fixture()
            if mutation == 'fragment-inode': metadata[21].st_ino += 1
            if mutation == 'fd-owner': metadata[21].st_uid = 1000
            if mutation == 'fragment-mode': metadata[21].st_mode = 0o100644
            if mutation == 'fragment-size': metadata[21].st_size += 1
            if mutation == 'fragment-mtime': metadata[21].st_mtime_ns += 1
            if mutation == 'link-inode': metadata[20].st_ino += 1
            if mutation == 'parent-inode': metadata[12].st_ino += 1
            with patch.object(module, 'RETAINED_SHA', hashlib.sha256(raw).hexdigest()), \
                 patch.object(module.os, 'fstat', side_effect=lambda fd: metadata[fd]), \
                 patch.object(Path, 'lstat', autospec=True, side_effect=lambda path: metadata[10+paths.index(path)]), \
                 patch.object(module.os, 'stat', side_effect=named), \
                 patch.object(module.os, 'readlink', return_value=str(module.RETAINED_FRAGMENT) + ('x' if mutation == 'link-target' else '')), \
                 patch.object(module.os, 'pread', return_value=raw + (b'x' if mutation == 'hash' else b'')):
                if mutation == 'none':
                    self.assertEqual(obj.recheck(), raw)
                    self.assertTrue(obj.admits(module.RETAINED_LINK, module.RETAINED_FRAGMENT))
                    self.assertFalse(obj.admits(module.RETAINED_LINK, module.RETAINED_FRAGMENT.parent / 'other'))
                    self.assertFalse(obj.admits(module.RETAINED_LINK.parent / 'other.service', module.RETAINED_FRAGMENT))
                else:
                    with self.assertRaises(module.Refused, msg=mutation):
                        obj.recheck()

    def test_frozen_old_query_and_literal_pin(self):
        module = load('retained_activation_guest_guard')
        self.assertEqual(hashlib.sha256((SUPPORT / 'generator_filter_guest_guard.py').read_bytes()).hexdigest(),
                         '67e541ea5c764a05b669267b248d3df77ca3402569df5116bfff9346ff9ea0bd')
        self.assertEqual(module.RETAINED_SHA, hashlib.sha256((SUPPORT.parent / 'fixtures' /
            'omavless-k1-versioned-config-reference.service').read_bytes()).hexdigest())
        self.assertNotIn('/run', module.TARGET_ROOTS)
        self.assertNotIn(str(module.RETAINED_FRAGMENT.parent), module.TARGET_ROOTS)

    def test_constructor_rejects_wrong_owner_mode_links_and_capability_xattr(self):
        for mutation in ('none', 'parent-owner', 'parent-writable', 'stage-mode',
                         'link-owner', 'link-type', 'file-owner', 'file-mode',
                         'file-hardlink', 'file-empty', 'file-oversize', 'xattr'):
            module, _, raw, paths, metadata, named = self.fixture()
            if mutation == 'parent-owner': metadata[11].st_uid = 1000
            if mutation == 'parent-writable': metadata[11].st_mode |= 0o020
            if mutation == 'stage-mode': metadata[12].st_mode = 0o40755
            if mutation == 'link-owner': metadata[20].st_uid = 1000
            if mutation == 'link-type': metadata[20].st_mode = 0o100600
            if mutation == 'file-owner': metadata[21].st_uid = 1000
            if mutation == 'file-mode': metadata[21].st_mode = 0o100644
            if mutation == 'file-hardlink': metadata[21].st_nlink = 2
            if mutation == 'file-empty': metadata[21].st_size = 0
            if mutation == 'file-oversize': metadata[21].st_size = 16385
            with patch.object(module, 'RETAINED_SHA', hashlib.sha256(raw).hexdigest()), \
                 patch.object(module.os, 'open', side_effect=[10, 11, 12, 13, 14, 20, 21]) as opened, \
                 patch.object(module.os, 'fstat', side_effect=lambda fd: metadata[fd]), \
                 patch.object(Path, 'lstat', autospec=True, side_effect=lambda path: metadata[10+paths.index(path)]), \
                 patch.object(module.os, 'stat', side_effect=named), \
                 patch.object(module.os, 'readlink', return_value=str(module.RETAINED_FRAGMENT)), \
                 patch.object(module.os, 'listxattr', return_value=['security.capability'] if mutation == 'xattr' else []), \
                 patch.object(module.os, 'pread', return_value=raw):
                if mutation == 'none':
                    obj = module.RetainedActivation()
                    self.assertEqual(obj.recheck(), raw)
                    self.assertTrue(all(call.args[1] & os.O_NOFOLLOW for call in opened.call_args_list))
                    self.assertTrue(all(call.args[1] & os.O_CLOEXEC for call in opened.call_args_list))
                    self.assertEqual(opened.call_args_list[-1].kwargs, {'dir_fd': 12})
                    self.assertEqual(opened.call_args_list[-2].kwargs, {'dir_fd': 14})
                else:
                    with self.assertRaises(module.Refused, msg=mutation):
                        module.RetainedActivation()


if __name__ == '__main__':
    unittest.main()
