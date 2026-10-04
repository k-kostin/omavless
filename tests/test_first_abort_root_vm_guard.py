# SPDX-License-Identifier: MIT
"""Pure/local-only root guard tests: no VM, sudo, matrix or credential changes."""
import hashlib
import importlib.util
import json
from contextlib import ExitStack, contextmanager
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
stage = load("stage_abort_guard", "stage_root_guard.py")


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

    @contextmanager
    def main_harness(self):
        """Actual main sequencing, simulated already-admitted private objects."""
        nonce = "a" * 32
        path = Path("/run/ov-abort-root-" + nonce) / "root_vm_guard.py"
        value = {"guard_sha256": "b" * 64, "elf_sha256": "c" * 64,
                 "host_frozen": {"size": 8}, "head": "d" * 40, "host_build_target": "/host/build"}
        evidence = SimpleNamespace(write=Mock(), create=Mock(return_value=999999),
                                   directory=SimpleNamespace(fd=999998))
        real_write = guard.Evidence.write
        def held(parent, name, maximum, mode, uid, expected=None):
            return SimpleNamespace(bytes=lambda: b"" if name == "vm_guard.py" else b"{}", recheck=Mock(),
                before=SimpleNamespace(st_size=8, st_dev=17, st_ino=29), fd=57,
                path=parent.path / name, sha="c" * 64)
        before = {"constant": True, "network": {}}
        with ExitStack() as stack:
            stack.enter_context(patch.object(guard, "__file__", str(path)))
            stack.enter_context(patch.object(guard, "sys", SimpleNamespace(
                argv=[str(path), "--run-reviewed-root-observed-abort", "e" * 64],
                flags=SimpleNamespace(isolated=1), dont_write_bytecode=True)))
            stack.enter_context(patch.object(guard.os, "getresuid", return_value=(0, 0, 0)))
            stack.enter_context(patch.object(guard.os, "getresgid", return_value=(0, 0, 0)))
            stack.enter_context(patch.object(guard, "Directory", side_effect=lambda path, uid: SimpleNamespace(path=path, recheck=Mock())))
            stack.enter_context(patch.object(guard, "HeldFile", side_effect=held))
            stack.enter_context(patch.object(guard.types, "ModuleType", return_value=legacy))
            stack.enter_context(patch.object(legacy, "copy_receipt", return_value=value))
            stack.enter_context(patch.object(legacy, "command"))  # Restore main's assignment on exit.
            stack.enter_context(patch.object(guard, "Evidence", return_value=evidence))
            stack.enter_context(patch.object(guard.os, "pread", return_value=b"\x7fELF"))
            stack.enter_context(patch.object(guard, "command", return_value=(0, b"kvm\n")))
            snapshot = stack.enter_context(patch.object(legacy, "snapshot", side_effect=[before, before]))
            matrix = stack.enter_context(patch.object(guard, "execute_matrix", return_value=""))
            cases = stack.enter_context(patch.object(guard, "inspect_cases"))
            scan = stack.enter_context(patch.object(guard, "executable_absence", return_value=44))
            printed = stack.enter_context(patch("builtins.print"))
            yield SimpleNamespace(evidence=evidence, real_write=real_write, snapshot=snapshot,
                                  matrix=matrix, cases=cases, scan=scan, printed=printed, before=before)

    def test_main_before_baseline_write_or_fsync_failure_never_starts_matrix(self):
        for failure in ("write", "fsync"):
            with self.subTest(failure=failure), self.main_harness() as h:
                if failure == "write":
                    h.evidence.write.side_effect = OSError("synthetic write failure")
                    with self.assertRaises(OSError):
                        guard.main()
                else:
                    h.evidence.write.side_effect = lambda name, value: h.real_write(h.evidence, name, value)
                    with patch.object(guard.os, "write", side_effect=lambda fd, data: len(data)), \
                         patch.object(guard.os, "fsync", side_effect=OSError("synthetic fsync failure")) as sync:
                        with self.assertRaises(OSError):
                            guard.main()
                        self.assertEqual(sync.call_count, 1)
                h.matrix.assert_not_called()
                h.cases.assert_not_called()
                h.scan.assert_not_called()
                h.printed.assert_not_called()
                self.assertEqual(h.snapshot.call_count, 1)
                self.assertEqual(h.evidence.write.call_count, 1)

    def test_main_post_matrix_failures_never_emit_pass_result_or_retry(self):
        for failure in ("cases", "scan", "after-write", "comparison"):
            with self.subTest(failure=failure), self.main_harness() as h:
                if failure == "cases":
                    h.cases.side_effect = guard.Refused()
                elif failure == "scan":
                    h.scan.side_effect = legacy.Refused()
                elif failure == "after-write":
                    def write(name, value):
                        if name == "baseline-after.json":
                            raise OSError("synthetic after-baseline failure")
                    h.evidence.write.side_effect = write
                else:
                    h.snapshot.side_effect = [h.before, {"constant": False, "network": {}}]
                with self.assertRaises((guard.Refused, legacy.Refused, OSError)):
                    guard.main()
                self.assertEqual(h.matrix.call_count, 1)
                self.assertEqual(h.cases.call_count, 1)
                self.assertEqual(h.scan.call_count, 0 if failure == "cases" else 1)
                self.assertEqual(h.snapshot.call_count, 1 if failure in ("cases", "scan") else 2)
                self.assertNotIn("result.json", [call.args[0] for call in h.evidence.write.call_args_list])
                h.printed.assert_not_called()

    def test_main_success_control_requires_both_baselines_cases_and_observation(self):
        with self.main_harness() as h:
            guard.main()
            self.assertEqual((h.matrix.call_count, h.cases.call_count, h.scan.call_count, h.snapshot.call_count), (1, 1, 1, 2))
            self.assertEqual([call.args[0] for call in h.evidence.write.call_args_list],
                             ["baseline-before.json", "baseline-after.json", "result.json"])
            h.printed.assert_called_once_with("T4_ROOT_OBSERVED_PROCESS_LOSS_NOT_POWERLOSS_OR_PRODUCT_PASS")

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


