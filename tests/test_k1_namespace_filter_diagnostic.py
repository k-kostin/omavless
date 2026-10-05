"""Pinned-original snapshot diagnostics stay read-only and credential-free."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SUPPORT = ROOT / "crates/omavless-netguard/tests/support"
spec = importlib.util.spec_from_file_location("snapshot_diagnostic",
    SUPPORT / "namespace_filter_snapshot_diagnostic.py")
diag = importlib.util.module_from_spec(spec)
spec.loader.exec_module(diag)


class DiagnosticTests(unittest.TestCase):
    def original(self):
        # Loading only definitions cannot invoke original main, unit or snapshot.
        return diag.load_original((SUPPORT / "namespace_filter_guest_guard.py").read_bytes())

    def fixture(self):
        guard = self.original()
        calls = []
        def command(args):
            calls.append(tuple(args))
            return b"kvm\n" if args[0] == "/usr/bin/systemd-detect-virt" else b""
        guard["command"] = command
        guard["pinned_file"] = lambda *args: None
        return guard, calls

    def test_original_bytes_must_match_before_import(self):
        original = (SUPPORT / "namespace_filter_guest_guard.py").read_bytes()
        with self.assertRaises(diag.DiagnosticRefused):
            diag.load_original(original + b"\n")
        self.assertFalse(self.original()["UNCERTAIN"])

    def test_snapshot_contents_are_discarded(self):
        guard, calls = self.fixture()
        guard["snapshot"] = lambda: {"private": "PRIVATE_SENTINEL"}
        result = diag.diagnose(guard)
        self.assertTrue(result["snapshot_complete"])
        self.assertFalse(result["runner_invoked"])
        self.assertNotIn("PRIVATE_SENTINEL", json.dumps(result))
        self.assertEqual(len(calls), 1)

    def test_original_activation_target_refusal_has_public_phase_only(self):
        guard, calls = self.fixture()
        with tempfile.TemporaryDirectory(prefix="ov-diagnostic-") as temp:
            root = Path(temp)
            (root / "outside").write_text("PRIVATE_TARGET_SENTINEL")
            (root / "activation").mkdir()
            (root / "activation/link").symlink_to(root / "outside")
            guard["ACTIVATION_ROOTS"] = [str(root / "activation")]
            guard["snapshot"] = guard["inventory"]
            result = diag.diagnose(guard)
            self.assertEqual(result["phase"], "activation_target_outside_allowlist")
            self.assertEqual(result["category"], "refused")
            self.assertEqual(result["original_source_line"], 148)
            self.assertNotIn(temp, json.dumps(result))
            self.assertNotIn("PRIVATE_TARGET_SENTINEL", json.dumps(result))

    def test_any_mutating_or_unknown_command_is_refused_before_dispatch(self):
        for command in [["/usr/bin/systemctl", "start", "private-unit-sentinel"],
                        ["/usr/bin/systemctl", "daemon-reload"],
                        ["/usr/bin/bash", "/run/omavless-k1-namespace-filter-fixture/runner.sh"],
                        ["/usr/bin/sh", "-c", "PRIVATE_ARGV_SENTINEL"]]:
            guard, calls = self.fixture()
            guard["snapshot"] = lambda: guard["command"](command)
            result = diag.diagnose(guard)
            self.assertFalse(result["snapshot_complete"])
            self.assertEqual(len(calls), 1)
            self.assertNotIn("SENTINEL", json.dumps(result))
            self.assertNotIn("private-unit", json.dumps(result))

    def test_unknown_latch_never_runs_second_snapshot_or_command(self):
        guard, calls = self.fixture()
        count = []
        def uncertain_snapshot():
            count.append(1)
            guard["UNCERTAIN"] = True
            raise guard["Refused"]()
        guard["snapshot"] = uncertain_snapshot
        first = diag.diagnose(guard)
        second = diag.diagnose(guard)
        self.assertEqual(first["category"], "process_uncertain")
        self.assertEqual(second["category"], "process_uncertain")
        self.assertEqual(count, [1])
        self.assertEqual(len(calls), 1)

    def test_exception_message_and_filename_never_reach_receipt(self):
        guard, calls = self.fixture()
        def fail():
            raise FileNotFoundError(2, "PRIVATE_MESSAGE_SENTINEL", "PRIVATE_PATH_SENTINEL")
        guard["snapshot"] = fail
        result = diag.diagnose(guard)
        self.assertEqual(result["category"], "missing")
        self.assertNotIn("PRIVATE_", json.dumps(result))


if __name__ == "__main__":
    unittest.main()
