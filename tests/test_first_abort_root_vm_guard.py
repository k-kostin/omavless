# SPDX-License-Identifier: MIT
"""Pure/local-only root guard tests: no VM, sudo, matrix or credential changes."""
import hashlib
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, Mock, patch

HERE = Path(__file__).parent / "first_abort_process"


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, HERE / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


guard = load("root_abort_guard", "root_vm_guard.py")
legacy = load("legacy_abort_guard_for_pure_tests", "vm_guard.py")


class RootGuards(unittest.TestCase):
    def setUp(self):
        guard.core = legacy
        legacy.UNCERTAIN = False
        legacy.RETAINED.clear()
        guard.RETAINED.clear()

    def tearDown(self):
        legacy.UNCERTAIN = False
        legacy.RETAINED.clear()
        for value in guard.RETAINED:
            for fd in ([value] if isinstance(value, int) else [getattr(value, "fd", None)]):
                if fd is not None:
                    try:
                        os.close(fd)
                    except OSError:
                        pass
        guard.RETAINED.clear()

    def fake_matrix(self):
        elf = SimpleNamespace(path=Path("/run/user/1000/ov-abort-root-" + "a" * 32) / "fixture",
                              fd=57, sha="b" * 64, before=SimpleNamespace(st_dev=17, st_ino=29), recheck=Mock())
        evidence = SimpleNamespace(create=Mock(side_effect=[101, 102]))
        return evidence, elf, {"host_build_target": "/host/build"}

    def test_legacy_guard_is_byte_exact_failed_evidence_not_edited(self):
        self.assertEqual(hashlib.sha256((HERE / "vm_guard.py").read_bytes()).hexdigest(), guard.LEGACY_SHA)

    def test_root_admission_rejects_mixed_saved_credentials_before_any_file_effect(self):
        for uids, gids in (((1000, 1000, 1000), (1000, 1000, 1000)),
                           ((0, 0, 1000), (0, 0, 0)), ((0, 0, 0), (0, 1000, 0))):
            with patch.object(guard.os, "getresuid", return_value=uids), \
                 patch.object(guard.os, "getresgid", return_value=gids), \
                 patch.object(guard, "Directory") as directory:
                with self.assertRaises(guard.Refused):
                    guard.main()
                directory.assert_not_called()

    def test_direct_matrix_uses_only_original_readonly_executable_fd_and_native_credentials(self):
        evidence, elf, receipt = self.fake_matrix()
        child = SimpleNamespace(pid=42, returncode=None)
        with patch.object(legacy, "spawn", return_value=child) as spawn, \
             patch.object(legacy, "await_exact", return_value=0) as wait, \
             patch.object(guard.os, "fstat", return_value=SimpleNamespace(st_size=0)), \
             patch.object(guard.os, "fsync"), patch.object(guard.os, "pread", return_value=b""):
            self.assertEqual(guard.execute_matrix(evidence, elf, receipt), "")
        args, kwargs = spawn.call_args
        self.assertEqual(args[0], [str(elf.path), "--exact", legacy.MATRIX, "--ignored", "--nocapture", "--test-threads=1"])
        for key, value in guard.CREDENTIALS.items():
            self.assertEqual(kwargs[key], value)
        self.assertEqual(kwargs["executable"], "/proc/self/fd/57")
        self.assertEqual(kwargs["pass_fds"], (57,))
        self.assertEqual(kwargs["env"]["HOME"], "/home/kdk_vm")
        self.assertEqual(kwargs["env"]["OMAVLESS_ABORT_EXPECTED_DEVICE"], "17")
        self.assertEqual(kwargs["env"]["OMAVLESS_ABORT_EXPECTED_INODE"], "29")
        self.assertNotIn("preexec_fn", kwargs)
        self.assertNotIn("shell", kwargs)
        self.assertEqual(set(kwargs["env"]), set(guard.ENV) | {
            "OMAVLESS_ABORT_FROZEN_ELF", "OMAVLESS_ABORT_FROZEN_SHA256", "OMAVLESS_ABORT_BUILD_TARGET",
            "OMAVLESS_ABORT_EXPECTED_DEVICE", "OMAVLESS_ABORT_EXPECTED_INODE"})
        wait.assert_called_once_with(child, 300)

    def test_nonzero_matrix_permanently_stops_before_any_after_operation(self):
        evidence, elf, receipt = self.fake_matrix()
        child = SimpleNamespace(pid=42, returncode=None)
        with patch.object(legacy, "spawn", return_value=child) as spawn, \
             patch.object(legacy, "await_exact", return_value=101), \
             patch.object(guard.os, "fstat") as observe, patch.object(guard.os, "fsync") as sync:
            with self.assertRaises(legacy.Refused):
                guard.execute_matrix(evidence, elf, receipt)
            with self.assertRaises(legacy.Refused):
                guard.execute_matrix(evidence, elf, receipt)
            with self.assertRaises(legacy.Refused):
                guard.executable_absence(elf.before)
            with self.assertRaises(legacy.Refused):
                guard.command(evidence, ["/usr/bin/pgrep", "-x", "mihomo"], (0, 1))
            observe.assert_not_called()
            sync.assert_not_called()
            self.assertEqual(spawn.call_count, 1)
            self.assertEqual(elf.recheck.call_count, 1)
            self.assertTrue(legacy.UNCERTAIN)

    def test_unknown_raw_wait_from_matrix_never_reaps_or_runs_afterchecks(self):
        for error in (ChildProcessError(), InterruptedError(), PermissionError()):
            legacy.UNCERTAIN = False
            evidence, elf, receipt = self.fake_matrix()
            child = SimpleNamespace(pid=42, returncode=None)
            with patch.object(legacy, "spawn", return_value=child), \
                 patch.object(legacy.os, "waitid", side_effect=error) as observe, \
                 patch.object(legacy.os, "waitpid") as reap, patch.object(guard.os, "fstat") as files:
                with self.assertRaises(legacy.Refused):
                    guard.execute_matrix(evidence, elf, receipt)
                with self.assertRaises(legacy.Refused):
                    guard.execute_matrix(evidence, elf, receipt)
                self.assertEqual(observe.call_count, 1)
                self.assertEqual(elf.recheck.call_count, 1)
                reap.assert_not_called()
                files.assert_not_called()

    def test_command_allowlist_rejects_arbitrary_root_surface_before_spawn_or_files(self):
        evidence = SimpleNamespace(create=Mock(), count=0)
        for args in (["/usr/bin/systemctl", "start", "omavless-runtime.service"],
                     ["/usr/bin/sh", "-c", "true"], ["/usr/bin/runuser", "-u", "root"],
                     ["/usr/bin/ip", "link", "set", "dev", "lo", "down"]):
            with patch.object(legacy, "spawn") as spawn:
                with self.assertRaises(guard.Refused):
                    guard.command(evidence, args)
                spawn.assert_not_called()
                evidence.create.assert_not_called()

    def test_readonly_user_manager_query_drops_credentials_without_runuser_or_pam(self):
        evidence = SimpleNamespace(create=Mock(side_effect=[101, 102]), count=0)
        fields = [argument for field in legacy.FIELDS for argument in ("-p", field)]
        args = ["/usr/bin/systemctl", "--user", "--no-pager", "show", "omavless-runtime.service", *fields]
        with patch.object(legacy, "spawn", return_value=SimpleNamespace(pid=42, returncode=None)) as spawn, \
             patch.object(legacy, "await_exact", return_value=0), \
             patch.object(guard.os, "fstat", return_value=SimpleNamespace(st_size=0)), \
             patch.object(guard.os, "pread", return_value=b""):
            guard.command(evidence, args)
        self.assertEqual(spawn.call_args.args[0], args)
        self.assertEqual(spawn.call_args.kwargs["env"], guard.ENV)
        for key, value in guard.CREDENTIALS.items():
            self.assertEqual(spawn.call_args.kwargs[key], value)
        self.assertNotIn("pass_fds", spawn.call_args.kwargs)

    def test_directory_admission_rejects_unsafe_ancestry_and_wrong_owner(self):
        with tempfile.TemporaryDirectory() as name:
            with self.assertRaises(guard.Refused):
                guard.Directory(Path(name), 0)

    def local_parent(self, root):
        # Bypass ONLY root-layout admission for local synthetic file mechanics.
        parent = object.__new__(guard.Directory)
        parent.path = root
        parent.fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        parent.before = os.fstat(parent.fd)
        guard.RETAINED.append(parent)
        return parent

    def test_original_file_fd_and_parent_are_not_recaptured_after_same_byte_swap(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            parent = self.local_parent(root)
            path = root / "fixture"
            path.write_bytes(b"synthetic bytes")
            path.chmod(0o500)
            held = guard.HeldFile(parent, "fixture", 4096, 0o500, os.getuid())
            replacement = root / "replacement"
            replacement.write_bytes(b"synthetic bytes")
            replacement.chmod(0o500)
            replacement.replace(path)
            with self.assertRaises(guard.Refused):
                held.recheck()

    def test_file_admission_rejects_root_user_owner_confusion_and_writable_mode(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            parent = self.local_parent(root)
            path = root / "fixture"
            path.write_bytes(b"synthetic bytes")
            path.chmod(0o500)
            with self.assertRaises(guard.Refused):
                guard.HeldFile(parent, "fixture", 4096, 0o500, os.getuid() + 1)
            path.chmod(0o700)
            with self.assertRaises(guard.Refused):
                guard.HeldFile(parent, "fixture", 4096, 0o500, os.getuid())

    def test_elf_symlink_and_extra_hardlink_are_rejected(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            parent = self.local_parent(root)
            path = root / "original"
            path.write_bytes(b"synthetic bytes")
            path.chmod(0o500)
            (root / "fixture").symlink_to(path)
            with self.assertRaises(OSError):
                guard.HeldFile(parent, "fixture", 4096, 0o500, os.getuid())
            os.link(path, root / "second")
            with self.assertRaises(guard.Refused):
                guard.HeldFile(parent, "original", 4096, 0o500, os.getuid())

    def test_root_observation_rejects_permission_and_pid_epoch_uncertainty(self):
        proc = MagicMock(name="process-path")
        proc.name = "42"
        proc.stat.return_value = SimpleNamespace(st_uid=1000)
        target = SimpleNamespace(st_dev=1, st_ino=2)
        with patch.object(guard.Path, "iterdir", return_value=[proc]), \
             patch.object(guard, "proc_fields", return_value=(b"S", b"123")), \
             patch.object(guard.os, "open", side_effect=PermissionError()) as opened:
            with self.assertRaises(legacy.Refused):
                guard.executable_absence(target)
            with self.assertRaises(legacy.Refused):
                guard.executable_absence(target)
            self.assertEqual(opened.call_count, 1)
        self.assertTrue(legacy.UNCERTAIN)
        legacy.UNCERTAIN = False
        metadata = os.stat(__file__)
        proc.__truediv__ = Mock(return_value=SimpleNamespace(stat=lambda: metadata))
        with patch.object(guard.Path, "iterdir", return_value=[proc]), \
             patch.object(guard, "proc_fields", side_effect=[(b"S", b"123"), (b"S", b"124")]), \
             patch.object(guard.os, "open", return_value=999999), patch.object(guard.os, "fstat", return_value=metadata):
            with self.assertRaises(legacy.Refused):
                guard.executable_absence(target)

    def test_matrix_identity_receipt_mismatch_refuses_before_case_traversal(self):
        elf = SimpleNamespace(before=SimpleNamespace(st_dev=17, st_ino=29))
        with patch.object(guard, "Directory") as directory:
            for text in ("", "matrix-executed-identity=17:30\n", "matrix-completed-identity=17:29\n"):
                with self.assertRaises(guard.Refused):
                    guard.inspect_cases(text, elf)
            directory.assert_not_called()

    def test_stable_exact_executable_is_presence_not_quiescence(self):
        metadata = os.stat(__file__)
        proc = MagicMock(name="process-path")
        proc.name = "42"
        proc.stat.return_value = SimpleNamespace(st_uid=1000)
        (proc / "exe").stat.return_value = metadata
        with patch.object(guard.Path, "iterdir", return_value=[proc]), \
             patch.object(guard, "proc_fields", return_value=(b"S", b"123")), \
             patch.object(guard.os, "open", return_value=999999), patch.object(guard.os, "fstat", return_value=metadata):
            with self.assertRaises(legacy.Refused):
                guard.executable_absence(metadata)
            self.assertTrue(legacy.UNCERTAIN)

    def test_stable_unrelated_inode_is_only_point_in_time_absence(self):
        metadata = os.stat(__file__)
        proc = MagicMock(name="process-path")
        proc.name = "42"
        proc.stat.return_value = SimpleNamespace(st_uid=1000)
        (proc / "exe").stat.return_value = metadata
        target = SimpleNamespace(st_dev=metadata.st_dev, st_ino=metadata.st_ino + 1)
        with patch.object(guard.Path, "iterdir", return_value=[proc]), \
             patch.object(guard, "proc_fields", return_value=(b"S", b"123")), \
             patch.object(guard.os, "open", return_value=999999), patch.object(guard.os, "fstat", return_value=metadata):
            self.assertEqual(guard.executable_absence(target), 1)
            self.assertFalse(legacy.UNCERTAIN)

    def test_before_baseline_is_fsynced_and_failed_main_never_retries(self):
        text = (HERE / "root_vm_guard.py").read_text()
        self.assertLess(text.index('evidence.write("baseline-before.json", before)'), text.index('output = execute_matrix'))
        self.assertIn("os.fsync(self.directory.fd)", text)
        for forbidden in ("os.kill(", "os.killpg(", "subprocess.run(", "runuser", "preexec_fn=", ".communicate(", "os.unlink("):
            self.assertNotIn(forbidden, text)

    def test_harmless_local_fd_exec_resolves_actual_original_executable(self):
        # This is not the fixture and never changes UID/GID/groups or invokes root.
        path = Path("/usr/bin/readlink").resolve()
        fd = os.open(path, os.O_RDONLY | os.O_CLOEXEC)
        try:
            done = subprocess.run([str(path), "/proc/self/exe"], executable=f"/proc/self/fd/{fd}",
                                  pass_fds=(fd,), close_fds=True, capture_output=True, check=True, timeout=5)
            self.assertEqual(Path(done.stdout.decode().strip()), path)
        finally:
            os.close(fd)


if __name__ == "__main__":
    unittest.main()
