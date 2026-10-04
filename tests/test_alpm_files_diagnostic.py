"""Pure ALPM format/metadata tests, using synthetic paths and contents only."""
import importlib.util
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).parent / "static_elf_provenance"
SPEC = importlib.util.spec_from_file_location("alpm_files_diagnostic", ROOT / "alpm_files_diagnostic.py")
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)
SUPERVISOR_SPEC = importlib.util.spec_from_file_location("alpm_files_supervisor", ROOT / "alpm_files_supervisor.py")
supervisor = importlib.util.module_from_spec(SUPERVISOR_SPEC)
SUPERVISOR_SPEC.loader.exec_module(supervisor)
assert supervisor.base is None
supervisor.base = supervisor.load_containment(ROOT.parent / "real_resolved_binary/probe.py")


def meta(**changes):
    data = dict(st_dev=31, st_ino=1, st_size=0, st_uid=0, st_gid=0, st_mode=0o100644,
                st_nlink=1, st_mtime_ns=1, st_ctime_ns=1)
    return SimpleNamespace(**dict(data, **changes))


class AlpmFilesTests(unittest.TestCase):
    def test_missing_staged_helper_has_no_repository_fallback(self):
        fixed = supervisor.STAGE / "containment.py"
        with patch.object(supervisor.os, "open", side_effect=FileNotFoundError) as opened:
            with self.assertRaises(FileNotFoundError):
                supervisor.load_containment(fixed)
        opened.assert_called_once()
        self.assertEqual(opened.call_args.args[0], fixed)

    def test_wrapper_supervisor_failure_stops_before_every_later_action(self):
        guard = (ROOT / "vm-guard-alpm-retained-fd.sh").read_text()
        block = guard.split('if ! env -i ', 1)[1].split('\nfi\n', 1)[0]
        with tempfile.TemporaryDirectory(dir="/var/tmp") as temporary:
            script = 'env() { return 1; }; task_stage="$1"\nif ! env -i ' + block + '\nfi\nprintf FORBIDDEN_LATER_ACTION\n'
            result = subprocess.run(["/bin/bash", "-c", script, "test", temporary],
                                    check=False, capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "ALPM_SHAPE_DIAGNOSTIC_NONPASS\n")
        self.assertNotIn("FORBIDDEN", result.stdout)

    def test_wrapper_pins_metadata_only_source_and_preservation(self):
        guard = (ROOT / "vm-guard-alpm-retained-fd.sh").read_text()
        self.assertIn(hashlib.sha256((ROOT / "alpm_files_diagnostic.py").read_bytes()).hexdigest(), guard)
        self.assertIn(hashlib.sha256((ROOT / "alpm_files_supervisor.py").read_bytes()).hexdigest(), guard)
        self.assertIn(supervisor.CONTAINMENT_SHA, guard)
        self.assertIn("set -o noclobber", guard)
        self.assertIn("umask 077", guard)
        self.assertIn('test ! -L "$task_stage"', guard)
        self.assertIn('stat -c %u:%g', guard)
        self.assertNotIn("timeout --", guard)
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
             patch.object(probe.os, "lseek"), \
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
                 patch.object(probe.os, "lseek"), \
                 patch.object(probe.time, "monotonic", return_value=now):
                with self.assertRaises(probe.Refused):
                    probe.measure(Path("/var/lib/pacman/local/meta-1-1/files"), 1)
                closed.assert_called_once_with(99)

    def test_capture_stops_at_first_legacy_failure_without_executing_any_tool(self):
        metadata = {"size": 0, "sha256": "empty", "uid": 0}
        with patch.object(probe.os, "getuid", return_value=1000), patch.object(probe.os, "geteuid", return_value=1000), \
             patch.object(probe.Path, "lstat", return_value=meta(st_mode=0o40755)), \
             patch.object(probe.os, "listdir", return_value=["meta-1-1", "later-1-1"]), \
             patch.object(probe, "open_record", return_value=(99, None, None, None)), \
             patch.object(probe.os, "close"), \
             patch.object(probe, "read_record", return_value=(b"", metadata)) as read, \
             patch.object(probe, "measure", return_value=
                  (b"%NAME%\nexample-meta\n%VERSION%\n1-1\n", {"sha256": "description"})) as measure:
            result = probe.capture()
        self.assertEqual(result["outcome"], "OBSERVED_ALPM_FILELIST_SHAPE")
        self.assertEqual(result["scanned_count"], 1)
        self.assertEqual(measure.call_count, 1)
        self.assertEqual(read.call_count, 2)
        self.assertEqual(result["package"]["name"], "example-meta")
        self.assertIs(result["readelf_executed"], False)
        self.assertIs(result["allowlist_adoption"], False)
        source = (ROOT / "alpm_files_diagnostic.py").read_text()
        for forbidden in ("subprocess", "os.exec", "os.system", "capture_static", "base.command"):
            self.assertNotIn(forbidden, source)

    def test_real_original_fd_retained_across_description_and_mutations(self):
        for mutation in ("unchanged", "replace", "inplace_revert", "rename_revert"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory(dir="/var/tmp") as temporary:
                root = Path(temporary)
                package = root / "meta-1-1"
                package.mkdir()
                selected = package / "files"
                selected.write_bytes(b"")
                (package / "desc").write_bytes(b"%NAME%\nexample-meta\n%VERSION%\n1-1\n")
                original = selected.stat()
                real_measure, real_open = probe.measure, os.open
                file_opens = []

                def opened(path, *args, **kwargs):
                    fd = real_open(path, *args, **kwargs)
                    if path == selected:
                        file_opens.append(fd)
                    return fd

                def description(path, deadline):
                    self.assertEqual(os.fstat(file_opens[0]).st_ino, original.st_ino)
                    if mutation in ("replace", "rename_revert"):
                        selected.rename(package / "saved")
                        selected.write_bytes(b"")
                        if mutation == "rename_revert":
                            selected.unlink()
                            (package / "saved").rename(selected)
                    elif mutation == "inplace_revert":
                        selected.write_bytes(b"changed")
                        selected.write_bytes(b"")
                        os.utime(selected, ns=(original.st_atime_ns, original.st_mtime_ns))
                    return real_measure(path, deadline)

                with patch.object(probe, "ROOT", root), patch.object(probe, "root_owned", return_value=True), \
                     patch.object(probe.os, "getuid", return_value=1000), \
                     patch.object(probe.os, "geteuid", return_value=1000), \
                     patch.object(probe.os, "open", side_effect=opened), \
                     patch.object(probe, "measure", side_effect=description):
                    result = probe.capture()
                self.assertEqual(len(file_opens), 1)
                with self.assertRaises(OSError):
                    os.fstat(file_opens[0])
                if mutation == "unchanged":
                    self.assertEqual(result["outcome"], "OBSERVED_ALPM_FILELIST_SHAPE")
                    self.assertEqual(result["original_open_fd"]["mtime_ns"], original.st_mtime_ns)
                    self.assertEqual(result["original_open_fd"]["ctime_ns"], original.st_ctime_ns)
                else:
                    self.assertEqual(result["outcome"], "NONPASS")
                    self.assertNotIn("original_open_fd", result)

    def test_supervisor_unknown_is_single_terminal_call_without_fallback(self):
        for status in ("known", "unknown", "incomplete"):
            with self.subTest(status=status), tempfile.TemporaryDirectory(dir="/var/tmp") as temporary:
                stage = Path(temporary)
                child = Mock(returncode=0 if status == "known" else None)
                def observed(process, seconds):
                    self.assertIs(process, child)
                    self.assertEqual(seconds, 15)
                    if status == "unknown":
                        raise supervisor.base.Refused("owned_wait_unknown_preserve")
                    return status == "known"
                with patch.object(supervisor, "STAGE", stage), \
                     patch.object(supervisor.os, "getuid", return_value=1000), \
                     patch.object(supervisor.os, "geteuid", return_value=1000), \
                     patch.object(supervisor.base, "private_parent"), \
                     patch.object(supervisor.base, "object_bytes"), \
                     patch.object(supervisor.base, "OwnedProcess", return_value=child) as spawn, \
                     patch.object(supervisor.base, "supervise", side_effect=observed) as observed_call:
                    result = supervisor.run_child(stage)
                spawn.assert_called_once()
                observed_call.assert_called_once()
                self.assertEqual(child.mock_calls, [])
                self.assertEqual(result["outcome"], "KNOWN_COMPLETED" if status == "known" else "NONPASS")
                if status != "known":
                    self.assertNotIn("returncode", result)

    def test_existing_child_output_refuses_before_launch(self):
        with tempfile.TemporaryDirectory(dir="/var/tmp") as temporary:
            stage = Path(temporary)
            (stage / "result.json").write_bytes(b"retained")
            with patch.object(supervisor, "STAGE", stage), \
                 patch.object(supervisor.os, "getuid", return_value=1000), \
                 patch.object(supervisor.os, "geteuid", return_value=1000), \
                 patch.object(supervisor.base, "private_parent"), \
                 patch.object(supervisor.base, "object_bytes"), \
                 patch.object(supervisor.base, "OwnedProcess") as spawn:
                result = supervisor.run_child(stage)
            self.assertEqual(result["outcome"], "NONPASS")
            spawn.assert_not_called()
            self.assertEqual((stage / "result.json").read_bytes(), b"retained")


if __name__ == "__main__":
    unittest.main()
