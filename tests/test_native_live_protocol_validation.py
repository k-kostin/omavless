# SPDX-License-Identifier: MIT
"""Offline synthetic coverage of the native V0 test tool; no host effects."""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('native_v0',Path(__file__).with_name('native_live_protocol_validation.py'))
v0=importlib.util.module_from_spec(spec);spec.loader.exec_module(v0)
PROFILE='11111111-1111-4111-8111-111111111111'
OTHER='22222222-2222-4222-8222-222222222222'
CASE={'class':'vless-xhttp','profileId':PROFILE}


class Guard:
    def __init__(self,stop=None):self.blocked=False;self.calls=[];self.stop=stop
    def require_terminal(self):pass
    def step(self,phase,effect):
        self.calls.append(phase)
        if len(self.calls)==self.stop:
            self.blocked=True;raise v0.auth.AuthorizationUnsettled()
        return effect()


class Host:
    def __init__(self):
        self.original={'connected':True,'profileId':OTHER,'mode':'rule'}
        self.desired=dict(self.original);self.last=OTHER;self.calls=[];self.connect_failure=None
        self.probe_failure=False;self.cleanup_failure=False;self.restore_failure=False;self.unavailable=False
    def snapshot(self):return {'desired':dict(self.desired),'lastProfileId':self.last}
    def settled(self,desired):v0.require(self.desired==desired,'restore_mismatch')
    def clean(self):v0.require(not self.desired['connected'],'manual_recovery_required')
    def fixture(self,record):
        if self.unavailable:raise v0.Refused('fixture_unavailable')
        return ['vless-xhttp'],'packet-up'
    def action(self,action,extra=None):
        self.calls.append(action)
        if action=='disconnect':
            if self.cleanup_failure and self.desired['profileId']==PROFILE:raise v0.Refused('manual_recovery_required')
            self.desired.update(connected=False,profileId='')
        if action=='connect':
            if extra['profileId']==PROFILE and self.connect_failure:raise self.connect_failure
            if extra['profileId']==OTHER and self.restore_failure:raise v0.Refused('transition_failed_restored')
            self.desired.update(connected=True,**extra);self.last=extra['profileId']
        if action=='mode':self.desired.update(extra)
    def evidence(self,record,authorization,baseline):
        authorization.step('socket_inspection',lambda:None)
        return {'nativeConfigAdmission':True,'fullVpnTun':True,'unixController':True,
            'tcpControllerAbsent':True,'https':not self.probe_failure,
            'tunUsedDuringProbe':not self.probe_failure,'probeClass':'probe_timeout' if self.probe_failure else 'pass'}


