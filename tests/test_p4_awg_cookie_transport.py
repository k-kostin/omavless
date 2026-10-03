# SPDX-License-Identifier: MIT
"""Pure guards only; actual transport needs an explicitly leased VM."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))
import p4_awg_cookie_transport as subject

fixture = Path(__file__).parent / "fixtures/p4_awg_peer"
sys.path.insert(0, str(fixture))
spec = importlib.util.spec_from_file_location("p4_cookie_builder", fixture / "build_cookie_peer.py")
builder = importlib.util.module_from_spec(spec); spec.loader.exec_module(builder)


def facts(negative=False, bypass=False):
    result = {name: 0 for name in subject.OBS_FIELDS}
    result.update(forced_load=1, mac1_no_mac2=1, cookie_corrupt_mode=int(negative))
    if negative: result.update(cookie=2, corrupted_cookie=2)
    elif bypass: result.update(response=1, transport=1)
    else: result.update(cookie=1, valid_mac2=1, mac2_after_cookie=1, response=1, transport=1)
    return result


class CookieTransportGuards(unittest.TestCase):
    def test_explicit_opt_in_precedes_receipt_and_outside_access(self):
        with patch.object(subject, "verify_receipt") as receipt, patch.object(subject.wg, "outside_snapshot") as snapshot:
            with self.assertRaises(subject.wg.Refused): subject.outer(argparse.Namespace(run=False, rounds=1))
            receipt.assert_not_called(); snapshot.assert_not_called()

    def test_only_reviewed_pin_precedes_receipt_and_outside_access(self):
        args = argparse.Namespace(run=True, rounds=1, source_sha="a"*40, upstream_sha="b"*40)
        with patch.object(subject.os, "getuid", return_value=1000), patch.object(subject, "verify_receipt") as receipt, patch.object(subject.wg, "outside_snapshot") as snapshot:
            with self.assertRaises(subject.wg.Refused): subject.outer(args)
            receipt.assert_not_called(); snapshot.assert_not_called()

    def test_cookie_server_role_does_not_weaken_client_native_fields(self):
        payload = b"set=1\nprivate_key=private-canary\nheader_protection_key=header-canary\ndisable_cookies=true\npublic_key=public-canary\n\n"
        actual, values = subject.server_policy(payload, {b"disable_cookies":b"1"}, "3.1")
        self.assertEqual(actual, payload.replace(b"disable_cookies=true\n", b"disable_cookies=false\n"))
        self.assertEqual(values[b"disable_cookies"], b"0")
        bypass, values = subject.server_policy(payload, {b"disable_cookies":b"1"}, "3.1", True)
        self.assertEqual(bypass, payload); self.assertEqual(values[b"disable_cookies"], b"1")
        with self.assertRaises(subject.wg.Refused): subject.server_policy(payload+ b"disable_cookies=true\n", {}, "3.1")
        with self.assertRaises(subject.wg.Refused): subject.server_policy(payload, {}, "3", True)

    def test_positive_requires_cryptographic_mac2_after_cookie(self):
        subject.check_facts(facts())
        for key, value in (("cookie",0),("valid_mac2",0),("mac2_after_cookie",0),("mac2_without_cookie",1),("forced_load",0),("cookie_frame_error",1)):
            with self.assertRaises(subject.wg.Refused): subject.check_facts({**facts(), key:value})

    def test_negative_requires_every_cookie_corrupt_and_zero_mac2_handshake(self):
        subject.check_facts(facts(True), True)
        for key, value in (("corrupted_cookie",1),("cookie",0),("valid_mac2",1),("response",1),("transport",1),("cookie_corrupt_mode",0)):
            with self.assertRaises(subject.wg.Refused): subject.check_facts({**facts(True),key:value}, True)
        subject.check_facts(facts(bypass=True), bypass=True)
        with self.assertRaises(subject.wg.Refused): subject.check_facts({**facts(bypass=True),"cookie":1}, bypass=True)

    def test_observation_rejects_private_fields_types_and_bounds(self):
        for bad in ({**facts(),"private_key":"canary"}, {**facts(),"cookie":True}, {**facts(),"cookie":10001}):
            with patch.object(subject.awg, "socket_payload", return_value=json.dumps(bad).encode()):
                with self.assertRaises(subject.wg.Refused) as error: subject.observation(Path("/synthetic"), 123)
                self.assertEqual(str(error.exception), "cookie_observation")

    def test_success_result_cannot_forward_private_canary_or_false_counts(self):
        outcome = {"positive":2,"corrupt_negative":2,"recovery":2,"bypass":1,"private_roundtrip":7,"interface_cleanup":True,
                   "generations":{"3":[{"positive":facts(),"corrupt_negative":facts(True),"recovery":facts()}],
                                  "3.1":[{"positive":facts(),"corrupt_negative":facts(True),"recovery":facts(),"bypass":facts(bypass=True)}]}}
        subject.verify_outcome(outcome,1)
        with self.assertRaises(subject.wg.Refused): subject.verify_outcome({**outcome,"private_key":"canary"},1)
        for key in ("positive", "corrupt_negative", "recovery", "bypass", "private_roundtrip"):
            with self.assertRaises(subject.wg.Refused): subject.verify_outcome({**outcome,key:True},1)
        with self.assertRaises(subject.wg.Refused): subject.verify_outcome({**outcome,"private_roundtrip":6},1)

    def test_builder_json_never_accepts_no_tests_skip_or_extra_test(self):
        package = "synthetic/device"
        good = [{"Package":package,"Action":"pass","Test":"TestActual"},{"Package":package,"Action":"pass"}]
        encode = lambda events: b"\n".join(json.dumps(event).encode() for event in events)
        builder.verify_tests(encode(good),package,["TestActual"],1)
        for bad in (good[1:],good+good,good+[{"Package":package,"Action":"skip"}],good+[{"Package":package,"Action":"pass","Test":"TestUnexpected"}],good+[{"Package":package,"Action":"fail"}]):
            with self.assertRaises(ValueError): builder.verify_tests(encode(bad),package,["TestActual"],1)

    def test_receipt_is_one_owned_bounded_regular_private_inode(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); root.chmod(0o700)
            path = root / "peer.receipt.json"
            path.write_bytes(b"{}"); path.chmod(0o644)
            with self.assertRaises(subject.wg.Refused): subject.verify_receipt(path, hashlib.sha256(b"{}").hexdigest(), "a"*64)
            path.chmod(0o600)
            linked = root / "linked"; os.link(path, linked)
            with self.assertRaises(subject.wg.Refused): subject.verify_receipt(path, hashlib.sha256(b"{}").hexdigest(), "a"*64)
            linked.unlink(); path.unlink(); os.mkfifo(path, 0o600)
            with self.assertRaises(subject.wg.Refused): subject.verify_receipt(path, "a"*64, "a"*64)
            path.unlink(); path.symlink_to(root / "missing")
            with self.assertRaises(OSError): subject.verify_receipt(path, "a"*64, "a"*64)

    def test_receipt_hash_and_duplicate_fields_refuse_private_canaries(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); root.chmod(0o700)
            path = root / "peer.receipt.json"
            for payload, expected in ((b'{"private_key":"canary","private_key":"canary"}', None), (b'{}', "a"*64)):
                path.write_bytes(payload); path.chmod(0o600)
                with self.assertRaises(subject.wg.Refused) as error:
                    subject.verify_receipt(path, expected or hashlib.sha256(payload).hexdigest(), "a"*64)
                self.assertEqual(str(error.exception), "cookie_peer_receipt")


if __name__ == "__main__": unittest.main()
