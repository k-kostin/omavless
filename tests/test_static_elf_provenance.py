"""Pure static-candidate evidence guards; no tool/candidate execution or VM."""
import errno
import hashlib
import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).parent / "static_elf_provenance"
SPEC = importlib.util.spec_from_file_location("static_elf_provenance", ROOT / "probe.py")
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)
RAW = (ROOT.parent / "real_resolved_binary/guest-inventory.json").read_bytes()
INVENTORY = json.loads(RAW)
DYNAMIC = b"Dynamic section at offset 0x1000 contains 2 entries:\n 0x0000000000000001 (NEEDED) Shared library: [libc.so.6]\n"


def meta(**changes):
    fields = dict(st_dev=31, st_ino=1, st_size=64, st_uid=0, st_gid=0,
                  st_mode=0o100755, st_nlink=1, st_mtime_ns=1, st_ctime_ns=1)
    return SimpleNamespace(**dict(fields, **changes))


class StaticElfTests(unittest.TestCase):
    def test_dynamic_decoder_keeps_basename_only_fixed_search_and_interpreter(self):
        raw = DYNAMIC + b" 0x000000000000001d (RUNPATH) Library runpath: [$ORIGIN:/usr/lib]\n [Requesting program interpreter: /lib64/ld-linux-x86-64.so.2]\n"
        result = probe.decode_readelf(raw)
        self.assertEqual(result["needed"], ["libc.so.6"])
        self.assertEqual(result["declared_search_tokens"], ["$ORIGIN", "/usr/lib"])
        self.assertEqual(result["interpreter"], "/lib64/ld-linux-x86-64.so.2")

    def test_private_loader_paths_tags_malformed_and_duplicate_needed_refuse(self):
        for raw in (DYNAMIC.replace(b"libc.so.6", b"/home/private.so"),
                    DYNAMIC.replace(b"libc.so.6", b"../private.so"),
                    DYNAMIC + b" 0x001d (RUNPATH) Library runpath: [/private/credentials]\n",
                    DYNAMIC + b" 0x001d (FILTER) Shared library: [other.so]\n",
                    DYNAMIC + b" [Requesting program interpreter: /home/private]\n", DYNAMIC + DYNAMIC,
                    DYNAMIC.replace(b"Shared library:", b"unexpected:"), b"not a decoded ELF"):
            with self.assertRaises(probe.Refused) as caught:
                probe.decode_readelf(raw)
            self.assertNotIn("private", str(caught.exception))
            self.assertNotIn("credentials", str(caught.exception))

    def test_needed_resolution_has_fixed_search_missing_ambiguity_and_no_arbitrary_paths(self):
        with patch.object(probe.os, "lstat", side_effect=[meta(), FileNotFoundError()]), \
             patch.object(probe, "canonical_public", return_value=("/usr/lib/libc.so.6", [])):
            self.assertEqual(probe.resolve_needed("libc.so.6")[0], "/usr/lib/libc.so.6")
        for effects in ([FileNotFoundError(), FileNotFoundError()], [meta(), meta()]):
            with patch.object(probe.os, "lstat", side_effect=effects), \
                 patch.object(probe, "canonical_public", return_value=("/usr/lib/libc.so.6", [])):
                with self.assertRaisesRegex(probe.Refused, "needed_missing_or_ambiguous"):
                    probe.resolve_needed("libc.so.6")
        with patch.object(probe.os, "lstat") as lstat:
            with self.assertRaises(probe.Refused):
                probe.resolve_needed("/home/private.so")
            lstat.assert_not_called()

    def test_symlink_escape_refuses_before_private_lookup(self):
        def lstat(path):
            self.assertNotIn("home", str(path))
            return meta(st_mode=0o120777) if str(path).endswith("libfoo.so") else meta(st_mode=0o40755)
        with patch.object(probe.os, "lstat", side_effect=lstat), \
             patch.object(probe.os, "readlink", return_value="/home/private.so"):
            with self.assertRaisesRegex(probe.Refused, "nonpublic_symlink_target"):
                probe.canonical_public("/usr/lib/libfoo.so")

    def test_package_owner_unknown_duplicate_and_private_name_refuse(self):
        for names in ([], ["one", "two"]):
            with self.assertRaisesRegex(probe.Refused, "package_owner_not_unique"):
                probe.owner_record("/usr/lib/a.so", (Path("/fixed"), {"/usr/lib/a.so": names}, {}))
        with patch.object(probe, "package_bytes", return_value=b"%NAME%\nsecret /private\n\n%VERSION%\n1\n"):
            with self.assertRaisesRegex(probe.Refused, "package_description_value"):
                probe.owner_record("/usr/lib/a.so", (Path("/fixed"), {"/usr/lib/a.so": ["pkg"]}, {"pkg": "x"}))

    def fake_capture(self, command_error=None, tool_hash=None, dependency_edge=None):
        next_fd, paths, stats = [90], {}, {}
        canonical = lambda path: INVENTORY["elfs"].get(path, {}).get("resolved_path", path)
        known = {row["resolved_path"]: row["sha256"] for row in INVENTORY["elfs"].values()}
        modes = {row["resolved_path"]: int(row["mode"], 8) for row in INVENTORY["elfs"].values()}
        def opened(path, flags):
            if dependency_edge and dependency_edge[0] != "/usr/lib/libedge-A.so":
                self.assertNotEqual(str(path), dependency_edge[0], "changed queued target opened before refusal")
            next_fd[0] += 1
            fd = next_fd[0]
            paths[fd] = str(path)
            stats[str(path)] = meta(st_ino=fd, st_mode=probe.stat.S_IFREG | modes.get(str(path), 0o755))
            if str(path) == probe.READELF:
                stats[str(path)] = meta(st_ino=29149, st_size=810072)
            return fd
        def digest(fd, deadline):
            path = paths[fd]
            return stats[path], (tool_hash or probe.READELF_SHA) if path == probe.READELF else known.get(path, "1" * 64)
        empty_hash = hashlib.sha256(b"").hexdigest()
        def owner(path, index):
            return {"name": "binutils", "version": "2.47-4", "file_list_sha256": empty_hash,
                    "description_sha256": empty_hash}, "binutils-2.47-4"
        outputs = [b"Dynamic section at offset 0x1000 contains 1 entry:\n 0x01 (NEEDED) Shared library: [libedge.so]\n"] if dependency_edge else []
        def run_tool(*args, **kwargs):
            return SimpleNamespace(stdout=outputs.pop(0) if outputs else b"There is no dynamic section in this file.\n", stderr=b"")
        def resolve(path):
            return dependency_edge if dependency_edge and path == "/usr/lib/libedge.so" else (canonical(str(path)), [])
        with patch.object(probe.os, "getuid", return_value=1000), patch.object(probe.os, "geteuid", return_value=1000), \
             patch.object(probe, "bounded_file", return_value=RAW), patch.object(probe, "package_index", return_value=(Path("/public"), {}, {})), \
             patch.object(probe, "canonical_public", side_effect=resolve), \
             patch.object(probe, "resolve_needed", return_value=("/usr/lib/libedge.so", "/usr/lib/libedge-A.so", [])), \
             patch.object(probe.os, "open", side_effect=opened), patch.object(probe.os, "close") as closed, \
             patch.object(probe, "digest_fd", side_effect=digest), \
             patch.object(probe.os, "fstat", side_effect=lambda fd: stats[paths[fd]]), \
             patch.object(probe.os, "lstat", side_effect=lambda path: stats[str(path)]), \
             patch.object(probe.os, "getxattr", side_effect=OSError(errno.ENODATA, "absent")), \
             patch.object(probe, "owner_record", side_effect=owner), patch.object(probe, "package_bytes", return_value=b""), \
             patch.object(probe.base, "UNSETTLED", []), \
             patch.object(probe.base, "command", side_effect=command_error or run_tool,
                          return_value=SimpleNamespace(stdout=b"There is no dynamic section in this file.\n", stderr=b"")) as command:
            result = probe.capture()
        return result, command, closed

    def test_queued_dependency_retains_discovered_target_and_chain_before_tool(self):
        for actual in (("/usr/lib/libedge-B.so", []),
                       ("/usr/lib/libedge-A.so", [{"path": "/usr/lib/libedge.so", "target": "changed-chain"}])):
            with self.subTest(actual=actual):
                result, command, closed = self.fake_capture(dependency_edge=actual)
                self.assertEqual(result["outcome"], "NONPASS")
                self.assertEqual(result["reason"], "queued_dependency_edge_changed")
                self.assertEqual(command.call_count, 16)  # seeds only; queued target never decoded
                self.assertEqual(closed.call_count, 17)  # tool + seed FDs only
                self.assertEqual(result["records"][0]["dependencies"][0]["resolved_path"], "/usr/lib/libedge-A.so")
        result, command, _ = self.fake_capture(dependency_edge=("/usr/lib/libedge-A.so", []))
        self.assertEqual(result["outcome"], "OBSERVED_STATIC_CANDIDATE_CLOSURE")
        self.assertEqual(command.call_count, 17)
        self.assertEqual(result["records"][-1]["resolved_path"], "/usr/lib/libedge-A.so")

    def test_finite_positive_capture_executes_only_pinned_tool_fd_not_candidates(self):
        result, command, closed = self.fake_capture()
        self.assertEqual(result.get("reason"), None)
        self.assertEqual(result["outcome"], "OBSERVED_STATIC_CANDIDATE_CLOSURE")
        self.assertEqual(len(result["records"]), 16)
        self.assertEqual(len(result["aliases"]), 17)
        self.assertEqual(command.call_count, 16)
        for call in command.call_args_list:
            self.assertEqual(call.args[0][0], "/proc/self/fd/91")
            self.assertEqual(call.args[0][1:4], ["--wide", "--dynamic", "--program-headers"])
            self.assertEqual(call.kwargs["pass_fds"][0], 91)
            self.assertEqual(call.kwargs["env"], {"PATH": "/usr/bin", "LANG": "C", "LC_ALL": "C"})
        self.assertEqual(closed.call_count, 17)
        self.assertIs(result["candidate_elf_executed"], False)
        self.assertIs(result["allowlist_adoption"], False)
        self.assertFalse(result["records"][-1]["matches_original_manifest"])

    def test_first_unknown_child_observation_stops_all_remaining_tool_calls(self):
        result, command, closed = self.fake_capture(command_error=probe.base.Refused("owned_state_quarantined"))
        self.assertEqual(result["outcome"], "NONPASS")
        self.assertEqual(result["reason"], "owned_state_quarantined")
        command.assert_called_once()
        self.assertEqual(closed.call_count, 2)

    def test_unpinned_tool_never_executes(self):
        result, command, closed = self.fake_capture(tool_hash="0" * 64)
        self.assertEqual(result["reason"], "readelf_pin")
        command.assert_not_called()
        closed.assert_called_once()

    def test_count_depth_total_and_deadline_bounds_stop_before_tool(self):
        for key, value, reason in (("MAX_COUNT", 0, "static_object_count"),
                                  ("MAX_DEPTH", -1, "static_depth_bound"),
                                  ("MAX_TOTAL", 0, "static_total_bound")):
            with patch.object(probe, key, value):
                result, command, _ = self.fake_capture()
            self.assertEqual(result["reason"], reason)
            command.assert_not_called()
        with patch.object(probe.time, "monotonic", side_effect=[0, 61]):
            result, command, _ = self.fake_capture()
        self.assertEqual(result["reason"], "capture_deadline_or_child_unknown")
        command.assert_not_called()

    def test_fd_digest_rejects_changed_or_nonelf_input(self):
        for block, final in ((b"private-data", meta()), (b"\x7fELF" + b"x" * 60, meta(st_ino=2))):
            with patch.object(probe.os, "fstat", side_effect=[meta(), final]), \
                 patch.object(probe.os, "lseek"), patch.object(probe.os, "read", side_effect=[block, b""]), \
                 patch.object(probe.time, "monotonic", return_value=0):
                with self.assertRaises(probe.Refused):
                    probe.digest_fd(99, 1)

    def test_outer_guard_pins_source_and_all_baseline_categories(self):
        guard = (ROOT / "vm-guard-queued-edge.sh").read_text()
        self.assertIn(hashlib.sha256((ROOT / "probe.py").read_bytes()).hexdigest(), guard)
        self.assertIn(probe.READELF_SHA, guard)
        for category in ("CANONICAL_EPOCH", "PRIVATE_FILES", "USER_SERVICE", "EXECUTABLE",
                         "NAMESPACE", "CORE_INVENTORY", "TUN_INVENTORY", "RESOLVER", "RESOLVCONF"):
            self.assertIn("check_category " + category, guard)

    def test_source_contains_no_loader_execution_or_network_setter(self):
        source = (ROOT / "probe.py").read_text()
        for forbidden in ("ldd", "subprocess.Popen", "SetLink", "RevertLink", "base.exercise(", "--run-inventory", "mount("):
            self.assertNotIn(forbidden, source)


if __name__ == "__main__":
    unittest.main()