class StageGuards(unittest.TestCase):
    def tearDown(self):
        for fd in stage.RETAINED:
            try:
                os.close(fd)
            except OSError:
                pass
        stage.RETAINED.clear()

    def test_loader_is_stdin_only_and_unprivileged_admission_has_no_filesystem_effect(self):
        with patch.object(stage.os, "getresuid", return_value=(1000, 1000, 1000)), \
             patch.object(stage, "directory") as directory, patch.object(stage.os, "mkdir") as mkdir:
            with self.assertRaises(RuntimeError):
                stage.main()
            directory.assert_not_called()
            mkdir.assert_not_called()
        with patch.object(stage.os, "getresuid", return_value=(0, 0, 0)), \
             patch.object(stage.os, "getresgid", return_value=(0, 0, 0)), \
             patch.object(stage, "directory") as directory:
            with self.assertRaises(RuntimeError):
                stage.main()  # __file__ is a pathname, not trusted reviewed stdin.
            directory.assert_not_called()

    def test_loader_original_source_fd_rejects_same_byte_replacement(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
            stage.RETAINED.append(fd)
            path = root / "root_vm_guard.py"
            path.write_bytes(b"reviewed synthetic source")
            path.chmod(0o500)
            source = stage.Source(fd, path.name, 4096, 0o500, uid=os.getuid())
            replacement = root / "replacement"
            replacement.write_bytes(source.data)
            replacement.chmod(0o500)
            replacement.replace(path)
            with self.assertRaises(RuntimeError):
                source.recheck()

    def test_loader_fixed_file_set_rejects_arbitrary_names_before_open(self):
        with patch.object(stage.os, "open") as opened:
            for name in ("../fixture", "/etc/passwd", "other.py"):
                with self.assertRaises(RuntimeError):
                    stage.Source(42, name, 4096, 0o500)
            opened.assert_not_called()

    def test_loader_wrong_mode_owner_or_xattr_refuses_before_reading_bytes(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
            stage.RETAINED.append(fd)
            path = root / "root_vm_guard.py"
            path.write_bytes(b"reviewed synthetic source")
            for fault in ("mode", "owner", "xattr"):
                path.chmod(0o700 if fault == "mode" else 0o500)
                uid = os.getuid() + 1 if fault == "owner" else os.getuid()
                with patch.object(stage.os, "listxattr", return_value=["user.synthetic"] if fault == "xattr" else []), \
                     patch.object(stage.os, "pread") as read:
                    with self.assertRaises(RuntimeError):
                        stage.Source(fd, path.name, 4096, 0o500, uid=uid)
                    read.assert_not_called()

    def test_loader_copy_refuses_elf_or_wrong_output_mode_before_creation(self):
        with patch.object(stage.os, "open") as opened:
            for name, mode in (("fixture", 0o500), ("receipt.json", 0o500), ("root_vm_guard.py", 0o700)):
                with self.assertRaises(RuntimeError):
                    stage.copy_new(42, SimpleNamespace(name=name), mode)
            opened.assert_not_called()

    def test_loader_receipt_duplicates_and_invalid_hashes_refuse(self):
        with self.assertRaises(RuntimeError):
            stage.receipt_hashes(b'{"schema":1,"schema":1}')
        value = {"schema": "t4-abort-vm-copy-v1", "head": "a" * 40, "elf_sha256": "b" * 64,
                 "guard_sha256": "c" * 64, "host_original": {}, "host_frozen": {}, "host_build_target": "/host/build"}
        self.assertEqual(stage.receipt_hashes(json.dumps(value)), ("b" * 64, "c" * 64))
        value["guard_sha256"] = "not-a-hash"
        with self.assertRaises(RuntimeError):
            stage.receipt_hashes(json.dumps(value))

    @contextmanager
    def main_harness(self, bad_hash=False):
        value = {"schema": "t4-abort-vm-copy-v1", "head": "a" * 40, "elf_sha256": "b" * 64,
                 "guard_sha256": "c" * 64, "host_original": {}, "host_frozen": {}, "host_build_target": "/host/build"}
        receipt = json.dumps(value).encode()
        digest = hashlib.sha256(receipt).hexdigest()
        hashes = {"receipt.json": digest, "vm_guard.py": stage.LEGACY_SHA,
                  "root_vm_guard.py": "c" * 64, "fixture": "b" * 64}
        if bad_hash:
            hashes[bad_hash] = "d" * 64
        with ExitStack() as stack:
            stack.enter_context(patch.object(stage, "__file__", "<stdin>"))
            stack.enter_context(patch.object(stage, "sys", SimpleNamespace(
                argv=["-", "--stage-reviewed-root-observed-abort", "a" * 32, digest],
                flags=SimpleNamespace(isolated=1), dont_write_bytecode=True)))
            stack.enter_context(patch.object(stage.os, "getresuid", return_value=(0, 0, 0)))
            stack.enter_context(patch.object(stage.os, "getresgid", return_value=(0, 0, 0)))
            stack.enter_context(patch.object(stage, "directory", return_value=(42, None)))
            stack.enter_context(patch.object(stage, "recheck_directory"))
            stack.enter_context(patch.object(stage, "Source", side_effect=lambda fd, name, *args, **kwargs:
                SimpleNamespace(name=name, sha=hashes[name], data=receipt, fd=57, recheck=Mock())))
            stack.enter_context(patch.object(stage.os, "pread", return_value=b"\x7fELF"))
            mkdir = stack.enter_context(patch.object(stage.os, "mkdir"))
            copy = stack.enter_context(patch.object(stage, "copy_new"))
            printed = stack.enter_context(patch("builtins.print"))
            yield SimpleNamespace(mkdir=mkdir, copy=copy, printed=printed)

    def test_loader_hash_mismatch_has_no_privileged_create(self):
        for name in ("receipt.json", "vm_guard.py", "root_vm_guard.py", "fixture"):
            with self.subTest(name=name), self.main_harness(bad_hash=name) as h:
                with self.assertRaises(RuntimeError):
                    stage.main()
                h.mkdir.assert_not_called()
                h.copy.assert_not_called()
                h.printed.assert_not_called()

    def test_loader_existing_stage_and_partial_copy_failure_never_retry_or_exec(self):
        for failure in ("exists", "copy"):
            with self.main_harness() as h:
                if failure == "exists":
                    h.mkdir.side_effect = FileExistsError()
                else:
                    h.copy.side_effect = OSError("synthetic partial copy")
                with self.assertRaises(OSError):
                    stage.main()
                self.assertEqual(h.mkdir.call_count, 1)
                self.assertEqual(h.copy.call_count, 0 if failure == "exists" else 1)
                h.printed.assert_not_called()
        source = (HERE / "stage_root_guard.py").read_text()
        for forbidden in ("subprocess", "exec(", "os.system", "os.unlink", "os.remove", "os.chown", "runuser"):
            self.assertNotIn(forbidden, source.replace("no subprocess", "no child"))


if __name__ == "__main__":
    unittest.main()
