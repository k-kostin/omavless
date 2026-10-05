# SPDX-License-Identifier: MIT
"""Pure 120s receipt/source gates; never invoke Go or the slow test in CI."""
import ast
import importlib.util
import json
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch

from tests import test_p4_default_elapsed_overlay as previous_guards

spec = importlib.util.spec_from_file_location("p4_default_rekey", Path(__file__).parent / "fixtures/p4_awg_peer/run_default_rekey_overlay.py")
subject = importlib.util.module_from_spec(spec)
spec.loader.exec_module(subject)


class DefaultRekeyGuards(unittest.TestCase):
    def events(self):
        return [{"Action": "output", "Package": subject.PACKAGE, "Test": subject.TEST, "Output": "    default_elapsed_rekey_test.go:450: p4_rekey_receipt pre_age_ns=117001000000 trigger_data_age_ns=121001000000 rekey_h1_age_ns=121002000000 observed_body_ns=121005000000 baseline_bidir=true pre_no_rekey=true echo_cancels_idle=true real_receive_aead=true new_indexed_sessions=true post_bidir=true client_h1=2 server_h1=0 server_h2=2 defaults_unchanged=true network_fds=false\n"}, {"Action": "pass", "Package": subject.PACKAGE, "Test": subject.TEST, "Elapsed": 121.01}, {"Action": "pass", "Package": subject.PACKAGE}]

    def encoded(self, events):
        return b"\n".join(json.dumps(event).encode() for event in events)

    def test_exact_single_causal_receipt(self):
        subject.verify_events(self.encoded(self.events()))

    def test_missing_duplicate_skipped_malformed_or_timeout_refuse(self):
        events = self.events()
        for wrong in ([], events[1:], events[:-1], events + events, events + [{"Action": "fail", "Package": subject.PACKAGE}], events + [{"Action": "skip", "Package": subject.PACKAGE}], events + [{"Action": "output", "Package": subject.PACKAGE, "Output": "panic: test timed out"}], events + [{"Package": subject.PACKAGE}], events + [{"Action": False, "Package": subject.PACKAGE}], events + [{"Action": "output", "Package": subject.PACKAGE, "Test": None, "Output": "PASS\n"}]):
            with self.assertRaises(ValueError):
                subject.verify_events(self.encoded(wrong))

    def test_shortened_age_wrong_cause_missing_aead_and_old_session_refuse(self):
        for old, new in (("pre_age_ns=117001000000", "pre_age_ns=120001000000"), ("trigger_data_age_ns=121001000000", "trigger_data_age_ns=1000000"), ("rekey_h1_age_ns=121002000000", "rekey_h1_age_ns=120000000000"), ("observed_body_ns=121005000000", "observed_body_ns=151000000000"), ("echo_cancels_idle=true", "echo_cancels_idle=false"), ("real_receive_aead=true", "real_receive_aead=false"), ("new_indexed_sessions=true", "new_indexed_sessions=false"), ("post_bidir=true", "post_bidir=false"), ("client_h1=2", "client_h1=3"), ("network_fds=false", "network_fds=true")):
            events = self.events()
            events[0]["Output"] = events[0]["Output"].replace(old, new)
            with self.subTest(field=old), self.assertRaises(ValueError):
                subject.verify_events(self.encoded(events))

    def test_case_actual_elapsed_includes_cleanup(self):
        for elapsed in (None, True, "121", 0, 119.99, 150.01, float("nan"), float("inf")):
            events = self.events()
            events[1]["Elapsed"] = elapsed
            with self.assertRaises(ValueError):
                subject.verify_events(self.encoded(events))

    def test_source_retains_real_pipeline_and_default_age(self):
        code = subject.OVERLAY.read_text()
        for forbidden in ("expiredRetransmitHandshake(", "keepKeyFreshSending(", "ConsumeMessageResponse(", "BeginSymmetricSession(", ".created =", ".Mod(", "net.Listen", "net.Dial", "os.Open("):
            self.assertNotIn(forbidden, code)
        for actual in ("[]conn.ReceiveFunc", "old.created.Add(117", "old.created.Add(121", "old.sendNonce.Load() >= RekeyAfterMessages", "cp.timers.newHandshake.IsPending", "p4RekeyMatch", "client.indexTable.Lookup", "server.indexTable.Lookup"):
            self.assertIn(actual, code)
        self.assertEqual(subject.SUPERVISOR_SHA, "00fa64cacdf72d65fcdd772208eca1bc4ca40ca954ef94444ff11eec65b729ec")

    def test_frozen_object_refuses_links_and_changed_modes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            member = root / "member"
            subject.owned.save(root, member.name, b"retained immutable input")
            self.assertEqual(subject.object_bytes(member, 100), b"retained immutable input")
            alias = root / "alias"
            alias.symlink_to(member)
            with self.assertRaises(OSError):
                subject.object_bytes(alias, 100)
            alias.unlink()
            os.link(member, alias)
            with self.assertRaises(ValueError):
                subject.object_bytes(member, 100)
            alias.unlink()
            member.chmod(0o620)
            with self.assertRaises(ValueError):
                subject.object_bytes(member, 100)

    def test_attempt_slot_and_supervision_uncertainty_are_not_pass(self):
        code = Path(subject.__file__).read_text()
        attempt = code.index('owned.save(artifacts, "execution-attempt.json"')
        execution = code.index('"tool", "test2json"')
        self.assertLess(attempt, execution)
        for guard in ('"supervision-refusal.json"', '"complete_output_retained": False',
                      '"unsettled_anchors": len(owned.UNSETTLED)', '"pass": False',
                      '"-test.count=1"', '"-test.timeout=180s"', 'if not owned.UNSETTLED:'):
            self.assertIn(guard, code)

    def test_read_epoch_diagnostic_is_not_elapsed_rekey_evidence(self):
        events = self.events()
        events[0]["Output"] = "    default_elapsed_rekey_test.go:300: p4_rekey_read_receipt first_offset=16 configured_padding=48 pending_offset=16 next_offset=64 empty_reads=1 emitted_packets=0 actual_worker=true network_fds=false\n"
        for event in events[:2]:
            event["Test"] = subject.DIAGNOSTIC
        events[1]["Elapsed"] = 0.001
        subject.verify_events(self.encoded(events), True)
        with self.assertRaises(ValueError):
            subject.verify_events(self.encoded(events))
        for old, new in (("pending_offset=16", "pending_offset=64"), ("next_offset=64", "next_offset=16"), ("empty_reads=1", "empty_reads=2"), ("emitted_packets=0", "emitted_packets=1"), ("actual_worker=true", "actual_worker=false")):
            wrong = json.loads(json.dumps(events))
            wrong[0]["Output"] = wrong[0]["Output"].replace(old, new)
            with self.assertRaises(ValueError):
                subject.verify_events(self.encoded(wrong), True)
        with self.assertRaises(ValueError):
            subject.verify_events(self.encoded(self.events()), True)

    def test_readiness_uses_real_read_entry_not_header_rewriting(self):
        code = subject.OVERLAY.read_text()
        for actual in ("m.activeOffset.Store(int32(offset))", "case m.readEntries <- offset:",
                       'd.IpcSet("s4=48\\n")', "m.input <- nil", "b.sends.Load() != 0",
                       "p4RekeyConfiguredRead(t, m,", "first != MessageTransportHeaderSize+48"):
            self.assertIn(actual, code)
        self.assertNotIn("offset = MessageTransportHeaderSize", code)


