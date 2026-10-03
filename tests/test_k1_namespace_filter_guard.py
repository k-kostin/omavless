"""Offline tests of preservation and uncertain-child refusal; no VM/root calls."""
import importlib.util
import hashlib
import os
import tempfile
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("namespace_filter_guard",
    ROOT / "crates/omavless-netguard/tests/support/namespace_filter_guest_guard.py")
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class NamespaceFilterGuardTests(unittest.TestCase):
    def setUp(self):
        guard.UNCERTAIN = False
        guard.RETAINED.clear()

    def test_guard_pins_the_exact_reviewed_shell(self):
        runner = ROOT / "crates/omavless-netguard/tests/support/namespace_filter_vm_fixture.sh"
        self.assertEqual(hashlib.sha256(runner.read_bytes()).hexdigest(), guard.RUNNER_SHA)

    def test_only_real_address_countdowns_are_normalized(self):
        before = {"address": [{"addr_info": [{"valid_life_time": 70}]}]}
        after = {"address": [{"addr_info": [{"valid_life_time": 69}]}]}
        self.assertTrue(guard.network_equal(before, after))
        for bad in [True, -1, 71, "69"]:
            self.assertFalse(guard.network_equal(before,
                {"address": [{"addr_info": [{"valid_life_time": bad}]}]}))
        self.assertFalse(guard.network_equal({"route": [{"valid_life_time": 70}]},
                                            {"route": [{"valid_life_time": 69}]}))
        self.assertFalse(guard.preserve({"network": {}, "package": "old"},
                                        {"network": {}, "package": "new"}))

    def test_actual_exit_code_is_required(self):
        for code in [0, 2]:
            child = SimpleNamespace(pid=123, returncode=None)
            seen = SimpleNamespace(si_pid=123, si_code=os.CLD_EXITED, si_status=code)
            with patch.object(guard.os, "waitid", return_value=seen), \
                 patch.object(guard.os, "waitpid", return_value=(123, code << 8)):
                self.assertEqual(guard.await_child(child), code)
                self.assertEqual(child.returncode, code)

    def test_waitid_uncertainty_never_retries_reaps_or_signals(self):
        child = SimpleNamespace(pid=123, returncode=None)
        with patch.object(guard.os, "waitid", side_effect=ChildProcessError()) as observe, \
             patch.object(guard.os, "waitpid") as reap, \
             patch.object(guard.os, "kill") as signal:
            with self.assertRaises(guard.Refused):
                guard.await_child(child)
            self.assertEqual(observe.call_count, 1)
            reap.assert_not_called()
            signal.assert_not_called()
            self.assertIsNone(child.returncode)

    def test_uncertain_final_reap_is_terminal(self):
        seen = SimpleNamespace(si_pid=123, si_code=os.CLD_EXITED, si_status=0)
        for result in [(0, 0), (124, 0), (123, 0x7f), (123, 2 << 8), ChildProcessError()]:
            child = SimpleNamespace(pid=123, returncode=None)
            with patch.object(guard.os, "waitid", return_value=seen) as observe, \
                 patch.object(guard.os, "waitpid") as reap:
                if isinstance(result, Exception):
                    reap.side_effect = result
                else:
                    reap.return_value = result
                guard.UNCERTAIN = False
                guard.RETAINED.clear()
                with self.assertRaises(guard.Refused):
                    guard.await_child(child)
                self.assertEqual(observe.call_count, 1)
                self.assertEqual(reap.call_count, 1)
                self.assertIsNone(child.returncode)

    def test_timeout_never_signals_or_reaps(self):
        with patch.object(guard.time, "monotonic", side_effect=[0, 46]), \
             patch.object(guard.os, "waitid") as observe, \
             patch.object(guard.os, "waitpid") as reap, \
             patch.object(guard.os, "kill") as signal:
            with self.assertRaises(guard.Refused):
                guard.await_child(SimpleNamespace(pid=123, returncode=None))
            observe.assert_not_called()
            reap.assert_not_called()
            signal.assert_not_called()

    def test_later_success_cannot_repair_uncertainty_or_start_another_command(self):
        child = SimpleNamespace(pid=123, returncode=None)
        later = SimpleNamespace(si_pid=123, si_code=os.CLD_EXITED, si_status=0)
        with patch.object(guard.os, "waitid", side_effect=[OSError(), later]) as observe, \
             patch.object(guard.os, "waitpid") as reap, \
             patch.object(guard, "OwnedProcess") as process, \
             patch.object(guard.tempfile, "TemporaryFile") as capture:
            for operation in [lambda: guard.await_child(child), lambda: guard.await_child(child),
                              lambda: guard.command(["/usr/bin/true"]),
                              lambda: guard.spawn(["/usr/bin/true"]), guard.snapshot]:
                with self.assertRaises(guard.Refused):
                    operation()
            self.assertEqual(observe.call_count, 1)
            reap.assert_not_called()
            process.assert_not_called()
            capture.assert_not_called()
            self.assertTrue(guard.UNCERTAIN)
            self.assertEqual(guard.RETAINED, [child])

    def test_owned_destructor_and_internal_poll_make_no_kernel_query(self):
        child = object.__new__(guard.OwnedProcess)
        child.returncode = None
        with patch.object(guard.os, "waitpid") as reap, \
             patch.object(guard.os, "waitid") as observe:
            child.__del__()
            self.assertIsNone(child._internal_poll())
            with self.assertRaises(guard.Refused):
                child.wait()
            with self.assertRaises(guard.Refused):
                child.poll()
            reap.assert_not_called()
            observe.assert_not_called()

    def test_cancellation_latches_without_later_query_or_signal(self):
        child = SimpleNamespace(pid=123, returncode=None)
        later = SimpleNamespace(si_pid=123, si_code=os.CLD_EXITED, si_status=0)
        with patch.object(guard.os, "waitid", side_effect=[None, later]) as observe, \
             patch.object(guard.time, "sleep", side_effect=KeyboardInterrupt()), \
             patch.object(guard.os, "waitpid") as reap, patch.object(guard.os, "kill") as signal:
            with self.assertRaises(guard.Refused):
                guard.await_child(child)
            with self.assertRaises(guard.Refused):
                guard.await_child(child)
            self.assertEqual(observe.call_count, 1)
            reap.assert_not_called()
            signal.assert_not_called()

    def test_public_command_capture_uses_actual_exit_and_private_file(self):
        with tempfile.TemporaryDirectory(prefix="ov-filter-command-") as temp, \
             patch.object(guard, "STAGE", Path(temp)):
            self.assertEqual(guard.command(["/usr/bin/printf", "fixture"]), b"fixture")
            with self.assertRaises(guard.Refused):
                guard.command(["/usr/bin/false"])
            self.assertFalse(guard.UNCERTAIN)  # actual nonzero exit, not uncertainty
            self.assertEqual(list(Path(temp).iterdir()), [])


if __name__ == "__main__":
    unittest.main()
