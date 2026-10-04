"""Pure fixed-FD diagnostic tests; no real mmap, daemon or VM access."""
import importlib.util
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).parent / "fixed_fd_filesystem"
SPEC = importlib.util.spec_from_file_location("fixed_fd_filesystem", ROOT / "probe.py")
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)
MOUNT = "17 1 0:31 /private-root /private-mount rw,secret - btrfs /private-source rw,secret\n"
MAP = "1000-2000 r--p 00000000 00:1d 26297 /usr/bin/dbus-daemon\n"
FIELDS = dict(st_dev=31, st_ino=26297, st_size=199176, st_uid=0, st_gid=0,
              st_mode=probe.stat.S_IFREG | 0o755, st_nlink=1, st_mtime_ns=1, st_ctime_ns=1)


class FixedFdTests(unittest.TestCase):
    def test_wrong_abi_never_loads_libc(self):
        with patch.object(probe.os, "uname", return_value=SimpleNamespace(machine="aarch64")), \
             patch.object(probe.ctypes, "CDLL") as load:
            with self.assertRaisesRegex(probe.Refused, "unsupported_abi"):
                probe.libc_calls()
            load.assert_not_called()
        self.assertEqual(probe.ctypes.sizeof(probe.Statfs), 120)
        self.assertEqual(probe.Statfs.fsid.offset, 56)
        self.assertEqual(probe.Statfs.spare.offset, 88)

    def test_mount_metadata_redacts_all_free_text(self):
        value = probe.mount_metadata("mnt_id:\t17\n", MOUNT)
        self.assertEqual(value, {"mount_id": 17, "parent_mount_id": 1,
                                "device_major": 0, "device_minor": 31, "filesystem_type": "btrfs"})
        self.assertNotIn("private", json.dumps(value))
        self.assertNotIn("secret", json.dumps(value))
        self.assertEqual(probe.mount_metadata("mnt_id: 17\n", MOUNT.replace("btrfs", "private-fstype"))["filesystem_type"], "other")

    def test_mount_unknown_and_duplicate_refuse_without_data(self):
        for info, rows in (("mnt_id: 18\n", MOUNT), ("mnt_id: 17\nmnt_id: 17\n", MOUNT),
                           ("mnt_id: 17\n", MOUNT * 2), ("mnt_id: secret\n", MOUNT),
                           ("mnt_id: 17\n", MOUNT.replace("0:31", "secret"))):
            with self.assertRaises(probe.Refused) as caught:
                probe.mount_metadata(info, rows)
            self.assertNotIn("secret", str(caught.exception))

    def test_mapping_is_exact_readonly_fixed_fd_range(self):
        self.assertEqual(probe.mapped_metadata(MAP, 4096), {"device": 29, "inode": 26297,
                         "length": 4096, "permissions": "r--p", "offset": 0})
        for text in (MAP.replace("r--p", "rw-p"), MAP.replace("r--p", "r-xp"),
                     MAP.replace("1000-2000", "1000-3000"), MAP.replace("00000000", "00001000"),
                     MAP.replace("/usr/bin/dbus-daemon", "/home/private"), MAP + MAP,
                     MAP.replace("/usr/bin/dbus-daemon", "/usr/bin/dbus-daemon (deleted)")):
            with self.assertRaises(probe.Refused):
                probe.mapped_metadata(text, 4096)

    def run_fake(self, *, fields=None, mmap_result=4096, unmap=0, metadata=None, statfs_result=0):
        libc = SimpleNamespace(fstatfs=Mock(return_value=statfs_result), mmap=Mock(return_value=mmap_result),
                               munmap=Mock(return_value=unmap))
        def read(path, limit):
            return {"/proc/self/fdinfo/99": "mnt_id: 17\n",
                    "/proc/self/mountinfo": MOUNT, "/proc/self/maps": MAP}[path]
        with patch.object(probe, "libc_calls", return_value=libc), \
             patch.object(probe.os, "getuid", return_value=1000), \
             patch.object(probe.os, "geteuid", return_value=1000), \
             patch.object(probe.os, "uname", return_value=SimpleNamespace(release="6.19.1-test")), \
             patch.object(probe.os, "open", return_value=99) as opened, \
             patch.object(probe.os, "fstat", side_effect=fields or [SimpleNamespace(**FIELDS)] * 2), \
             patch.object(probe.os, "close") as closed, \
             patch.object(probe, "bounded_text", side_effect=metadata or read):
            result = probe.observe()
        return result, libc, opened, closed

    def test_device_mismatch_is_observation_not_loaded_identity_proof(self):
        result, libc, opened, closed = self.run_fake()
        self.assertEqual(result["outcome"], "OBSERVED_READONLY_METADATA")
        self.assertFalse(result["device_equal"])
        self.assertTrue(result["inode_equal"])
        for key in ("mapped_bytes_accessed", "loaded_elf_identity_proven", "allowlist_adoption", "compatibility_acceptance"):
            self.assertIs(result[key], False)
        self.assertNotIn("sha256", json.dumps(result))
        self.assertNotIn("private", json.dumps(result))
        libc.mmap.assert_called_once_with(None, 4096, 1, 2, 99, 0)
        libc.munmap.assert_called_once_with(4096, 4096)
        opened.assert_called_once_with(probe.TARGET, probe.os.O_RDONLY | probe.os.O_NOFOLLOW | probe.os.O_NONBLOCK | probe.os.O_CLOEXEC)
        closed.assert_called_once_with(99)

    def test_changed_fd_refuses_before_or_after_mapping(self):
        wrong = SimpleNamespace(**dict(FIELDS, st_ino=26298))
        result, libc, _, closed = self.run_fake(fields=[wrong])
        self.assertEqual(result["reason"], "fixed_fd_identity_changed")
        libc.mmap.assert_not_called()
        closed.assert_called_once_with(99)
        result, libc, _, _ = self.run_fake(fields=[SimpleNamespace(**FIELDS), wrong])
        self.assertEqual(result["reason"], "fixed_fd_changed")
        self.assertEqual(result["outcome"], "NONPASS")
        libc.munmap.assert_called_once_with(4096, 4096)

    def test_failed_mapping_is_not_unmapped_and_failed_unmap_not_retried(self):
        result, libc, _, closed = self.run_fake(statfs_result=-1)
        self.assertEqual(result["reason"], "fstatfs_failed")
        libc.mmap.assert_not_called()
        closed.assert_called_once_with(99)
        result, libc, _, closed = self.run_fake(mmap_result=probe.ctypes.c_void_p(-1).value)
        self.assertEqual(result["reason"], "mmap_failed")
        libc.munmap.assert_not_called()
        closed.assert_called_once_with(99)
        result, libc, _, closed = self.run_fake(unmap=-1)
        self.assertEqual(result["outcome"], "NONPASS")
        self.assertEqual(result["cleanup_reason"], "munmap_failed")
        libc.munmap.assert_called_once()
        closed.assert_called_once_with(99)

    def test_outer_guard_pins_source_and_retains_all_baseline_categories(self):
        guard = (ROOT / "vm-guard.sh").read_text()
        self.assertIn(hashlib.sha256((ROOT / "probe.py").read_bytes()).hexdigest(), guard)
        for category in ("CANONICAL_EPOCH", "PRIVATE_FILES", "USER_SERVICE", "EXECUTABLE",
                         "NAMESPACE", "CORE_INVENTORY", "TUN_INVENTORY", "RESOLVER", "RESOLVCONF"):
            self.assertIn("check_category " + category, guard)
        self.assertIn("timeout --signal=TERM --kill-after=1 10", guard)
        self.assertIn("'address', 'route', 'rule', 'route6', 'rule6'", guard)

    def test_source_has_no_content_access_or_external_dispatch(self):
        source = (ROOT / "probe.py").read_text()
        for forbidden in ("subprocess", "os.system", "os.exec", "hashlib", "string_at(",
                          "from_address(", "os.read(", "map_files", "PROT_WRITE", "PROT_EXEC=", "mount("):
            self.assertNotIn(forbidden, source)


if __name__ == "__main__":
    unittest.main()
