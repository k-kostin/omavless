"""Pure pinned-copy tests; never mount, run namespaces or launch daemons."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).parent / "private_tmpfs_elf"
SPEC = importlib.util.spec_from_file_location("private_tmpfs_elf", ROOT / "probe.py")
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)
BLOCK = b"\x7fELFsynthetic-pinned-copy"
DIGEST = hashlib.sha256(BLOCK).hexdigest()


def metadata(**changes):
    fields = dict(st_dev=31, st_ino=123, st_uid=65534, st_gid=65534, st_mode=0o100755,
                  st_nlink=1, st_size=len(BLOCK), st_mtime_ns=1, st_ctime_ns=1)
    return SimpleNamespace(**dict(fields, **changes))


class TmpfsElfTests(unittest.TestCase):
    def test_only_copy_supervisor_has_larger_file_bound(self):
        with patch.object(probe.resource, "setrlimit") as limit:
            probe.copy_limits()
        self.assertEqual(limit.call_args_list[1].args,
                         (probe.resource.RLIMIT_FSIZE, (probe.MAX_ELF, probe.MAX_ELF)))
        source = (ROOT / "probe.py").read_text()
        self.assertIn("stdout=log, stderr=log, preexec_fn=base.limits", source)

    def test_wrapper_pins_new_source_schema_and_known_copy_only_outcome(self):
        guard = (ROOT / "vm-guard.sh").read_text()
        self.assertIn(hashlib.sha256((ROOT / "probe.py").read_bytes()).hexdigest(), guard)
        self.assertIn("private-tmpfs-resolved-inventory-v1", guard)
        self.assertIn("assert len(receipt['copies']) == 15", guard)
        self.assertIn("assert row['matches_previously_reviewed_loader_object'] is True", guard)

    def test_manifest_is_exact_frozen_sixteen_logical_fifteen_targets(self):
        raw = (ROOT.parent / "real_resolved_binary/guest-inventory.json").read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), probe.base.INVENTORY)
        value = json.loads(raw)
        self.assertEqual(len(probe.fixed_targets(value)), 15)
        for variant in (dict(value["elfs"], extra={}), {k: v for k, v in value["elfs"].items() if k != "/usr/bin/dbus-daemon"}):
            with self.assertRaisesRegex(probe.base.Refused, "copy_manifest_changed"):
                probe.fixed_targets({"elfs": variant})
        variant = copy.deepcopy(value)
        variant["elfs"]["/usr/bin/dbus-daemon"]["sha256"] = "0" * 64
        with self.assertRaises(probe.base.Refused):
            probe.fixed_targets(variant)

    def test_whole_store_requires_readonly_tmpfs_not_just_readonly_bind(self):
        text = "40 1 0:44 / /elf-copy-store ro,nosuid,nodev - tmpfs tmpfs ro\n"
        with patch.object(probe.Path, "read_text", return_value=text):
            probe.mount_policy(probe.STORE, readonly=True)
        for changed in (text.replace("tmpfs tmpfs", "btrfs source"), text.replace(" tmpfs ro", " tmpfs rw"),
                        text.replace("ro,nosuid,nodev", "rw,nosuid,nodev"),
                        text.replace("ro,nosuid,nodev", "ro,nodev"), text * 2,
                        text.replace("ro,nosuid,nodev", "ro,nosuid,nodev,noexec")):
            with patch.object(probe.Path, "read_text", return_value=changed):
                with self.assertRaises(probe.base.Refused):
                    probe.mount_policy(probe.STORE, readonly=True)

    def copy_fake(self, source=None, write=None, changed_path=None, digest=DIGEST):
        source = source or metadata()
        target = metadata(st_dev=44, st_ino=500, st_uid=0, st_gid=0)
        descriptors = iter((99, 100, 101))
        def fstat(fd):
            return source if fd == 99 else target
        with patch.object(probe.os, "open", side_effect=lambda *a: next(descriptors)) as opened, \
             patch.object(probe.os, "close") as closed, patch.object(probe.os, "fstat", side_effect=fstat), \
             patch.object(probe.os, "stat", return_value=changed_path or source), \
             patch.object(probe.os, "lseek"), patch.object(probe.os, "fchmod"), \
             patch.object(probe.os, "read", side_effect=[BLOCK, b""]) as read, \
             patch.object(probe.os, "write", return_value=len(BLOCK) if write is None else write), \
             patch.object(probe, "checked_digest", return_value=digest), \
             patch.object(probe.time, "monotonic", return_value=1):
            result = probe.copy_one("/usr/bin/dbus-daemon", {"mode": "0o755", "sha256": DIGEST},
                                    Path("/elf-copy-store/0"), 2, 1024)
        return result, opened, closed, read

    def test_copy_has_distinct_installed_and_namespace_root_provenance(self):
        result, opened, closed, read = self.copy_fake()
        self.assertEqual((result["source_device"], result["source_inode"]), (31, 123))
        self.assertEqual((result["device"], result["inode"], result["uid"], result["gid"]), (44, 500, 0, 0))
        self.assertEqual(result["sha256"], DIGEST)
        self.assertEqual(closed.call_count, 3)
        self.assertTrue(opened.call_args_list[1].args[1] & probe.os.O_EXCL)

    def test_source_shape_short_write_hash_and_same_byte_replacement_refuse(self):
        for changes in ({"st_uid": 0}, {"st_gid": 0}, {"st_mode": 0o100777},
                        {"st_size": 0}, {"st_size": 2048}, {"st_mode": 0o040755}):
            with self.subTest(changes=changes), self.assertRaises(probe.base.Refused):
                self.copy_fake(source=metadata(**changes))
        with self.assertRaisesRegex(probe.base.Refused, "copy_short_io"):
            self.copy_fake(write=1)
        with self.assertRaisesRegex(probe.base.Refused, "installed_source_hash"):
            self.copy_fake(digest="0" * 64)
        with self.assertRaisesRegex(probe.base.Refused, "installed_source_replaced"):
            self.copy_fake(changed_path=metadata(st_ino=124))

    def test_digest_rejects_mutation_nonelf_deadline_and_growth(self):
        for block, now, final in ((b"not-ELF", 1, metadata()), (BLOCK, 3, metadata()),
                                  (BLOCK, 1, metadata(st_ino=124)), (BLOCK + b"grow", 1, metadata())):
            with patch.object(probe.os, "fstat", side_effect=[metadata(), final]), \
                 patch.object(probe.os, "lseek"), patch.object(probe.os, "read", side_effect=[block, b""]), \
                 patch.object(probe.time, "monotonic", return_value=now):
                with self.assertRaises(probe.base.Refused):
                    probe.checked_digest(99, 2)

    def test_writable_copy_fd_refuses(self):
        for flags in ("01", "02"):
            with patch.object(probe.os, "listdir", return_value=["99"]), \
                 patch.object(probe.os, "fstat", return_value=metadata(st_dev=44)), \
                 patch.object(probe.Path, "read_text", return_value="flags:\t" + flags + "\n"):
                with self.assertRaisesRegex(probe.base.Refused, "writable_copy_fd"):
                    probe.no_writable_copy_fds(44)

    def test_unknown_mapping_never_opened_or_admitted(self):
        child = SimpleNamespace(pid=42)
        with patch.object(probe.base, "child_status", return_value=None), \
             patch.object(probe.Path, "read_text", return_value="1-2 r-xp 0 00:1f 123 /usr/lib/unknown.so\n"), \
             patch.object(probe, "COPY_RECEIPTS", {}), patch.object(probe, "measure_object") as measure:
            with self.assertRaises(probe.ObjectIdentityRefused) as caught:
                probe.inventory_child(child, {"elfs": {}})
        self.assertEqual(caught.exception.diagnostic["path"], "/usr/lib/unknown.so")
        self.assertFalse(caught.exception.diagnostic["allowlist_adoption"])
        measure.assert_not_called()

    def test_same_bytes_wrong_copy_inode_never_hashed(self):
        path = "/usr/bin/dbus-daemon"
        value = metadata(st_dev=44, st_ino=500, st_uid=0, st_gid=0)
        record = {"device": 44, "inode": 500, "size": value.st_size, "mode": value.st_mode,
                  "uid": 0, "gid": 0, "sha256": DIGEST}
        with patch.object(probe, "COPY_RECEIPTS", {path: record}), patch.object(probe, "COPY_FDS", {path: 88}), \
             patch.object(probe.os, "open", return_value=99), patch.object(probe.os, "close"), \
             patch.object(probe.os, "fstat", side_effect=[metadata(st_dev=44, st_ino=501, st_uid=0, st_gid=0), value]), \
             patch.object(probe, "checked_digest") as digest:
            with self.assertRaisesRegex(probe.base.Refused, "copy_target_identity"):
                probe.verify_target(path, 2)
            digest.assert_not_called()
        with patch.object(probe, "COPY_RECEIPTS", {path: record}), \
             patch.object(probe, "verify_target") as verify, patch.object(probe.os, "open") as opened:
            for identity in ((44, 501), (45, 500)):
                with self.assertRaisesRegex(probe.base.Refused, "mapped_copy_identity"):
                    probe.measure_object(path, identity, 2)
            verify.assert_not_called()
            opened.assert_not_called()

    def test_copy_refusal_precedes_every_overlay_and_daemon(self):
        source = (ROOT / "probe.py").read_text()
        self.assertLess(source.index('base.require(len(staged) == 15'), source.index('"--bind", str(destination)'))
        self.assertLess(source.index('prepare_copies(inventory, base.decode(original))'), source.index('receipt = observe(inventory)'))
        for forbidden in ("SetLinkDNS", "SetLinkDomains", "SetLinkDefaultRoute", "RevertLink", "base.exercise(", "core_config"):
            self.assertNotIn(forbidden, source)

    def test_first_copy_failure_stops_remaining_copies_and_all_overlays(self):
        original = {name: "old" for name in probe.base.NS}
        with patch.object(probe, "COPY_RECEIPTS", {}), patch.object(probe, "COPY_FDS", {}), \
             patch.object(probe.os, "geteuid", return_value=0), patch.object(probe.os, "getegid", return_value=0), \
             patch.object(probe.base, "namespace", return_value="new"), patch.object(probe.base, "validate_maps"), \
             patch.object(probe.Path, "read_text", return_value=""), patch.object(probe.Path, "mkdir"), \
             patch.object(probe.Path, "resolve", autospec=True, side_effect=lambda value, **kw: value), \
             patch.object(probe.os, "getpid", return_value=100), patch.object(probe.os, "readlink", return_value="100"), \
             patch.object(probe.os, "stat", return_value=metadata(st_uid=0)), \
             patch.object(probe, "mount_policy"), patch.object(probe.base, "command") as command, \
             patch.object(probe, "copy_one", side_effect=[{"size": 10}, probe.base.Refused("injected_copy_unknown")]) as copy_one:
            with self.assertRaisesRegex(probe.base.Refused, "injected_copy_unknown"):
                probe.prepare_copies({"elfs": probe.EXPECTED_ELFS}, original)
        self.assertEqual(copy_one.call_count, 2)
        command.assert_called_once_with(["/usr/bin/mount", "-t", "tmpfs", "-o",
                                        "size=128m,mode=0755,nosuid,nodev", "tmpfs", str(probe.STORE)])


if __name__ == "__main__":
    unittest.main()
