# SPDX-License-Identifier: MIT
"""Synthetic artifact/receipt guards only; no core or broker is launched."""
import hashlib
import json
import os
from pathlib import Path
import platform
import tempfile
import unittest
from unittest.mock import patch

from test_core_connections_adapter import INTEROP, COMPOSITION, REVIEW


class DNSInteropTests(unittest.TestCase):
    def fixture(self, directory):
        data = bytearray(64)
        data[:7] = b"\x7fELF\x02\x01\x01"
        data[18:20] = {"x86_64": 62, "aarch64": 183}[platform.machine()].to_bytes(2, "little")
        path = directory / "fixture"
        path.write_bytes(data)
        path.chmod(0o700)
        return path, bytes(data), hashlib.sha256(data).hexdigest()

    def events(self):
        result = [{"Package": INTEROP.PACKAGE, "Action": "start"}]
        for _ in range(20):
            result.append({"Package": INTEROP.PACKAGE, "Test": INTEROP.TEST, "Action": "run"})
            for name in (INTEROP.TEST + "/" + s for s in INTEROP.SCENARIOS):
                result.extend({"Package": INTEROP.PACKAGE, "Test": name, "Action": a}
                              for a in ("run", "output", "pass"))
            result.append({"Package": INTEROP.PACKAGE, "Test": INTEROP.TEST, "Action": "pass"})
        result.append({"Package": INTEROP.PACKAGE, "Action": "pass"})
        return result

    def raw(self, events):
        return b"\n".join(json.dumps(event).encode("ascii") for event in events)

    def test_exact_parent_and_four_subcases_are_required(self):
        events = self.events()
        INTEROP.receipt(self.raw(events))
        invalid = ([], events[:-1], events + events[-1:], events + events[1:2],
                   [e for e in events if e.get("Test") != INTEROP.TEST + "/success"],
                   [e for e in events if e.get("Test") != INTEROP.TEST])
        for value in invalid:
            with self.subTest(length=len(value)), self.assertRaises(RuntimeError):
                INTEROP.receipt(self.raw(value))

    def test_skip_failure_bad_action_and_test_shapes_refuse(self):
        events = self.events()
        for event in ({"Action": "skip"}, {"Action": "fail"}, {"Action": "invented"},
                      {"Action": []}, {}, {"Action": None}, {"Action": "pass", "Test": None},
                      {"Action": "pass", "Test": 1}, {"Action": "pass", "Test": "unknown"},
                      {"Action": "pass", "Package": "foreign"}):
            bad = {"Package": INTEROP.PACKAGE, **event}
            with self.subTest(event=event), self.assertRaises(RuntimeError):
                INTEROP.receipt(self.raw(events + [bad]))
        for raw in (b"not json", b"[]", b'null',
                    b'{"Action":"pass","Action":"pass"}', b"x" * (4 * 1024 * 1024 + 1)):
            with self.assertRaises(RuntimeError):
                INTEROP.receipt(raw)

    def test_unstarted_premature_and_reordered_receipts_refuse(self):
        events = self.events()
        for values in (list(reversed(events)), events[1:] + events[:1],
                       [events[0], events[-1], *events[1:-1]],
                       [events[0], events[1], events[4], events[2], events[3], *events[5:]]):
            with self.assertRaises(RuntimeError):
                INTEROP.receipt(self.raw(values))

    def test_checked_snapshot_refuses_alias_mode_hash_and_wrong_arch(self):
        with tempfile.TemporaryDirectory(prefix="di-unit-") as name:
            root = Path(name)
            path, data, digest = self.fixture(root)
            self.assertEqual(INTEROP.snapshot_fixture(path, digest), data)
            alias = root / "alias"
            alias.symlink_to(path)
            with self.assertRaises(OSError):
                INTEROP.snapshot_fixture(alias, digest)
            alias.unlink()
            os.link(path, alias)
            with self.assertRaises(RuntimeError):
                INTEROP.snapshot_fixture(path, digest)
            alias.unlink()
            for mode in (0o777, 0o600, 0o4700):
                path.chmod(mode)
                with self.assertRaises(RuntimeError):
                    INTEROP.snapshot_fixture(path, digest)
            path.chmod(0o700)
            with self.assertRaises(RuntimeError):
                INTEROP.snapshot_fixture(path, "0" * 64)
            with patch.object(INTEROP.platform, "machine", return_value="unsupported"):
                with self.assertRaises(RuntimeError):
                    INTEROP.snapshot_fixture(path, digest)

    def test_snapshot_checks_post_read_identity_and_caps(self):
        with tempfile.TemporaryDirectory(prefix="di-unit-") as name:
            path, data, digest = self.fixture(Path(name))
            original_read = INTEROP.os.read
            changed = False
            def mutate(descriptor, count):
                nonlocal changed
                result = original_read(descriptor, count)
                if not changed:
                    changed = True
                    path.chmod(0o755)
                return result
            with patch.object(INTEROP.os, "read", side_effect=mutate):
                with self.assertRaises(RuntimeError):
                    INTEROP.snapshot_fixture(path, digest)
            with patch.object(INTEROP.os, "getxattr", return_value=b"synthetic"):
                with self.assertRaises(RuntimeError):
                    INTEROP.snapshot_fixture(path, digest)

    def test_missing_optin_member_refuses_before_export_or_compilation(self):
        with tempfile.TemporaryDirectory(prefix="di-unit-") as name:
            root = Path(name)
            path, _, digest = self.fixture(root)
            for arguments in ({"fixture": path}, {"fixture_sha": digest},
                              {"fixture": path, "fixture_sha": "unknown"}):
                with patch.object(REVIEW, "run") as run, patch.object(COMPOSITION, "export") as export:
                    with self.assertRaises(RuntimeError):
                        COMPOSITION.exercise(root, root, root, root, **arguments)
                    run.assert_not_called()
                    export.assert_not_called()

    def test_overlay_is_only_interop_socket_test_and_is_reversed(self):
        data = COMPOSITION.INTEROP_TEST_PATCH.read_bytes()
        self.assertEqual(hashlib.sha256(data).hexdigest(), COMPOSITION.INTEROP_TEST_SHA256)
        paths = [line for line in data.decode().splitlines() if line.startswith("diff --git ")]
        self.assertEqual(paths, ["diff --git a/listener/sing_tun/system_dns_interop_linux_test.go "
                                 "b/listener/sing_tun/system_dns_interop_linux_test.go"])
        source = (COMPOSITION.INTEROP_TEST_PATCH.parent / "managed_composition.py").read_text()
        self.assertIn("apply(source, INTEROP_TEST_PATCH, reverse=True)", source)
        self.assertIn('DNS_REVISION + ":" + dns_interop.CORPUS_PATH', source)
        self.assertIn("matrix_receipt(dns, DNS_TESTS, (DNS_INTEROP_SKIP,))", source)


if __name__ == "__main__":
    unittest.main()
