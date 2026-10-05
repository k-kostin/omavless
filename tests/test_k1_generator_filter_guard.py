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
spec = importlib.util.spec_from_file_location("generator_filter_guard",
    ROOT / "crates/omavless-netguard/tests/support/generator_filter_guest_guard.py")
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class NamespaceFilterGuardTests(unittest.TestCase):
    def setUp(self):
        guard.UNCERTAIN = False
        guard.RETAINED.clear()

    def test_guard_pins_the_exact_reviewed_shell(self):
        runner = ROOT / "crates/omavless-netguard/tests/support/generator_filter_vm_fixture.sh"
        self.assertEqual(hashlib.sha256(runner.read_bytes()).hexdigest(), guard.RUNNER_SHA)

    def test_runner_failure_stops_before_any_next_snapshot(self):
        source = (ROOT / "crates/omavless-netguard/tests/support/generator_filter_guest_guard.py").read_text()
        after = source.split("code = await_child(child)", 1)[1]
        self.assertLess(after.index("require(code == 0)"), after.index("after = snapshot()"))
        self.assertIn("query_guard_unchanged", source)

    def test_actual_outer_main_failure_never_queries_after_runner_exit(self):
        with tempfile.TemporaryDirectory(prefix="ov-typed-outer-") as temp, \
             patch.object(guard, "STAGE", Path(temp)), \
             patch.object(guard.Path, "lstat", return_value=SimpleNamespace(
                 st_mode=0o040700, st_uid=0, st_gid=0)), \
             patch.object(guard.os, "geteuid", return_value=0), \
             patch.dict(guard.os.environ, {"OMAVLESS_K1_NAMESPACE_FILTER_GUARD": "1"}), \
             patch.object(guard.sys, "argv", ["fixed-fixture"]), \
             patch.object(guard, "LINK", SimpleNamespace(exists=lambda: False, is_symlink=lambda: False)), \
             patch.object(guard, "CGROUP", SimpleNamespace(exists=lambda: False)), \
             patch.object(guard, "pinned_file", return_value=(1, 2)), \
             patch.object(guard, "command", return_value=b"kvm\n") as command, \
             patch.object(guard, "snapshot", return_value={"synthetic": True}) as snapshot, \
             patch.object(guard, "spawn") as spawn, \
             patch.object(guard, "await_child", return_value=2):
            with self.assertRaises(guard.Refused):
                guard.main()
            snapshot.assert_called_once()
            command.assert_called_once_with(["/usr/bin/systemd-detect-virt", "--vm"])
            spawn.assert_called_once()
            self.assertTrue((Path(temp) / "runner.log").exists())
            self.assertFalse((Path(temp) / "result.json").exists())

    def test_target_scope_adds_only_already_inventoried_user_generators(self):
        old = {"/usr/lib/systemd", "/etc/systemd", "/run/systemd", "/home/kdk_vm/.config/systemd"}
        added = set(guard.TARGET_ROOTS) - old
        self.assertEqual(added, {"/run/user/1000/systemd/" + name
                                for name in ("generator", "generator.early", "generator.late")})
        self.assertTrue(added.issubset(guard.ACTIVATION_ROOTS))
        self.assertEqual(set(guard.TARGET_ROOTS) & {"/run", "/run/user", "/run/user/1000", "/home"}, set())

    def test_actual_generator_links_and_resolved_escape_refusals(self):
        for name in ("generator", "generator.early", "generator.late"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temp:
                base = Path(temp)
                root = base / name
                root.mkdir()
                (root / "target.service").write_bytes(b"synthetic activation fixture")
                (root / "link.service").symlink_to("target.service")
                with patch.object(guard, "ACTIVATION_ROOTS", (str(root),)), \
                     patch.object(guard, "TARGET_ROOTS", (str(root),)):
                    accepted = guard.inventory()
                    self.assertIn(str(root / "target.service"), accepted)
                    (base / "outside.service").write_bytes(b"outside never admitted")
                    for raw in ("../outside.service", str(base / "outside.service")):
                        (root / "link.service").unlink()
                        (root / "link.service").symlink_to(raw)
                        with self.assertRaises(guard.Refused):
                            guard.inventory()
                    (root / "chain.service").symlink_to("../outside.service")
                    (root / "link.service").unlink()
                    (root / "link.service").symlink_to("chain.service")
                    with self.assertRaises(guard.Refused):
                        guard.inventory()
                    (root / "link.service").unlink()
                    (root / "chain.service").unlink()
                    (root / "link.service").symlink_to("absent.service")
                    with self.assertRaises(FileNotFoundError):
                        guard.inventory()

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
