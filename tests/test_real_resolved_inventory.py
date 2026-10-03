"""Pure loader-inventory counterexamples. No daemon, namespace or host command."""
import hashlib
import importlib.util
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).parent / "real_resolved_inventory"
SPEC = importlib.util.spec_from_file_location("loader_inventory", ROOT / "probe.py")
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


class InventoryGuards(unittest.TestCase):
    def test_original_containment_is_exact_frozen_bytes(self):
        self.assertEqual(hashlib.sha256(probe.CONTAINMENT_BYTES).hexdigest(), probe.CONTAINMENT_SHA)

    def test_public_unknown_object_is_inventory_not_admission(self):
        line = "1-2 r-xp 0 00:1f 123 /usr/lib/unknown-public.so\n"
        child = SimpleNamespace(pid=42, returncode=None)
        record = {"path": "/usr/lib/unknown-public.so", "device": 31, "inode": 123,
                  "size": 10, "sha256": "1" * 64}
        with patch.object(probe.base, "child_status", return_value=None), \
             patch.object(probe.Path, "read_text", return_value=line), \
             patch.object(probe, "measure_object", return_value=record):
            rows = probe.inventory_child(child, {"elfs": {}})
        self.assertEqual(rows, [dict(record, matches_previously_reviewed_loader_object=False)])

    def test_deleted_nonpublic_and_ambiguous_paths_refuse(self):
        for path in ("/usr/lib/x.so (deleted)", "/home/private.so", "/tmp/fixture.so",
                     "/usr/lib/../bin/thing", "/memfd:private", "/usr/lib/x.so\ttail"):
            with self.assertRaises(probe.base.Refused):
                probe.map_objects("1-2 r-xp 0 00:1f 123 " + path + "\n")
        with self.assertRaises(probe.base.Refused):
            probe.map_objects("1-2 r-xp 0 00:1f 123 /usr/lib/x.so\n"
                              "2-3 r-xp 0 00:1f 124 /usr/lib/x.so\n")

    def test_mapping_changes_are_not_a_stable_snapshot(self):
        child = SimpleNamespace(pid=42, returncode=None)
        with patch.object(probe.base, "child_status", return_value=None), \
             patch.object(probe.Path, "read_text", side_effect=[
                 "1-2 r-xp 0 00:1f 123 /usr/lib/x.so\n", "1-2 r-xp 0 00:1f 124 /usr/lib/x.so\n"]), \
             patch.object(probe, "measure_object", return_value={"sha256": "1" * 64}):
            with self.assertRaisesRegex(probe.base.Refused, "mapping_snapshot_changed"):
                probe.inventory_child(child, {"elfs": {}})

    def test_known_dependency_change_cannot_be_relabelled_new_inventory(self):
        child = SimpleNamespace(pid=42, returncode=None)
        inventory = {"elfs": {"/usr/lib/x.so": {"resolved_path": "/usr/lib/x.so", "sha256": "0" * 64}}}
        with patch.object(probe.base, "child_status", return_value=None), \
             patch.object(probe.Path, "read_text", return_value="1-2 r-xp 0 00:1f 123 /usr/lib/x.so\n"), \
             patch.object(probe, "measure_object", return_value={"sha256": "1" * 64}):
            with self.assertRaisesRegex(probe.base.Refused, "reviewed_loader_object_changed"):
                probe.inventory_child(child, inventory)

    def test_inode_replacement_deadline_and_non_elf_refuse(self):
        block = b"\x7fELFsynthetic"
        fields = dict(st_dev=31, st_ino=123, st_size=len(block), st_mode=probe.stat.S_IFREG | 0o644,
                      st_uid=65534, st_gid=65534, st_mtime_ns=1, st_ctime_ns=1, st_nlink=1)
        with patch.object(probe.os, "open", return_value=99), patch.object(probe.os, "close"), \
             patch.object(probe.os, "fstat", return_value=SimpleNamespace(**fields)), \
             patch.object(probe.os, "read", side_effect=[block, b""]), \
             patch.object(probe.time, "monotonic", return_value=1):
            row = probe.measure_object("/usr/lib/x.so", (31, 123), 2)
            self.assertEqual(row["sha256"], hashlib.sha256(block).hexdigest())
        for identity, content, now, changed in (((31, 124), block, 1, False),
                ((31, 123), b"not-an-ELF!!!", 1, False), ((31, 123), block, 3, False),
                ((31, 123), block, 1, True)):
            altered = dict(fields, st_ctime_ns=2) if changed else fields
            with patch.object(probe.os, "open", return_value=99), patch.object(probe.os, "close"), \
                 patch.object(probe.os, "fstat", side_effect=[SimpleNamespace(**fields), SimpleNamespace(**altered)]), \
                 patch.object(probe.os, "read", side_effect=[content, b""]), \
                 patch.object(probe.time, "monotonic", return_value=now):
                with self.assertRaises(probe.base.Refused):
                    probe.measure_object("/usr/lib/x.so", identity, 2)

    def test_inventory_source_has_no_broker_core_or_dns_effect_dispatch(self):
        source = (ROOT / "probe.py").read_text()
        for forbidden in ("base.exercise(", "cap_exec(", "SetLinkDNS", "SetLinkDomains",
                          "SetLinkDefaultRoute", "RevertLink", "--serve", "core_config"):
            self.assertNotIn(forbidden, source)
        self.assertNotIn("subprocess.Popen(", source)
        self.assertNotIn(".poll(", source)
        self.assertNotIn(".wait(", source.replace("base.wait(", ""))


if __name__ == "__main__":
    unittest.main()
