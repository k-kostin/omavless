"""Pure package-bound diagnosis controls; no VM or executable candidate."""
from contextlib import contextmanager
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).parent / "static_elf_provenance"


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


probe = load("package_bound_diagnostic")
validator = load("package_bound_validator")
supervisor = load("package_bound_supervisor")
assert supervisor.base is None
supervisor.base = supervisor.load_containment(ROOT.parent / "real_resolved_binary/probe.py")


class PackageBoundTests(unittest.TestCase):
    @contextmanager
    def fixture(self, size=None):
        with tempfile.TemporaryDirectory(dir="/var/tmp") as temporary:
            root = Path(temporary)
            package = root / "example-1-1"
            package.mkdir()
            selected = package / "files"
            with selected.open("wb") as stream:
                stream.truncate(probe.LEGACY_LIMIT + 1 if size is None else size)
            (package / "desc").write_bytes(b"%NAME%\nexample\n%VERSION%\n1-1\n")
            with patch.object(probe, "ROOT", root), patch.object(probe, "root_owned", return_value=True), \
                 patch.object(probe.os, "getuid", return_value=1000), patch.object(probe.os, "geteuid", return_value=1000):
                yield selected

    def test_real_positive_retains_both_fds_hash_and_diagnostic_only_bound(self):
        opened, real_open = [], os.open
        def record_open(path, *args, **kwargs):
            fd = real_open(path, *args, **kwargs)
            opened.append((path, fd))
            return fd
        with self.fixture() as selected, patch.object(probe.os, "open", side_effect=record_open):
            result = probe.capture()
        self.assertEqual(result["outcome"], "OBSERVED_PACKAGE_FILE_BOUND")
        self.assertEqual([path.name for path, fd in opened], ["files", "desc"])
        for path, fd in opened:
            with self.assertRaises(OSError): os.fstat(fd)
        self.assertEqual(result["original_open_fd"]["size"], probe.LEGACY_LIMIT + 1)
        self.assertEqual(result["original_open_fd"]["sha256"], hashlib.sha256(b"\0" * (probe.LEGACY_LIMIT + 1)).hexdigest())
        self.assertFalse(result["static_limit_changed"])
        self.assertEqual(result["read_bytes_including_rechecks"], 2 * (result["scanned_bytes"] + result["description_open_fd"]["size"]))
        # Only local ownership is substituted to exercise root-owned receipt types.
        for key in ("original_open_fd", "description_open_fd"):
            result[key].update(uid=0, gid=0)
        validator.validate(result, {"schema": "package-bound-owned-child-v1", "outcome": "KNOWN_COMPLETED", "returncode": 0})

    def test_real_replacement_reversion_and_description_mutation_are_terminal(self):
        for change in ("replace", "revert", "rename_revert", "desc_revert"):
            with self.subTest(change=change), self.fixture() as selected:
                original = selected.stat()
                real_read = probe.read_record
                calls = []
                def observed(record, *args, **kwargs):
                    calls.append(record[1].name)
                    value = real_read(record, *args, **kwargs)
                    if calls == ["files", "desc"]:
                        if change in ("replace", "rename_revert"):
                            selected.rename(selected.with_name("saved"))
                            selected.write_bytes(b"x")
                            if change == "rename_revert":
                                selected.unlink()
                                selected.with_name("saved").rename(selected)
                        elif change == "revert":
                            with selected.open("r+b") as stream:
                                stream.write(b"x")
                                stream.seek(0)
                                stream.write(b"\0")
                            os.utime(selected, ns=(original.st_atime_ns, original.st_mtime_ns))
                        else:
                            desc = selected.with_name("desc")
                            before, raw = desc.stat(), desc.read_bytes()
                            desc.write_bytes(raw + b"x")
                            desc.write_bytes(raw)
                            os.utime(desc, ns=(before.st_atime_ns, before.st_mtime_ns))
                    return value
                with patch.object(probe, "read_record", side_effect=observed):
                    result = probe.capture()
                self.assertEqual(result["outcome"], "NONPASS")
                self.assertNotIn("original_open_fd", result)
                self.assertLessEqual(len(calls), 4)

    def test_limits_nonregular_duplicate_description_and_missing_race_refuse(self):
        for change in ("huge", "fifo", "duplicate_desc", "missing_after_open", "total", "deadline"):
            with self.subTest(change=change), self.fixture() as selected:
                if change == "huge":
                    with selected.open("wb") as stream: stream.truncate(probe.DIAGNOSTIC_LIMIT + 1)
                elif change == "fifo":
                    selected.unlink()
                    os.mkfifo(selected)
                elif change == "duplicate_desc":
                    selected.with_name("desc").write_bytes(b"%NAME%\nexample\n%NAME%\nother\n%VERSION%\n1-1\n")
                with patch.object(probe, "TOTAL_LIMIT", 1 if change == "total" else probe.TOTAL_LIMIT), \
                     patch.object(probe.time, "monotonic", side_effect=[0, 16] if change == "deadline" else None, return_value=0), \
                     patch.object(probe, "check", side_effect=FileNotFoundError if change == "missing_after_open" else probe.check):
                    result = probe.capture()
                self.assertEqual(result["outcome"], "NONPASS")
                self.assertNotIn("package", result)

    def test_legacy_size_pass_is_not_diagnosed_and_count_bound_is_unchanged(self):
        with self.fixture(size=probe.LEGACY_LIMIT):
            self.assertEqual(probe.capture()["reason"], "original_size_predicate_failure_not_reproduced")
        with self.fixture(), patch.object(probe.os, "listdir", return_value=[str(n) for n in range(4097)]):
            self.assertEqual(probe.capture()["reason"], "package_count_bound")

    def test_exact_pins_and_diagnostic_has_no_tool_execution_surface(self):
        guard = (ROOT / "vm-guard-package-bound.sh").read_text()
        for name in ("package_bound_diagnostic", "package_bound_supervisor", "package_bound_validator"):
            self.assertIn(hashlib.sha256((ROOT / (name + ".py")).read_bytes()).hexdigest(), guard)
        source = (ROOT / "package_bound_diagnostic.py").read_text()
        for forbidden in ("subprocess", "os.exec", "os.system", "SetLink", "base.command"):
            self.assertNotIn(forbidden, source)

    def test_supervisor_unknown_nonzero_and_existing_output_never_continue(self):
        for status in ("success", "nonzero", "unknown", "existing"):
            with self.subTest(status=status), tempfile.TemporaryDirectory(dir="/var/tmp") as temporary:
                stage = Path(temporary)
                child = Mock(returncode=1 if status == "nonzero" else 0)
                if status == "existing": (stage / "result.json").write_bytes(b"retained")
                with patch.object(supervisor, "STAGE", stage), patch.object(supervisor.os, "getuid", return_value=1000), \
                     patch.object(supervisor.os, "geteuid", return_value=1000), patch.object(supervisor.base, "private_parent"), \
                     patch.object(supervisor.base, "object_bytes"), patch.object(supervisor.base, "OwnedProcess", return_value=child) as spawn, \
                     patch.object(supervisor.base, "supervise", side_effect=supervisor.base.Refused() if status == "unknown" else None, return_value=True) as wait:
                    result = supervisor.run_child(stage)
                self.assertEqual(spawn.call_count, 0 if status == "existing" else 1)
                self.assertEqual(wait.call_count, 0 if status == "existing" else 1)
                self.assertEqual(child.mock_calls, [])
                self.assertEqual(result["outcome"], "KNOWN_COMPLETED" if status == "success" else "NONPASS")
                if status == "unknown": self.assertNotIn("returncode", result)

    def test_wrapper_failure_branches_stop_before_later_action(self):
        guard = (ROOT / "vm-guard-package-bound.sh").read_text()
        for role in ("supervisor.py", "validator.py"):
            line = next(row for row in guard.splitlines() if row.startswith("if ! env ") and role in row)
            body = guard.split(line, 1)[1].split("\nfi\n", 1)[0]
            with tempfile.TemporaryDirectory(dir="/var/tmp") as temporary:
                script = 'task_stage="$1"; env() { return 1; };\n' + line + body + '\nfi\nprintf FORBIDDEN\n'
                result = subprocess.run(["/bin/bash", "-c", script, "test", temporary], capture_output=True, text=True)
            self.assertEqual(result.returncode, 1)
            self.assertNotIn("FORBIDDEN", result.stdout)

    def test_strict_validator_duplicate_boolean_and_nested_errors_refuse(self):
        for raw in (b'{"a":1,"a":2}', b'NaN', b' ' * 16385):
            with self.assertRaises(ValueError): validator.decode(raw)
        with self.assertRaises(ValueError):
            validator.validate({}, {"schema": "package-bound-owned-child-v1", "outcome": "KNOWN_COMPLETED", "returncode": False})

    def test_no_helper_fallback_and_no_private_description_output(self):
        path = supervisor.STAGE / "containment.py"
        with patch.object(supervisor.os, "open", side_effect=FileNotFoundError) as opened:
            with self.assertRaises(FileNotFoundError): supervisor.load_containment(path)
        opened.assert_called_once()
        self.assertEqual(opened.call_args.args[0], path)
        with self.fixture() as selected:
            selected.with_name("desc").write_bytes(b"%NAME%\nexample\n%VERSION%\n1-1\n%DESC%\nsynthetic-private-description\n")
            result = probe.capture()
        self.assertEqual(result["outcome"], "OBSERVED_PACKAGE_FILE_BOUND")
        self.assertNotIn("synthetic-private-description", json.dumps(result))

    def test_actual_shell_validator_rejects_malformed_receipts_before_next_action(self):
        guard = (ROOT / "vm-guard-package-bound.sh").read_text()
        line = next(row for row in guard.splitlines() if row.startswith("if ! env ") and "validator.py" in row)
        body = guard.split(line, 1)[1].split("\nfi\n", 1)[0]
        with self.fixture():
            original = probe.capture()
        for key in ("original_open_fd", "description_open_fd"):
            original[key].update(uid=0, gid=0)
        for fault in ("valid", "duplicate", "false", "nested", "size"):
            value = json.loads(json.dumps(original))
            owned = {"schema": "package-bound-owned-child-v1", "outcome": "KNOWN_COMPLETED", "returncode": 0}
            if fault == "false": owned["returncode"] = False
            if fault == "nested": value["description_open_fd"]["inode"] = True
            if fault == "size": value["original_open_fd"]["size"] = probe.DIAGNOSTIC_LIMIT + 1
            raw = json.dumps(value)
            if fault == "duplicate": raw = raw.replace('"schema":', '"schema":"duplicate","schema":', 1)
            with self.subTest(fault=fault), tempfile.TemporaryDirectory(dir="/var/tmp") as temporary:
                stage = Path(temporary)
                (stage / "result.json").write_text(raw)
                (stage / "supervisor-receipt.json").write_text(json.dumps(owned))
                code = 'import importlib.util,pathlib,sys;s=importlib.util.spec_from_file_location("v",sys.argv[1]);v=importlib.util.module_from_spec(s);s.loader.exec_module(v);p=pathlib.Path(sys.argv[2]);v.validate(v.decode((p/"result.json").read_bytes()),v.decode((p/"supervisor-receipt.json").read_bytes()))'
                script = 'task_stage="$1"; code="$2"; validator="$3"; env() { /usr/bin/python3 -c "$code" "$validator" "$task_stage"; };\n' + line + body + '\nfi\nprintf AFTER_VALIDATED\n'
                result = subprocess.run(["/bin/bash", "-c", script, "test", temporary, code, str(ROOT / "package_bound_validator.py")], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0 if fault == "valid" else 1)
            self.assertEqual(result.stdout, "AFTER_VALIDATED" if fault == "valid" else "")


if __name__ == "__main__":
    unittest.main()