class RekeySupervisionGuards(unittest.TestCase):
    def test_uncertain_positive_channel_close_retains_graph_and_stops_next_close(self):
        owned = subject.owned
        for leaf in ("selector", "stdout", "stderr"):
            child, selector = MagicMock(), MagicMock()
            child.pid = 424242
            selector.select.return_value = []
            selector.get_map.return_value = {}
            failed = {"selector": selector, "stdout": child.stdout, "stderr": child.stderr}[leaf]
            failed.close.side_effect = OSError("synthetic close uncertainty")
            with self.subTest(leaf=leaf), patch.object(owned, "HELD_GRAPHS", []) as graphs, patch.object(owned, "UNSETTLED", []) as retained, patch.object(owned.subprocess, "Popen", return_value=child) as spawn, patch.object(owned.selectors, "DefaultSelector", return_value=selector), patch.object(owned.os, "set_blocking"), patch.object(owned.os, "waitid", return_value=SimpleNamespace(si_pid=child.pid)), patch.object(owned.os, "waitpid", return_value=(child.pid, 0)) as reap, patch.object(owned, "members", return_value=[]), patch.object(owned.os, "killpg") as signal_group:
                with self.assertRaisesRegex(owned.Unsettled, "owned_channel_close_unknown_preserve"):
                    owned.command([], Path.cwd(), {}, 3)
                self.assertEqual(graphs, [{"child": child, "selector": selector}])
                self.assertEqual(retained, [child])
                failed.close.assert_called_once()
                if leaf == "selector": child.stdout.close.assert_not_called()
                if leaf != "stderr": child.stderr.close.assert_not_called()
                reap.assert_called_once()
                signal_group.assert_not_called()
                with self.assertRaisesRegex(owned.Unsettled, "prior_owned_group_unsettled"):
                    owned.command([], Path.cwd(), {}, 3)
                spawn.assert_called_once()

    def test_other_helper_bodies_and_export_bytes_remain_exact(self):
        old = ast.parse(Path(previous_guards.subject.__file__).read_text())
        new = ast.parse(subject.SUPERVISOR.read_text())
        for name in ("members", "private_directory", "save"):
            self.assertEqual(ast.dump(next(node for node in old.body if isinstance(node, ast.FunctionDef) and node.name == name)),
                             ast.dump(next(node for node in new.body if isinstance(node, ast.FunctionDef) and node.name == name)))
        self.assertEqual(subject.owned.HELPER_BYTES, previous_guards.subject.HELPER_BYTES)

    def test_first_main_wait_unknown_never_recovers_from_second_success(self):
        owned = subject.owned
        for failure in (OSError("first wait unknown"), ChildProcessError("lost anchor")):
            child, selector = MagicMock(), MagicMock()
            child.pid = 424242
            selector.select.return_value = []
            with self.subTest(error=type(failure).__name__), patch.object(owned, "HELD_GRAPHS", []) as graphs, patch.object(owned, "UNSETTLED", []) as retained, patch.object(owned.subprocess, "Popen", return_value=child) as spawn, patch.object(owned.selectors, "DefaultSelector", return_value=selector), patch.object(owned.os, "set_blocking"), patch.object(owned.os, "waitid", side_effect=[failure, SimpleNamespace(si_pid=child.pid)]) as wait_state, patch.object(owned.os, "waitpid") as reap, patch.object(owned.os, "killpg") as signal_group, patch.object(owned, "members") as inventory:
                with self.assertRaisesRegex(owned.Unsettled, "main_wait_anchor_unknown_preserve"):
                    owned.command([], Path.cwd(), {}, 3)
                self.assertEqual(retained, [child])
                self.assertEqual(graphs, [{"child": child, "selector": selector}])
                self.assertEqual(wait_state.call_count, 1)
                reap.assert_not_called()
                signal_group.assert_not_called()
                inventory.assert_not_called()
                child.wait.assert_not_called()
                selector.close.assert_not_called()
                child.stdout.close.assert_not_called()
                child.stderr.close.assert_not_called()
                with self.assertRaisesRegex(owned.Unsettled, "prior_owned_group_unsettled"):
                    owned.command([], Path.cwd(), {}, 3)
                self.assertEqual(spawn.call_count, 1)

    def test_final_reap_unknown_never_becomes_success_or_recovers(self):
        owned = subject.owned
        for failure in (OSError("reap unknown"), ChildProcessError("ECHILD")):
            child, selector = MagicMock(), MagicMock()
            child.pid = 424242
            selector.select.return_value = []
            selector.get_map.return_value = {}
            with self.subTest(error=type(failure).__name__), patch.object(owned, "HELD_GRAPHS", []) as graphs, patch.object(owned, "UNSETTLED", []) as retained, patch.object(owned.subprocess, "Popen", return_value=child) as spawn, patch.object(owned.selectors, "DefaultSelector", return_value=selector), patch.object(owned.os, "set_blocking"), patch.object(owned.os, "waitid", return_value=SimpleNamespace(si_pid=child.pid)) as wait_state, patch.object(owned.os, "waitpid", side_effect=[failure, (child.pid, 0)]) as reap, patch.object(owned.os, "killpg") as signal_group, patch.object(owned, "members", return_value=[]) as inventory:
                with self.assertRaisesRegex(owned.Unsettled, "owned_final_reap_unknown_preserve"):
                    owned.command([], Path.cwd(), {}, 3)
                self.assertEqual(retained, [child])
                self.assertEqual(graphs, [{"child": child, "selector": selector}])
                self.assertEqual(wait_state.call_count, 1)
                self.assertEqual(reap.call_count, 1)
                self.assertEqual(inventory.call_count, 1)
                signal_group.assert_not_called()
                child.wait.assert_not_called()
                selector.close.assert_not_called()
                child.stdout.close.assert_not_called()
                child.stderr.close.assert_not_called()
                with self.assertRaises(owned.Unsettled):
                    owned.command([], Path.cwd(), {}, 3)
                self.assertEqual(spawn.call_count, 1)

    def test_raw_reap_requires_exact_pid_and_exited_status(self):
        owned = subject.owned
        child = MagicMock(pid=424242)
        for returned in ((0, 0), (child.pid + 1, 0), (child.pid, 0x7f)):
            with self.subTest(returned=returned), patch.object(owned, "UNSETTLED", []) as retained, patch.object(owned.os, "waitpid", return_value=returned) as reap:
                with self.assertRaises(owned.Unsettled):
                    owned.reap(child, 1, known_exited=True)
                self.assertEqual(retained, [child])
                self.assertEqual(reap.call_count, 1)
                child.wait.assert_not_called()
        for raw, code in ((3 << 8, 3), (9, -9)):
            with patch.object(owned.os, "waitpid", return_value=(child.pid, raw)):
                self.assertEqual(owned.reap(child, 1, known_exited=True), code)
                self.assertEqual(child.returncode, code)


# Exercise the seven existing actual/mocked command controls against the new
# helper without editing the sealed helper or duplicating their fixtures.
for _name in ("test_supervisor_normal_and_timeout_owned_child",
              "test_selector_setup_failure_still_cancels_and_reaps",
              "test_unsettled_anchor_refuses_another_launch",
              "test_cancellation_inventory_failure_retains_unreaped_anchor",
              "test_unknown_or_lost_anchor_preserves_without_signal_or_reap",
              "test_live_output_limit_cancels_and_reaps",
              "test_closed_stdio_orphan_is_not_mistaken_for_quiescence"):
    def _run(self, name=_name):
        with patch.object(previous_guards, "subject", subject.owned):
            getattr(previous_guards.ElapsedGuards(name), name)()
    setattr(RekeySupervisionGuards, _name, _run)


if __name__ == "__main__":
    unittest.main()
