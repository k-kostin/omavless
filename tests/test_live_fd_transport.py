"""Real host-native O_EXCL transfers into synthetic private temporary roots."""
import hashlib
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from tests.frontier_fixture_helpers import FIELDS, fixed_vm_os, fixed_vm_process_os

from tests.live_fd_tmpfs_review2 import transport
FROZEN_PINS = dict(transport.PINS)


class FacadeTests(unittest.TestCase):
    def test_modeled_owner_keeps_real_fd_identity_modes_bytes_and_shared_os(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = root / "fixed"
            target.write_bytes(b"synthetic")
            target.chmod(0o600)
            proxy = fixed_vm_os(root)
            directory = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
            try:
                fd = proxy.open("fixed", os.O_RDONLY | os.O_NOFOLLOW, dir_fd=directory)
                try:
                    real = os.fstat(fd)
                    modeled = proxy.fstat(fd)
                    self.assertEqual(tuple(getattr(real, name) for name in FIELDS),
                                     tuple(getattr(modeled, name) for name in FIELDS))
                    self.assertEqual((modeled.st_uid, modeled.st_gid), (1000, 1000))
                    self.assertEqual((real.st_uid, real.st_gid), (os.getuid(), os.getgid()))
                    self.assertEqual(proxy.read(fd, 64), b"synthetic")
                    self.assertIsNot(proxy, os)
                finally:
                    os.close(fd)
            finally:
                os.close(directory)

    def test_supervisor_facade_models_account_without_replacing_shared_calls(self):
        uid, euid = os.getuid, os.geteuid
        proxy = fixed_vm_process_os(os)
        self.assertEqual((proxy.getuid(), proxy.geteuid()), (1000, 1000))
        self.assertIs(os.getuid, uid)
        self.assertIs(os.geteuid, euid)
        self.assertIs(proxy.open, os.open)


class TransportTests(unittest.TestCase):
    def test_all_eight_transport_pins_match_exact_source_generation(self):
        root = Path(__file__).parent
        for name, expected in FROZEN_PINS.items():
            folder = "live_fd_tmpfs_review2"
            source = name
            if name == "bridge.py":
                folder = "live_fd_tmpfs"
            elif name in ("admission.py", "copy-manifest.json"):
                folder = "reviewed_tmpfs_elf"
            elif name in ("containment.py", "guest-inventory.json"):
                folder = "real_resolved_binary"
                source = "probe.py" if name == "containment.py" else name
            self.assertEqual(hashlib.sha256((root / folder / source).read_bytes()).hexdigest(), expected)
        self.assertEqual(len(FROZEN_PINS), 8)

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(dir="/var/tmp")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.parent = self.root.joinpath(*transport.PARTS[:-1])
        self.parent.mkdir(parents=True, mode=0o700)
        for parent in (self.root / "home", self.root / "home/kdk_vm"):
            parent.chmod(0o700)
        self.target = self.parent / transport.PARTS[-1]
        for mock in (patch.object(transport, "os", fixed_vm_os(self.root)),
                     patch.object(transport, "PINS", {"probe.py": hashlib.sha256(b"synthetic").hexdigest(),
                                                       "copy-manifest.json": hashlib.sha256(b"data").hexdigest()})):
            mock.start()
            self.addCleanup(mock.stop)

    def test_actual_create_and_two_payloads_exact_modes_then_no_overwrite(self):
        transport.stage("create", None, b"")
        for name, raw, mode in (("probe.py", b"synthetic", 0o500), ("copy-manifest.json", b"data", 0o600)):
            transport.stage("put", name, raw)
            path = self.target / name
            self.assertEqual(path.read_bytes(), raw)
            self.assertEqual(path.stat().st_mode & 0o777, mode)
            before = path.stat()
            with self.assertRaises(FileExistsError):
                transport.stage("put", name, raw)
            self.assertEqual(transport.identity(before), transport.identity(path.stat()))
        with self.assertRaises(FileExistsError):
            transport.stage("create", None, b"")

    def test_unknown_path_hash_and_oversize_before_any_effect(self):
        for name, raw in (("../probe.py", b"synthetic"), ("probe.py", b"wrong"),
                          ("probe.py", b"x" * 131073)):
            with self.assertRaises(RuntimeError):
                transport.stage("put", name, raw)
            self.assertFalse(self.target.exists())

    def test_short_write_keeps_partial_file_and_never_retries(self):
        transport.stage("create", None, b"")
        real_write = os.write
        with patch.object(transport.os, "write", side_effect=lambda fd, data: real_write(fd, data[:1])) as write:
            with self.assertRaises(RuntimeError):
                transport.stage("put", "probe.py", b"synthetic")
        write.assert_called_once()
        self.assertEqual((self.target / "probe.py").read_bytes(), b"s")

    def test_symlink_unknown_entry_and_writable_ancestor_refuse(self):
        transport.stage("create", None, b"")
        link = self.target / "probe.py"
        link.symlink_to("/not-opened")
        with self.assertRaises(FileExistsError):
            transport.stage("put", "probe.py", b"synthetic")
        link.unlink()
        extra = self.target / "unexpected"
        extra.write_bytes(b"")
        with self.assertRaises(RuntimeError):
            transport.stage("put", "probe.py", b"synthetic")
        extra.unlink()
        self.parent.chmod(0o777)
        with self.assertRaises(RuntimeError):
            transport.stage("put", "probe.py", b"synthetic")
        self.assertFalse(link.exists())

    def test_unknown_metadata_stops_before_create(self):
        with patch.object(transport.os, "fstat", side_effect=OSError("synthetic")), self.assertRaises(OSError):
            transport.stage("create", None, b"")
        self.assertFalse(self.target.exists())

    def test_wrong_modeled_vm_owner_still_refuses_without_host_module_patch(self):
        self.assertIsNot(transport.os, os)
        original = transport.os.fstat
        def wrong(fd):
            value = original(fd)
            value.st_uid = 1001
            return value
        with patch.object(transport.os, "fstat", side_effect=wrong):
            with self.assertRaises(RuntimeError):
                transport.stage("create", None, b"")
        self.assertFalse(self.target.exists())


if __name__ == "__main__":
    unittest.main()
