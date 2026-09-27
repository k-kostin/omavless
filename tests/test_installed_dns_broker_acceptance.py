#!/usr/bin/python3
# SPDX-License-Identifier: MIT
"""No-effect checks for the installed-owner DNS acceptance gate."""
import importlib.util
import io
from pathlib import Path
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


if __name__ == "__main__":
    unittest.main()
