# SPDX-License-Identifier: MIT
"""CPU receipt/owned-child guards; no Go build, VM or network in ordinary CI."""
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("p4_default_elapsed", Path(__file__).parent / "fixtures/p4_awg_peer/run_default_elapsed_overlay.py")
subject = importlib.util.module_from_spec(spec)
spec.loader.exec_module(subject)


class ElapsedGuards(unittest.TestCase):
    def events(self, count=1):
        events = []
        for _ in range(count):
            events += [{"Action": "output", "Package": subject.PACKAGE, "Test": subject.TEST, "Output": "    default_elapsed_retry_test.go:249: p4_elapsed_receipt retry_target_ns=5123000000 observed_retry_ns=5124000000 quiet_window_ns=5501000000 malformed_worker_refused=true actual_session=true h1_initial=1 h1_retry=1 h1_after=0 h4_after=1 defaults_unchanged=true\n"}, {"Action": "pass", "Package": subject.PACKAGE, "Test": subject.TEST}]
        return events + [{"Action": "pass", "Package": subject.PACKAGE}]

    def encoded(self, events):
        return b"\n".join(json.dumps(event).encode() for event in events)

    def test_exact_one_and_two_actual_numeric_receipts(self):
        for count in (1, 2):
            subject.verify_events(self.encoded(self.events(count)), count)

    def test_missing_duplicate_skip_timeout_or_malformed_never_pass(self):
        events = self.events()
        wrong = [[], events[1:], events[:-1], events + events, self.events(2), events + [{"Action": "fail", "Package": subject.PACKAGE, "Test": subject.TEST}], events + [{"Action": "output", "Package": subject.PACKAGE, "Output": "panic: test timed out"}], events + [{"Package": subject.PACKAGE}], events + [{"Action": "output", "Package": subject.PACKAGE, "Test": None}]]
        for candidate in wrong:
            with self.subTest(candidate=candidate), self.assertRaises(ValueError):
                subject.verify_events(self.encoded(candidate), 1)

    def test_callback_only_or_early_retry_short_window_and_bad_worker_refuse(self):
        output = self.events()[0]["Output"]
        for old, new in (("observed_retry_ns=5124000000", "observed_retry_ns=1000000"), ("retry_target_ns=5123000000", "retry_target_ns=1000000"), ("quiet_window_ns=5501000000", "quiet_window_ns=1000000"), ("malformed_worker_refused=true", "malformed_worker_refused=false"), ("h1_after=0", "h1_after=1"), ("defaults_unchanged=true", "defaults_unchanged=false")):
            events = self.events()
            events[0]["Output"] = output.replace(old, new)
            with self.subTest(field=old), self.assertRaises(ValueError):
                subject.verify_events(self.encoded(events), 1)

    def test_supervisor_normal_and_timeout_owned_child(self):
        env = {"PATH": "/usr/bin:/bin"}
        code, out, err = subject.command([sys.executable, "-I", "-c", "print('fixed')"], Path.cwd(), env, 3)
        self.assertEqual((code, out, err), (0, b"fixed\n", b""))
        with self.assertRaisesRegex(ValueError, "fixed_command_timeout"):
            subject.command([sys.executable, "-I", "-c", "import time; time.sleep(60)"], Path.cwd(), env, 0.05)
        self.assertEqual(subject.UNSETTLED, [])

    def test_selector_setup_failure_still_cancels_and_reaps(self):
        spawned = []
        original = subprocess.Popen
        def capture(*args, **kwargs):
            child = original(*args, **kwargs)
            spawned.append(child)
            return child
        with patch.object(subject.subprocess, "Popen", capture), patch.object(subject.selectors, "DefaultSelector", side_effect=OSError("setup")):
            with self.assertRaisesRegex(OSError, "setup"):
                subject.command([sys.executable, "-I", "-c", "import time; time.sleep(60)"], Path.cwd(), {"PATH": "/usr/bin:/bin"}, 3)
        self.assertEqual(len(spawned), 1)
        self.assertEqual(spawned[0].returncode, -signal.SIGKILL)
        self.assertFalse(Path(f"/proc/{spawned[0].pid}").exists())
        self.assertEqual(subject.UNSETTLED, [])

    def test_unsettled_anchor_refuses_another_launch(self):
        with patch.object(subject, "UNSETTLED", [object()]), patch.object(subject.subprocess, "Popen") as spawn:
            with self.assertRaisesRegex(subject.Unsettled, "prior_owned_group_unsettled"):
                subject.command([], Path.cwd(), {}, 1)
            spawn.assert_not_called()

    def test_closed_stdio_orphan_is_not_mistaken_for_quiescence(self):
        spawned = []
        original = subprocess.Popen
        def capture(*args, **kwargs):
            child = original(*args, **kwargs)
            spawned.append(child)
            return child
        code = "import os,time; child=os.fork(); os.close(1); os.close(2); time.sleep(60) if child==0 else os._exit(0)"
        with patch.object(subject.subprocess, "Popen", capture):
            with self.assertRaisesRegex(ValueError, "fixed_command_descendant_survives"):
                subject.command([sys.executable, "-I", "-c", code], Path.cwd(), {"PATH": "/usr/bin:/bin"}, 5)
        self.assertEqual(spawned[0].returncode, 0)
        self.assertEqual(subject.members(spawned[0].pid), [])
        self.assertFalse(Path(f"/proc/{spawned[0].pid}").exists())
        self.assertEqual(subject.UNSETTLED, [])


if __name__ == "__main__":
    unittest.main()
