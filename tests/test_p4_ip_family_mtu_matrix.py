# SPDX-License-Identifier: MIT
"""Pure measurement/receipt guards. No core, namespace, TUN or network calls."""
import copy
import importlib.util
import json
from pathlib import Path
import sys
import unittest
import errno
import os
import socket
import tarfile
import tempfile
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0,str(Path(__file__).parent))
import p4_ip_family_mtu_matrix as subject

spec=importlib.util.spec_from_file_location("p4_matrix_build",Path(__file__).parent/"fixtures/p4_matrix_peer/build.py")
builder=importlib.util.module_from_spec(spec); spec.loader.exec_module(builder)


class MatrixGuards(unittest.TestCase):
    def test_http_reuse_precedes_bind_and_failure_is_fixed_class(self):
        calls=[]
        class Listener:
            reuse=False
            def setsockopt(self,*args): calls.append(("option",args)); self.reuse=True
            def bind(self,where):
                calls.append(("bind",where))
                if not self.reuse: raise OSError(errno.EADDRINUSE,"private detail")
            def listen(self,count): calls.append(("listen",count))
            def close(self): calls.append(("close",))
        # Model the repeated-bind counterexample, not live host TCP behavior.
        with self.assertRaises(OSError): Listener().bind(("10.203.0.1",8089))
        calls.clear(); listener=Listener()
        self.assertIs(subject.http_listener(lambda _:listener,"10.203.0.1"),listener)
        self.assertEqual(calls[0],("option",(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)))
        self.assertEqual(calls[1],("bind",("10.203.0.1",8089)))
        class Failing(Listener):
            def bind(self,where): raise OSError(errno.EADDRINUSE,"private detail")
        with self.assertRaises(subject.wg.Refused) as caught: subject.http_listener(lambda _:Failing(),"10.203.0.1")
        self.assertEqual(caught.exception.stage,"fixture_http_bind_in_use")

    def test_readiness_keeps_exact_child_role(self):
        for stage in ("peer_readiness","service_readiness","core_readiness"):
            with self.assertRaises(subject.wg.Refused) as caught: subject.readiness(SimpleNamespace(poll=lambda:2),SimpleNamespace(exists=lambda:False),stage)
            self.assertEqual(caught.exception.stage,stage)

    def test_private_diagnostics_are_bounded_logs_not_config_keys(self):
        with tempfile.TemporaryDirectory() as directory:
            cache=Path(directory); cache.chmod(0o700); root=cache/"matrix-test"; root.mkdir(mode=0o700)
            cell=root/"cell-00"; cell.mkdir(mode=0o700)
            subject.wg.private_write(cell/"service.log",b'{"status":"REFUSE","stage":"fixture_http_bind_in_use"}\n')
            subject.wg.private_write(cell/"positive.conf",b"synthetic private config never archived")
            result=subject.retain_private_diagnostics(root,cache)
            archive=cache/result["archive_leaf"]
            self.assertEqual(archive.stat().st_mode&0o777,0o600)
            self.assertTrue(result["private_raw_logs"] and result["synthetic_config_keys_excluded"])
            self.assertEqual(result["log_count"],1)
            with tarfile.open(archive) as stream: self.assertEqual(stream.getnames(),["cell-00/service.log"])
            with self.assertRaises(FileExistsError): subject.retain_private_diagnostics(root,cache)

    def test_private_diagnostics_refuse_symlink_and_oversized_logs(self):
        with tempfile.TemporaryDirectory() as directory:
            cache=Path(directory); cache.chmod(0o700); root=cache/"matrix-test"; root.mkdir(mode=0o700)
            cell=root/"cell-00"; cell.mkdir(mode=0o700); log=cell/"service.log"
            log.symlink_to("missing")
            with self.assertRaises(OSError): subject.retain_private_diagnostics(root,cache)
            log.unlink()
            subject.wg.private_write(log,b"")
            with log.open("wb") as file: file.truncate(2*1024*1024+1)
            with self.assertRaises(subject.wg.Refused): subject.retain_private_diagnostics(root,cache)

    def test_both_render_phases_preserve_the_mutable_cell_record(self):
        record={"status":"REFUSE","flavor":"3.1"}; identity=id(record)
        with patch.object(subject.wg,"command",return_value=SimpleNamespace(stdout=b'{"private_roundtrip":true,"flavor":"3.1"}')) as command:
            self.assertIsNone(subject.render_cell_configs(Path("/fixture"),SimpleNamespace(renderer="/renderer"),"3.1",6,6,1280,record))
        self.assertEqual(id(record),identity)
        self.assertEqual(record,{"status":"REFUSE","flavor":"3.1","private_roundtrips":2})
        self.assertEqual(command.call_count,2)
        self.assertIn("positive",command.call_args_list[0].args[0])
        self.assertIn("negative",command.call_args_list[1].args[0])
        record.update({"measured":"synthetic"})
        self.assertEqual(record["measured"],"synthetic")

    def test_exact_matrix_and_payload_bounds(self):
        self.assertEqual(len(subject.CELLS),24)
        self.assertEqual(len(set(subject.CELLS)),24)
        for _,_,family,mtu in subject.CELLS:
            for size in (mtu-1,mtu,mtu+1):
                self.assertEqual(subject.payload_size(family,size)+(28 if family==4 else 48),size)
                self.assertEqual(len(subject.fixed_payload(subject.payload_size(family,size))),subject.payload_size(family,size))
        self.assertEqual(len(subject.BODY),65536)

    def test_only_literal_socks_addresses(self):
        for family in (4,6):
            frame=subject.socks_address(family,subject.address(family),8090)
            self.assertEqual(subject.decode_address(frame),(subject.address(family),8090,len(frame)))
        for bad in (b"",b"\x03\x01a\x00\x01",b"\x01\x01"):
            with self.assertRaises(subject.wg.Refused): subject.decode_address(bad)

    def flow(self,direction,size,payload,**change):
        return {"direction":direction,"family":4,"size":size,"id":5,"offset":0,"more":False,"fragment":False,"fragment_payload_size":size-20,"source_port":12345 if direction=="rx" else 8090,"dest_port":8090 if direction=="rx" else 12345,"payload_sha256":subject.sha(payload),**change}

    def receipt(self,payload,**change):
        return {"sha256":subject.sha(payload),"size":len(payload),"family":4,"source_family":4,"route_mtu":1280,"ack_sent":True,"echo_sent":True,"echo_emsgsize":False,**change}

    def assess(self,size,**change):
        payload=subject.fixed_payload(subject.payload_size(4,size))
        before={"flows":[]}; after={"flows":[self.flow("rx",size,payload),self.flow("tx",size,payload)]}
        args=dict(before=before,after=after,receipt=self.receipt(payload),payload=payload,replies={"ack":True,"echo":True},family=4,inner_size=size,mtu=1280)
        args.update(change)
        return subject.assess_datagram(**args)

    def test_boundaries_and_oversize_not_compliance(self):
        for size in (1279,1280): self.assertEqual(self.assess(size)["forward"],"observed-unfragmented")
        result=self.assess(1281)
        self.assertIn("NOT-MTU-compliance",result["forward"])
        self.assertIn("NOT-MTU-compliance",result["reverse"])

    def test_unrelated_hist_or_hash_and_missing_ack_refuse(self):
        for change in ({"after":{"flows":[]}}, {"replies":{"ack":False,"echo":True}}, {"receipt":{}}):
            with self.assertRaises(subject.wg.Refused): self.assess(1280,**change)
        payload=subject.fixed_payload(1252)
        flow=self.flow("rx",1280,payload,payload_sha256="0"*64)
        with self.assertRaises(subject.wg.Refused): self.assess(1280,after={"flows":[flow]})

    def test_response_refusal_is_directional_fixture_policy(self):
        payload=subject.fixed_payload(1253)
        result=subject.assess_datagram({"flows":[]},{"flows":[self.flow("rx",1281,payload)]},self.receipt(payload,echo_sent=False,echo_emsgsize=True),payload,{"ack":True,"echo":False},4,1281,1280)
        self.assertIn("NOT-MTU-compliance",result["forward"])
        self.assertIn("fixture-reverse-local-EMSGSIZE",result["reverse"])
        for receipt in (self.receipt(payload,echo_sent=False,echo_emsgsize=False),self.receipt(payload,echo_sent=False,echo_emsgsize=True,route_mtu=1420)):
            with self.assertRaises(subject.wg.Refused): subject.assess_datagram({"flows":[]},{"flows":[self.flow("rx",1281,payload)]},receipt,payload,{"ack":True,"echo":False},4,1281,1280)

    def test_reverse_packets_cannot_claim_echo_when_service_refuses(self):
        payload=subject.fixed_payload(1253)
        receipt=self.receipt(payload,echo_sent=False,echo_emsgsize=True)
        cases=[[self.flow("tx",1281,payload)], [self.flow("tx",1280,payload,fragment=True,more=True,fragment_payload_size=1256,payload_sha256=""),self.flow("tx",25,payload,fragment=True,offset=1256,fragment_payload_size=5,payload_sha256="",source_port=0,dest_port=0)]]
        for tx in cases:
            with self.assertRaises(subject.wg.Refused): subject.assess_datagram({"flows":[]},{"flows":[self.flow("rx",1281,payload),*tx]},receipt,payload,{"ack":True,"echo":False},4,1281,1280)

    def test_negative_requires_real_request_stages_and_outbound_delta(self):
        attempt={"socks_authenticated":True,"socks_request_sent":True,"http_request_sent":False,"udp_datagram_sent":False}
        http={"status":"TRANSPORT-REFUSED","stage":"upstream_connect_refused","attempt":attempt}
        udp={"status":"PASS","ack":False,"echo":False,"attempt":{**attempt,"udp_datagram_sent":True}}
        before={"outer":{"client:4:loopback:148":1}}; after={"outer":{"client:4:loopback:148":2}}
        subject.validate_negative_clients(http,udp,before,after)
        for bad_http,bad_udp,bad_after in [({**http,"stage":"child_privileges"},udp,after),({**http,"attempt":{**attempt,"socks_request_sent":False}},udp,after),(http,{**udp,"attempt":attempt},after),(http,{**udp,"ack":True},after),(http,udp,before)]:
            with self.assertRaises(subject.wg.Refused): subject.validate_negative_clients(bad_http,bad_udp,before,bad_after)

    def test_fragment_coverage_rejects_atomic_gap_overlap_and_duplicates(self):
        payload=b"x"*1253
        first=self.flow("rx",1280,payload,fragment=True,more=True,fragment_payload_size=1256,payload_sha256="")
        last=self.flow("rx",25,payload,fragment=True,offset=1256,fragment_payload_size=5,payload_sha256="",source_port=0,dest_port=0)
        self.assertEqual(len(subject.fragment_coverage([first,last],"rx",4,len(payload))),2)
        for bad in ([first], [first,{**last,"offset":1255}], [first,{**last,"offset":1257}], [first,last,last], [{**first,"more":False}], [first,{**last,"id":6}]):
            self.assertIsNone(subject.fragment_coverage(bad,"rx",4,len(payload)))

    def test_builder_exact_counts_and_malformed_event_fields(self):
        events=[{"Action":"pass","Package":builder.PACKAGE,"Test":name} for name in builder.CASES]+[{"Action":"pass","Package":builder.PACKAGE}]
        encode=lambda rows:b"\n".join(json.dumps(row).encode() for row in rows)
        builder.verify_events(encode(events),1)
        for fields in ({},{"Action":None},{"Action":False},{"Action":"unknown"},{"Action":"skip"},{"Action":"output","Test":None},{"Action":"pass","Test":"unexpected"}):
            with self.assertRaises(ValueError): builder.verify_events(encode(events+[{"Package":builder.PACKAGE,**fields}]),1)
        for wrong in ([],events[1:],events+events):
            with self.assertRaises(ValueError): builder.verify_events(encode(wrong),1)

    def test_partial_measurements_never_become_all_measured(self):
        rows=[]
        for flavor,outer,inner,mtu in subject.CELLS:
            row={"flavor":flavor,"outer":outer,"inner":inner,"mtu":mtu,"engine":subject.ENGINE,"http_bytes":65536,"recovery_exact_http_bytes":65536,"wrong_key_no_direct":True,"udp":[{"inner_ip_size":size,"status":"MEASURED"} for size in (92 if inner==4 else 112,mtu-1,mtu,mtu+1)]}
            row["status"]=subject.classify_cell(row); rows.append(row)
        rows[0]["udp"][-1].update(status="NONPASS",stage="udp_size_inconclusive")
        rows[0]["status"]=subject.classify_cell(rows[0])
        result={"status":"PARTIAL-NONPASS","engine":subject.ENGINE,"cells":rows,"cell_count":24,**subject.matrix_counts(rows)}
        subject.validate_matrix_result(result)
        self.assertEqual(result["measured_cells"],23); self.assertEqual(result["partial_cells"],1)
        self.assertEqual(result["attempted_udp_cases"],96); self.assertEqual(result["nonpass_udp_cases"],1)
        for change in ({"status":"MEASURED"},{"measured_cells":24},{"cells":rows[:-1]},{"cells":[*rows[:-1],rows[0]]}):
            with self.assertRaises(subject.wg.Refused): subject.validate_matrix_result({**result,**change})
        partial=copy.deepcopy(rows[0]); partial["udp"].pop()
        with self.assertRaises(subject.wg.Refused): subject.classify_cell(partial)


if __name__=="__main__": unittest.main()
