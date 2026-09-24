#!/usr/bin/python3
# SPDX-License-Identifier: MIT
"""Opt-in developer harness for the installed Rust owner, never a backend fallback.

Private exports are kept in memory and classified by the production Rust parser.
Only fixed public classes leave the harness. No provider/config import or policy
change. Each host effect uses the existing real-terminal human barrier.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import socket
import stat
import struct
import subprocess
import tempfile
import time
import urllib.parse
import uuid

spec = importlib.util.spec_from_file_location('v0_installed', Path(__file__).with_name('installed_native_acceptance.py'))
installed = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installed)
gate = installed.gate
auth = installed.auth
MAX_CASES = 8
MAX_FILE = 65536
MAX_RESPONSE = 262144
ID = re.compile(r'[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\Z')
CLASSES = {'vless-xhttp':'vless', 'vless-encryption':'vless', 'vless-reality-pq':'vless',
           'trojan':'trojan', 'hysteria2-normal':'hysteria2',
           'hysteria2-udp-restricted':'hysteria2', 'tuic-v5':'tuic'}
MODES = {'default','auto','packet-up','stream-up','stream-one'}
PUBLIC_ERRORS = {'invalid_request','unsupported_version','unknown_method','invalid_argument',
    'permission_denied','capability_unavailable','conflict','busy','not_found','core_rejected',
    'subscription_unavailable','transition_failed_restored','manual_recovery_required',
    'daemon_restarting','internal_error'}
# Only a definitive refusal or an explicitly restored transaction permits a
# separately attended cleanup. Internal/core failures can leave uncertain state.
DEFINITE_REFUSALS = {'invalid_request','unsupported_version','unknown_method','invalid_argument',
    'permission_denied','capability_unavailable','conflict','busy','not_found',
    'transition_failed_restored'}
PUBLIC_FAILURES = PUBLIC_ERRORS | {'fixture_unavailable','fixture_changed','runtime_changed',
    'owned_runtime_not_verified','restore_mismatch','service_ownership','tun_ownership',
    'tcp_controller_config','probe_timeout','native_timeout','native_outcome_unknown'}


class Refused(Exception):
    pass


def require(value, code):
    if not value:
        raise Refused(code)


def pairs(items):
    result = {}
    for key, value in items:
        require(key not in result, 'invalid_json')
        result[key] = value
    return result


def decode(data):
    try:
        return json.loads(data.decode('utf-8'), object_pairs_hook=pairs,
            parse_constant=lambda _: (_ for _ in ()).throw(Refused('invalid_json')))
    except (ValueError, UnicodeError, RecursionError):
        raise Refused('invalid_json') from None


def response(data, request_id):
    require(len(data)<=MAX_RESPONSE and data.endswith(b'\n') and data.count(b'\n')==1,'response_frame')
    reply=decode(data)
    require(isinstance(reply,dict) and reply.get('api')=='omavless.control'
        and type(reply.get('version')) is int and reply['version']==1
        and reply.get('id')==request_id and type(reply.get('ok')) is bool
        and type(reply.get('revision')) is int and 0<=reply['revision']<2**63,'response_envelope')
    fields={'api','version','id','ok','revision','result' if reply['ok'] else 'error'}
    require(set(reply)==fields,'response_envelope')
    if not reply['ok']:
        error=reply['error']
        require(isinstance(error,dict),'response_envelope')
        code=error.get('code')
        raise Refused(code if isinstance(code,str) and code in PUBLIC_ERRORS else 'native_request_failed')
    require(isinstance(reply['result'],dict),'response_envelope')
    return reply


def outside_git(path):
    require(path.is_absolute() and path == Path(os.path.normpath(path)), 'unsafe_path')
    for part in (path, *path.parents):
        require(not part.is_symlink(), 'unsafe_path')
    require(not any((p / '.git').exists() for p in path.parents), 'private_file_inside_git')


def load_cases(path):
    outside_git(path)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        info = os.fstat(stream.fileno())
        require(stat.S_ISREG(info.st_mode) and info.st_uid == os.getuid()
            and stat.S_IMODE(info.st_mode) == 0o600 and info.st_size <= MAX_FILE, 'unsafe_cases_file')
        data = stream.read(MAX_FILE + 1)
        require(len(data) <= MAX_FILE, 'cases_bound')
    payload = decode(data)
    require(isinstance(payload, dict) and set(payload) == {'schemaVersion','cases'}
        and type(payload['schemaVersion']) is int and payload['schemaVersion'] == 2, 'cases_schema')
    cases = payload['cases']
    require(isinstance(cases, list) and 0 < len(cases) <= MAX_CASES, 'cases_bound')
    seen = set()
    for case in cases:
        require(isinstance(case, dict) and set(case) == {'class','profileId'}, 'cases_schema')
        kind, profile = case['class'], case['profileId']
        require(isinstance(kind,str) and kind in CLASSES and kind not in seen
            and isinstance(profile,str) and ID.fullmatch(profile), 'case_invalid')
        seen.add(kind)
    restricted = next((c for c in cases if c['class']=='hysteria2-udp-restricted'), None)
    if restricted:
        require(any(c['class']=='hysteria2-normal' and c['profileId']==restricted['profileId'] for c in cases),
            'restricted_pair_missing')
    return cases


def private_write(path, payload):
    outside_git(path)
    info = path.parent.stat()
    require(info.st_uid == os.getuid() and stat.S_IMODE(info.st_mode) == 0o700,
        'output_directory_not_private')
    require(not os.path.lexists(path), 'output_exists')
    encoded = (json.dumps(payload, ensure_ascii=True, indent=2)+'\n').encode()
    require(len(encoded) <= MAX_FILE, 'output_bound')
    fd, temporary = tempfile.mkstemp(prefix='.v0-', dir=path.parent)
    try:
        with os.fdopen(fd,'wb') as stream:
            os.fchmod(stream.fileno(),0o600)
            stream.write(encoded);stream.flush();os.fsync(stream.fileno())
        # Link is atomic and refuses overwriting an existing destination.
        os.link(temporary,path,follow_symlinks=False)
        directory = os.open(path.parent, os.O_DIRECTORY | os.O_NOFOLLOW)
        try: os.fsync(directory)
        finally: os.close(directory)
    finally:
        os.unlink(temporary)


def classify(preview, uri):
    """Only inspect allowlisted enums AFTER Rust canonical validation succeeded."""
    require(isinstance(preview,dict) and preview.get('kind')=='profile'
        and preview.get('version')==1 and isinstance(preview.get('profile'),dict), 'fixture_unavailable')
    profile=preview['profile']; protocol=profile.get('protocol'); classes=[]; xhttp='not-applicable'
    if protocol=='vless':
        features=profile.get('experimentalFeatures',[])
        require(isinstance(features,list), 'fixture_unavailable')
        if profile.get('transport')=='xhttp':
            values=urllib.parse.parse_qs(urllib.parse.urlsplit(uri).query,keep_blank_values=True).get('mode',[])
            xhttp=values[0] if len(values)==1 and values[0] else 'default'
            require(len(values)<=1 and xhttp in MODES, 'fixture_unavailable')
            classes.append('vless-xhttp')
        if 'VLESS Encryption' in features: classes.append('vless-encryption')
        if 'REALITY PQ' in features: classes.append('vless-reality-pq')
    elif protocol=='trojan': classes=['trojan']
    elif protocol=='hysteria2': classes=['hysteria2-normal','hysteria2-udp-restricted']
    elif protocol=='tuic': classes=['tuic-v5'] # canonical TUIC parser accepts only v5
    return classes, xhttp


class Native:
    def __init__(self, binary_hash):
        self.binary_hash=binary_hash
        self.runtime=Path('/run/user') / str(os.getuid())
        self.home=Path.home()
        self.instance=None
        require(re.fullmatch(r'[0-9a-f]{64}',binary_hash), 'binary_identity_required')
        require(installed.valid_environment(os.environ,self.home,self.runtime), 'environment_mismatch')
        self.check_binary()
        require(installed.command(['/usr/bin/omavless','plugin','target'])==b'rust\n','native_owner_required')
        self.instance=self.read('ui.snapshot')['result']['instanceId']

    def check_binary(self):
        info=Path('/usr/bin/omavless').lstat()
        require(stat.S_ISREG(info.st_mode) and info.st_uid==0 and not info.st_mode & 0o022,'binary_identity')
        with open('/usr/bin/omavless','rb') as stream:
            require(hashlib.file_digest(stream,'sha256').hexdigest()==self.binary_hash,'binary_changed')

    def read(self, method, params=None):
        require(method in {'ui.snapshot','runtime.observation','profiles.export','imports.classify','plugin.action'},
            'method_not_allowed')
        self.check_binary()
        path=self.runtime / 'omavless/control.sock'
        info=path.lstat(); parent=path.parent.lstat()
        require(stat.S_ISSOCK(info.st_mode) and info.st_uid==os.getuid() and stat.S_IMODE(info.st_mode)==0o600
            and stat.S_ISDIR(parent.st_mode) and parent.st_uid==os.getuid() and stat.S_IMODE(parent.st_mode)==0o700,
            'socket_identity')
        request_id='v0-'+uuid.uuid4().hex
        frame=(json.dumps(dict(api='omavless.control',version=1,id=request_id,method=method,params=params or {}))+'\n').encode()
        require(len(frame)<=65536,'request_bound')
        deadline=time.monotonic()+(125 if method=='plugin.action' else 10)
        with socket.socket(socket.AF_UNIX) as connection:
            connection.settimeout(5);connection.connect(str(path))
            pid,uid,_=struct.unpack('3i',connection.getsockopt(socket.SOL_SOCKET,socket.SO_PEERCRED,12))
            require(uid==os.getuid() and pid==int(installed.unit(installed.UNIT,'MainPID')),'socket_peer')
            connection.sendall(frame);connection.shutdown(socket.SHUT_WR)
            data=bytearray()
            while True:
                remaining=deadline-time.monotonic();require(remaining>0,'native_timeout')
                connection.settimeout(remaining)
                chunk=connection.recv(min(4096,MAX_RESPONSE+1-len(data)))
                if not chunk: break
                data.extend(chunk);require(len(data)<=MAX_RESPONSE,'response_bound')
        reply=response(data,request_id)
        if method=='ui.snapshot' and self.instance is not None:
            require(reply['result'].get('instanceId')==self.instance,'runtime_changed')
        return reply

    def snapshot(self): return self.read('ui.snapshot')['result']
    def observation(self): return self.read('runtime.observation')['result']

    def action(self, name, extra=None):
        require(name in {'connect','disconnect','mode'},'action_not_allowed')
        current=self.read('ui.snapshot')
        params=dict(instanceId=self.instance,expectedRevision=current['revision'],
            operationId='v0-'+uuid.uuid4().hex,action=name,**(extra or {}))
        return self.read('plugin.action',params)

    def fixture(self, record):
        snap=self.snapshot()
        require(any(p['id']==record and p.get('missing') is False for p in snap['profiles']),'fixture_unavailable')
        exported=self.read('profiles.export',{'profileId':record,'purpose':'file'})
        require(exported['result'].get('format')=='uri','fixture_unavailable')
        uri=exported['result'].get('content')
        require(isinstance(uri,str) and len(uri.encode())<=32768,'fixture_unavailable')
        preview=self.read('imports.classify',{'input':uri})
        require(preview['revision']==exported['revision'],'fixture_changed')
        return classify(preview['result'],uri)

    def inventory(self):
        snapshot=self.snapshot(); rows=snapshot.get('profiles')
        require(isinstance(rows,list) and len(rows)<=256,'inventory_bound')
        counts={kind:0 for kind in CLASSES if kind!='hysteria2-udp-restricted'}
        selected={}
        # Prefer the original active record when it represents the class.
        rows=sorted(rows,key=lambda row:row['id']!=snapshot['desired']['profileId'])
        for row in rows:
            if row.get('missing') is not False: continue
            kinds,_=self.fixture(row['id'])
            for kind in kinds:
                if kind not in counts: continue
                counts[kind]+=1;selected.setdefault(kind,row['id'])
        return counts,[{'class':kind,'profileId':record} for kind,record in selected.items()]

    def clean(self):
        require(installed.clean_disconnected_observation(self.observation()),'manual_recovery_required')
        require(not installed.tuns() and not any(v==b'mihomo' for v in gate.processes().values())
            and not os.path.lexists(self.runtime/'omavless/mihomo.sock'),'manual_recovery_required')

    def settled(self, desired):
        snapshot=self.snapshot();observed=self.observation();facts=observed.get('facts') or {}
        require(all(snapshot['desired'].get(k)==desired[k] for k in ('connected','profileId','mode')),'restore_mismatch')
        if not desired['connected']:
            self.clean();return
        require(observed.get('availability')=='observed' and observed.get('lastKnownActual')=='connected'
            and observed.get('manualRecoveryRequired') is False and observed['desired']['mode']==desired['mode']
            and all(facts.get(k) is True for k in ('ownedCoreRunning','desiredProfileMatchesOwned','ownedControllerConfigVerified'))
            and all(type(facts.get(k)) is int and facts[k]==v for k,v in
                [('visibleMihomoCount',1),('visibleTunCount',1),('managedTunCount',1),('ownedAuxiliaryMihomoCount',0)]),
            'owned_runtime_not_verified')

    def evidence(self, record, authorization, baseline):
        self.settled({'connected':True,'profileId':record,'mode':'global'})
        pid=int(installed.unit(installed.UNIT,'MainPID'))
        require(installed.unit(installed.UNIT,'ActiveState')=='active'
            and installed.unit(installed.LEGACY,'ActiveState')=='inactive','service_ownership')
        group=(Path('/sys/fs/cgroup')/installed.unit(installed.UNIT,'ControlGroup').lstrip('/')).resolve(strict=True)
        require(group.is_relative_to('/sys/fs/cgroup') and group!=Path('/sys/fs/cgroup'),'service_ownership')
        cores={p for p,name in gate.processes().items() if name==b'mihomo'}
        require(len(cores)==1 and cores<=gate.descendants(pid)
            and cores<=set(map(int,gate.bounded(group/'cgroup.procs').split())),'service_ownership')
        core=next(iter(cores)); links=installed.tuns();require(len(links)==1,'tun_ownership')
        tun=next(iter(links))
        require(os.getpgid(core)==core,'service_ownership')
        gate.core_controller(self.runtime/'omavless/mihomo.sock',core,'global')
        config=gate.bounded(self.home/'.config/omavless/config.yaml',5242880)
        port=installed.template_policy(config)
        require(re.findall(rb'''(?m)^[ \t]*["']?(external-controller(?:-[A-Za-z0-9_-]+)?)["']?[ \t]*:''',config)
            ==[b'external-controller-unix'],'tcp_controller_config')
        addresses={installed.proc_address(entry['local']) for link in json.loads(installed.command(
            ['/usr/bin/ip','-j','address','show','dev',tun])) for entry in link.get('addr_info',[])}
        rows=installed.listener_rows(gate.bounded(Path('/proc/net/tcp'),1048576),gate.bounded(Path('/proc/net/tcp6'),1048576))
        proof=authorization.step('socket_inspection',lambda:installed.command(['/usr/bin/sudo','/usr/bin/ss','-H','-ltnpe'],timeout=None))
        installed.classify_listeners(baseline,rows,port,addresses,proof,core)
        before=installed.counters(tun)
        try:
            probe=subprocess.run(installed.probe_args(tun),stdin=subprocess.DEVNULL,capture_output=True,timeout=25)
        except subprocess.TimeoutExpired:
            raise Refused('probe_timeout') from None
        good,used,code=gate.https_probe_evidence(probe,before,installed.counters(tun))
        self.settled({'connected':True,'profileId':record,'mode':'global'})
        return {'nativeConfigAdmission':True,'fullVpnTun':True,'unixController':True,
            'tcpControllerAbsent':True,'https':good,'tunUsedDuringProbe':used,'probeClass':code}


def run_cases(host,cases,authorization):
    authorization.require_terminal()
    # No safe network-switch fixture is implemented. An assertion flag alone
    # cannot turn two runs on the same network into a controlled network pair.
    require(not any(c['class']=='hysteria2-udp-restricted' for c in cases),'safe_restricted_network_unavailable')
    initial=host.snapshot();original=initial['desired'];host.settled(original)
    require(original['connected'] or all(c['profileId']==initial['lastProfileId'] for c in cases),
        'disconnected_profile_restore_unavailable')
    results=[];touched=False;restore='not-needed';blocked=False
    try:
        if original['connected']:
            touched=True;authorization.step('disconnect',lambda:host.action('disconnect'));host.clean()
        for case in cases:
            kind=case['class'];record=case['profileId']
            row={'caseClass':kind,'protocol':CLASSES[kind],'state':'FAIL','cleanup':False,
                'nativeConfigAdmission':False,'fullVpnTun':False,'https':False,'tunUsedDuringProbe':False}
            attempted=False;connect_returned=False
            try:
                kinds,xhttp=host.fixture(record)
                require(kind in kinds,'fixture_unavailable')
                row['xhttpMode']=xhttp
                host.clean();baseline=gate.tcp_listeners()
                attempted=True;touched=True
                authorization.step('connect',lambda:host.action('connect',{'profileId':record,'mode':'global'}))
                connect_returned=True
                row.update(host.evidence(record,authorization,baseline))
                row['state']='PASS' if row['https'] and row['tunUsedDuringProbe'] else 'FAIL'
            except Refused as error:
                row['errorCode']=str(error) if str(error) in PUBLIC_FAILURES else 'live_check_failed'
                if str(error)=='fixture_unavailable':row['state']='FIXTURE UNAVAILABLE'
                if attempted and not connect_returned and str(error) not in DEFINITE_REFUSALS:
                    blocked=True
                if str(error) in {'manual_recovery_required','runtime_changed','daemon_restarting'}:
                    blocked=True
            except auth.AuthorizationUnsettled:
                blocked=True;row['errorCode']='human_authorization_unsettled'
            except BaseException:
                row['errorCode']='live_check_failed'
                if attempted and not connect_returned:blocked=True
            finally:
                if attempted and not authorization.blocked and not blocked:
                    try:
                        authorization.step('disconnect',lambda:host.action('disconnect'));host.clean();row['cleanup']=True
                    except BaseException:
                        blocked=True;row['cleanup']=False
                elif not attempted:row['cleanup']=True
                else:blocked=True
                if not row['cleanup']:row['state']='FAIL'
                results.append(row)
            if blocked:break
    except BaseException:
        blocked=True
    finally:
        if touched:
            restore='manual-recovery-required'
            if not blocked and not authorization.blocked:
                try:
                    host.clean()
                    if original['connected']:
                        authorization.step('connect',lambda:host.action('connect',{'profileId':original['profileId'],'mode':original['mode']}))
                    elif host.snapshot()['desired']['mode']!=original['mode']:
                        authorization.step('restore_mode',lambda:host.action('mode',{'mode':original['mode']}))
                    host.settled(original)
                    require(host.snapshot()['lastProfileId']==initial['lastProfileId'],'restore_mismatch')
                    restore='restored'
                except BaseException:pass
        if blocked:restore='manual-recovery-required'
    return {'schemaVersion':2,'scope':'native_v0_live_validation','probeHost':'example.com',
        'restoration':restore,'results':results,'requestedCases':len(cases),
        'complete':len(results)==len(cases) and all(r['state']=='PASS' and r['cleanup'] for r in results)
            and restore in ('restored','not-needed')}


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime-sha256',required=True)
    modes=parser.add_mutually_exclusive_group(required=True)
    modes.add_argument('--inventory',action='store_true')
    modes.add_argument('--prepare-cases',type=Path)
    modes.add_argument('--cases',type=Path)
    parser.add_argument('--output',type=Path)
    parser.add_argument('--confirm-live',action='store_true')
    args=parser.parse_args(argv)
    if args.cases:
        require(args.confirm_live and args.output is not None,'explicit_live_consent_required')
        authorization=auth.HumanAuthorization();authorization.require_terminal()
        cases=load_cases(args.cases)
        outside_git(args.output)
        require(not os.path.lexists(args.output),'output_exists')
        info=args.output.parent.stat()
        require(info.st_uid==os.getuid() and stat.S_IMODE(info.st_mode)==0o700,'output_directory_not_private')
    host=Native(args.runtime_sha256)
    if not args.cases:
        counts,cases=host.inventory()
        if args.prepare_cases:
            require(bool(cases),'fixture_unavailable')
            private_write(args.prepare_cases,{'schemaVersion':2,'cases':cases})
        print(json.dumps({'counts':counts,'casesPrepared':len(cases) if args.prepare_cases else 0}))
        return 0
    payload=run_cases(host,cases,authorization)
    payload['runtimeSha256']=args.runtime_sha256
    private_write(args.output,payload)
    # Results are explicit allowlisted enums/booleans. Never print any input.
    print(json.dumps(payload))
    return 0 if payload['complete'] else 1


if __name__=='__main__':
    try:
        raise SystemExit(main())
    except Refused as error:
        print(json.dumps({'ok':False,'errorCode':str(error)}));raise SystemExit(2)
    except (Exception,KeyboardInterrupt):
        print('{"ok":false,"errorCode":"validation_stopped"}');raise SystemExit(2)
