"""Pure copy/map integration controls; never mounts or launches any ELF."""
import hashlib
import os
from pathlib import Path
from types import SimpleNamespace
import tempfile
import time
import unittest
from unittest.mock import Mock, patch

from tests.reviewed_tmpfs_elf import admission, bridge

ROOT = Path(__file__).parent / "reviewed_tmpfs_elf"
RAW = (ROOT / "copy-manifest.json").read_bytes()
BLOCK = b"\x7fELFsynthetic-not-executable"


def fixture():
    base = SimpleNamespace(UNSETTLED=[], command=Mock(), child_status=Mock(return_value=None))
    value = bridge.Bridge(base, admission, RAW)
    value._child_binding = Mock()  # Explicit pure map controls, separately tested below.
    return value


class ReviewedBridgeTests(unittest.TestCase):
    def test_strict_mapping_grammar_and_direct_child_binding(self):
        good = "1000-2000 r--p 00000000 00:2c 1 /usr/lib/known.so\n"
        for text in (good.replace("r--p", "r--q"), good.replace("1000-2000", "2000-1000"),
                     good.replace("00000000", "xyz"), good + good,
                     good.replace("known.so", "../known.so")):
            with self.assertRaises(bridge.Refused):
                bridge.map_objects(text)
        obj = fixture()
        valid_stat = "123 (synthetic) S 42 " + "0 " * 20
        def link(path):
            return "42" if path == "/proc/self" else "pid:[12]"
        with patch.object(bridge.os, "getpid", return_value=42), \
             patch.object(bridge.os, "readlink", side_effect=link), \
             patch.object(bridge, "bounded_text", return_value=valid_stat):
            bridge.Bridge._child_binding(obj, SimpleNamespace(pid=123))
        with patch.object(bridge.os, "getpid", return_value=42), \
             patch.object(bridge.os, "readlink", return_value="999"), \
             patch.object(bridge, "bounded_text") as read, self.assertRaises(bridge.Refused):
            bridge.Bridge._child_binding(obj, SimpleNamespace(pid=123))
        read.assert_not_called()
    def test_store_requires_ro_superblock_and_perfile_flags(self):
        good = "40 1 0:44 / /elf-copy-store ro,nosuid,nodev - tmpfs tmpfs ro\n"
        with patch.object(bridge, "bounded_text", return_value=good):
            bridge.mount_policy(bridge.STORE, True)
        for wrong in (good.replace("tmpfs tmpfs ro", "tmpfs tmpfs rw"),
                      good.replace("ro,nosuid,nodev", "rw,nosuid,nodev"),
                      good.replace("ro,nosuid,nodev", "ro,nodev"),
                      good.replace("tmpfs tmpfs", "btrfs source"), good * 2):
            with patch.object(bridge, "bounded_text", return_value=wrong), self.assertRaises(bridge.Refused):
                bridge.mount_policy(bridge.STORE, True)

    def assert_terminal(self, obj):
        with patch.object(obj, "_verify_all") as verify, \
             patch.object(bridge, "bounded_text") as read:
            for action in (lambda: obj.prepare({}), lambda: obj.verify(time.monotonic() + 5),
                           lambda: obj.inventory(SimpleNamespace(pid=123), time.monotonic() + 5)):
                with self.assertRaises(bridge.Refused):
                    action()
            verify.assert_not_called()
            read.assert_not_called()

    def test_first_unknown_boundary_is_permanent_without_retry(self):
        obj = fixture()
        with patch.object(obj, "_boundary", side_effect=OSError("unknown")) as boundary:
            with self.assertRaises(OSError):
                obj.prepare({})
            self.assert_terminal(obj)
            self.assertEqual(boundary.call_count, 1)
        obj.base.command.assert_not_called()

    def test_unsettled_cleared_later_never_reopens_bridge(self):
        obj = fixture()
        obj.state = "ready"
        obj.base.UNSETTLED.append("unknown")
        with self.assertRaises(bridge.Refused):
            obj.verify(time.monotonic() + 1)
        obj.base.UNSETTLED.clear()
        self.assert_terminal(obj)

    def test_all_admission_copy_recheck_freeze_precede_first_bind(self):
        obj = fixture()
        events = []
        paths = sorted(obj.manifest["source_provenance"])
        sources = Mock()
        sources.files = dict.fromkeys(paths)
        sources.__enter__ = Mock(return_value=sources)
        sources.__exit__ = Mock(side_effect=lambda *_: events.append("sources_closed"))
        sources.recheck.side_effect = lambda _: events.append("recheck")
        obj.base.command.side_effect = lambda argv: events.append(
            "bind" if "--bind" in argv else "freeze" if "remount,ro,nosuid,nodev" in argv else "mount")
        def copy(_sources, path, _name, _fd, _deadline):
            events.append("copy")
            obj.records[path], obj.fds[path] = {"size": 1, "sha256": "a" * 64}, 500 + len(obj.fds)
        metadata = SimpleNamespace(st_dev=44, st_ino=99, st_mode=0o40755, st_uid=0,
                                   st_gid=0, st_nlink=2, st_size=4096, st_mtime_ns=1, st_ctime_ns=1)
        with patch.object(obj, "_boundary"), patch.object(bridge.os, "mkdir"), \
             patch.object(bridge, "mount_policy"), patch.object(bridge.os, "open", return_value=99), \
             patch.object(bridge.os, "close"), \
             patch.object(admission, "Sources", side_effect=lambda *_: (events.append("admitted_all"), sources)[1]), \
             patch.object(obj, "_copy", side_effect=copy), \
             patch.object(bridge, "no_writable_fds", side_effect=lambda _: events.append("no_write_fd")), \
             patch.object(bridge.os, "fstat", return_value=metadata), \
             patch.object(bridge.os, "stat", return_value=metadata), \
             patch.object(admission, "digest", return_value="a" * 64), \
             patch.object(obj, "_verify_store"), \
             patch.object(bridge.os, "fstatvfs", return_value=SimpleNamespace(f_flag=os.ST_RDONLY)), \
             patch.object(bridge.Path, "resolve", autospec=True,
                          side_effect=lambda path, **_: Path(obj.manifest["elfs"][str(path)]["resolved_path"])), \
             patch.object(obj, "_verify_all", side_effect=lambda _: events.append("verified")):
            obj.prepare({})
        self.assertEqual(obj.state, "ready")
        first_bind = events.index("bind")
        prefix = events[:first_bind]
        self.assertLess(prefix.index("admitted_all"), prefix.index("copy"))
        self.assertEqual(prefix.count("copy"), 16)
        self.assertEqual(prefix[-4:], ["no_write_fd", "freeze", "recheck", "sources_closed"])
        self.assertEqual(prefix.count("recheck"), 2)
        self.assertEqual(events[-1], "verified")

    def test_maps_unknown_and_wrong_identity_refuse_before_open_or_hash(self):
        path = "/usr/lib/libbrotlicommon.so.1.2.0"
        for mapped, device, inode in ((path, 44, 2), (path, 45, 1), ("/usr/lib/unknown.so", 44, 1)):
            obj = fixture()
            obj.state = "ready"
            obj.records[path] = {"device": 44, "inode": 1}
            text = f"1000-2000 r--p 0 00:{device:02x} {inode} {mapped}\n"
            with patch.object(bridge, "bounded_text", return_value=text), \
                 patch.object(obj, "_verify_target") as hashed, self.assertRaises(bridge.Refused):
                obj.inventory(SimpleNamespace(pid=123), time.monotonic() + 1)
            hashed.assert_not_called()
            self.assert_terminal(obj)

    def test_two_complete_map_passes_and_late_drift_refusal(self):
        obj = fixture()
        obj.state = "ready"
        path = "/usr/lib/libbrotlicommon.so.1.2.0"
        obj.records[path] = {"device": 44, "inode": 1, "size": 24, "sha256": "a" * 64}
        good = f"1000-2000 r--p 0 00:2c 1 {path}\n"
        with patch.object(bridge, "bounded_text", return_value=good) as read, \
             patch.object(obj, "_verify_target") as hashed:
            first = obj.inventory(SimpleNamespace(pid=123), time.monotonic() + 1)
            second = obj.inventory(SimpleNamespace(pid=123), time.monotonic() + 1)
            self.assertEqual(first, second)
            self.assertEqual(read.call_count, 4)
            self.assertEqual(hashed.call_count, 2)
        with patch.object(bridge, "bounded_text", side_effect=[good, good.replace(" 1 /", " 2 /")]), \
             patch.object(obj, "_verify_target"), self.assertRaisesRegex(bridge.Refused, "mapping_changed"):
            obj.inventory(SimpleNamespace(pid=123), time.monotonic() + 1)
        self.assert_terminal(obj)

    def test_deleted_nonpublic_conflicting_and_empty_maps_refuse(self):
        good = "1000-2000 r--p 0 00:2c 1 /usr/lib/known.so\n"
        for text in ("", good.replace("known.so", "known.so (deleted)"),
                     good.replace("/usr/lib", "/private"), good + good.replace(" 1 /", " 2 /")):
            with self.assertRaises(bridge.Refused):
                bridge.map_objects(text)

    def test_writable_descriptor_and_unknown_inventory_refuse(self):
        with patch.object(bridge.os, "open", return_value=9), patch.object(bridge.os, "close"), \
             patch.object(bridge.os, "listdir", return_value=["9", "10"]), \
             patch.object(bridge.os, "fstat", return_value=SimpleNamespace(st_dev=44)), \
             patch.object(bridge.fcntl, "fcntl", return_value=os.O_WRONLY), \
             self.assertRaisesRegex(bridge.Refused, "copy_writable_fd"):
            bridge.no_writable_fds(44)
        with patch.object(bridge.os, "open", return_value=9), patch.object(bridge.os, "close"), \
             patch.object(bridge.os, "listdir", return_value=["9", "10"]), \
             patch.object(bridge.os, "fstat", side_effect=OSError("unknown")), self.assertRaises(OSError):
            bridge.no_writable_fds(44)

    def test_actual_exclusive_copy_same_fd_bytes_and_no_write_fd_left(self):
        obj = fixture()
        real_fstat = os.fstat
        def namespace_stat(fd):
            value = real_fstat(fd)
            names = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
            return SimpleNamespace(**{n: getattr(value, n) for n in names}, st_uid=0, st_gid=0)
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryFile() as source:
            source.write(BLOCK)
            source.flush()
            path = "/usr/lib/synthetic.so"
            row = {"mode": 0o100600, "sha256": hashlib.sha256(BLOCK).hexdigest()}
            sources = SimpleNamespace(files={path: (source.fileno(), os.fstat(source.fileno()))},
                                      table={path: row})
            store = os.open(directory, bridge.FLAGS | os.O_DIRECTORY)
            try:
                with patch.object(bridge.os, "fstat", side_effect=namespace_stat):
                    obj._copy(sources, path, "0", store, time.monotonic() + 1)
                    self.assertEqual((Path(directory) / "0").read_bytes(), BLOCK)
                    self.assertEqual(obj.records[path]["sha256"], row["sha256"])
                    with self.assertRaises(FileExistsError):
                        obj._copy(sources, path, "0", store, time.monotonic() + 1)
                self.assertEqual(bridge.fcntl.fcntl(obj.fds[path], bridge.fcntl.F_GETFL) & os.O_ACCMODE,
                                 os.O_RDONLY)
            finally:
                obj.close()
                os.close(store)

    def test_short_write_wrong_destination_shape_and_hash_refuse(self):
        source = SimpleNamespace(st_dev=31, st_ino=1, st_size=len(BLOCK))
        good = dict(st_dev=44, st_ino=2, st_mode=0o100755, st_uid=0, st_gid=0,
                    st_nlink=1, st_size=len(BLOCK), st_mtime_ns=1, st_ctime_ns=1)
        path = "/usr/lib/synthetic.so"
        expected = hashlib.sha256(BLOCK).hexdigest()
        inputs = SimpleNamespace(files={path: (10, source)},
                                 table={path: {"mode": 0o100755, "sha256": expected}})
        for shape, written, digest in (({}, 1, expected), ({"st_uid": 65534}, len(BLOCK), expected),
                                       ({"st_nlink": 2}, len(BLOCK), expected),
                                       ({"st_size": 1}, len(BLOCK), expected),
                                       ({}, len(BLOCK), "0" * 64)):
            obj = fixture()
            value = SimpleNamespace(**dict(good, **shape))
            with patch.object(bridge.os, "open", side_effect=[20, 21]), \
                 patch.object(bridge.os, "close") as closed, \
                 patch.object(bridge.os, "lseek"), patch.object(bridge.os, "fchmod"), \
                 patch.object(bridge.os, "fsync"), \
                 patch.object(bridge.os, "read", side_effect=[BLOCK, b""]), \
                 patch.object(bridge.os, "write", return_value=written), \
                 patch.object(bridge.os, "fstat", return_value=value), \
                 patch.object(admission, "digest", return_value=digest):
                with self.assertRaises(bridge.Refused):
                    obj._copy(inputs, path, "0", 99, time.monotonic() + 1)
                obj.close()
                self.assertEqual(len(closed.call_args_list), len(set(c.args[0] for c in closed.call_args_list)))

    def test_replaced_store_refuses_and_retained_store_closes_once(self):
        obj = fixture()
        obj.state = "ready"
        obj.store_fd = 9
        fields = dict(st_dev=44, st_ino=1, st_mode=0o40755, st_uid=0, st_gid=0,
                      st_nlink=2, st_size=4096, st_mtime_ns=1, st_ctime_ns=1)
        before = SimpleNamespace(**fields)
        obj.store_identity = admission.identity(before)
        with patch.object(bridge.os, "fstat", return_value=before), \
             patch.object(bridge.os, "stat", return_value=SimpleNamespace(**dict(fields, st_ino=2))), \
             self.assertRaisesRegex(bridge.Refused, "store_replaced"):
            obj._verify_store()
        with patch.object(bridge.os, "close") as close:
            obj.close()
            obj.close()
            close.assert_called_once_with(9)
        self.assert_terminal(obj)

    def test_unknown_target_verification_is_terminal(self):
        obj = fixture()
        obj.state = "ready"
        with patch.object(obj, "_verify_all", side_effect=OSError("unknown")), self.assertRaises(OSError):
            obj.verify(time.monotonic() + 1)
        self.assert_terminal(obj)


if __name__ == "__main__":
    unittest.main()
