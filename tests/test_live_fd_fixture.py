"""Pure fixed launcher/receipt/wrapper tests; no namespace or guest execution."""
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

from tests.live_fd_tmpfs import probe, validate_receipt as validator

ROOT = Path(__file__).parent / "live_fd_tmpfs"
MANIFEST = (ROOT.parent / "reviewed_tmpfs_elf/copy-manifest.json").read_bytes()


def receipt():
    originals = json.loads(MANIFEST)["source_provenance"]
    copies = {}
    for index, (path, row) in enumerate(sorted(originals.items())):
        copies[path] = dict(device=44, inode=index + 1, size=row["size"], mode=row["mode"],
                            uid=0, gid=0, nlink=1, sha256=row["sha256"],
                            source_device=row["device"], source_inode=row["inode"])
    maps = {}
    for name, executable in (("bus", "/usr/bin/dbus-daemon"),
                              ("resolved", "/usr/lib/systemd/systemd-resolved")):
        paths = sorted((executable, "/usr/lib/libc.so.6", "/usr/lib/ld-linux-x86-64.so.2"))
        maps[name] = [{"path": path, **{k: copies[path][k] for k in ("device", "inode", "size", "sha256")}}
                      for path in paths]
    return dict(schema="live-fd-tmpfs-inventory-v1", outcome="OBSERVED_INVENTORY_ONLY",
                source_sha256=validator.PROBE_SHA, pins=validator.PINS, known_outer_returncode=0,
                broker_executed=False, core_executed=False, dns_mutations=False, compatibility_acceptance=False,
                receipt=dict(initial=maps, final=copy.deepcopy(maps), copies=copies,
                             known_stop_status={"bus": -15, "resolved": 0},
                             live_during_both_passes=True, pid_namespace_and_direct_child_bound=True))


