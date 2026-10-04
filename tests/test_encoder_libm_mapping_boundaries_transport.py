"""Real host-native O_EXCL transfers into synthetic private temporary roots."""
import hashlib
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from tests.encoder_libm_live_mapping import transport
FROZEN_PINS = dict(transport.PINS)


class TransportTests(unittest.TestCase):
    def test_all_eight_transport_pins_match_exact_source_generation(self):
        root = Path(__file__).parent
        for name, expected in FROZEN_PINS.items():
            folder = "encoder_libm_live_mapping"
            source = name
            if name in ("admission.py", "copy-manifest.json"):
                folder = "encoder_libm_copy_admission"
            elif name in ("containment.py", "guest-inventory.json"):
                folder = "real_resolved_binary"
                source = "probe.py" if name == "containment.py" else name
            self.assertEqual(hashlib.sha256((root / folder / source).read_bytes()).hexdigest(), expected)
        self.assertEqual(len(FROZEN_PINS), 9)

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(dir="/var/tmp")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.parent = self.root.joinpath(*transport.PARTS[:-1])
        self.parent.mkdir(parents=True, mode=0o700)
        for parent in (self.root / "home", self.root / "home/kdk_vm"):
            parent.chmod(0o700)
        self.target = self.parent / transport.PARTS[-1]
        original_open, original_stat = os.open, os.stat
        for mock in (patch.object(transport.os, "open", side_effect=lambda name, *a, **kw:
                                 original_open(self.root if name == "/" else name, *a, **kw)),
                     patch.object(transport.os, "stat", side_effect=lambda name, *a, **kw:
                                  original_stat(self.root if name == "/" else name, *a, **kw)),
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


if __name__ == "__main__":
    unittest.main()
