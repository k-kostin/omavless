# SPDX-License-Identifier: MIT
"""Synthetic guard tests only; no namespace, peer, network or installed source."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import socket
import stat
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("p4_smoke", Path(__file__).with_name("p4_wg_loopback_smoke.py"))
subject = importlib.util.module_from_spec(spec)
spec.loader.exec_module(subject)


class FakeSocket:
    def __init__(self, group, pid=123):
        self.payload = [b"HTTP/1.1 200 OK\r\nContent-Length: 99\r\n\r\n" + json.dumps(group).encode(), b""]
        self.pid = pid

    def __enter__(self): return self
    def __exit__(self, *_args): return False
    def settimeout(self, _value): pass
    def connect(self, _path): pass
    def sendall(self, _data): pass
    def getsockopt(self, level, option, size):
        assert (level, option, size) == (socket.SOL_SOCKET, socket.SO_PEERCRED, 12)
        return struct.pack("3i", self.pid, 1000, 1000)
    def recv(self, _bound): return self.payload.pop(0)


class P4LoopbackGuardTests(unittest.TestCase):
    def test_no_opt_in_refuses_before_host_snapshot(self):
        args = argparse.Namespace(run=False, rounds=3)
        with patch.object(subject, "outside_snapshot") as snapshot:
            with self.assertRaises(subject.Refused) as raised:
                subject.outer(args)
            self.assertEqual(raised.exception.stage, "explicit_vm_opt_in")
            snapshot.assert_not_called()

    def test_missing_module_never_installs_or_loads(self):
        args = argparse.Namespace(run=True, rounds=3, source_sha="a" * 40)
        with patch.object(subject.os, "getuid", return_value=1000), patch.object(Path, "is_dir", return_value=False), patch.object(subject, "command") as command:
            with self.assertRaises(subject.Refused) as raised:
                subject.outer(args)
            self.assertEqual(raised.exception.stage, "stock_wireguard_unavailable")
            command.assert_not_called()

    def test_private_creation_is_exclusive_and_restrictive(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            secret = root / "synthetic.key"
            subject.private_write(secret, b"synthetic-canary")
            self.assertEqual(stat.S_IMODE(secret.stat().st_mode), 0o600)
            with self.assertRaises(FileExistsError):
                subject.private_write(secret, b"replacement")
            link = root / "symlink"
            link.symlink_to(secret)
            with self.assertRaises(OSError):
                subject.private_write(link, b"replacement")
            self.assertEqual(secret.read_bytes(), b"synthetic-canary")

    def test_binary_mismatch_has_no_copy_effect(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            source.write_bytes(b"synthetic public binary")
            source.chmod(0o500)
            target = root / "target"
            with self.assertRaises(subject.Refused) as raised:
                subject.copy_binary(source, target, "0" * 64)
            self.assertEqual(raised.exception.stage, "binary_identity")
            self.assertFalse(target.exists())

    def test_command_failure_never_echoes_fragments(self):
        failure = subprocess.TimeoutExpired("private-canary-command", 1, output=b"private-canary-output")
        with patch.object(subject.subprocess, "run", side_effect=failure):
            with self.assertRaises(subject.Refused) as raised:
                subject.command(["ignored"], "fixed_stage")
            self.assertEqual(str(raised.exception), "fixed_stage")
        result = subprocess.CompletedProcess(["ignored"], 0, b"x" * (subject.BODY_LIMIT + 1), b"canary")
        with patch.object(subject.subprocess, "run", return_value=result):
            with self.assertRaises(subject.Refused) as raised:
                subject.command(["ignored"], "fixed_stage")
            self.assertEqual(str(raised.exception), "fixed_stage")

    def test_controller_requires_exact_owned_pid(self):
        with patch.object(subject.socket, "socket", return_value=FakeSocket({}, pid=999)):
            with self.assertRaises(subject.Refused) as raised:
                subject.controller(Path("/synthetic.sock"), 123)
            self.assertEqual(raised.exception.stage, "controller_peer")

    def test_selector_must_exclude_direct_and_other_outbounds(self):
        safe = {"type":"Selector", "now":"P4 loopback", "all":["P4 loopback"]}
        with patch.object(subject.socket, "socket", return_value=FakeSocket(safe)):
            subject.controller(Path("/synthetic.sock"), 123)
        for wrong in ({**safe, "now":"DIRECT"}, {**safe, "all":["P4 loopback", "DIRECT"]}, {**safe, "type":"URLTest"}):
            with patch.object(subject.socket, "socket", return_value=FakeSocket(wrong)):
                with self.assertRaises(subject.Refused) as raised:
                    subject.controller(Path("/synthetic.sock"), 123)
                self.assertEqual(raised.exception.stage, "no_direct_fallback")

    def test_same_parent_namespace_refuses_before_effects(self):
        args = argparse.Namespace(scratch="/synthetic", parent_pid="9", parent_net="10", parent_user="11", parent_net_fd="20", parent_user_fd="21")
        with patch.object(subject, "private_directory", return_value=True), patch.object(subject.os, "getuid", return_value=0), patch.object(subject.os, "getppid", return_value=9), patch.object(subject.os, "fstat", side_effect=lambda fd: argparse.Namespace(st_ino=10 if fd == 20 else 11)), patch.object(subject, "namespace", side_effect=lambda name, pid="self": 10 if name == "net" else 11), patch.object(subject, "private_write") as write, patch.object(subject, "command") as command:
            with self.assertRaises(subject.Refused) as raised:
                subject.namespace_run(args)
            self.assertEqual(raised.exception.stage, "namespace_identity")
            write.assert_not_called(); command.assert_not_called()

    def test_capability_and_nnp_proof_is_required(self):
        process = argparse.Namespace(pid=123, poll=lambda: None)
        safe = "NoNewPrivs:\t1\nCapEff:\t00000000\nCapPrm:\t00000000\nCapAmb:\t00000000\n"
        with patch.object(subject, "namespace", return_value=10), patch.object(Path, "read_text", return_value=safe):
            subject.check_process(process, 10)
        for wrong in (safe.replace("NoNewPrivs:\t1", "NoNewPrivs:\t0"), safe.replace("CapEff:\t00000000", "CapEff:\t00000001")):
            with patch.object(subject, "namespace", return_value=10), patch.object(Path, "read_text", return_value=wrong):
                with self.assertRaises(subject.Refused) as raised:
                    subject.check_process(process, 10)
                self.assertEqual(raised.exception.stage, "child_privileges")


if __name__ == "__main__":
    unittest.main()