class LiveFdFixtureTests(unittest.TestCase):
    def test_entire_dependency_graph_is_pinned(self):
        self.assertEqual(hashlib.sha256((ROOT / "probe.py").read_bytes()).hexdigest(), validator.PROBE_SHA)
        self.assertEqual(probe.PINS, validator.PINS)
        for name, expected in probe.PINS.items():
            path = ROOT / name
            if name in ("containment.py", "guest-inventory.json"):
                path = ROOT.parent / "real_resolved_binary" / ("probe.py" if name == "containment.py" else name)
            if name in ("admission.py", "copy-manifest.json"):
                path = ROOT.parent / "reviewed_tmpfs_elf" / name
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), expected)
        guard = (ROOT / "vm-guard.sh").read_text()
        for expected in (*probe.PINS.values(), validator.PROBE_SHA,
                         hashlib.sha256((ROOT / "validate_receipt.py").read_bytes()).hexdigest()):
            self.assertIn(expected, guard)
        self.assertNotIn("if not path.exists", (ROOT / "probe.py").read_text())

    def test_fixed_loader_refuses_alternative_directory_before_reads(self):
        with patch.object(probe, "read_input") as read, self.assertRaises(RuntimeError):
            probe.load_inputs(Path("/arbitrary"))
        read.assert_not_called()

    def test_valid_receipt_and_strict_nested_counterexamples(self):
        good = receipt()
        self.assertEqual(validator.validate(json.dumps(good).encode(), MANIFEST), good)
        variants = []
        for field, wrong in (("known_outer_returncode", False), ("pins", {}),
                             ("broker_executed", 0), ("outcome", "PASS")):
            value = copy.deepcopy(good)
            value[field] = wrong
            variants.append(value)
        for field, wrong in (("copies", "x" * 16), ("initial", "x" * 2),
                             ("known_stop_status", {"bus": False, "resolved": 0}),
                             ("pid_namespace_and_direct_child_bound", 1)):
            value = copy.deepcopy(good)
            value["receipt"][field] = wrong
            variants.append(value)
        value = copy.deepcopy(good)
        value["receipt"]["final"]["bus"][0]["inode"] = False
        variants.append(value)
        value = copy.deepcopy(good)
        value["receipt"]["copies"]["/usr/bin/dbus-daemon"]["source_inode"] += 1
        variants.append(value)
        for value in variants:
            with self.assertRaises(ValueError):
                validator.validate(json.dumps(value).encode(), MANIFEST)
        raw = json.dumps(good).encode()
        for malformed in (raw.replace(b'"known_outer_returncode": 0', b'"known_outer_returncode": 0, "known_outer_returncode": 0'),
                          b"NaN", b"{}", raw + b"x", b"x" * 131073):
            with self.assertRaises(ValueError):
                validator.validate(malformed, MANIFEST)

    def test_actual_wrapper_fragment_terminal_on_failed_or_malformed_receipt(self):
        guard = (ROOT / "vm-guard.sh").read_text()
        fragment = guard.split("task_failed=0\n", 1)[1].split("check_category() {", 1)[0]
        self.assertNotIn("timeout --", fragment)
        cases = [b'{"known_outer_returncode":0,"known_outer_returncode":0}',
                 json.dumps(dict(receipt(), known_outer_returncode=False)).encode()]
        value = receipt()
        value["receipt"]["copies"] = "x" * 16
        cases.append(json.dumps(value).encode())
        value = receipt()
        value["receipt"]["initial"]["bus"][0]["size"] = False
        cases.append(json.dumps(value).encode())
        cases.append(json.dumps(receipt()).encode())
        for index, raw in enumerate(cases):
            with tempfile.TemporaryDirectory() as directory:
                stage = Path(directory)
                (stage / "scratch").mkdir()
                (stage / "probe.py").write_text("raise SystemExit(0)\n")
                # Execute the real inert validation function on synthetic data;
                # the actual guest main path remains fixed and is never invoked.
                (stage / "validate_receipt.py").write_text(
                    "import importlib.util\n"
                    f"spec=importlib.util.spec_from_file_location('strict', {str(ROOT / 'validate_receipt.py')!r})\n"
                    "m=importlib.util.module_from_spec(spec); spec.loader.exec_module(m)\n"
                    f"m.validate({raw!r}, {MANIFEST!r})\n")
                result = subprocess.run(["/bin/bash", "-c", 'set -euo pipefail\n' + fragment + "printf AFTER_ALLOWED"],
                                        env={"PATH": "/usr/bin", "task_stage": directory},
                                        capture_output=True, timeout=5)
                self.assertEqual(result.returncode, 0 if index == len(cases) - 1 else 1)
                self.assertEqual(b"AFTER_ALLOWED" in result.stdout, index == len(cases) - 1)
        with tempfile.TemporaryDirectory() as directory:
            stage = Path(directory)
            (stage / "scratch").mkdir()
            (stage / "probe.py").write_text("raise SystemExit(1)\n")
            (stage / "validate_receipt.py").write_text("print('SHOULD_NOT_VALIDATE')\n")
            result = subprocess.run(["/bin/bash", "-c", fragment + "printf AFTER_FORBIDDEN"],
                                    env={"PATH": "/usr/bin", "task_stage": directory}, capture_output=True, timeout=5)
            self.assertEqual(result.returncode, 1)
            self.assertNotIn(b"SHOULD_NOT_VALIDATE", result.stdout)
            self.assertNotIn(b"AFTER_FORBIDDEN", result.stdout)

    def test_observer_unknown_copy_verification_never_spawns_or_stops(self):
        base = Mock()
        copies = Mock()
        copies.verify.side_effect = OSError("unknown")
        with self.assertRaises(OSError):
            probe.observe(base, copies, Mock())
        base.OwnedProcess.assert_not_called()
        base.stop.assert_not_called()
        copies.close.assert_not_called()

    def test_unknown_maps_after_launch_never_queries_or_stops_children_again(self):
        base = Mock(UNSETTLED=[], ENV={})
        base.OwnedProcess.side_effect = [Mock(pid=123), Mock(pid=124)]
        copies = Mock()
        copies.inventory.side_effect = OSError("mapping unknown")
        with patch.object(probe, "create"), patch("builtins.open", return_value=Mock()), \
             self.assertRaises(OSError):
            probe.observe(base, copies, Mock())
        self.assertEqual(copies.inventory.call_count, 1)
        self.assertEqual(copies.verify.call_count, 1)
        base.stop.assert_not_called()
        copies.close.assert_not_called()

    def test_actual_main_unknown_and_known_nonpass_never_read_result_or_after_query(self):
        from types import SimpleNamespace
        for unknown in (True, False):
            base = Mock(NS=("user", "net", "mnt", "pid", "uts"))
            base.namespace.side_effect = lambda name: name + ":[123]"
            child = SimpleNamespace(returncode=1)
            base.OwnedProcess.return_value = child
            if unknown:
                base.supervise.side_effect = RuntimeError("unknown")
            else:
                base.supervise.return_value = True
            raw = {name: b"{}" for name in probe.PINS}
            with patch.object(probe, "__file__", str(probe.STAGE / "probe.py")), \
                 patch.object(probe.sys, "argv", [str(probe.STAGE / "probe.py"), "--run-reviewed-inventory"]), \
                 patch.object(probe.os, "getuid", return_value=1000), \
                 patch.object(probe.os, "geteuid", return_value=1000), \
                 patch.object(probe, "private_inputs"), \
                 patch.object(probe, "load_inputs", return_value=(base, None, None, raw)), \
                 patch.object(probe, "read_input", return_value=b"synthetic") as read, \
                 patch.object(probe.Path, "mkdir"), patch.object(probe.Path, "open") as opened, \
                 patch.object(probe, "create") as create, self.assertRaises(RuntimeError):
                opened.return_value.__enter__.return_value = Mock()
                probe.main()
            read.assert_called_once_with(probe.STAGE / "probe.py")
            self.assertEqual(base.namespace.call_count, 5)
            base.supervise.assert_called_once_with(child, 65)
            self.assertTrue(all(call.args[0].parent == probe.INPUTS for call in create.call_args_list))
            base.stop.assert_not_called()

    def test_outer_unknown_supervision_has_no_result_read_or_recovery_branch(self):
        source = (ROOT / "probe.py").read_text()
        tail = source.split("completed = base.supervise(child, 65)", 1)[1]
        self.assertLess(tail.index("isolated_child_nonpass"), tail.index('read_input(STAGE / "child.stdout")'))
        self.assertNotIn("finally:", source.split("def observe", 1)[1].split("def isolated", 1)[0])
        for word in ("child.poll(", "child.wait(", "subprocess.run(", "subprocess.Popen("):
            self.assertNotIn(word, source)
        guard = (ROOT / "vm-guard.sh").read_text()
        self.assertIn("set -C\numask 077", guard)
        self.assertIn("stat.S_IMODE(before.st_mode) == mode", guard)


if __name__ == "__main__":
    unittest.main()
