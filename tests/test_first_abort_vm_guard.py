# SPDX-License-Identifier: MIT
"""Pure/local synthetic guards; never runs VM commands or the process matrix."""
import importlib.util
import hashlib
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("abort_vm_guard", Path(__file__).parent / "first_abort_process/vm_guard.py")
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


class Guards(unittest.TestCase):
    def setUp(self):
        guard.UNCERTAIN = False
        guard.RETAINED.clear()

    def tearDown(self):
        guard.UNCERTAIN = False
        guard.RETAINED.clear()

    def test_unknown_wait_never_retries_reaps_or_spawns(self):
        for error in (ChildProcessError(), InterruptedError(), OSError()):
            guard.UNCERTAIN = False
            child = SimpleNamespace(pid=42, returncode=None)
            with patch.object(guard.os, "waitid", side_effect=error) as observe, \
                 patch.object(guard.os, "waitpid") as reap, \
                 patch.object(guard, "OwnedProcess") as spawn, \
                 patch.object(guard.tempfile, "TemporaryFile") as files:
                with self.assertRaises(guard.Refused):
                    guard.await_exact(child, 1)
                for call in (lambda: guard.await_exact(child, 1), lambda: guard.spawn(["never"]),
                             lambda: guard.command("unused", ["never"])):
                    with self.assertRaises(guard.Refused):
                        call()
                self.assertEqual(observe.call_count, 1)
                reap.assert_not_called()
                spawn.assert_not_called()
                files.assert_not_called()

    def test_exact_reap_mismatch_permanently_quarantines(self):
        seen = SimpleNamespace(si_pid=42, si_code=os.CLD_EXITED, si_status=7)
        for result in (ChildProcessError(), InterruptedError(), (0, 0), (41, 7 << 8), (42, 0), (42, 0x7f)):
            guard.UNCERTAIN = False
            child = SimpleNamespace(pid=42, returncode=None)
            with patch.object(guard.os, "waitid", return_value=seen) as observe, \
                 patch.object(guard.os, "waitpid", side_effect=[result]) as reap:
                with self.assertRaises(guard.Refused):
                    guard.await_exact(child, 1)
                with self.assertRaises(guard.Refused):
                    guard.await_exact(child, 1)
                self.assertEqual((observe.call_count, reap.call_count), (1, 1))
                self.assertIsNone(child.returncode)

    def test_wrong_observation_shape_never_reaps(self):
        for seen in (SimpleNamespace(si_pid=41, si_code=os.CLD_EXITED, si_status=0),
                     SimpleNamespace(si_pid=42, si_code=os.CLD_STOPPED, si_status=19),
                     SimpleNamespace(si_pid=42, si_code=os.CLD_KILLED, si_status=0)):
            guard.UNCERTAIN = False
            with patch.object(guard.os, "waitid", return_value=seen), patch.object(guard.os, "waitpid") as reap:
                with self.assertRaises(guard.Refused):
                    guard.await_exact(SimpleNamespace(pid=42, returncode=None), 1)
                reap.assert_not_called()
                self.assertTrue(guard.UNCERTAIN)

    def test_exact_exit_and_timeout_are_distinct(self):
        child = SimpleNamespace(pid=42, returncode=None)
        seen = SimpleNamespace(si_pid=42, si_code=os.CLD_EXITED, si_status=7)
        with patch.object(guard.os, "waitid", return_value=seen), patch.object(guard.os, "waitpid", return_value=(42, 7 << 8)):
            self.assertEqual(guard.await_exact(child, 1), 7)
            self.assertEqual(child.returncode, 7)
        with patch.object(guard.os, "waitid") as observe, patch.object(guard.os, "waitpid") as reap:
            with self.assertRaises(guard.Refused):
                guard.await_exact(SimpleNamespace(pid=43, returncode=None), 0)
            observe.assert_not_called()
            reap.assert_not_called()
            self.assertTrue(guard.UNCERTAIN)

    def test_already_reaped_pid_is_never_observed_again(self):
        with patch.object(guard.os, "waitid") as observe, patch.object(guard.os, "waitpid") as reap:
            with self.assertRaises(guard.Refused):
                guard.await_exact(SimpleNamespace(pid=42, returncode=0), 1)
            observe.assert_not_called()
            reap.assert_not_called()

    def test_destructor_never_hidden_polls_and_commands_are_fixed_readonly(self):
        child = object.__new__(guard.OwnedProcess)
        with patch.object(guard.OwnedProcess, "_internal_poll") as poll, patch.object(guard.os, "waitpid") as reap:
            child.__del__()
            poll.assert_not_called()
            reap.assert_not_called()
        self.assertEqual(guard.ROOT_UNITS, ("omavless-dns-broker.service", "systemd-resolved.service",
                         "omavless-k1-namespace-filter-fixture.service", "omavless-k1-generator-filter-fixture.service"))
        source = Path(guard.__file__).read_text()
        for forbidden in ("os.kill(", "os.killpg(", "subprocess.run(", ".communicate(", "remove(", "unlink(", "rmtree("):
            self.assertNotIn(forbidden, source)

    def test_pin_keeps_original_fd_and_rejects_same_bytes_new_inode(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            os.chmod(root, 0o700)
            path = root / "fixture"
            path.write_bytes(b"synthetic ELF identity fixture")
            os.chmod(path, 0o500)
            held = guard.Pin(path, 4096, 0o500, hashlib.sha256(path.read_bytes()).hexdigest())
            replacement = root / "replacement"
            replacement.write_bytes(path.read_bytes())
            os.chmod(replacement, 0o500)
            replacement.replace(path)
            with self.assertRaises(guard.Refused):
                held.recheck()

    def test_network_exception_is_only_exact_address_countdown(self):
        before = {"address": [{"addr_info": [{"valid_life_time": 100, "preferred_life_time": 50}]}], "route6": [{"metric": 3}]}
        after = {"address": [{"addr_info": [{"valid_life_time": 99, "preferred_life_time": 49}]}], "route6": [{"metric": 3}]}
        self.assertTrue(guard.network_equal(before, after))
        after["route6"][0]["metric"] = 4
        self.assertFalse(guard.network_equal(before, after))
        self.assertFalse(guard.network_equal({"valid_life_time": 100}, {"valid_life_time": 99}))
        self.assertFalse(guard.network_equal(after, before))

    def test_copy_provenance_is_host_lexical_not_guest_canonicalized(self):
        item = {"path": "/host/build/deps/fixture", "device": 1, "inode": 2, "uid": 1000, "gid": 1000,
                "mode": "0755", "nlink": 1, "size": 8, "sha256": "a" * 64}
        value = {"schema": "t4-abort-vm-copy-v1", "head": "b" * 40, "host_build_target": "/host/build",
                 "host_original": item, "host_frozen": dict(item, path="/host/frozen/fixture", inode=3, mode="0500"),
                 "elf_sha256": "a" * 64, "guard_sha256": "c" * 64}
        with patch.object(guard.os.path, "realpath", side_effect=AssertionError("no invented guest source path")):
            self.assertIs(guard.copy_receipt(value), value)
        value["host_frozen"]["path"] = "/host/build/fake"
        with self.assertRaises(guard.Refused):
            guard.copy_receipt(value)
        for path in ("relative", "/a/../b", "/a//b", "/a/", "/a/./b", "/a\0b"):
            self.assertFalse(guard.normalized(path))


if __name__ == "__main__":
    unittest.main()
