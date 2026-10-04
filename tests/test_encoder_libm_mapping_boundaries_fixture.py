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

from tests.encoder_libm_live_mapping import probe, validate_receipt as validator

ROOT = Path(__file__).parent / "encoder_libm_live_mapping"
MANIFEST = (ROOT.parent / "encoder_libm_copy_admission/copy-manifest.json").read_bytes()


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
    return dict(schema="encoder-libm-mapping-inventory-v1", outcome="OBSERVED_INVENTORY_ONLY",
                source_sha256=validator.PROBE_SHA, pins=validator.PINS, known_outer_returncode=0,
                broker_executed=False, core_executed=False, dns_mutations=False, compatibility_acceptance=False,
                receipt=dict(initial=maps, final=copy.deepcopy(maps), copies=copies,
                             shutdown={name: dict(pid=100+i, starttime=99, namespaces={"pid": [5, 1000], "net": [5, 1001]}, signal="SIGTERM", signal_count=1, exit_code=0, state="zero-reaped") for i, name in enumerate(("bus", "resolved"))},
                             live_during_both_passes=True, pid_namespace_and_direct_child_bound=True))


class LiveFdReview2FixtureTests(unittest.TestCase):
    def test_entire_dependency_graph_is_pinned(self):
        self.assertEqual(hashlib.sha256((ROOT / "probe.py").read_bytes()).hexdigest(), validator.PROBE_SHA)
        self.assertEqual(probe.PINS, validator.PINS)
        for name, expected in probe.PINS.items():
            path = ROOT / name
            if name in ("containment.py", "guest-inventory.json"):
                path = ROOT.parent / "real_resolved_binary" / ("probe.py" if name == "containment.py" else name)
            if name in ("admission.py", "copy-manifest.json"):
                path = ROOT.parent / "encoder_libm_copy_admission" / name
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
                             ("shutdown", {"bus": False, "resolved": 0}),
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
        from tests.encoder_libm_live_mapping import lifecycle
        base, copies = Mock(), Mock()
        copies.verify.side_effect = OSError("unknown")
        session = lifecycle.Session()
        with patch.object(session, "spawn") as spawn, self.assertRaises(OSError):
            probe.observe(base, copies, Mock(), session)
        spawn.assert_not_called()
        self.assertTrue(session.sealed)
        copies.close.assert_not_called()

    def test_unknown_maps_after_launch_never_queries_or_stops_children_again(self):
        from tests.encoder_libm_live_mapping import lifecycle
        base = Mock(ENV={})
        copies = Mock()
        copies.inventory.side_effect = OSError("mapping unknown")
        session = lifecycle.Session()
        with patch.object(session, "spawn", side_effect=[Mock(pid=123), Mock(pid=124)]), \
             patch.object(session, "anchor"), patch.object(session, "ready"), \
             patch.object(session, "shutdown") as shutdown, \
             patch.object(probe, "create"), patch("builtins.open", return_value=Mock()), \
             self.assertRaises(OSError):
            probe.observe(base, copies, Mock(), session)
        self.assertEqual(copies.inventory.call_count, 1)
        self.assertEqual(copies.verify.call_count, 1)
        shutdown.assert_not_called()
        copies.close.assert_not_called()
        self.assertTrue(session.sealed)

    def test_no_old_cleanup_dynamic_reachability(self):
        source = (ROOT / "probe.py").read_text()
        for forbidden in ("base.stop(", "base.supervise(", "base.wait_child(", "base.command(",
                          "base.wait(", "base.reap_child(", "--kill-child", "killpg("):
            self.assertNotIn(forbidden, source)
        for binding in ("base.command = session.command", "base.child_status = session.live",
                        "base.no_directory_fds = session.no_directory_fds"):
            self.assertIn(binding, source)
        tail = source.split("except BaseException:\n            session.sealed = True", 1)[1].split("        print(json.dumps(receipt", 1)[0]
        self.assertIn("time.sleep(3600)", tail)
        for forbidden in ("read(", "waitid", "waitpid", "kill(", "print(", "close("):
            self.assertNotIn(forbidden, tail)
        source = (ROOT / "lifecycle.py").read_text()
        self.assertNotIn("SIGKILL", source)
        self.assertNotIn("killpg", source)

    def test_outer_unknown_and_known_nonzero_never_read_result(self):
        from types import SimpleNamespace
        for failure in (RuntimeError('unknown'), RuntimeError('known nonzero')):
            base = Mock(NS=('user', 'net', 'mnt', 'pid', 'uts'))
            base.namespace.side_effect = lambda name: name + ':[123]'
            child = SimpleNamespace(returncode=None)
            session = Mock()
            session.spawn.return_value = child
            session.settle_zero.side_effect = failure
            lifecycle = Mock()
            lifecycle.Session.return_value = session
            raw = {name: b'{}' for name in probe.PINS}
            with patch.object(probe, '__file__', str(probe.STAGE / 'probe.py')), \
                 patch.object(probe.sys, 'argv', ['probe.py', '--run-reviewed-inventory']), \
                 patch.object(probe.os, 'getuid', return_value=1000), \
                 patch.object(probe.os, 'geteuid', return_value=1000), \
                 patch.object(probe, 'private_inputs'), \
                 patch.object(probe, 'load_inputs', return_value=(base, None, None, lifecycle, raw)), \
                 patch.object(probe, 'read_input', return_value=b'synthetic') as read, \
                 patch.object(probe.Path, 'mkdir'), patch.object(probe.Path, 'open') as opened, \
                 patch.object(probe, 'create') as create, self.assertRaises(RuntimeError):
                opened.return_value.__enter__.return_value = Mock()
                probe.main()
            read.assert_called_once_with(probe.STAGE / 'probe.py')
            session.settle_zero.assert_called_once_with(child, 65)
            self.assertTrue(all(call.args[0].parent == probe.INPUTS for call in create.call_args_list))
            self.assertNotIn('--kill-child=SIGKILL', session.spawn.call_args.args[0])

    def test_shutdown_receipt_requires_exact_zero_and_one_signal(self):
        for key, wrong in (('exit_code', -15), ('exit_code', False), ('signal_count', 2),
                           ('signal_count', True), ('signal', 'SIGKILL'), ('starttime', 0),
                           ('namespaces', {'pid': [5, False], 'net': [5, 1001]})):
            value = receipt()
            value['receipt']['shutdown']['resolved'][key] = wrong
            with self.assertRaises(ValueError):
                validator.validate(json.dumps(value).encode(), MANIFEST)


if __name__ == "__main__":
    unittest.main()
