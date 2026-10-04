"""Inert admission tests: no ELF execution, namespaces, mounts or guest calls."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import time
import unittest
from contextlib import ExitStack
from types import SimpleNamespace
from unittest.mock import patch

ROOT = Path(__file__).parent / "reviewed_tmpfs_elf"
SPEC = importlib.util.spec_from_file_location("reviewed_copy_admission", ROOT / "admission.py")
admission = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(admission)
BLOCK = b"\x7fELFsynthetic-not-executable"


class ReviewedCopyTests(unittest.TestCase):
    def fake_sources(self, change=None, fail_stat=False):
        data = (ROOT / "copy-manifest.json").read_bytes()
        table = admission.manifest(data)["source_provenance"]
        handles, metadata, opened, closed = {}, {}, [], []

        def open_path(name, flags, *, dir_fd=None):
            path = name if dir_fd is None else handles[dir_fd].rstrip("/") + "/" + name
            fd = len(handles) + 10
            handles[fd] = path
            opened.append((path, flags))
            if path in table:
                row = table[path]
                fields = dict(st_dev=row["device"], st_ino=row["inode"], st_size=row["size"],
                              st_mode=row["mode"], st_uid=65534, st_gid=65534,
                              st_nlink=row["nlink"], st_mtime_ns=1, st_ctime_ns=1)
            else:
                owner = 0 if path == "/" else 65534
                fields = dict(st_dev=31, st_ino=fd, st_size=4096, st_mode=0o40755,
                              st_uid=owner, st_gid=owner, st_nlink=2, st_mtime_ns=1, st_ctime_ns=1)
            if change:
                fields.update(change(path))
            metadata[path] = SimpleNamespace(**fields)
            return fd

        def fstat(fd):
            if fail_stat:
                raise OSError("injected fstat failure")
            return metadata[handles[fd]]

        def path_stat(name, *, dir_fd=None, follow_symlinks=False):
            self.assertIs(follow_symlinks, False)
            path = name if dir_fd is None else handles[dir_fd].rstrip("/") + "/" + name
            return metadata[path]

        with ExitStack() as stack:
            stack.enter_context(patch.object(admission.os, "open", side_effect=open_path))
            stack.enter_context(patch.object(admission.os, "fstat", side_effect=fstat))
            stack.enter_context(patch.object(admission.os, "stat", side_effect=path_stat))
            stack.enter_context(patch.object(admission.os, "close", side_effect=closed.append))
            digested = stack.enter_context(patch.object(admission, "digest",
                side_effect=lambda fd, size, deadline: table[handles[fd]]["sha256"]))
            try:
                with admission.Sources(data, time.monotonic() + 1) as held:
                    self.assertEqual(len(held.files), 16)
                    self.assertFalse(closed)
                    self.assertEqual(digested.call_count, 32)
            finally:
                self.assertEqual(sorted(handles), sorted(closed))
                self.assertTrue(all(flags & os.O_NOFOLLOW for _, flags in opened))
                self.assertFalse(any(flags & (os.O_CREAT | os.O_WRONLY | os.O_RDWR)
                                     for _, flags in opened))

    def test_all_original_sources_and_parents_remain_held_through_final_rehash(self):
        self.fake_sources()

    def test_wrong_original_shape_and_parent_owner_refuse_and_close(self):
        target = "/usr/lib/libbrotlicommon.so.1.2.0"
        for field, value in (("st_dev", 29), ("st_ino", 8047), ("st_size", 141263),
                             ("st_mode", 0o100777), ("st_uid", 0), ("st_gid", 0),
                             ("st_nlink", 2)):
            with self.subTest(field=field), self.assertRaisesRegex(admission.Refused, "source_identity"):
                self.fake_sources(lambda path: {field: value} if path == target else {})
        for field, value in (("st_uid", 0), ("st_mode", 0o40777)):
            with self.assertRaisesRegex(admission.Refused, "source_parent_shape"):
                self.fake_sources(lambda path: {field: value} if path == "/usr/lib" else {})

    def test_unknown_fstat_closes_just_opened_fd(self):
        with self.assertRaises(OSError):
            self.fake_sources(fail_stat=True)

    def test_exact_manifest_adds_only_manually_reviewed_object(self):
        raw = (ROOT / "copy-manifest.json").read_bytes()
        value = admission.manifest(raw)
        original = json.loads((ROOT.parent / "real_resolved_binary/guest-inventory.json").read_bytes())
        extra = "/usr/lib/libbrotlicommon.so.1.2.0"
        self.assertEqual({k: v for k, v in value["elfs"].items() if k != extra}, original["elfs"])
        self.assertEqual(len(value["elfs"]), 17)
        self.assertEqual(len(value["source_provenance"]), 16)
        row = value["source_provenance"][extra]
        self.assertEqual((row["device"], row["inode"], row["size"]), (31, 8046, 141264))
        self.assertEqual((row["package"]["name"], row["package"]["version"]), ("brotli", "1.2.0-1"))
        self.assertNotIn("/usr/lib/libbrotlicommon.so.1", value["elfs"])
        self.assertNotIn("/usr/lib/ld-linux-x86-64.so.2", value["elfs"])
        for path, row in value["elfs"].items():
            self.assertEqual(row["sha256"], value["source_provenance"][row["resolved_path"]]["sha256"])
        for wrong in (raw + b"\n", raw.replace(b"8046", b"8047"), b"{}", b"x" * 65537):
            with self.assertRaisesRegex(admission.Refused, "manifest_pin"):
                admission.manifest(wrong)

    def held(self, directory):
        path = Path(directory) / "object"
        path.write_bytes(BLOCK)
        parent = os.open(directory, admission.FLAGS | os.O_DIRECTORY)
        fd = os.open("object", admission.FLAGS, dir_fd=parent)
        obj = admission.Sources.__new__(admission.Sources)
        obj._state = "open"
        obj.table = {"/usr/lib/object": {"sha256": hashlib.sha256(BLOCK).hexdigest()}}
        obj._files = {"/usr/lib/object": (fd, os.fstat(fd))}
        obj._parents = {"/usr/lib": (parent, os.fstat(parent))}
        return obj, path

    def test_same_original_fd_hash_control(self):
        with tempfile.TemporaryDirectory() as directory:
            obj, _ = self.held(directory)
            try:
                fd, before = obj.files["/usr/lib/object"]
                self.assertEqual(admission.digest(fd, before.st_size, time.monotonic() + 1),
                                 hashlib.sha256(BLOCK).hexdigest())
            finally:
                obj.close()

    def assert_sealed_without_io(self, obj):
        with patch.object(admission.os, "fstat") as fstat, \
             patch.object(admission.os, "stat") as pathstat, \
             patch.object(admission.os, "open") as opened, \
             patch.object(admission.os, "read") as read, \
             patch.object(admission, "digest") as hashed:
            for action in (lambda: obj.recheck(time.monotonic() + 100),
                           lambda: obj._recheck(time.monotonic() + 100),
                           obj.__enter__, lambda: obj.files, lambda: obj.parents,
                           lambda: obj._parent("/usr/lib")):
                with self.assertRaisesRegex(admission.Refused, "sources_sealed"):
                    action()
            for operation in (fstat, pathstat, opened, read, hashed):
                operation.assert_not_called()

    def test_deadline_then_valid_clock_stays_permanently_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            obj, _ = self.held(directory)
            try:
                with self.assertRaisesRegex(admission.Refused, "source_deadline"):
                    obj.recheck(0)
                self.assertEqual(obj._state, "refused")
                self.assert_sealed_without_io(obj)
            finally:
                obj.close()

    def test_unknown_fstat_then_restored_metadata_stays_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            obj, _ = self.held(directory)
            try:
                with patch.object(admission.os, "fstat", side_effect=OSError("unknown")), \
                     self.assertRaises(OSError):
                    obj.recheck(time.monotonic() + 1)
                self.assertEqual(obj._state, "refused")
                self.assert_sealed_without_io(obj)
            finally:
                obj.close()

    def test_closed_sources_never_admit_and_close_is_idempotent(self):
        with tempfile.TemporaryDirectory() as directory:
            obj, _ = self.held(directory)
            obj.close()
            self.assertEqual(obj._state, "closed")
            self.assert_sealed_without_io(obj)
            with patch.object(admission.os, "close") as closed:
                obj.close()
                closed.assert_not_called()

    def test_unknown_close_never_retries_fd_number(self):
        with tempfile.TemporaryDirectory() as directory:
            obj, _ = self.held(directory)
            original_close = os.close
            def close_then_unknown(fd):
                original_close(fd)
                raise OSError("close status unknown")
            with patch.object(admission.os, "close", side_effect=close_then_unknown) as closed:
                with self.assertRaises(OSError):
                    obj.close()
                self.assertEqual(closed.call_count, 2)
                obj.close()
                self.assertEqual(closed.call_count, 2)
            self.assert_sealed_without_io(obj)

    def test_real_same_bytes_replacement_refuses_before_parent_walk(self):
        with tempfile.TemporaryDirectory() as directory:
            obj, path = self.held(directory)
            try:
                path.rename(path.with_name("old"))
                path.write_bytes(BLOCK)
                with self.assertRaisesRegex(admission.Refused, "source_replaced"):
                    obj.recheck(time.monotonic() + 1)
            finally:
                obj.close()

    def test_real_content_change_and_reversion_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            obj, path = self.held(directory)
            try:
                path.write_bytes(BLOCK[:-1] + b"!")
                path.write_bytes(BLOCK)
                with self.assertRaisesRegex(admission.Refused, "source_replaced"):
                    obj.recheck(time.monotonic() + 1)
            finally:
                obj.close()

    def test_deadline_growth_short_content_and_nonelf_refuse(self):
        with tempfile.TemporaryFile() as source:
            for data, size, deadline in ((BLOCK, len(BLOCK), 0),
                                         (BLOCK, len(BLOCK) - 1, time.monotonic() + 1),
                                         (BLOCK, len(BLOCK) + 1, time.monotonic() + 1),
                                         (b"not elf", 7, time.monotonic() + 1)):
                source.seek(0)
                source.truncate()
                source.write(data)
                source.flush()
                with self.assertRaises(admission.Refused):
                    admission.digest(source.fileno(), size, deadline)

    def test_parent_replacement_and_change_reversion_refuse(self):
        from types import SimpleNamespace
        fields = dict(st_dev=31, st_ino=1, st_mode=0o40755, st_uid=65534, st_gid=65534,
                      st_nlink=2, st_size=4, st_mtime_ns=1, st_ctime_ns=1)
        before = SimpleNamespace(**fields)
        obj = admission.Sources.__new__(admission.Sources)
        obj._files = {}
        obj._parents = {"/": (1, before)}
        for change in ({"st_ino": 2}, {"st_ctime_ns": 2}, {"st_mode": 0o40777}):
            obj._state = "open"
            after = SimpleNamespace(**dict(fields, **change))
            with patch.object(admission.os, "fstat", return_value=before), \
                 patch.object(admission.os, "stat", return_value=after), \
                 self.assertRaisesRegex(admission.Refused, "source_parent_replaced"):
                obj.recheck(time.monotonic() + 1)

    def test_no_launcher_or_manifest_discovery(self):
        import ast
        tree = ast.parse((ROOT / "admission.py").read_text())
        names = {node.id for node in ast.walk(tree) if isinstance(node, ast.Name)}
        self.assertFalse(names & {"subprocess", "ctypes", "exec", "eval", "socket"})
        self.assertNotIn("__main__", (ROOT / "admission.py").read_text())


if __name__ == "__main__":
    unittest.main()
