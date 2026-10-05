"""Four-MiB package lists only; historical measured sources remain frozen."""
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from tests import test_static_elf_provenance as previous
from tests import test_static_capture_supervisor as previous_supervisor
from tests import test_static_receipt_validator as previous_validator

ROOT = Path(__file__).parent / "static_elf_provenance"
SPEC = importlib.util.spec_from_file_location("static_four_mib", ROOT / "probe_four_mib.py")
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)
assert probe.base is None
probe.base = probe.load_containment(ROOT.parent / "real_resolved_binary/probe.py")


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


new_supervisor = load("static_capture_supervisor_four_mib")
assert new_supervisor.base is None
new_supervisor.base = new_supervisor.load_containment(ROOT.parent / "real_resolved_binary/probe.py")
new_validator = load("validate_static_receipt_four_mib")


class FourMibTests(previous.StaticElfTests):
    def setUp(self):
        self.binding = patch.object(previous, "probe", probe)
        self.binding.start()
        self.addCleanup(self.binding.stop)

    def test_only_files_dispatch_gets_four_mib_description_stays_two(self):
        for name, limit in (("files", 4 * 1024 * 1024), ("desc", 2 * 1024 * 1024)):
            path = Path("/var/lib/pacman/local/example-1-1") / name
            with patch.object(probe.Path, "lstat", return_value=previous.meta()), \
                 patch.object(probe, "bounded_file", return_value=b"") as read:
                self.assertEqual(probe.package_bytes(path), b"")
            self.assertEqual(read.call_args.args, (path, limit))
            self.assertIn("expected", read.call_args.kwargs)

    def test_real_four_mib_boundary_and_oversize_description(self):
        for size, limit, accepted in ((4 * 1024 * 1024, 4 * 1024 * 1024, True),
                                      (4 * 1024 * 1024 + 1, 4 * 1024 * 1024, False),
                                      (2 * 1024 * 1024 + 1, 2 * 1024 * 1024, False)):
            with self.subTest(size=size, limit=limit), tempfile.TemporaryDirectory(dir="/var/tmp") as temporary:
                path = Path(temporary) / "synthetic-record"
                with path.open("wb") as stream:
                    stream.truncate(size)
                if accepted:
                    self.assertEqual(len(probe.bounded_file(path, limit, path.lstat())), size)
                else:
                    with self.assertRaisesRegex(probe.Refused, "bounded_file_shape"):
                        probe.bounded_file(path, limit, path.lstat())

    def test_legitimate_2484429_byte_synthetic_files_list_keeps_one_owner(self):
        size = 2484429
        rows = [b"%FILES%\n", b"usr/lib/synthetic.so\n"]
        length, count = sum(map(len, rows)), 0
        while size - length > 80:
            row = f"usr/share/icons/synthetic-{count:08d}\n".encode()
            rows.append(row)
            length += len(row)
            count += 1
        rows.append(b"usr/share/icons/" + b"x" * (size - length - len(b"usr/share/icons/") - 1) + b"\n")
        raw = b"".join(rows)
        self.assertEqual(len(raw), size)
        with tempfile.TemporaryDirectory(dir="/var/tmp") as temporary:
            path = Path(temporary) / "files"
            path.write_bytes(raw)
            self.assertEqual(probe.bounded_file(path, 4 * 1024 * 1024, path.lstat()), raw)
            with self.assertRaises(probe.Refused):
                probe.bounded_file(path, 2 * 1024 * 1024, path.lstat())
        index = self.package_index({"base-3-3": b"", "synthetic-1-1": raw})
        self.assertEqual(index[1], {"/usr/lib/synthetic.so": ["synthetic-1-1"]})
        self.assertEqual(index[2]["synthetic-1-1"], hashlib.sha256(raw).hexdigest())

    def test_exact_clone_deltas_and_new_wrapper_pins(self):
        old_stage, new_stage = "t3-static-elf-empty-record-review-1", "t3-static-elf-four-mib-review-1"
        before = (ROOT / "probe.py").read_text()
        expected = before.replace('data = bounded_file(path, 2 * 1024 * 1024, expected=value)',
                                  'limit = 4 * 1024 * 1024 if path.name == "files" else 2 * 1024 * 1024\n    data = bounded_file(path, limit, expected=value)').replace(old_stage, new_stage)
        self.assertEqual((ROOT / "probe_four_mib.py").read_text(), expected)
        self.assertEqual(hashlib.sha256((ROOT / "probe.py").read_bytes()).hexdigest(),
                         "edbe782db2863b49187f3ee9b6c0e65d20fa951038c2c3e04834965f286bf641")
        self.assertEqual((ROOT / "validate_static_receipt_four_mib.py").read_text(),
                         (ROOT / "validate_static_receipt.py").read_text().replace(old_stage, new_stage))
        new_probe_hash = hashlib.sha256((ROOT / "probe_four_mib.py").read_bytes()).hexdigest()
        self.assertEqual((ROOT / "static_capture_supervisor_four_mib.py").read_text(),
                         (ROOT / "static_capture_supervisor.py").read_text().replace(old_stage, new_stage).replace(
                             "edbe782db2863b49187f3ee9b6c0e65d20fa951038c2c3e04834965f286bf641", new_probe_hash))
        wrapper = (ROOT / "vm-guard-four-mib.sh").read_text()
        for name in ("probe_four_mib.py", "static_capture_supervisor_four_mib.py", "validate_static_receipt_four_mib.py"):
            self.assertIn(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), wrapper)
        expected_wrapper = (ROOT / "vm-guard-empty-record.sh").read_text().replace(old_stage, new_stage)
        for before_name, after_name in (("probe.py", "probe_four_mib.py"),
                                       ("static_capture_supervisor.py", "static_capture_supervisor_four_mib.py"),
                                       ("validate_static_receipt.py", "validate_static_receipt_four_mib.py")):
            expected_wrapper = expected_wrapper.replace(hashlib.sha256((ROOT / before_name).read_bytes()).hexdigest(),
                                                        hashlib.sha256((ROOT / after_name).read_bytes()).hexdigest())
        self.assertEqual(wrapper, expected_wrapper)


class FourMibSupervisorTests(previous_supervisor.StaticSupervisorTests):
    def setUp(self):
        binding = patch.object(previous_supervisor, "supervisor", new_supervisor)
        binding.start()
        self.addCleanup(binding.stop)

    def test_exact_source_eligibility_and_private_create_only_wrapper(self):
        guard = (ROOT / "vm-guard-four-mib.sh").read_text()
        for name in ("probe_four_mib.py", "static_capture_supervisor_four_mib.py", "validate_static_receipt_four_mib.py"):
            self.assertIn(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), guard)
        self.assertEqual(hashlib.sha256((ROOT / "probe_four_mib.py").read_bytes()).hexdigest(), new_supervisor.PROBE_SHA)
        for marker in (new_supervisor.CONTAINMENT_SHA, new_supervisor.INVENTORY_SHA, "set -o noclobber", "umask 077"):
            self.assertIn(marker, guard)


class FourMibValidatorTests(previous_validator.ReceiptTests):
    def setUp(self):
        binding = patch.object(previous_validator, "validator", new_validator)
        binding.start()
        self.addCleanup(binding.stop)


if __name__ == "__main__":
    unittest.main()
