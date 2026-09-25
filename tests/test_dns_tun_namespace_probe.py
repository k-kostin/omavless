"""No namespaces/devices/privileged work: safety guards with injected adapters."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("dns_ns_probe", Path(__file__).with_name("dns_tun_namespace_probe.py"))
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class NamespaceProbeTests(unittest.TestCase):
    def run_parent(self, result=None, failure=None, uid=1000, argv=None):
        output = io.StringIO()
        with patch.object(probe.sys, "argv", argv or ["probe"]), \
             patch.object(probe.os, "geteuid", return_value=uid), \
             patch.object(probe, "namespace", side_effect=lambda kind: "original-" + kind), \
             patch.object(probe.subprocess, "run", return_value=result, side_effect=failure) as command, \
             contextlib.redirect_stdout(output):
            code = probe.main()
        return code, output.getvalue(), command

    def test_root_parent_refuses_before_namespace_or_device_work(self):
        code, _, command = self.run_parent(uid=0)
        self.assertEqual(code, 2)
        command.assert_not_called()

    def test_extra_args_refuse(self):
        code, _, command = self.run_parent(argv=["probe", "--host"])
        self.assertEqual(code, 2)
        command.assert_not_called()

    def test_parent_always_requests_both_new_namespaces(self):
        result = subprocess.CompletedProcess([], 0, b'{"isolated":true}', b"")
        code, _, command = self.run_parent(result)
        self.assertEqual(code, 0)
        args = command.call_args.args[0]
        self.assertEqual(args[:6], ["/usr/bin/unshare", "--user", "--map-root-user", "--net", "--", probe.sys.executable])
        self.assertEqual(args[-3:], ["--isolated-child", "original-net", "original-user"])
        self.assertEqual(command.call_args.kwargs["stdin"], subprocess.DEVNULL)

    def test_namespace_failure_does_not_echo_stderr(self):
        result = subprocess.CompletedProcess([], 1, b"", b"private-sentinel")
        code, text, _ = self.run_parent(result)
        self.assertEqual(code, 1)
        self.assertNotIn("sentinel", text)

    def test_missing_unshare_or_timeout_is_bounded(self):
        for failure in [OSError("private-sentinel"), subprocess.TimeoutExpired("private-sentinel", 10)]:
            with self.subTest(failure=type(failure)):
                code, text, _ = self.run_parent(failure=failure)
                self.assertEqual(code, 1)
                self.assertNotIn("sentinel", text)

    def test_projection_rejects_values_keys_types_and_oversized_output(self):
        for raw in [b'{"isolated":"private-sentinel"}', b'{"private-sentinel":true}',
                    b'{"isolated":1}', b"{}", b"[]", b"\xff", b" " * 1025]:
            code, text, _ = self.run_parent(subprocess.CompletedProcess([], 0, raw, b""))
            self.assertEqual(code, 1)
            self.assertNotIn("sentinel", text)

    def test_child_requires_distinct_net_and_user_namespace_before_inventory(self):
        for net, user in [("original-net", "new-user"), ("new-net", "original-user")]:
            with patch.object(probe, "namespace", side_effect=lambda kind: net if kind == "net" else user), \
                 patch.object(probe, "links") as links, patch.object(probe, "open_tun") as opening:
                with self.assertRaises(RuntimeError):
                    probe.child("original-net", "original-user")
                links.assert_not_called()
                opening.assert_not_called()

    def test_child_refuses_nonfresh_namespace_before_device_open(self):
        with patch.object(probe, "namespace", side_effect=lambda kind: "new-" + kind), \
             patch.object(probe.os, "geteuid", return_value=0), \
             patch.object(probe, "links", return_value=[{"ifname": "lo"}, {"ifname": "foreign"}]), \
             patch.object(probe, "open_tun") as opening:
            with self.assertRaises(RuntimeError):
                probe.child("original-net", "original-user")
            opening.assert_not_called()

    def test_device_closed_when_ioctl_fails(self):
        with patch.object(probe.os, "open", return_value=77), \
             patch.object(probe.fcntl, "ioctl", side_effect=OSError("not exposed")), \
             patch.object(probe.os, "close") as close:
            with self.assertRaises(OSError): probe.open_tun()
            close.assert_called_once_with(77)

    def test_safe_success_projection(self):
        raw = b'{"isolated":true,"same_name_and_index_reused":true,"old_fd_attached_after_replacement":false}'
        code, text, _ = self.run_parent(subprocess.CompletedProcess([], 0, raw, b""))
        self.assertEqual(code, 0)
        self.assertEqual(json.loads(text), json.loads(raw))


if __name__ == "__main__":
    unittest.main()
