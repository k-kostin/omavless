# SPDX-License-Identifier: MIT
"""Deterministic acceptance-tool helpers; no service, TUN or live socket work."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import struct
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch


SPEC = importlib.util.spec_from_file_location(
    "native_service_acceptance_fixture", Path(__file__).with_name("native_service_acceptance.py")
)
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)


class NativeServiceAcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="omavless-host-tool-unit-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.proc = self.root / "proc"
        self.proc.mkdir()

    def mapped_path(self, value):
        path = Path(value)
        return self.proc / path.relative_to("/proc") if path.is_relative_to("/proc") else path

    def proc_file(self, relative, data):
        path = self.proc / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return path

    def test_bounded_reader_accepts_exact_limit_and_rejects_one_more(self):
        path = self.root / "bounded"
        path.write_bytes(b"abcd")
        self.assertEqual(PROBE.bounded(path, 4), b"abcd")
        with self.assertRaisesRegex(PROBE.Failure, "^observation_oversized$"):
            PROBE.bounded(path, 3)

    def test_private_write_creates_regular_0600_file(self):
        path = self.root / "private"
        PROBE.private_write(path, "synthetic")
        self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
        self.assertTrue(stat.S_ISREG(path.stat().st_mode))
        self.assertEqual(path.read_bytes(), b"synthetic")

    def test_private_write_refuses_overwrite_and_symlink(self):
        path = self.root / "private"
        path.write_bytes(b"original")
        alias = self.root / "alias"
        alias.symlink_to(path)
        for target in (path, alias):
            with self.assertRaises(OSError):
                PROBE.private_write(target, b"replacement")
        self.assertEqual(path.read_bytes(), b"original")

    def test_processes_ignore_nonnumeric_and_disappearing_entries(self):
        self.proc_file("12/comm", b"mihomo\n")
        self.proc_file("13/comm", b"omavless\n")
        self.proc_file("self/comm", b"ignored\n")
        (self.proc / "14").mkdir()
        with patch.object(PROBE, "Path", self.mapped_path):
            self.assertEqual(PROBE.processes(), {12: b"mihomo", 13: b"omavless"})

    def test_processes_do_not_hide_permission_errors(self):
        self.proc_file("12/comm", b"fixture\n")
        with patch.object(PROBE, "Path", self.mapped_path), patch.object(
            PROBE, "bounded", side_effect=PermissionError
        ), self.assertRaises(PermissionError):
            PROBE.processes()

    def test_descendants_include_all_threads_and_recursive_children(self):
        self.proc_file("10/task/10/children", b"11 ")
        self.proc_file("10/task/20/children", b"12 11 ")
        self.proc_file("11/task/11/children", b"13 ")
        self.proc_file("12/task/12/children", b"")
        # 13 disappearing before its task directory is read is permissible.
        with patch.object(PROBE, "Path", self.mapped_path):
            self.assertEqual(PROBE.descendants(10), {11, 12, 13})

    def test_descendants_refuse_excessive_tree(self):
        self.proc_file("10/task/10/children", " ".join(map(str, range(100, 357))).encode())
        with patch.object(PROBE, "Path", self.mapped_path), self.assertRaisesRegex(
            PROBE.Failure, "^descendant_bound$"
        ):
            PROBE.descendants(10)

    @staticmethod
    def tcp_table(*entries, ipv6=False):
        header = b"  sl local_address rem_address st tx_queue rx_queue tr tm->when retrnsmt uid timeout inode\n"
        if ipv6:
            header = header.replace(b"rem_address", b"remote_address")
        return header + b"".join(
            f"0: 00000000:0000 00000000:0000 {state} 0:0 00:0 0 1000 0 {inode}\n".encode()
            for state, inode in entries
        )

    def test_tcp_listener_snapshot_includes_both_families_only_listening(self):
        self.proc_file("net/tcp", self.tcp_table(("0A", 111), ("01", 222)))
        self.proc_file("net/tcp6", self.tcp_table(("0A", 333), ipv6=True))
        with patch.object(PROBE, "Path", self.mapped_path):
            self.assertEqual(PROBE.tcp_listeners(), {"111", "333"})

    def test_tcp_malformed_row_fails_closed(self):
        self.proc_file("net/tcp", self.tcp_table() + b"malformed\n")
        self.proc_file("net/tcp6", self.tcp_table(ipv6=True))
        with patch.object(PROBE, "Path", self.mapped_path), self.assertRaises(PROBE.Failure):
            PROBE.tcp_listeners()

    def test_tcp_empty_headerless_and_invalid_numeric_fields_fail_closed(self):
        self.proc_file("net/tcp6", self.tcp_table(ipv6=True))
        for payload in (b"", b"malformed header\n", self.tcp_table(("QQ", 123)),
                        self.tcp_table(("0A", "not-an-inode"))):
            self.proc_file("net/tcp", payload)
            with patch.object(PROBE, "Path", self.mapped_path), self.assertRaises(PROBE.Failure):
                PROBE.tcp_listeners()

    def test_tcp_absence_requires_no_listener_inode_owned_by_core(self):
        directory = self.proc / "12/fd"
        directory.mkdir(parents=True)
        (directory / "3").symlink_to("socket:[111]")
        (directory / "4").symlink_to("/dev/null")
        with patch.object(PROBE, "Path", self.mapped_path):
            self.assertFalse(PROBE.tcp_absent(12, {"111"}))
            self.assertTrue(PROBE.tcp_absent(12, {"222"}))

    def test_tcp_fd_permission_denial_is_not_proven_not_true(self):
        directory = self.proc / "12/fd"
        directory.mkdir(parents=True)
        (directory / "3").symlink_to("socket:[111]")
        with patch.object(PROBE, "Path", self.mapped_path), patch.object(
            PROBE.os, "readlink", side_effect=PermissionError
        ):
            self.assertIsNone(PROBE.tcp_absent(12, {"111"}))

    def controller(self, peer_pid=12, peer_uid=None, mode="global", status=200, payload=None):
        path, connection, response = MagicMock(), MagicMock(), MagicMock()
        path.lstat.return_value.st_mode = stat.S_IFSOCK | 0o600
        path.lstat.return_value.st_uid = os.getuid()
        path.parent.stat.return_value.st_mode = stat.S_IFDIR | 0o700
        connection.getsockopt.return_value = struct.pack(
            "3i", peer_pid, os.getuid() if peer_uid is None else peer_uid, 0
        )
        response.status = status
        response.read.return_value = json.dumps({"mode": mode}).encode() if payload is None else payload
        socket_factory = MagicMock()
        socket_factory.return_value.__enter__.return_value = connection
        return path, connection, response, socket_factory

    def test_controller_checks_peer_before_sending_any_request(self):
        for pid, uid in ((99, os.getuid()), (12, os.getuid() + 1)):
            path, connection, response, factory = self.controller(peer_pid=pid, peer_uid=uid)
            with patch.object(PROBE.socket, "socket", factory), patch.object(
                PROBE.http.client, "HTTPResponse", return_value=response
            ), self.assertRaisesRegex(PROBE.Failure, "^core_socket_peer$"):
                PROBE.core_controller(path, 12, "global")
            connection.sendall.assert_not_called()

    def test_controller_accepts_exact_peer_and_requested_mode(self):
        path, connection, response, factory = self.controller(mode="Global")
        with patch.object(PROBE.socket, "socket", factory), patch.object(
            PROBE.http.client, "HTTPResponse", return_value=response
        ):
            PROBE.core_controller(path, 12, "global")
        self.assertTrue(connection.sendall.call_args.args[0].startswith(b"GET /configs "))
        response.read.assert_called_once_with(65537)

    def test_controller_refuses_wrong_mode_non200_or_oversized_response(self):
        for arguments in ({"mode": "rule"}, {"status": 503}, {"payload": b"x" * 65537}):
            path, _, response, factory = self.controller(**arguments)
            with patch.object(PROBE.socket, "socket", factory), patch.object(
                PROBE.http.client, "HTTPResponse", return_value=response
            ), self.assertRaises(PROBE.Failure):
                PROBE.core_controller(path, 12, "global")

    def test_cli_requires_explicit_run_before_any_acceptance_work(self):
        with patch.object(sys, "argv", [
            "native_service_acceptance.py", "--binary", "/synthetic/omavless"
        ]), patch.object(PROBE, "acceptance") as acceptance:
            with self.assertRaisesRegex(PROBE.Failure, "^explicit_run_required$"):
                PROBE.main()
            acceptance.assert_not_called()

    def test_public_emit_produces_only_supplied_sanitized_fields(self):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            PROBE.emit(result="FAIL", classification="synthetic_failure")
        self.assertEqual(json.loads(output.getvalue()), {
            "result": "FAIL", "classification": "synthetic_failure",
        })


if __name__ == "__main__":
    unittest.main()
