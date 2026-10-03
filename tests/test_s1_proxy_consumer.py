# SPDX-License-Identifier: MIT
"""Offline child-context harness checks; never uses installed GIO/core/bus."""
import importlib.util
import json
from pathlib import Path
import socket
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("s1_consumer", ROOT / "tests/s1_proxy_consumer/private_environment.py")
GATE = importlib.util.module_from_spec(spec)
spec.loader.exec_module(GATE)
DESKTOP = {"XDG_CURRENT_DESKTOP": "Hyprland", "XDG_SESSION_DESKTOP": "Hyprland", "DESKTOP_SESSION": "omarchy"}


class ChildProxyConsumerTests(unittest.TestCase):
    def test_child_environment_is_exact_private_and_never_copies_parent_proxy_values(self):
        parent = dict(DESKTOP, http_proxy="synthetic-private-parent", GIO_USE_PROXY_RESOLVER="synthetic-forced", LD_PRELOAD="synthetic")
        with patch.dict(GATE.os.environ, parent, clear=True):
            saved = dict(GATE.os.environ)
            original = GATE.selected({"http_proxy": "", "HTTP_PROXY": "literal=\nUnicode-Я"})
            values = GATE.child_environment(Path("/synthetic/private"), original)
            self.assertEqual(values["http_proxy"], "")
            self.assertEqual(values["HTTP_PROXY"], "literal=\nUnicode-Я")
            self.assertNotIn("https_proxy", values)
            self.assertNotIn("GIO_USE_PROXY_RESOLVER", values)
            self.assertNotIn("LD_PRELOAD", values)
            self.assertEqual(values["DBUS_SESSION_BUS_ADDRESS"], "unix:path=/synthetic/private/absent-user-bus")
            self.assertEqual(dict(GATE.os.environ), saved)
            restored = GATE.child_environment(Path("/synthetic/private"), GATE.selected())
            self.assertNotIn("http_proxy", restored)

    def test_wrong_desktop_and_incomplete_oversized_nul_or_unknown_context_refuse(self):
        with patch.dict(GATE.os.environ, {}, clear=True):
            with self.assertRaisesRegex(ValueError, "^Actual Hyprland context required$"):
                GATE.child_environment(Path("/synthetic"), GATE.selected())
        with patch.dict(GATE.os.environ, DESKTOP, clear=True):
            for values in ({}, GATE.selected({"http_proxy": "\x00"}), GATE.selected({"http_proxy": "x" * 1025})):
                with self.assertRaises(ValueError):
                    GATE.child_environment(Path("/synthetic"), values)
        with self.assertRaises(ValueError):
            GATE.selected({"arbitrary": "value"})

    def mocked_consumer(self, root, report, *, failure=None):
        with patch.dict(GATE.os.environ, DESKTOP, clear=True), patch.object(GATE.subprocess, "Popen") as create:
            process = create.return_value
            process.returncode = 0
            def communicate(payload, timeout):
                create.call_args.kwargs["stdout"].write(json.dumps(report).encode())
            process.communicate.side_effect = communicate
            result = GATE.consumer(root, GATE.selected(), 12345, 12346, "none", "synthetic", require_failure=failure)
            self.assertEqual(create.call_count, 1)
            self.assertEqual(create.call_args.args[0], ["/usr/bin/gjs", str(GATE.SCRIPT)])
            self.assertNotIn("synthetic", " ".join(create.call_args.args[0]))
            process.kill.assert_not_called()
            return result

    def test_consumer_accepts_only_exact_typed_bounded_receipt(self):
        with tempfile.TemporaryDirectory(prefix="s1-consumer-unit-") as name:
            root = Path(name)
            good = {"ok": True, "resolver": "GLibproxyResolver", "selection": "none", "sent": True}
            self.assertTrue(self.mocked_consumer(root, good))
            for bad in (dict(good, ok=1), dict(good, sent=1), dict(good, selection="http"),
                        dict(good, extra="synthetic-private"), dict(good, resolver="GSimpleProxyResolver"), {"ok": False, "phase": "unexpected"}):
                with self.assertRaisesRegex(ValueError, "^Ambiguous private GIO receipt$"):
                    self.mocked_consumer(root, bad)
            self.assertFalse(self.mocked_consumer(root, {"ok": False, "phase": "dial"}, failure="dial"))
            with self.assertRaisesRegex(ValueError, "^Expected private consumer failure stage missing$"):
                self.mocked_consumer(root, {"ok": False, "phase": "resolver"}, failure="dial")

    def test_consumer_timeout_reaps_only_owned_child_without_retry_or_raw_output(self):
        with tempfile.TemporaryDirectory(prefix="s1-consumer-unit-") as name:
            with patch.dict(GATE.os.environ, DESKTOP, clear=True), patch.object(GATE.subprocess, "Popen") as create:
                process = create.return_value
                process.communicate.side_effect = subprocess.TimeoutExpired("synthetic-private", 8)
                with self.assertRaises(subprocess.TimeoutExpired):
                    GATE.consumer(Path(name), GATE.selected(), 12345, 12346, "none", "synthetic")
                process.kill.assert_called_once()
                process.wait.assert_called_once_with(timeout=3)
                self.assertEqual(create.call_count, 1)
                self.assertIs(create.call_args.kwargs["stderr"], subprocess.DEVNULL)

    def test_readiness_refusal_never_launches_consumer_or_delivers_request(self):
        target = Mock(port=12345, receipts=[])
        with patch.object(GATE, "readiness", return_value=False), patch.object(GATE, "consumer") as consumer:
            self.assertFalse(GATE.admit(Path("/synthetic"), GATE.selected(), target, 12346, "http", "synthetic"))
            consumer.assert_not_called()
            self.assertEqual(target.receipts, [])

    def test_consumer_success_without_actual_target_receipt_is_not_success(self):
        target = Mock(port=12345, receipts=[])
        with patch.object(GATE, "readiness", return_value=True), patch.object(GATE, "consumer", return_value=True):
            with self.assertRaisesRegex(ValueError, "^Real HTTP application receipt required$"):
                GATE.admit(Path("/synthetic"), GATE.selected(), target, 12346, "http", "synthetic")

    def test_missing_and_bound_only_listeners_fail_protocol_readiness(self):
        with GATE.Target() as target:
            with socket.socket() as bound:
                bound.bind(("127.0.0.1", 0))
                port = bound.getsockname()[1]
                self.assertFalse(GATE.readiness(port, target.port))
                bound.listen(1)
                self.assertFalse(GATE.readiness(port, target.port))
            self.assertEqual(target.receipts, [])

    def test_actual_target_receipt_is_strict_and_owned_thread_cleanup_completes(self):
        with GATE.Target() as target:
            with socket.create_connection(("127.0.0.1", target.port), timeout=1) as connection:
                GATE.http_receipt(connection, "synthetic")
            self.assertEqual(target.receipts, ["synthetic"])
        self.assertFalse(target.thread.is_alive())

    def test_unpinned_or_unsafe_core_refuses_before_process_creation(self):
        with tempfile.TemporaryDirectory(prefix="s1-consumer-unit-") as name:
            root = Path(name)
            core = root / "core"
            core.write_bytes(b"synthetic-not-a-core")
            with patch.object(GATE.subprocess, "Popen") as create:
                for path in (Path("relative"), core):
                    with self.assertRaises(ValueError):
                        GATE.exercise(path, root)
                create.assert_not_called()

    def test_fixture_uses_default_resolver_not_forced_module_or_client_endpoint(self):
        data = GATE.SCRIPT.read_text()
        self.assertIn("Gio.ProxyResolver.get_default()", data)
        self.assertIn("client.connect_to_uri(uri, request.target, null)", data)
        for forbidden in ("set_proxy_resolver", "SimpleProxyResolver", "Gio.Settings", "GLib.setenv", "set_enable_proxy"):
            self.assertNotIn(forbidden, data)


if __name__ == "__main__":
    unittest.main()
