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
import subprocess
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

    def private_store(self, profiles=None, payload=None):
        path = self.root / "private-store.json"
        if profiles is None:
            profiles = [{"id":"private-original-record", "name":"Private original name",
                         "protocol":"vless", "uri":"vless://synthetic-secret@192.0.2.1:443?type=xhttp#private-name",
                         "subscriptionId":"private-subscription", "subscriptionKey":"private-key",
                         "missing":False, "favorite":True, "extra":{"retained":"protocol-marker"}}]
        path.write_bytes(payload if payload is not None else json.dumps({"version":3,"profiles":profiles}).encode())
        path.chmod(0o600)
        return path

    def test_private_fixture_is_detached_without_rewriting_protocol_or_source(self):
        path = self.private_store()
        before = path.read_bytes()
        source = json.loads(before)["profiles"][0]
        copied = PROBE.load_private_vless(path)
        self.assertEqual(path.read_bytes(), before)
        self.assertEqual(copied["id"], PROBE.PUBLIC_PROFILE_ID)
        self.assertEqual(copied["name"], "Synthetic")
        self.assertEqual(copied["subscriptionId"], "")
        self.assertEqual(copied["subscriptionKey"], "")
        for field in ("uri", "protocol", "missing", "favorite", "extra"):
            self.assertEqual(copied[field], source[field])

    def test_private_fixture_selects_only_existing_nonmissing_vless(self):
        path = self.private_store()
        usable = json.loads(path.read_bytes())["profiles"][0]
        rejected = [dict(usable, protocol="trojan"), dict(usable, missing=True),
                    dict(usable, missing="false"), dict(usable, uri="trojan://synthetic"), {"protocol":"vless"}]
        path = self.private_store(profiles=rejected + [usable])
        self.assertEqual(PROBE.load_private_vless(path)["uri"], usable["uri"])
        path = self.private_store(profiles=rejected)
        with self.assertRaisesRegex(PROBE.Failure, "^vless_fixture_unavailable$"):
            PROBE.load_private_vless(path)

    def test_private_fixture_permissions_owner_type_and_symlink_bounds(self):
        path = self.private_store()
        for mode in (0o644, 0o400, 0o660):
            path.chmod(mode)
            with self.assertRaisesRegex(PROBE.Failure, "^private_store_unsafe$"):
                PROBE.load_private_vless(path)
        path.chmod(0o600)
        with patch.object(PROBE.os, "getuid", return_value=os.getuid() + 1), self.assertRaisesRegex(PROBE.Failure, "^private_store_unsafe$"):
            PROBE.load_private_vless(path)
        alias = self.root / "alias"
        alias.symlink_to(path)
        parent_alias = self.root / "parent-alias"
        parent_alias.symlink_to(self.root, target_is_directory=True)
        fifo = self.root / "fifo"
        os.mkfifo(fifo, 0o600)
        for bad in (alias, parent_alias / path.name, self.root, fifo, Path("relative-private-path")):
            with self.assertRaises(PROBE.Failure):
                PROBE.load_private_vless(bad)

    def test_private_fixture_prefers_usable_last_selection_then_first_usable(self):
        path = self.private_store()
        first = json.loads(path.read_bytes())["profiles"][0]
        second = dict(first, id="second-private-record", uri="vless://second-synthetic@192.0.2.2:443")
        for last, second_missing, expected in [
            (second["id"], False, second["uri"]),
            (second["id"], True, first["uri"]),
            ("unknown-private-record", False, first["uri"]),
            (None, False, first["uri"]),
        ]:
            document = {"version":3,"lastId":last,"profiles":[first,dict(second,missing=second_missing)]}
            path = self.private_store(payload=json.dumps(document).encode())
            before = path.read_bytes()
            self.assertEqual(PROBE.load_private_vless(path)["uri"], expected)
            self.assertEqual(path.read_bytes(), before)

    def test_private_fixture_malformed_duplicate_nonutf8_and_count_bounds_are_safe(self):
        payloads = [b"private-secret", b"\xff", b"[]", b'{"version":3,"profiles":[],"profiles":[]}',
                    b'{"version":3,"profiles":[],"nested":{"key":1,"key":2}}',
                    b'{"version":3,"profiles":[],"value":NaN}',
                    json.dumps({"version":3,"profiles":[{}] * 257}).encode()]
        for payload in payloads:
            path = self.private_store(payload=payload)
            with self.assertRaises(PROBE.Failure) as caught:
                PROBE.load_private_vless(path)
            self.assertNotIn("private-secret", str(caught.exception))
            self.assertNotIn(str(path), str(caught.exception))
        with path.open("wb") as stream:
            stream.truncate(PROBE.PRIVATE_STORE_LIMIT + 1)
        with self.assertRaisesRegex(PROBE.Failure, "^private_store_oversized$"):
            PROBE.load_private_vless(path)

    def test_private_fixture_changed_during_read_is_refused(self):
        path = self.private_store()
        original = PROBE.os.fstat
        calls = []
        def changed(fd):
            info = original(fd)
            calls.append(fd)
            if len(calls) == 2:
                replacement = MagicMock(wraps=info)
                replacement.st_mtime_ns = info.st_mtime_ns + 1
                return replacement
            return info
        with patch.object(PROBE.os, "fstat", side_effect=changed), self.assertRaisesRegex(PROBE.Failure, "^private_store_changed$"):
            PROBE.load_private_vless(path)

    def test_real_template_and_probe_have_only_fixed_public_network_targets(self):
        tun = "ovna0123456789"
        synthetic = PROBE.route_template(tun)
        real = PROBE.route_template(tun, True)
        self.assertIn("auto-route: false", synthetic)
        self.assertIn("dns: {enable: false}", synthetic)
        self.assertIn("MATCH,DIRECT", synthetic)
        self.assertIn("auto-route: true", real)
        self.assertIn("auto-detect-interface: true", real)
        self.assertIn("dns-hijack: [any:53]", real)
        self.assertNotIn("external-controller:", real)
        args = PROBE.https_probe_args(tun)
        self.assertEqual(args[args.index("--interface") + 1], "if!" + tun)
        self.assertEqual(args[-1], "https://example.com/")
        self.assertEqual(args[args.index("--max-time") + 1], "15")
        self.assertNotIn("--location", args)
        for helper in (PROBE.route_template, PROBE.https_probe_args):
            with self.assertRaisesRegex(PROBE.Failure, "^fixture_tun_invalid$"):
                helper("$(private-shell-input)")

    def test_https_tun_evidence_requires_success_and_both_counter_increments(self):
        result = subprocess.CompletedProcess([], 0, b"200", b"private-stderr")
        self.assertEqual(PROBE.https_probe_evidence(result, (10, 20), (11, 21)), (True, True, "pass"))
        for after in ((10, 21), (11, 20), (0, 0)):
            self.assertEqual(PROBE.https_probe_evidence(result, (10, 20), after), (True, False, "tun_probe_evidence_unavailable"))
        for code, public in ((6, "probe_dns_failed"), (28, "probe_timeout"), (45, "probe_interface_unavailable")):
            result.returncode = code
            self.assertEqual(PROBE.https_probe_evidence(result, (10, 20), (11, 21)), (False, False, public))
        result.returncode = 0
        result.stdout = b"403 private-hostname"
        self.assertEqual(PROBE.https_probe_evidence(result, (10, 20), (11, 21)), (False, False, "https_probe_failed"))

    def test_real_fixture_refuses_non_global_before_reading_private_source(self):
        options = MagicMock(private_vless_store="/private/source", mode="rule")
        with patch.object(PROBE, "load_private_vless") as load, self.assertRaisesRegex(PROBE.Failure, "^private_fixture_requires_full_vpn$"):
            PROBE.acceptance(options)
        load.assert_not_called()

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
