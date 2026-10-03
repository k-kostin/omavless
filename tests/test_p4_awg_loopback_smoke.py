# SPDX-License-Identifier: MIT
"""Pure guards only: no real transport or namespace in normal CI."""
import argparse
import contextlib
import io
import json
from pathlib import Path
import struct
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0,str(Path(__file__).parent))
import p4_awg_loopback_smoke as subject


class FakeSocket:
    def __init__(self,data,pid=123): self.data=[data]; self.pid=pid
    def __enter__(self): return self
    def __exit__(self,*_args): return False
    def settimeout(self,_value): pass
    def connect(self,_path): pass
    def sendall(self,_payload): pass
    def getsockopt(self,*_args): return struct.pack("3i",self.pid,0,0)
    def recv(self,_bound): return self.data.pop(0) if self.data else b""


class AwgSmokeGuards(unittest.TestCase):
    def test_opt_in_refuses_before_any_snapshot(self):
        with patch.object(subject.wg,"outside_snapshot") as snapshot:
            with self.assertRaises(subject.wg.Refused):
                subject.outer(argparse.Namespace(run=False,rounds=1))
            snapshot.assert_not_called()

    def test_only_reviewed_upstream_pin_is_accepted(self):
        args=argparse.Namespace(run=True,rounds=1,source_sha="a"*40,upstream_sha="b"*40)
        with patch.object(subject.os,"getuid",return_value=1000),patch.object(subject.wg,"outside_snapshot") as snapshot:
            with self.assertRaises(subject.wg.Refused) as error: subject.outer(args)
            self.assertEqual(error.exception.stage,"source_identity")
            snapshot.assert_not_called()

    def test_fixture_does_not_disable_obfuscation(self):
        self.assertEqual(subject.FIELDS["Jc"],"4")
        self.assertEqual([subject.FIELDS[f"H{i}"] for i in range(1,5)],["101","202","303","404"])
        self.assertTrue(all(int(subject.FIELDS[f"S{i}"])>=12 for i in range(1,5)))
        self.assertEqual(subject.FIELDS["ContentPaddingAddition"],"37")
        self.assertEqual(len({subject.FIELDS[f"I{i}"] for i in range(1,6)}),5)

    def test_uapi_requires_owned_peer_pid(self):
        with patch.object(subject.socket,"socket",return_value=FakeSocket(b"private-canary",pid=9)):
            with self.assertRaises(subject.wg.Refused) as error: subject.socket_payload(Path("/synthetic"),123,b"get=1\n\n")
            self.assertEqual(str(error.exception),"awg_peer_identity")

    def test_uapi_refuses_unterminated_and_oversized_private_frame(self):
        for data in (b"private-canary",b"x"*(subject.wg.BODY_LIMIT+1)):
            with patch.object(subject.socket,"socket",return_value=FakeSocket(data)):
                with self.assertRaises(subject.wg.Refused) as error: subject.socket_payload(Path("/synthetic"),123,b"get=1\n\n")
                self.assertEqual(str(error.exception),"awg_uapi_bound")

    def test_duplicate_private_get_fields_are_not_accepted(self):
        with patch.object(subject,"socket_payload",return_value=b"private_key=canary\nprivate_key=other\nerrno=0\n\n"):
            with self.assertRaises(subject.wg.Refused) as error: subject.peer_get(Path("/synthetic"),123)
            self.assertEqual(str(error.exception),"awg_uapi_frame")

    def test_observation_rejects_unexpected_or_secret_fields(self):
        fields={key:1 for key in ("junk","special","special_mask","init","response","transport","protected","trailers","padding_size_matches")}
        for bad in ({**fields,"private_key":"private-canary"},{**fields,"junk":True},{**fields,"junk":10001}):
            with patch.object(subject,"socket_payload",return_value=json.dumps(bad).encode()):
                with self.assertRaises(subject.wg.Refused) as error: subject.observation(Path("/synthetic"),123)
                self.assertEqual(str(error.exception),"awg_observation")

    def test_diagnostic_contains_no_message_arguments_or_locals(self):
        output=io.StringIO()
        with contextlib.redirect_stdout(output): subject.diagnostic("ValueError",47,"socket_payload")
        self.assertEqual(json.loads(output.getvalue()),{"status":"REFUSED","stage":"unexpected_fixed_failure","failure_class":"ValueError","failure_function":"socket_payload","failure_line":47})
        output=io.StringIO()
        with contextlib.redirect_stdout(output): subject.diagnostic("private canary",-1,"secret/provider-url")
        self.assertNotIn("canary",output.getvalue()); self.assertNotIn("provider",output.getvalue())

    def test_same_namespace_refuses_before_credential_or_tun_effects(self):
        args=argparse.Namespace(scratch="/synthetic",parent_pid="9",parent_net="10",parent_user="11",parent_net_fd="20",parent_user_fd="21")
        with patch.object(subject.wg,"private_directory",return_value=True),patch.object(subject.os,"getuid",return_value=0),patch.object(subject.os,"getppid",return_value=9),patch.object(subject.os,"fstat",side_effect=lambda fd:argparse.Namespace(st_ino=10 if fd==20 else 11)),patch.object(subject.wg,"namespace",side_effect=lambda name,pid="self":10 if name=="net" else 11),patch.object(subject,"private_keys") as keys,patch.object(subject.wg,"command") as command:
            with self.assertRaises(subject.wg.Refused) as error: subject.namespace_run(args)
            self.assertEqual(str(error.exception),"namespace_identity")
            keys.assert_not_called(); command.assert_not_called()


if __name__=="__main__": unittest.main()
