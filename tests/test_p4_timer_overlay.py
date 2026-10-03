# SPDX-License-Identifier: MIT
"""Pure timer receipt guards; never run Go or networking in ordinary CI."""
import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("p4_timer_overlay", Path(__file__).parent / "fixtures/p4_awg_peer/run_timer_overlay.py")
subject = importlib.util.module_from_spec(spec)
spec.loader.exec_module(subject)


class TimerReceiptGuards(unittest.TestCase):
    def events(self, count=1):
        names = ["TestP4TimerBoundaries", *(f"TestP4TimerBoundaries/{case}" for case in subject.CASES)]
        return [{"Action": "pass", "Package": "github.com/amnezia-vpn/amneziawg-go/v3/device", "Test": name} for _ in range(count) for name in names] + [{"Action": "pass", "Package": "github.com/amnezia-vpn/amneziawg-go/v3/device"}]

    def encoded(self, events):
        return b"\n".join(json.dumps(event).encode() for event in events)

    def test_exact_repeated_receipts(self):
        subject.verify_events(self.encoded(self.events(20)), 20)

    def test_no_tests_partial_duplicates_or_wrong_count_refuse(self):
        events = self.events()
        for wrong in ([], events[-1:], events[1:], events[:-1], events + events, self.events(2)):
            with self.assertRaises(ValueError): subject.verify_events(self.encoded(wrong), 1)

    def test_failure_skip_extra_case_wrong_package_and_shape_refuse(self):
        for event in ({"Action": "skip", "Package": "github.com/amnezia-vpn/amneziawg-go/v3/device"}, {"Action": "fail", "Package": "github.com/amnezia-vpn/amneziawg-go/v3/device"}, {"Action": "pass", "Package": "github.com/amnezia-vpn/amneziawg-go/v3/device", "Test": "TestP4CookieUnderload"}, {"Action": "pass", "Package": "unexpected"}, []):
            with self.assertRaises(ValueError): subject.verify_events(self.encoded(self.events() + [event]), 1)


if __name__ == "__main__": unittest.main()
