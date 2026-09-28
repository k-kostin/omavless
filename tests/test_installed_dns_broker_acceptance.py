#!/usr/bin/python3
# SPDX-License-Identifier: MIT
"""No-effect checks for the installed-owner DNS acceptance gate."""
import importlib.util
import io
import json
import os
import signal
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch


spec = importlib.util.spec_from_file_location(
    "installed_dns_broker_acceptance", Path(__file__).with_name("installed_dns_broker_acceptance.py"))
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class Terminal(io.StringIO):
    def isatty(self):
        return True


class InstalledDnsBrokerAcceptanceTests(unittest.TestCase):
    def test_durable_selection_requires_private_exact_regular_marker(self):
        with tempfile.TemporaryDirectory() as directory:
            config = Path(directory)
            os.chmod(config, 0o700)
            marker = config / "managed-dns-selection"
            with self.assertRaisesRegex(gate.gate.Failure, "^installed_core_not_selected$"):
                gate.durable_core_selection(config, os.getuid())
            marker.write_bytes(gate.SELECTION_BYTES)
            os.chmod(marker, 0o600)
            gate.durable_core_selection(config, os.getuid())
            marker.write_bytes(b"managed-dns-source-pair-v1\nextra")
            with self.assertRaisesRegex(gate.gate.Failure, "^installed_core_not_selected$"):
                gate.durable_core_selection(config, os.getuid())
            marker.write_bytes(gate.SELECTION_BYTES)
            os.chmod(marker, 0o644)
            with self.assertRaisesRegex(gate.gate.Failure, "^installed_core_not_selected$"):
                gate.durable_core_selection(config, os.getuid())
            marker.unlink()
            marker.symlink_to(config / "other")
            with self.assertRaisesRegex(gate.gate.Failure, "^installed_core_not_selected$"):
                gate.durable_core_selection(config, os.getuid())

    @staticmethod
    def stat_row(pid, state, parent, group, started=51):
        return (f"{pid} (synthetic (core)) {state} {parent} {group} "
                + "0 " * 16 + str(started) + "\n").encode()

    def test_refused_terminal_precedes_installed_or_private_access(self):
        authorization = gate.auth.HumanAuthorization(io.StringIO(), io.StringIO())
        with patch.object(gate, "installed_identity") as identity, \
                patch.object(gate.installed, "cli") as cli, \
                self.assertRaises(gate.auth.AuthorizationUnsettled):
            gate.run_gate(authorization, "0" * 64, "0" * 64)
        identity.assert_not_called()
        cli.assert_not_called()

    def test_missing_broker_pin_refuses_before_file_or_service_access(self):
        with patch.object(Path, "lstat") as inspect, \
                patch.object(gate, "fixed_command") as command, \
                self.assertRaisesRegex(gate.gate.Failure, "^broker_pin_required$"):
            gate.broker_identity(None)
        inspect.assert_not_called()
        command.assert_not_called()

    def test_root_broker_projection_requires_fixed_service_and_process(self):
        command = ("{ path=/usr/lib/omavless/omavless-dns-broker ; "
                   "argv[]=/usr/lib/omavless/omavless-dns-broker --serve ; "
                   "ignore_errors=no ; start_time=[test] ; stop_time=[n/a] ; "
                   "pid=42 ; code=(null) ; status=0/0 }")
        status = [b"Name:\tomavless-dns-br", b"Uid:\t0\t0\t0\t0"]
        cgroup = b"0::/system.slice/omavless-dns-broker.service\n"
        gate.running_broker_projection(command, 42, status, cgroup)
        for altered in (
            (command.replace(" --serve", " --other"), 42, status, cgroup),
            (command, 43, status, cgroup),
            (command, 42, status[1:], cgroup),
            (command, 42, [status[0], b"Uid:\t1000\t1000\t1000\t1000"], cgroup),
            (command, 42, status, b"0::/other.slice/omavless-dns-broker.service\n"),
        ):
            with self.assertRaisesRegex(gate.gate.Failure, "^running_broker_unverified$"):
                gate.running_broker_projection(*altered)

    def test_capability_core_projection_uses_parent_uid_and_exact_caps(self):
        status = (b"Name:\tmihomo\nState:\tS (sleeping)\nPPid:\t41\n"
                  b"Uid:\t1000\t1000\t1000\t1000\nCapEff:\t0000000000003400\n")
        gate.core_process_projection(status, 41, 1000)
        for altered in (status.replace(b"PPid:\t41", b"PPid:\t42"),
                        status.replace(b"S (sleeping)", b"Z (zombie)"),
                        status.replace(b"1000\t1000", b"1001\t1001"),
                        status.replace(b"0000000000003400", b"0000000000000000")):
            with self.assertRaisesRegex(gate.gate.Failure, "^owned_core_unverified$"):
                gate.core_process_projection(altered, 41, 1000)

    def test_stat_parser_uses_final_parenthesis_and_bounds_identity(self):
        self.assertEqual(gate.process_stat(self.stat_row(42, "Z", 41, 42)),
                         (42, b"Z", 41, 42, 51))
        for malformed in (b"42 (bad) Z 41\n", b"42 bad Z 41 42\n",
                          self.stat_row(42, "Z", 41, 42) + b"x" * 4096):
            with self.assertRaisesRegex(gate.gate.Failure, "^core_crash_unverified$"):
                gate.process_stat(malformed)

    def test_zombie_leader_is_not_a_live_group_member(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for pid, state, group in ((42, "Z", 42), (43, "S", 900)):
                path = root / str(pid)
                path.mkdir()
                (path / "stat").write_bytes(self.stat_row(pid, state, 41, group))
            self.assertEqual(gate.live_core_group_members(42, root), 0)
            (root / "43" / "stat").write_bytes(self.stat_row(43, "S", 41, 42))
            self.assertEqual(gate.live_core_group_members(42, root), 1)

    def test_pidfd_crash_signals_only_verified_owned_core(self):
        row = self.stat_row(42, "S", 41, 42)
        with patch.object(gate.gate, "bounded", return_value=row), \
                patch.object(gate.os, "pidfd_open", return_value=7) as opened, \
                patch.object(gate.os, "close") as closed, \
                patch.object(signal, "pidfd_send_signal") as sent:
            self.assertEqual(gate.crash_verified_core(42, 41), 51)
        opened.assert_called_once_with(42, 0)
        sent.assert_called_once_with(7, signal.SIGKILL, None, 0)
        closed.assert_called_once_with(7)
        with patch.object(gate.gate, "bounded", return_value=self.stat_row(42, "S", 999, 42)), \
                patch.object(gate.os, "pidfd_open") as opened:
            with self.assertRaisesRegex(gate.gate.Failure, "^core_crash_unverified$"):
                gate.crash_verified_core(42, 41)
        opened.assert_not_called()

    def test_crash_effect_needs_its_own_authorization_barrier(self):
        authorization = gate.auth.HumanAuthorization(Terminal("ready\nsettled\n"), Terminal())
        value = authorization.step("core_crash", lambda: "signalled")
        self.assertEqual(value, "signalled")
        self.assertEqual(authorization.output.getvalue().count("Type ready"), 1)

    def test_crash_release_uses_fresh_facts_not_last_known_actual(self):
        observed = {"availability": "observed", "lastKnownActual": "connected",
                    "manualRecoveryRequired": False,
                    "facts": {"ownedCoreRunning": False, "visibleTunCount": 0}}
        output = []
        with patch.object(gate.gate, "bounded", return_value=self.stat_row(42, "Z", 41, 42)), \
                patch.object(gate, "live_core_group_members", return_value=0), \
                patch.object(gate.installed, "cli", return_value={"result": observed}), \
                patch.object(gate.installed, "tuns", return_value=set()), \
                patch.object(gate, "broker_state") as broker, \
                patch.object(gate.gate, "emit", side_effect=lambda **item: output.append(item)):
            gate.crash_release_evidence(42, 41, 51)
        broker.assert_called_once_with(0)
        self.assertEqual(output, [{"core_dead_pinned": True,
                                   "live_core_group_members": 0,
                                   "tun_released": True, "dns_released": True,
                                   "fresh_owned_core_running": False,
                                   "last_known_connected": True}])

    def test_private_action_keeps_profile_out_of_process_arguments(self):
        secret_id = "private-profile-id"
        request_id = "installed-gate-" + "a" * 32
        reply = json.dumps({"api": "omavless.control", "version": 1,
                            "id": request_id, "ok": True, "revision": 8,
                            "result": {"applied": True}}).encode() + b"\n"
        class Connection:
            def __init__(self):
                self.sent = None
                self.replied = False
            def __enter__(self):
                return self
            def __exit__(self, *_):
                return False
            def settimeout(self, _):
                pass
            def connect(self, _):
                pass
            def getsockopt(self, *_):
                return gate.struct.pack("3i", 41, gate.os.getuid(), 0)
            def sendall(self, value):
                self.sent = value
            def shutdown(self, _):
                pass
            def recv(self, _):
                if not self.replied:
                    self.replied = True
                    return reply
                return b""
        connection = Connection()
        metadata = Mock(st_mode=gate.stat.S_IFSOCK | 0o600, st_uid=gate.os.getuid())
        directory = Mock(st_mode=gate.stat.S_IFDIR | 0o700, st_uid=gate.os.getuid())
        with patch.object(gate.installed, "cli", return_value={
                "revision": 7, "result": {"instanceId": "runtime-instance"}}), \
                patch.object(Path, "lstat", side_effect=[directory, metadata]), \
                patch.object(gate.socket, "socket", return_value=connection), \
                patch.object(gate.uuid, "uuid4", return_value=Mock(hex="a" * 32)), \
                patch.object(gate.subprocess, "run") as process:
            gate.owned_action(41, "connect", secret_id, "global")
        request = json.loads(connection.sent)
        self.assertEqual(request["params"]["profileId"], secret_id)
        self.assertEqual(request["params"]["expectedRevision"], 7)
        process.assert_not_called()

    def test_profile_selection_is_exact_and_never_prints_private_fields(self):
        candidate = {"id": "selected", "protocol": "vless", "missing": False,
                     "uri": "private material"}
        state = {"lastProfileId": "selected", "profiles": [
            {"id": "other", "protocol": "vless", "missing": False}, candidate]}
        self.assertEqual(gate.selected_profile(state), "selected")
        self.assertEqual(gate.selected_profile(state, 0), "other")
        self.assertEqual(gate.selected_profile(state, 1), "selected")
        for index in (-1, 2, True):
            with self.assertRaisesRegex(gate.gate.Failure, "^vless_fixture_unavailable$"):
                gate.selected_profile(state, index)
        state["profiles"][1]["missing"] = True
        with self.assertRaisesRegex(gate.gate.Failure, "^vless_fixture_unavailable$"):
            gate.selected_profile(state)
        with self.assertRaisesRegex(gate.gate.Failure, "^vless_fixture_unavailable$"):
            gate.selected_profile(state, 1)

    def test_each_mode_effect_needs_its_own_human_barrier(self):
        words = "ready\nsettled\n" * 3
        authorization = gate.auth.HumanAuthorization(Terminal(words), Terminal())
        effects, checks = [], []
        gate.mode_sequence(authorization, effects.append, checks.append)
        self.assertEqual(effects, ["rule", "direct", "global"])
        self.assertEqual(checks, effects)
        self.assertEqual(authorization.output.getvalue().count("Type ready"), 3)

    def test_mode_refusal_stops_before_the_next_effect(self):
        authorization = gate.auth.HumanAuthorization(Terminal("ready\nsettled\nno\n"), Terminal())
        effects = []
        with self.assertRaises(gate.auth.AuthorizationUnsettled):
            gate.mode_sequence(authorization, effects.append, Mock())
        self.assertEqual(effects, ["rule"])
        self.assertTrue(authorization.blocked)

    def test_failed_mode_verification_prevents_further_effects(self):
        authorization = gate.auth.HumanAuthorization(Terminal("ready\nsettled\n" * 3), Terminal())
        effects = []
        def verify(mode):
            raise gate.gate.Failure("dns_readback_missing")
        with self.assertRaisesRegex(gate.gate.Failure, "^dns_readback_missing$"):
            gate.mode_sequence(authorization, effects.append, verify)
        self.assertEqual(effects, ["rule"])

    def test_tun_https_tries_fixed_independent_targets_without_leaking_urls(self):
        failed = Mock(returncode=28, stdout=b"000")
        passed = Mock(returncode=0, stdout=b"204")
        counters = [(0, 0), (1, 1), (1, 1), (2, 2)]
        output = []
        with patch.object(gate.gate, "tun_counters", side_effect=counters), \
                patch.object(gate.subprocess, "run", side_effect=[failed, passed]) as run, \
                patch.object(gate.gate, "emit", side_effect=lambda **item: output.append(item)):
            self.assertTrue(gate.probe_tun_https())
        self.assertEqual(run.call_count, 2)
        self.assertEqual([call.args[0][-1] for call in run.call_args_list],
                         list(gate.HTTPS_TARGETS[:2]))
        self.assertEqual([item["https_target"] for item in output], [1, 2])
        self.assertEqual(output[-1]["classification"], "pass")
        self.assertTrue(output[-1]["tun_rx_moved"])
        self.assertTrue(output[-1]["tun_tx_moved"])
        self.assertNotIn("https://", str(output))

    def test_direct_ip_diagnostic_uses_fixed_resolve_and_reports_only_classes(self):
        class Process:
            returncode = 0
            def __init__(self):
                self.poll_count = 0
            def __enter__(self):
                return self
            def __exit__(self, *_):
                return False
            def poll(self):
                self.poll_count += 1
                return None if self.poll_count == 1 else 0
            def communicate(self, timeout=None):
                return b"200", b""
        output = []
        with patch.object(gate.gate, "tun_counters", side_effect=[(0, 0), (1, 1)]), \
                patch.object(gate.subprocess, "Popen", return_value=Process()) as popen, \
                patch.object(gate, "core_tun_tracker_count", return_value=1), \
                patch.object(gate.time, "sleep"), \
                patch.object(gate.gate, "emit", side_effect=lambda **item: output.append(item)):
            gate.probe_direct_ip_tun(42)
        arguments = popen.call_args.args[0]
        self.assertEqual(arguments[-1], gate.DIRECT_IP_HTTPS)
        self.assertEqual(arguments[-3:-1], ["--resolve", gate.DIRECT_IP_RESOLVE])
        self.assertEqual(output, [{"direct_ip_https": True, "direct_ip_tun_used": True,
                                   "direct_ip_classification": "pass",
                                   "direct_ip_tracker_observed": True,
                                   "direct_ip_tun_trackers_seen": True,
                                   "direct_ip_tun_rx_moved": True,
                                   "direct_ip_tun_tx_moved": True}])
        self.assertNotIn("cloudflare", str(output))

    def test_optional_tracker_failure_cannot_abort_https_diagnostic(self):
        class Process:
            returncode = 28
            def __init__(self):
                self.poll_count = 0
            def __enter__(self):
                return self
            def __exit__(self, *_):
                return False
            def poll(self):
                self.poll_count += 1
                return None if self.poll_count == 1 else 0
            def communicate(self, timeout=None):
                return b"000", b"private transport error"
        output = []
        with patch.object(gate.gate, "tun_counters", side_effect=[(0, 0), (1, 1)]), \
                patch.object(gate.subprocess, "Popen", return_value=Process()), \
                patch.object(gate, "core_tun_tracker_count", side_effect=ValueError("private")), \
                patch.object(gate.time, "sleep"), \
                patch.object(gate.gate, "emit", side_effect=lambda **item: output.append(item)):
            gate.probe_direct_ip_tun(42)
        self.assertEqual(output[0]["direct_ip_classification"], "probe_timeout")
        self.assertFalse(output[0]["direct_ip_tracker_observed"])
        self.assertNotIn("private", str(output))

    def test_tracker_projection_does_not_emit_private_metadata(self):
        payload = json.dumps({"connections": [
            {"metadata": {"type": "Tun", "host": "private.example"}},
            {"metadata": {"type": "HTTP", "host": "private.example"}},
        ]}).encode()
        self.assertEqual(gate.tun_tracker_count(payload), 1)
        self.assertEqual(gate.tun_tracker_count(b'{"connections":null}'), 0)
        for invalid in (b"{}", b'{"connections":{}}',
                        b'{"connections":[{"metadata":"private"}]}'):
            with self.assertRaisesRegex(gate.gate.Failure, "^core_controller_response$"):
                gate.tun_tracker_count(invalid)

    def test_route_projection_emits_only_fixed_classes_and_rule_count(self):
        output = []
        def command(arguments):
            if arguments[-2:] == ["get", "1.1.1.1"]:
                value = [{"dev": "Meta", "gateway": "private"}]
            elif arguments[-2:] == ["get", "198.18.0.78"]:
                value = [{"dev": "eth0", "gateway": "private"}]
            else:
                value = [{"priority": 0}, {"priority": 32766}]
            return Mock(stdout=json.dumps(value).encode())
        with patch.object(gate, "fixed_command", side_effect=command), \
                patch.object(gate.gate, "bounded", return_value=b"1\n"), \
                patch.object(gate.gate, "emit", side_effect=lambda **item: output.append(item)):
            gate.route_projection()
        self.assertEqual(output, [{"public_route": "Meta", "fakeip_route": "other",
                                   "ipv4_rule_count": 2, "all_rp_filter": 1,
                                   "tun_rp_filter": 1}])
        self.assertNotIn("private", str(output))

    def test_ufw_projection_is_read_only_and_never_claims_packet_verdict(self):
        output = []
        for exit_code, expected in ((0, "active"), (3, "unknown")):
            with patch.object(gate.subprocess, "run", return_value=Mock(
                    returncode=exit_code, stdout=b"private", stderr=b"private")) as run, \
                    patch.object(gate.gate, "emit", side_effect=lambda **item: output.append(item)):
                self.assertEqual(gate.ufw_projection(), expected)
            self.assertEqual(run.call_args.args[0], [
                "/usr/bin/systemctl", "--system", "is-active", "--quiet", "ufw.service"])
            self.assertEqual(run.call_args.kwargs["stdin"], gate.subprocess.DEVNULL)
            self.assertEqual(run.call_args.kwargs["timeout"], 5)
        self.assertEqual(output, [
            {"ufw_service": "active", "tun_ingress_policy": "review_required"},
            {"ufw_service": "unknown", "tun_ingress_policy": "unverified"},
        ])
        self.assertNotIn("private", str(output))

    def test_ufw_projection_failure_remains_unknown_and_has_no_host_effect(self):
        output = []
        with patch.object(gate.subprocess, "run", side_effect=OSError("private")), \
                patch.object(gate.gate, "emit", side_effect=lambda **item: output.append(item)):
            self.assertEqual(gate.ufw_projection(), "unknown")
        self.assertEqual(output, [{"ufw_service": "unknown",
                                   "tun_ingress_policy": "unverified"}])

    def test_core_proxy_diagnostic_is_fixed_and_not_tun_acceptance(self):
        output = []
        with patch.object(gate.subprocess, "run", return_value=Mock(returncode=0, stdout=b"204")) as run, \
                patch.object(gate.gate, "emit", side_effect=lambda **item: output.append(item)):
            self.assertTrue(gate.probe_core_proxy_https())
        arguments = run.call_args.args[0]
        self.assertEqual(arguments[-1], gate.HTTPS_TARGETS[0])
        self.assertIn("http://127.0.0.1:7890", arguments)
        self.assertEqual(output, [{"core_proxy_https": True}])

    def test_failure_classification_is_specific_but_cannot_echo_private_text(self):
        self.assertEqual(gate.failure_classification(
            gate.gate.Failure("broker_retention_mismatch")), "broker_retention_mismatch")
        self.assertEqual(gate.failure_classification(
            gate.auth.AuthorizationUnsettled()), "human_authorization_unsettled")
        for error in (gate.gate.Failure("secret.example.com"),
                      gate.gate.Failure("private name"),
                      gate.gate.Failure("private_token"), ValueError("private")):
            self.assertEqual(gate.failure_classification(error), "installed_dns_gate_failed")
        self.assertEqual(gate.failure_type(FileNotFoundError("private")), "FileNotFoundError")
        self.assertEqual(gate.failure_type(ValueError("private")), "ValueError")
        self.assertEqual(gate.failure_type(gate.gate.Failure("private")), "other")


if __name__ == "__main__":
    unittest.main()
