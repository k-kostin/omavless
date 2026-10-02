#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Opt-in disposable-VM AWG 3/3.1 peer smoke; no installed/profile access.

The imported WG helper supplies bounded private subprocesses, controller and
outside-state guards. This is developer test glue, not a protocol parser or
production lifecycle. The independent peer is official pinned userspace AWG.
"""
import argparse
import base64
import fcntl
import json
import os
from pathlib import Path
import re
import shutil
import signal
import socket
import stat
import struct
import subprocess
import sys
import tempfile
import time

import p4_wg_loopback_smoke as wg

UPSTREAM = "b5928efb6ca19f0153958460c3d141f04abc5c2e"
# Narrow singleton, disjoint, non-WG headers avoid the independently documented
# upstream RandomTrailers/wide-H1-H3 classification issue. Jc is deliberately 4.
FIELDS = {"Jc":"4", "Jmin":"64", "Jmax":"66", "S1":"24", "S2":"32",
          "S3":"40", "S4":"48", "H1":"101", "H2":"202", "H3":"303", "H4":"404",
          "ContentPaddingAddition":"37", "RekeyAfterTime":"2", "RekeyTimeout":"1",
          "RejectAfterTime":"30", "KeepaliveTimeout":"1", "MaxHandshakeAttempts":"4"}
UAPI_NAMES = {"HeaderProtectionKey":"header_protection_key", "ContentPaddingAddition":"content_padding_addition",
              "RekeyAfterTime":"rekey_after_time", "RekeyTimeout":"rekey_timeout",
              "RejectAfterTime":"reject_after_time", "KeepaliveTimeout":"keepalive_timeout",
              "MaxHandshakeAttempts":"max_handshake_attempts", "RandomTrailers":"random_trailers",
              "DisableCookies":"disable_cookies"}
for number in range(1,6):
    FIELDS[f"I{number}"] = "<b 0x" + (b"P4AW" + bytes([48+number]) + b"x"*(14+number)).hex() + ">"


def socket_payload(path, peer_pid, payload=None):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
        client.settimeout(2)
        client.connect(str(path))
        pid, _, _ = struct.unpack("3i",client.getsockopt(socket.SOL_SOCKET,socket.SO_PEERCRED,12))
        wg.require(pid == peer_pid,"awg_peer_identity")
        if payload is not None:
            client.sendall(payload)
        data = bytearray()
        while not data.endswith(b"\n\n" if payload is not None else b"\n"):
            part = client.recv(4096)
            wg.require(part and len(data)+len(part) <= wg.BODY_LIMIT,"awg_uapi_bound")
            data.extend(part)
    return bytes(data)


def peer_get(root, pid):
    data = socket_payload(root/"peer.sock",pid,b"get=1\n\n")
    pairs = [line.split(b"=",1) for line in data.strip().splitlines()]
    wg.require(all(len(pair)==2 for pair in pairs),"awg_uapi_frame")
    values = {}
    for key,value in pairs:
        wg.require(key not in values,"awg_uapi_frame")
        values[key] = value
    wg.require(values.get(b"errno") == b"0","awg_uapi_result")
    return values  # Private in-memory only: contains synthetic credentials.


def observation(root,pid):
    data = json.loads(socket_payload(root/"stats.sock",pid))
    wg.require(set(data)=={"junk","special","special_mask","init","response","transport","protected","trailers","padding_size_matches"}
               and all(type(n) is int and 0 <= n <= 10000 for n in data.values()),"awg_observation")
    return data


def private_keys(root,generation):
    keys = {name:wg.command(["/usr/bin/wg","genkey"],"key_generation").stdout.strip()
            for name in ("server","client","wrong","header","wrong_header")}
    keys["psk"] = wg.command(["/usr/bin/wg","genpsk"],"key_generation").stdout.strip()
    wg.require(keys["client"] != keys["wrong"] and keys["header"] != keys["wrong_header"],"negative_key_identity")
    public = {name:wg.command(["/usr/bin/wg","pubkey"],"key_generation",data=keys[name]).stdout.strip()
              for name in ("server","client")}
    for name,value in keys.items():
        wg.private_write(root/f"{name}.key",value)
    fields = {**FIELDS,"HeaderProtectionKey":keys["header"].decode()}
    if generation == "3.1":
        fields.update(RandomTrailers="true",DisableCookies="true")
    for name,key,header in (("positive",keys["client"],keys["header"]),
                            ("negative",keys["wrong"],keys["header"]),
                            ("header",keys["client"],keys["wrong_header"])):
        options = {**fields,"HeaderProtectionKey":header.decode()}
        conf = b"[Interface]\nPrivateKey = "+key+b"\nAddress = 10.203.0.2/32\nMTU = 1420\n"
        conf += "".join(f"{field} = {value}\n" for field,value in options.items()).encode()
        conf += b"[Peer]\nPublicKey = "+public["server"]+b"\nPresharedKey = "+keys["psk"]+b"\nAllowedIPs = 10.203.0.1/32\nEndpoint = 127.0.0.1:51888\nPersistentKeepalive = 1\n"
        # The shared renderer accepts only fixed positive/negative file names.
        wg.private_write(root/f"{name}.conf",conf)
    payload = b"set=1\nprivate_key="+base64.b64decode(keys["server"]).hex().encode()+b"\nlisten_port=51889\nreplace_peers=true\n"
    expected = {}
    for name,value in fields.items():
        uapi = UAPI_NAMES.get(name,name.lower()).encode()
        rendered = base64.b64decode(value).hex().encode() if name=="HeaderProtectionKey" else value.encode()
        payload += uapi+b"="+rendered+b"\n"
        expected[uapi] = b"1" if rendered==b"true" else rendered
    payload += b"public_key="+base64.b64decode(public["client"]).hex().encode()+b"\npreshared_key="+base64.b64decode(keys["psk"]).hex().encode()+b"\nallowed_ip=10.203.0.2/32\npersistent_keepalive_interval=0\n\n"
    return public["client"],payload,expected


def start_peer(root,args):
    tun_fd = os.open("/dev/net/tun",os.O_RDWR|os.O_CLOEXEC)
    process = None
    try:
        # IFF_TUN | IFF_NO_PI, TUNSETIFF. The FD owns a nonpersistent interface
        # created in this proven fresh namespace, never moved from outside.
        fcntl.ioctl(tun_fd,0x400454ca,struct.pack("16sH",b"wg-p4",0x0001|0x1000))
        for command in (("address","add","10.203.0.1/32","dev",wg.DEVICE),
                        ("link","set",wg.DEVICE,"mtu","1420"),
                        ("link","set",wg.DEVICE,"up"),
                        ("route","add","10.203.0.2/32","dev",wg.DEVICE)):
            wg.command(["/usr/bin/ip",*command],"awg_interface_setup")
        env = {**wg.ENV,"P4_TUN_FD":str(tun_fd),"P4_PARENT_net_FD":args.parent_net_fd,
               "P4_PARENT_user_FD":args.parent_user_fd}
        with wg.log_handle(root/"peer.log") as log:
            process = subprocess.Popen(wg.dropped([args.peer,str(root)]),stdin=subprocess.DEVNULL,
                stdout=log,stderr=log,env=env,preexec_fn=wg.child_limit,
                pass_fds=(tun_fd,int(args.parent_net_fd),int(args.parent_user_fd)))
        deadline = time.monotonic()+5
        while not (root/"stats.sock").exists():
            wg.require(process.poll() is None and time.monotonic()<deadline,"awg_peer_readiness")
            time.sleep(.05)
        wg.check_process(process,wg.namespace("net"))
        return process,tun_fd
    except BaseException:
        wg.stop(process)
        os.close(tun_fd)
        raise


def namespace_run(args):
    root = Path(args.scratch)
    wg.require(wg.private_directory(root) and os.getuid()==0 and os.getppid()==int(args.parent_pid)
               and os.fstat(int(args.parent_net_fd)).st_ino==int(args.parent_net)
               and os.fstat(int(args.parent_user_fd)).st_ino==int(args.parent_user)
               and wg.namespace("net")!=int(args.parent_net)
               and wg.namespace("user")!=int(args.parent_user),"namespace_identity")
    wg.private_write(root/"namespace-identity.json",json.dumps({"net":wg.namespace("net"),"user":wg.namespace("user")}).encode())
    links = json.loads(wg.command(["/usr/bin/ip","-j","link","show"],"namespace_inventory").stdout)
    routes = json.loads(wg.command(["/usr/bin/ip","-j","route","show","table","all"],"namespace_inventory").stdout)
    wg.require([row["ifname"] for row in links]==["lo"] and not routes,"namespace_interfaces")
    wg.command(["/usr/bin/ip","link","set","lo","up"],"namespace_loopback")
    result = {"positive":0,"negative":0,"header_negative":0,"recovery":0,"private_roundtrip":0,"rekey":0,
              "generations":{},"net_inode":wg.namespace("net"),"user_inode":wg.namespace("user")}
    for generation in ("3","3.1"):
        result["generations"][generation] = []
        for number in range(args.rounds):
            round_root = root/f"g{generation}-r{number+1}"
            round_root.mkdir(mode=0o700)
            public,payload,expected = private_keys(round_root,generation)
            peer=http=None; tun_fd=http_fd=None
            try:
                peer,tun_fd = start_peer(round_root,args)
                wg.require(socket_payload(round_root/"peer.sock",peer.pid,payload)==b"errno=0\n\n","awg_peer_configuration")
                actual = peer_get(round_root,peer.pid)
                wg.require(all(actual.get(key)==value for key,value in expected.items()),"awg_fields_active")
                def stats(public_key):
                    values=peer_get(round_root,peer.pid)
                    wg.require(values.get(b"public_key")==base64.b64decode(public_key).hex().encode(),"peer_identity")
                    return tuple(int(values[key]) for key in (b"last_handshake_time_sec",b"rx_bytes",b"tx_bytes"))
                wg.require(stats(public)==(0,0,0),"fresh_peer")
                with wg.log_handle(round_root/"http.log") as log:
                    http_fd=os.open("/proc/self/ns/net",os.O_RDONLY|os.O_CLOEXEC)
                    http=wg.launch([sys.executable,Path(wg.__file__).resolve(),"--http-child","--parent-pid",str(os.getpid()),"--parent-net-fd",str(http_fd)],log,pass_fds=(http_fd,))
                    deadline=time.monotonic()+3
                    while True:
                        wg.require(http.poll() is None and time.monotonic()<deadline,"http_readiness")
                        try:
                            with socket.create_connection(("10.203.0.1",8089),timeout=.1): break
                        except OSError: time.sleep(.05)
                    wg.check_process(http,result["net_inode"])
                    for phase in ("positive","negative"):
                        rendered=wg.command(wg.dropped([args.renderer,str(round_root),phase,generation]),"private_render")
                        wg.require(json.loads(rendered.stdout)=={"private_roundtrip":True,"flavor":generation},"private_roundtrip")
                        result["private_roundtrip"]+=1
                    wg.phase(round_root,Path(args.core),"positive","positive",public,result["net_inode"],observe=stats)
                    result["positive"]+=1
                    active=observation(round_root,peer.pid)
                    wg.require(active["junk"]>=4 and active["special"]>=5 and active["special_mask"]==31 and active["init"]>=1
                               and active["response"]>=1 and active["transport"]>=1 and active["protected"]>=3
                               and active["padding_size_matches"]>=1,"awg_wire_fields")
                    wg.require(active["trailers"]>0 if generation=="3.1" else active["trailers"]==0,"awg_generation_wire")
                    # The 2-second custom rekey is observed while the same core
                    # stays alive in a separately bounded phase below.
                    rekey_phase(round_root,args,public,result["net_inode"],stats)
                    result["rekey"]+=1
                    # Erase the previous authenticated peer/session/timers so
                    # background responses cannot contaminate a refusal proof.
                    wg.require(socket_payload(round_root/"peer.sock",peer.pid,payload)==b"errno=0\n\n"
                               and stats(public)==(0,0,0),"awg_negative_reset")
                    wg.phase(round_root,Path(args.core),"negative","negative",public,result["net_inode"],observe=stats)
                    result["negative"]+=1
                    # Render the wrong header key through the very same native
                    # store path (not a YAML splice) using a dedicated child root.
                    wrong=round_root/"wrong-header"; wrong.mkdir(mode=0o700)
                    wg.private_write(wrong/"negative.conf",(round_root/"header.conf").read_bytes())
                    rendered=wg.command(wg.dropped([args.renderer,str(wrong),"negative",generation]),"private_render")
                    wg.require(json.loads(rendered.stdout)=={"private_roundtrip":True,"flavor":generation},"private_roundtrip")
                    result["private_roundtrip"]+=1
                    wg.require(socket_payload(round_root/"peer.sock",peer.pid,payload)==b"errno=0\n\n"
                               and stats(public)==(0,0,0),"awg_negative_reset")
                    wg.phase(wrong,Path(args.core),"negative","header",public,result["net_inode"],observe=stats)
                    result["header_negative"]+=1
                    wg.require(socket_payload(round_root/"peer.sock",peer.pid,payload)==b"errno=0\n\n"
                               and stats(public)==(0,0,0),"awg_negative_reset")
                    wg.phase(round_root,Path(args.core),"positive","recovery",public,result["net_inode"],observe=stats)
                    result["recovery"]+=1
                    result["generations"][generation].append(observation(round_root,peer.pid))
                    wg.check_process(peer,result["net_inode"])
            except BaseException as error:
                if peer is not None and peer.poll() is not None:
                    log=(round_root/"peer.log").read_bytes()
                    for name in ("RoutineReceiveIncoming","HeaderProtectionCipher","RoutineReadFromTUN","RoutineEncryption","RoutineSequentialReceiver","SendKeepalive"):
                        if name.encode() in log and b"panic:" in log:
                            raise wg.Refused("awg_peer_panic_"+name) from None
                    raise wg.Refused("awg_peer_exited") from None
                if isinstance(error,wg.Refused) and peer is not None:
                    error.facts=observation(round_root,peer.pid)
                    values=peer_get(round_root,peer.pid)
                    error.facts.update(handshake=int(values[b"last_handshake_time_sec"]),rx=int(values[b"rx_bytes"]),tx=int(values[b"tx_bytes"]))
                raise
            finally:
                wg.stop(http); wg.stop(peer)
                for descriptor in (http_fd,tun_fd):
                    if descriptor is not None: os.close(descriptor)
                links=json.loads(wg.command(["/usr/bin/ip","-j","link","show"],"namespace_inventory").stdout)
                wg.require([row["ifname"] for row in links]==["lo"],"awg_interface_cleanup")
    result["interface_cleanup"]=True
    print(json.dumps(result,sort_keys=True))


def rekey_phase(root,args,public,net,stats):
    child=None; sock=root/"mihomo.sock"
    try:
        with wg.log_handle(root/"rekey.log") as log:
            child=wg.launch([args.core,"-d",root,"-f",root/"positive.yaml"],log)
            deadline=time.monotonic()+5
            while not sock.exists():
                wg.require(child.poll() is None and time.monotonic()<deadline,"core_readiness")
                time.sleep(.05)
            wg.controller(sock,child.pid); sock.chmod(0o600)
            def request():
                response=wg.command(wg.dropped(["/usr/bin/curl","--silent","--fail","--noproxy","","--socks5","127.0.0.1:7898","--max-time","3","http://10.203.0.1:8089/"]),"transport_request")
                wg.require(response.stdout==wg.RESPONSE,"positive_transport")
            request(); before=stats(public)[0]
            time.sleep(3)
            request()
            deadline=time.monotonic()+3
            while stats(public)[0]<=before:
                wg.require(child.poll() is None and time.monotonic()<deadline,"awg_rekey")
                time.sleep(.05)
            wg.check_process(child,net)
    finally:
        wg.stop(child)
        if sock.exists(): sock.unlink()


def outer(args):
    wg.require(args.run and os.getuid()!=0 and 1<=args.rounds<=3,"explicit_vm_opt_in")
    wg.require(re.fullmatch(r"[a-f0-9]{40}",args.source_sha or "") and args.upstream_sha==UPSTREAM,"source_identity")
    for tool in ("unshare","ip","wg","curl","setpriv","getcap"):
        wg.require(Path(f"/usr/bin/{tool}").is_file(),"missing_tool")
    wg.require(Path("/dev/net/tun").is_char_device(),"tun_unavailable")
    before=wg.outside_snapshot()
    sources={str(Path(__file__).resolve()):wg.digest(Path(__file__).resolve()),str(Path(wg.__file__).resolve()):wg.digest(Path(wg.__file__).resolve())}
    cache=Path.home()/".cache"/"omavless-p4-awg-loopback"
    cache.mkdir(mode=0o700,exist_ok=True)
    wg.require(wg.private_directory(cache),"scratch_parent")
    root=Path(tempfile.mkdtemp(prefix="run-",dir=cache))
    child=None; descriptors=[]; outcome=None
    try:
        for name,source,identity in (("core",args.core,args.expected_core_sha256),
                                     ("renderer",args.renderer,args.expected_renderer_sha256),
                                     ("peer",args.peer,args.expected_peer_sha256)):
            wg.copy_binary(Path(source),root/name,identity)
        descriptors=[os.open(f"/proc/self/ns/{name}",os.O_RDONLY|os.O_CLOEXEC) for name in ("net","user")]
        argv=["/usr/bin/unshare","--user","--map-root-user","--net",sys.executable,str(Path(__file__).resolve()),
              "--namespace-child","--scratch",str(root),"--parent-pid",str(os.getpid()),
              "--parent-net",str(wg.namespace("net")),"--parent-user",str(wg.namespace("user")),
              "--parent-net-fd",str(descriptors[0]),"--parent-user-fd",str(descriptors[1]),
              "--core",str(root/"core"),"--renderer",str(root/"renderer"),"--peer",str(root/"peer"),"--rounds",str(args.rounds)]
        with wg.log_handle(root/"namespace-result.json") as log:
            child=subprocess.Popen(argv,stdin=subprocess.DEVNULL,stdout=log,stderr=log,env=wg.ENV,
                                   start_new_session=True,pass_fds=descriptors)
            code=child.wait(timeout=150)
        data=(root/"namespace-result.json").read_bytes()
        wg.require(len(data)<=wg.BODY_LIMIT,"namespace_result_bound")
        outcome=json.loads(data)
        if code!=0:
            stage=outcome.get("stage","") if isinstance(outcome,dict) else ""
            # Never forward arbitrary child text, even when it happens to look
            # like an exception. Only our finite literal refusal vocabulary.
            wg.require(stage in SAFE_STAGES,"namespace_smoke")
            if stage=="unexpected_fixed_failure":
                diagnostic(outcome.get("failure_class"),outcome.get("failure_line"),outcome.get("failure_function"))
            error=wg.Refused(stage)
            if isinstance(outcome.get("facts"),dict): error.facts=outcome["facts"]
            raise error
        wg.require(all(outcome.get(name)==2*args.rounds for name in ("positive","negative","header_negative","recovery","rekey"))
                   and outcome.get("private_roundtrip")==6*args.rounds and outcome.get("interface_cleanup") is True,"namespace_result")
    finally:
        if child is not None:
            for action in (signal.SIGTERM,signal.SIGKILL):
                try: os.killpg(child.pid,action)
                except ProcessLookupError: break
                try: child.wait(timeout=3)
                except subprocess.TimeoutExpired: pass
            wg.require(child.poll() is not None,"owned_namespace_cleanup")
        for descriptor in descriptors: os.close(descriptor)
        identity=root/"namespace-identity.json"
        if identity.exists():
            net=json.loads(identity.read_bytes())["net"]
            for entry in Path("/proc").iterdir():
                if entry.name.isdecimal():
                    try: inode=wg.namespace("net",entry.name)
                    except OSError: continue
                    wg.require(inode!=net,"namespace_process_cleanup")
        wg.require(root.parent==cache and wg.private_directory(root),"scratch_cleanup_target")
        shutil.rmtree(root)
        wg.require(not root.exists() and wg.outside_snapshot()==before,"outside_state_changed")
        wg.require(all(wg.digest(Path(path))==identity for path,identity in sources.items()),"harness_identity")
    outcome.update(status="PASS",source_sha=args.source_sha,upstream_sha=args.upstream_sha,
                   harness_sha256=sources[str(Path(__file__).resolve())],wg_helper_sha256=sources[str(Path(wg.__file__).resolve())],
                   core_sha256=args.expected_core_sha256,renderer_sha256=args.expected_renderer_sha256,peer_sha256=args.expected_peer_sha256,
                   scratch_cleanup=True,outside_state_unchanged=True,installed_runtime_unchanged=True)
    print(json.dumps(outcome,sort_keys=True))


SAFE_STAGES={"namespace_identity","namespace_interfaces","namespace_inventory","namespace_loopback",
             "awg_peer_identity","awg_uapi_bound","awg_uapi_frame","awg_uapi_result","awg_peer_readiness",
             "awg_peer_configuration","awg_fields_active","awg_interface_setup","awg_interface_cleanup",
             "awg_observation","awg_wire_fields","awg_generation_wire","awg_rekey","awg_negative_reset","fresh_peer","peer_identity",
             "http_readiness","child_identity","child_privileges","private_render","private_roundtrip",
             "core_validation","core_readiness","controller_collision","controller_peer","controller_frame",
             "controller_bound","no_direct_fallback","transport_request","positive_transport","handshake_transfer",
             "negative_direct_bypass","negative_peer_unchanged","owned_child_cleanup","controller_cleanup",
             "key_generation","negative_key_identity","unexpected_fixed_failure"}
SAFE_STAGES.update("awg_peer_panic_"+name for name in ("RoutineReceiveIncoming","HeaderProtectionCipher","RoutineReadFromTUN","RoutineEncryption","RoutineSequentialReceiver","SendKeepalive"))
SAFE_STAGES.add("awg_peer_exited")


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run",action="store_true")
    for name in ("namespace-child",): parser.add_argument(f"--{name}",action="store_true",help=argparse.SUPPRESS)
    for name in ("scratch","parent-net","parent-user","parent-pid","parent-net-fd","parent-user-fd"):
        parser.add_argument(f"--{name}",help=argparse.SUPPRESS)
    parser.add_argument("--core",default="/usr/bin/mihomo")
    for name in ("renderer","peer","expected-core-sha256","expected-renderer-sha256","expected-peer-sha256","source-sha","upstream-sha"):
        parser.add_argument(f"--{name}")
    parser.add_argument("--rounds",type=int,default=3)
    args=parser.parse_args()
    try:
        namespace_run(args) if args.namespace_child else outer(args)
    except wg.Refused as error:
        outcome={"status":"REFUSED","stage":error.stage}
        facts=getattr(error,"facts",None)
        allowed={"junk","special","special_mask","init","response","transport","protected","trailers","padding_size_matches","handshake","rx","tx"}
        if isinstance(facts,dict) and set(facts)<=allowed and all(type(value) is int and 0<=value<=2**63 for value in facts.values()): outcome["facts"]=facts
        print(json.dumps(outcome,sort_keys=True)); return 2
    except (Exception,KeyboardInterrupt) as error:
        trace=error.__traceback__
        while trace.tb_next is not None: trace=trace.tb_next
        diagnostic(type(error).__name__,trace.tb_lineno,trace.tb_frame.f_code.co_name)
        return 2
    return 0


def diagnostic(kind,line,function):
    # Class/function identifiers come from trusted fixture/interpreter code,
    # never an exception's message, arguments, locals or private file contents.
    def identifier(value):
        return value if isinstance(value,str) and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]{0,63}",value) else "other"
    print(json.dumps({"status":"REFUSED","stage":"unexpected_fixed_failure",
                      "failure_class":identifier(kind),"failure_function":identifier(function),
                      "failure_line":line if type(line) is int and 1<=line<=2000 else 0},sort_keys=True))


if __name__=="__main__": sys.exit(main())
