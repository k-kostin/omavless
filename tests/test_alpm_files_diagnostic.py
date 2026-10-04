"""Pure ALPM format/metadata tests, using synthetic paths and contents only."""
import importlib.util
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).parent / "static_elf_provenance"
SPEC = importlib.util.spec_from_file_location("alpm_files_diagnostic", ROOT / "alpm_files_diagnostic.py")
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


def meta(**changes):
    data = dict(st_dev=31, st_ino=1, st_size=0, st_uid=0, st_gid=0, st_mode=0o100644,
                st_nlink=1, st_mtime_ns=1, st_ctime_ns=1)
    return SimpleNamespace(**dict(data, **changes))


class AlpmFilesTests(unittest.TestCase):
    def test_wrapper_pins_metadata_only_source_and_preservation(self):
        guard = (ROOT / "vm-guard-alpm-shape.sh").read_text()
        self.assertIn(hashlib.sha256((ROOT / "alpm_files_diagnostic.py").read_bytes()).hexdigest(), guard)
        for category in ("CANONICAL_EPOCH", "PRIVATE_FILES", "USER_SERVICE", "EXECUTABLE",
                         "NAMESPACE", "CORE_INVENTORY", "TUN_INVENTORY", "RESOLVER", "RESOLVCONF"):
            self.assertIn("check_category " + category, guard)

    def test_empty_and_empty_lines_are_valid_zero_ownership_edges(self):
        for raw in (b"", b"\n", b"\n\n"):
            self.assertEqual(probe.proposed_file_entries(raw), [])
            summary = probe.shape(raw)
            self.assertFalse(summary["legacy_exactly_one_files_marker"])
            self.assertEqual(summary["proposed_format"], "VALID")
            self.assertEqual(summary["proposed_entry_count"], 0)
        self.assertTrue(probe.shape(b"")["zero_bytes"])
        self.assertFalse(probe.shape(b"\n")["zero_bytes"])

    def test_files_and_corresponding_backup_follow_primary_format(self):
        raw = b"%FILES%\netc/\netc/test.conf\nusr/lib/test.so\n\n%BACKUP%\netc/test.conf\td41d8cd98f00b204e9800998ecf8427e\n"
        self.assertEqual(probe.proposed_file_entries(raw), ["etc/", "etc/test.conf", "usr/lib/test.so"])

    def test_malformed_nonempty_and_duplicate_sections_stay_refused(self):
        for raw in (b" \n", b"# comment\n", b"%FILES%\n", b"%BACKUP%\nx\n", b"%UNKNOWN%\nx\n",
                    b"%FILES%\na\n%FILES%\nb\n", b"%FILES%\na\n%BACKUP%\n",
                    b"%FILES%\na\n%BACKUP%\nb\td41d8cd98f00b204e9800998ecf8427e\n",
                    b"%FILES%\n/etc/private\n", b"%FILES%\n../private\n", b"%FILES%\na\na\n",
                    b"%FILES%\na\n%BACKUP%\na\tbad\n", b"\xef\xbb\xbf%FILES%\na\n"):
            with self.subTest(raw=raw), self.assertRaises(probe.Refused):
                probe.proposed_file_entries(raw)
        with self.assertRaises(UnicodeError):
            probe.proposed_file_entries(b"\xff")

    def test_summary_never_retains_file_or_backup_names_or_unknown_headers(self):
        raw = b"%FILES%\nhome/synthetic-private\n%PRIVATE_HEADER%\nsecret-provider\n"
        output = json.dumps(probe.shape(raw))
        for secret in ("synthetic-private", "PRIVATE_HEADER", "secret-provider"):
            self.assertNotIn(secret, output)

    def test_measure_zero_byte_file_retains_original_fd_metadata(self):
        file_stat, parent = meta(), meta(st_mode=0o40755)
        with patch.object(probe.Path, "lstat", side_effect=[parent, file_stat, parent]), \
             patch.object(probe.os, "open", return_value=99) as opened, patch.object(probe.os, "close") as closed, \
             patch.object(probe.os, "fstat", return_value=file_stat), patch.object(probe.os, "read", return_value=b""), \
             patch.object(probe.time, "monotonic", return_value=0):
            raw, record = probe.measure(Path("/var/lib/pacman/local/meta-1-1/files"), 1)
        self.assertEqual(raw, b"")
        self.assertEqual(record["size"], 0)
        self.assertEqual(record["sha256"], "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        self.assertTrue(opened.call_args.args[1] & probe.os.O_NOFOLLOW)
        closed.assert_called_once_with(99)

    def test_replacement_wrong_owner_and_timeout_are_terminal(self):
        parent = meta(st_mode=0o40755)
        for actual, final, now in ((meta(st_uid=1000), meta(), 0), (meta(), meta(st_ino=2), 0), (meta(), meta(), 2)):
            with patch.object(probe.Path, "lstat", side_effect=[parent, final, parent]), \
                 patch.object(probe.os, "open", return_value=99), patch.object(probe.os, "close") as closed, \
                 patch.object(probe.os, "fstat", return_value=actual), patch.object(probe.os, "read", return_value=b""), \
                 patch.object(probe.time, "monotonic", return_value=now):
                with self.assertRaises(probe.Refused):
                    probe.measure(Path("/var/lib/pacman/local/meta-1-1/files"), 1)
                closed.assert_called_once_with(99)

    def test_capture_stops_at_first_legacy_failure_without_executing_any_tool(self):
        metadata = {"size": 0, "sha256": "empty", "uid": 0}
        with patch.object(probe.os, "getuid", return_value=1000), patch.object(probe.os, "geteuid", return_value=1000), \
             patch.object(probe.Path, "lstat", return_value=meta(st_mode=0o40755)), \
             patch.object(probe.os, "listdir", return_value=["meta-1-1", "later-1-1"]), \
             patch.object(probe, "measure", side_effect=[(b"", metadata),
                  (b"%NAME%\nexample-meta\n%VERSION%\n1-1\n", {"sha256": "description"}), (b"", metadata)]) as measure:
            result = probe.capture()
        self.assertEqual(result["outcome"], "OBSERVED_ALPM_FILELIST_SHAPE")
        self.assertEqual(result["scanned_count"], 1)
        self.assertEqual(measure.call_count, 3)
        self.assertEqual(result["package"]["name"], "example-meta")
        self.assertIs(result["readelf_executed"], False)
        self.assertIs(result["allowlist_adoption"], False)
        source = (ROOT / "alpm_files_diagnostic.py").read_text()
        for forbidden in ("subprocess", "os.exec", "os.system", "capture_static", "base.command"):
            self.assertNotIn(forbidden, source)


if __name__ == "__main__":
    unittest.main()
