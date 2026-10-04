"""Pure fixed static capture orchestration, never a VM or readelf execution."""
import hashlib
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).parent / "static_elf_provenance"
SPEC = importlib.util.spec_from_file_location("static_capture_supervisor", ROOT / "static_capture_supervisor.py")
supervisor = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(supervisor)
assert supervisor.base is None
supervisor.base = supervisor.load_containment(ROOT.parent / "real_resolved_binary/probe.py")


class StaticSupervisorTests(unittest.TestCase):
    def test_exact_source_eligibility_and_private_create_only_wrapper(self):
        guard = (ROOT / "vm-guard-empty-record.sh").read_text()
        for name in ("probe.py", "static_capture_supervisor.py"):
            self.assertIn(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), guard)
        self.assertEqual(hashlib.sha256((ROOT / "probe.py").read_bytes()).hexdigest(), supervisor.PROBE_SHA)
        for digest in (supervisor.CONTAINMENT_SHA, supervisor.INVENTORY_SHA,
                       "a72f12f3dd8c560554a3ba818bcecbda9178db5befbcc64d91bbb6a241fc21bc"):
            self.assertIn(digest, guard)
        for marker in ("set -o noclobber", "umask 077", 'test ! -L "$task_stage"', "stat -c %u:%g"):
            self.assertIn(marker, guard)
        self.assertNotIn("timeout --", guard)

    def test_missing_helper_has_no_repository_fallback(self):
        path = supervisor.STAGE / "containment.py"
        with patch.object(supervisor.os, "open", side_effect=FileNotFoundError) as opened:
            with self.assertRaises(FileNotFoundError):
                supervisor.load_containment(path)
        opened.assert_called_once()
        self.assertEqual(opened.call_args.args[0], path)

    def test_fixed_child_known_nonzero_unknown_and_timeout_are_terminal(self):
        for status in ("success", "nonzero", "unknown", "timeout"):
            with self.subTest(status=status), tempfile.TemporaryDirectory(dir="/var/tmp") as temporary:
                stage = Path(temporary)
                child = Mock(returncode=0 if status == "success" else 1 if status == "nonzero" else None)
                def observed(process, seconds):
                    self.assertIs(process, child)
                    self.assertEqual(seconds, 75)
                    if status == "unknown":
                        raise supervisor.base.Refused("owned_wait_unknown_preserve")
                    return status != "timeout"
                with patch.object(supervisor, "STAGE", stage), \
                     patch.object(supervisor.os, "getuid", return_value=1000), \
                     patch.object(supervisor.os, "geteuid", return_value=1000), \
                     patch.object(supervisor.base, "private_parent"), \
                     patch.object(supervisor.base, "object_bytes") as inputs, \
                     patch.object(supervisor.base, "OwnedProcess", return_value=child) as spawn, \
                     patch.object(supervisor.base, "supervise", side_effect=observed) as observe:
                    result = supervisor.run_child(stage)
                self.assertEqual(inputs.call_args_list[0].args, (stage / "probe.py", supervisor.PROBE_SHA, 32768))
                self.assertEqual(inputs.call_args_list[1].args, (stage / "guest-inventory.json", supervisor.INVENTORY_SHA, 32768))
                spawn.assert_called_once()
                self.assertEqual(spawn.call_args.args[0], ["/usr/bin/python3", str(stage / "probe.py"), "--capture-static-public-provenance"])
                self.assertTrue(spawn.call_args.kwargs["start_new_session"])
                observe.assert_called_once()
                self.assertEqual(child.mock_calls, [])
                self.assertEqual(result["outcome"], "KNOWN_COMPLETED" if status == "success" else "NONPASS")
                if status in ("unknown", "timeout"):
                    self.assertNotIn("returncode", result)

    def test_unpinned_input_or_existing_output_never_launches(self):
        for failure in ("pin", "existing"):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory(dir="/var/tmp") as temporary:
                stage = Path(temporary)
                if failure == "existing":
                    (stage / "result.json").write_bytes(b"retained")
                with patch.object(supervisor, "STAGE", stage), \
                     patch.object(supervisor.os, "getuid", return_value=1000), \
                     patch.object(supervisor.os, "geteuid", return_value=1000), \
                     patch.object(supervisor.base, "private_parent"), \
                     patch.object(supervisor.base, "object_bytes", side_effect=supervisor.base.Refused() if failure == "pin" else None), \
                     patch.object(supervisor.base, "OwnedProcess") as spawn:
                    self.assertEqual(supervisor.run_child(stage)["outcome"], "NONPASS")
                spawn.assert_not_called()
                if failure == "existing":
                    self.assertEqual((stage / "result.json").read_bytes(), b"retained")

    def test_actual_wrapper_failed_supervisor_branch_has_no_later_action(self):
        guard = (ROOT / "vm-guard-empty-record.sh").read_text()
        block = guard.split('if ! env -i ', 1)[1].split('\nfi\n', 1)[0]
        with tempfile.TemporaryDirectory(dir="/var/tmp") as temporary:
            script = 'env() { return 1; }; task_stage="$1"\nif ! env -i ' + block + '\nfi\nprintf FORBIDDEN_LATER_ACTION\n'
            result = subprocess.run(["/bin/bash", "-c", script, "test", temporary],
                                    check=False, capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "STATIC_ELF_DIAGNOSTIC_NONPASS\n")


if __name__ == "__main__":
    unittest.main()
