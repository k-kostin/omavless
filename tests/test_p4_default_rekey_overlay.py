# SPDX-License-Identifier: MIT
"""Pure 120s receipt/source gates; never invoke Go or the slow test in CI."""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest

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
        self.assertEqual(subject.SUPERVISOR_SHA, "dcec0ed6b5bbde5fc6015f8a66dd1a3255cdfa2f3c2d6b8802d046edacfde165")

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


if __name__ == "__main__":
    unittest.main()