class Cases(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='native-v0-unit-');self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name);self.root.chmod(0o700);self.path=self.root/'cases.json'
    def write(self,payload=None):
        self.path.write_text(json.dumps(payload or {'schemaVersion':2,'cases':[CASE]}));self.path.chmod(0o600)
        return self.path
    def test_valid_and_legacy_schema_refusal(self):
        self.assertEqual(v0.load_cases(self.write()),[CASE])
        for version in (1,True,3):
            with self.assertRaises(v0.Refused):v0.load_cases(self.write({'schemaVersion':version,'cases':[CASE]}))
    def test_shape_and_public_slug_allowlist(self):
        for case in ({**CASE,'class':'private-provider-name'}, {**CASE,'uri':'private'},
                     {**CASE,'profileId':'https://private.invalid/key'}, {**CASE,'class':[]},{}):
            with self.assertRaises(v0.Refused):v0.load_cases(self.write({'schemaVersion':2,'cases':[case]}))
        for cases in ([],[CASE,CASE],[CASE]*9):
            with self.assertRaises(v0.Refused):v0.load_cases(self.write({'schemaVersion':2,'cases':cases}))
    def test_exact_permissions_symlink_owner_and_bounds(self):
        self.write()
        for mode in (0o400,0o644,0o660):
            self.path.chmod(mode)
            with self.assertRaises(v0.Refused):v0.load_cases(self.path)
        self.path.chmod(0o600)
        link=self.root/'alias';link.symlink_to(self.path)
        with self.assertRaises(v0.Refused):v0.load_cases(link)
        with patch.object(v0.os,'getuid',return_value=os.getuid()+1),self.assertRaises(v0.Refused):v0.load_cases(self.path)
        self.path.write_bytes(b' '*65537)
        with self.assertRaises(v0.Refused):v0.load_cases(self.path)
    def test_duplicate_json_utf8_and_trailing_data(self):
        for data in (b'{"schemaVersion":2,"schemaVersion":2,"cases":[]}',b'\xff',b'{}{}',b'NaN',b'[]'):
            self.path.write_bytes(data);self.path.chmod(0o600)
            with self.assertRaises(v0.Refused):v0.load_cases(self.path)
    def test_cases_and_results_cannot_be_written_in_git(self):
        (self.root/'.git').write_text('synthetic-worktree-marker')
        with self.assertRaises(v0.Refused):v0.load_cases(self.write())
        with self.assertRaises(v0.Refused):v0.private_write(self.root/'result.json',{})
    def test_atomic_private_output_never_overwrites(self):
        out=self.root/'result.json';v0.private_write(out,{'public':True})
        self.assertEqual(out.stat().st_mode&0o777,0o600)
        with self.assertRaises(v0.Refused):v0.private_write(out,{'public':False})
        self.assertEqual(json.loads(out.read_bytes()),{'public':True})
        self.root.chmod(0o755)
        with self.assertRaises(v0.Refused):v0.private_write(self.root/'other.json',{})
    def test_restricted_requires_same_real_record_pair(self):
        restricted={'class':'hysteria2-udp-restricted','profileId':PROFILE}
        with self.assertRaises(v0.Refused):v0.load_cases(self.write({'schemaVersion':2,'cases':[restricted]}))
        normal={'class':'hysteria2-normal','profileId':PROFILE}
        self.assertEqual(len(v0.load_cases(self.write({'schemaVersion':2,'cases':[normal,restricted]}))),2)
    def test_canonical_feature_classification_does_not_invent_pq(self):
        ordinary={'version':1,'kind':'profile','profile':{'protocol':'vless','transport':'tcp','security':'reality','experimentalFeatures':[]}}
        self.assertEqual(v0.classify(ordinary,'vless://synthetic'),([], 'not-applicable'))
        ordinary['profile']['experimentalFeatures']=['VLESS Encryption','REALITY PQ']
        self.assertEqual(v0.classify(ordinary,'vless://synthetic')[0],['vless-encryption','vless-reality-pq'])
    def test_xhttp_mode_only_from_validated_xhttp(self):
        value={'version':1,'kind':'profile','profile':{'protocol':'vless','transport':'xhttp','experimentalFeatures':[]}}
        self.assertEqual(v0.classify(value,'vless://synthetic?mode=packet-up'),(['vless-xhttp'],'packet-up'))
        for mode in ('private','packet-up&mode=stream-up'):
            with self.assertRaises(v0.Refused):v0.classify(value,'vless://synthetic?mode='+mode)
        self.assertEqual(v0.classify(value,'vless://synthetic')[1],'default')
        for family,kind in [('trojan','trojan'),('hysteria2','hysteria2-normal'),('tuic','tuic-v5')]:
            value['profile']['protocol']=family
            self.assertIn(kind,v0.classify(value,'synthetic')[0])
    def run_case(self,host=None,guard=None,cases=None):
        host=host or Host();guard=guard or Guard()
        with patch.object(v0.gate,'tcp_listeners',return_value=set()):
            result=v0.run_cases(host,cases or [CASE],guard)
        return result,host,guard
    def test_success_one_case_full_vpn_cleanup_original_restoration(self):
        result,host,guard=self.run_case()
        self.assertTrue(result['complete']);self.assertEqual(result['restoration'],'restored')
        self.assertEqual(host.desired,host.original)
        self.assertEqual(guard.calls,['disconnect','connect','socket_inspection','disconnect','connect'])
        self.assertEqual(result['results'][0]['xhttpMode'],'packet-up')
    def test_probe_failure_is_failure_and_still_disconnects_restores(self):
        host=Host();host.probe_failure=True
        result,host,_=self.run_case(host)
        self.assertFalse(result['complete']);self.assertTrue(result['results'][0]['cleanup'])
        self.assertEqual(result['restoration'],'restored');self.assertEqual(host.desired,host.original)
    def test_definitive_connect_failure_cleans_and_restores(self):
        host=Host();host.connect_failure=v0.Refused('transition_failed_restored')
        result,host,_=self.run_case(host)
        self.assertFalse(result['complete']);self.assertEqual(result['restoration'],'restored')
        self.assertEqual(host.calls,['disconnect','connect','disconnect','connect'])
    def test_unknown_connect_outcome_never_compensates(self):
        host=Host();host.connect_failure=TimeoutError('private endpoint key')
        result,host,_=self.run_case(host)
        self.assertEqual(host.calls,['disconnect','connect'])
        self.assertEqual(result['restoration'],'manual-recovery-required')
        self.assertNotIn('private',json.dumps(result))
    def test_core_internal_restart_errors_do_not_authorize_compensation(self):
        for code in ('core_rejected','internal_error','daemon_restarting','native_request_failed'):
            host=Host();host.connect_failure=v0.Refused(code)
            result,host,_=self.run_case(host)
            self.assertEqual(host.calls,['disconnect','connect'])
            self.assertEqual(result['restoration'],'manual-recovery-required')
    def test_response_envelope_strict_types_and_safe_errors(self):
        base=dict(api='omavless.control',version=1,id='synthetic',ok=True,revision=0,result={})
        encode=lambda value:(json.dumps(value)+'\n').encode()
        self.assertEqual(v0.response(encode(base),'synthetic'),base)
        for delta in ({'version':True},{'revision':True},{'revision':-1},{'revision':2**63},
                      {'ok':1},{'result':[]},{'private':'secret'},{'id':'wrong'}):
            with self.assertRaises(v0.Refused):v0.response(encode({**base,**delta}),'synthetic')
        for code in ([],{},None,'https://private.invalid/password'):
            error={k:v for k,v in base.items() if k!='result'}
            error.update(ok=False,error={'code':code,'message':'private key material'})
            with self.assertRaisesRegex(v0.Refused,'^native_request_failed$'):
                v0.response(encode(error),'synthetic')
        for frame in (encode(base).rstrip(),encode(base)*2,b'\xff\n',b'{}\n'):
            with self.assertRaises(v0.Refused):v0.response(frame,'synthetic')
    def test_error_codes_match_production_rust_contract(self):
        source=(Path(__file__).parents[1]/'crates/omavless-control-protocol/src/lib.rs').read_text()
        block=source.split('pub const fn as_str(self)')[1].split('pub const fn message(self)')[0]
        self.assertEqual(v0.PUBLIC_ERRORS,set(v0.re.findall(r'Self::\w+ => "([a-z_]+)"',block)))
    def test_manual_recovery_definitive_error_stops_other_effects(self):
        host=Host();host.connect_failure=v0.Refused('manual_recovery_required')
        result,host,_=self.run_case(host)
        self.assertEqual(result['restoration'],'manual-recovery-required')
        self.assertEqual(host.calls,['disconnect','connect'])
    def test_cleanup_or_restoration_failure_is_hard_blocker(self):
        for field in ('cleanup_failure','restore_failure'):
            host=Host();setattr(host,field,True)
            result,_,_=self.run_case(host)
            self.assertFalse(result['complete']);self.assertEqual(result['restoration'],'manual-recovery-required')
    def test_authorization_stop_at_each_effect_has_no_later_action(self):
        for index in range(1,6):
            result,host,guard=self.run_case(guard=Guard(stop=index))
            self.assertEqual(len(guard.calls),index);self.assertFalse(result['complete'])
            self.assertEqual(result['restoration'],'manual-recovery-required')
    def test_unavailable_fixture_does_not_connect_it(self):
        host=Host();host.unavailable=True
        result,host,_=self.run_case(host)
        self.assertEqual(result['results'][0]['state'],'FIXTURE UNAVAILABLE')
        self.assertEqual(host.calls,['disconnect','connect']) # original restore only
    def test_restricted_network_is_not_faked_by_a_boolean(self):
        host=Host();guard=Guard()
        with self.assertRaisesRegex(v0.Refused,'safe_restricted_network_unavailable'):
            self.run_case(host,guard,[{'class':'hysteria2-udp-restricted','profileId':PROFILE}])
        self.assertEqual(host.calls,[])
    def test_disconnected_last_profile_cannot_be_silently_changed(self):
        host=Host();host.desired={'connected':False,'profileId':'','mode':'rule'}
        with self.assertRaisesRegex(v0.Refused,'disconnected_profile_restore_unavailable'):self.run_case(host)
        self.assertEqual(host.calls,[])
    def test_shareable_result_never_contains_input_record_or_secret(self):
        result,_,_=self.run_case()
        for secret in (PROFILE,OTHER,'"profileId"','"password"','"credential"','"uri"','controllerPath'):
            self.assertNotIn(secret,json.dumps(result))


if __name__=='__main__':unittest.main()
