#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Opt-in synthetic private-netns characterization, NOT normal activation.

24 cells: pinned AWG engine (standard-WG mode / AWG3 / AWG3.1), outer/inner
IPv4/IPv6 and inner MTU1280/1420. No DNS/default route/sysctl/module/package
operation. External strict canonical host guard remains mandatory acceptance.
Ordinary CI imports pure guards only; no live execution without explicit VM ack.
"""
import argparse
import base64
import errno
import fcntl
import hashlib
import ipaddress
import itertools
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
import threading
import time

import p4_wg_loopback_smoke as wg
import p4_awg_loopback_smoke as awg

CELLS = tuple(itertools.product(("wireguard", "3", "3.1"), (4, 6), (4, 6), (1280, 1420)))
ENGINE = "official-amneziawg-go-v3-including-standard-WG-mode"
BODY = bytes(range(256)) * 256
ACK = b"P4ACK:"
PORT = 8090
ENV = {"PATH":"/usr/bin:/bin", "LANG":"C.UTF-8"}


def address(family, client=False):
    return ("10.203.0.2" if client else "10.203.0.1") if family == 4 else ("fd20:203::2" if client else "fd20:203::1")


def payload_size(family, inner_size):
    wg.require(family in (4,6) and 64 <= inner_size <= 1421, "size_policy")
    return inner_size - (28 if family == 4 else 48)


def fixed_payload(size):
    nonce = os.urandom(16)
    return nonce + bytes(range(256)) * ((size-16)//256) + bytes(range((size-16)%256))


def sha(payload):
    return hashlib.sha256(payload).hexdigest()


def recv_exact(sock, count):
    out = bytearray()
    while len(out) < count:
        block = sock.recv(count-len(out)); wg.require(block, "socks_frame"); out.extend(block)
    return bytes(out)


def socks_address(family, host, port):
    return bytes([1 if family == 4 else 4]) + ipaddress.ip_address(host).packed + struct.pack("!H",port)


def decode_address(data, pos=0):
    wg.require(len(data)>pos and data[pos] in (1,4), "socks_literal_only")
    size = 4 if data[pos]==1 else 16
    wg.require(len(data)>=pos+1+size+2, "socks_frame")
    return str(ipaddress.ip_address(data[pos+1:pos+1+size])), struct.unpack("!H",data[pos+1+size:pos+size+3])[0],pos+size+3


def socks_request(command, family, host, port):
    s=socket.create_connection(("127.0.0.1",7898),timeout=3); s.settimeout(3)
    try:
        s.sendall(b"\x05\x01\x00"); wg.require(recv_exact(s,2)==b"\x05\x00", "socks_auth")
        s.sendall(bytes([5,command,0])+socks_address(family,host,port))
        head=recv_exact(s,4); wg.require(head[:3]==b"\x05\x00\x00" and head[3] in (1,4), "socks_reply")
        frame=head[3:]+recv_exact(s,(4 if head[3]==1 else 16)+2)
        bind,bind_port,_=decode_address(frame)
        return s,bind,bind_port
    except BaseException: s.close(); raise


def http_bytes(family):
    s,_,_=socks_request(1,family,address(family),8089)
    with s:
        s.sendall(b"GET /matrix HTTP/1.1\r\nHost: fixture\r\nConnection: close\r\n\r\n")
        data=bytearray()
        while True:
            part=s.recv(16384)
            if not part: break
            data.extend(part); wg.require(len(data)<=len(BODY)+4096,"http_bound")
    header,body=bytes(data).split(b"\r\n\r\n",1)
    wg.require(header==b"HTTP/1.1 200 OK\r\nContent-Length: 65536\r\nConnection: close" and body==BODY,"http_exact_64k")
    return len(body)


def udp_request(family, payload):
    control,host,port=socks_request(3,4,"0.0.0.0",0)
    wg.require(host=="127.0.0.1" and 1<=port<=65535,"udp_relay_identity")
    replies=[]
    with control,socket.socket(socket.AF_INET,socket.SOCK_DGRAM) as client:
        client.settimeout(2); client.connect((host,port))
        client.send(b"\x00\x00\x00"+socks_address(family,address(family),PORT)+payload)
        for _ in range(2):
            try: data,_,flags,_=client.recvmsg(4096)
            except socket.timeout: break
            wg.require(not flags & socket.MSG_TRUNC and data[:3]==b"\x00\x00\x00","udp_complete_frame")
            source,source_port,pos=decode_address(data,3)
            wg.require(source==address(family) and source_port==PORT,"udp_response_identity")
            replies.append(data[pos:])
    expected_ack=ACK+hashlib.sha256(payload).digest()
    wg.require(all(reply in (payload,expected_ack) for reply in replies) and len(set(replies))==len(replies),"udp_exact_response")
    return {"ack":expected_ack in replies,"echo":payload in replies}


def read_json_socket(path,pid):
    return json.loads(awg.socket_payload(path,pid))


def observe(root,pid):
    data=read_json_socket(root/"stats.sock",pid)
    wg.require(isinstance(data,dict) and set(data)=={"packets","outer","malformed","write_errors","flows"},"observation_shape")
    wg.require(type(data["malformed"]) is int and type(data["write_errors"]) is int and data["malformed"]==data["write_errors"]==0,"observation_errors")
    for name in ("packets","outer"):
        wg.require(isinstance(data[name],dict) and len(data[name])<=256 and all(isinstance(key,str) and type(value) is int and 0<value<=100000 for key,value in data[name].items()),"observation_bound")
    wg.require(all(re.fullmatch(r"(?:client|peer):(?:4|6):loopback:[1-9][0-9]{0,3}",key) and int(key.rsplit(":",1)[1])<4096 for key in data["outer"]),"outer_shape")
    wg.require(all(re.fullmatch(r"(?:rx|tx):(?:4|6):[1-9][0-9]{1,4}:[01]",key) for key in data["packets"]),"packet_shape")
    wg.require(isinstance(data["flows"],list) and len(data["flows"])<=256,"flow_bound")
    keys={"direction","family","size","id","offset","more","fragment","fragment_payload_size","source_port","dest_port","payload_sha256"}
    for flow in data["flows"]:
        wg.require(isinstance(flow,dict) and set(flow)==keys and flow["direction"] in ("rx","tx") and type(flow["family"]) is int and flow["family"] in (4,6) and type(flow["more"]) is bool and type(flow["fragment"]) is bool and isinstance(flow["payload_sha256"],str) and (flow["payload_sha256"]=="" or re.fullmatch("[a-f0-9]{64}",flow["payload_sha256"])) and all(type(flow[field]) is int and 0<=flow[field]<=2**32-1 for field in ("size","id","offset","fragment_payload_size","source_port","dest_port")),"flow_shape")
    return data


def fragment_coverage(flows,direction,family,payload_length):
    """Exact nonoverlapping coverage of one UDP datagram; atomic v6 != fragment."""
    candidates=[f for f in flows if f["direction"]==direction and f["family"]==family and f["fragment"]]
    groups={}
    for f in candidates: groups.setdefault(f["id"],[]).append(f)
    valid=[]
    for group in groups.values():
        ordered=sorted(group,key=lambda f:f["offset"])
        if len(ordered)<2 or ordered[0]["offset"]!=0 or not ordered[0]["more"]: continue
        if direction=="rx" and ordered[0]["dest_port"]!=PORT or direction=="tx" and ordered[0]["source_port"]!=PORT: continue
        end=0; okay=True
        for index,f in enumerate(ordered):
            length=f["fragment_payload_size"]
            if f["offset"]!=end or length<=0 or f["more"]!=(index<len(ordered)-1) or f["more"] and length%8!=0: okay=False; break
            end+=length
        if okay and end==payload_length+8: valid.append(ordered)
    wg.require(len(valid)<=1,"ambiguous_fragment_identity")
    return valid[0] if valid else None


def assess_datagram(before,after,receipt,payload,replies,family,inner_size,mtu):
    wg.require(after["flows"][:len(before["flows"])]==before["flows"],"flow_prefix")
    flows=after["flows"][len(before["flows"]):]
    digest=sha(payload)
    wg.require(receipt.get("sha256")==digest and receipt.get("size")==len(payload) and receipt.get("family")==family and receipt.get("source_family")==family and receipt.get("route_mtu")==mtu,"udp_service_receipt")
    wg.require(replies["ack"] and receipt.get("ack_sent") is True,"udp_forward_ack")
    result={"inner_ip_size":inner_size,"app_bytes":len(payload),"sha256":digest}
    for direction in ("rx","tx"):
        exact=[f for f in flows if f["direction"]==direction and f["family"]==family and not f["fragment"] and f["payload_sha256"]==digest and f["size"]==inner_size]
        fragments=fragment_coverage(flows,direction,family,len(payload))
        wg.require(not (exact and fragments) and len(exact)<=1,"ambiguous_udp_shape")
        if exact: state="observed-unfragmented" if inner_size<=mtu else "observed-over-M-unfragmented-NOT-MTU-compliance"
        elif fragments: state="observed-reassembled-fragments"
        elif direction=="tx" and receipt.get("echo_emsgsize") is True and receipt.get("echo_sent") is False and inner_size>mtu: state="fixture-reverse-local-EMSGSIZE-under-explicit-PMTUDISC_DO"
        else: raise wg.Refused("udp_size_inconclusive")
        if inner_size<=mtu: wg.require(state=="observed-unfragmented" and replies["echo"] and receipt.get("echo_sent") is True,"udp_at_or_below_mtu")
        result["forward" if direction=="rx" else "reverse"]=state
    wg.require(replies["echo"]==receipt.get("echo_sent") and (replies["echo"] or receipt.get("echo_emsgsize") is True),"udp_echo_attribution")
    return result


def zero_capabilities():
    values=dict(line.split(":",1) for line in Path("/proc/self/status").read_text().splitlines() if ":" in line)
    wg.require(values.get("NoNewPrivs","").strip()=="1" and all(int(values.get(name,"1").strip(),16)==0 for name in ("CapEff","CapPrm","CapInh","CapAmb","CapBnd")),"child_privileges")


def namespace_authority(args, *, owner):
    root=Path(args.scratch)
    wg.require(root.is_absolute() and wg.private_directory(root) and os.getuid()==0 and os.getppid()==int(args.parent_pid),"namespace_identity")
    for name in ("net","user"):
        fd=int(getattr(args,f"parent_{name}_fd")); expected=int(getattr(args,f"parent_{name}"))
        wg.require(os.readlink(f"/proc/self/fd/{fd}").startswith(name+":[") and os.fstat(fd).st_ino==expected and wg.namespace(name)!=expected,"namespace_identity")
    if not owner: zero_capabilities()
    return root


def service_child(args):
    root=namespace_authority(args,owner=False); family=int(args.inner); server=address(family)
    af=socket.AF_INET if family==4 else socket.AF_INET6
    seen={}; lock=threading.Lock()
    def new_socket(kind):
        sock=socket.socket(af,kind)
        if family==6: sock.setsockopt(socket.IPPROTO_IPV6,socket.IPV6_V6ONLY,1)
        return sock
    http=new_socket(socket.SOCK_STREAM); http.bind((server,8089)); http.listen(4)
    udp=new_socket(socket.SOCK_DGRAM); udp.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1); udp.bind((server,PORT))
    control=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM); control.bind(str(root/"service.sock")); os.chmod(root/"service.sock",0o600); control.listen(4)
    def http_worker():
        while True:
            client,_=http.accept()
            with client:
                client.settimeout(3)
                try:
                    data=bytearray()
                    while b"\r\n\r\n" not in data:
                        block=client.recv(1024)
                        if not block or len(data)+len(block)>4096: break
                        data.extend(block)
                    if bytes(data)==b"GET /matrix HTTP/1.1\r\nHost: fixture\r\nConnection: close\r\n\r\n":
                        client.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 65536\r\nConnection: close\r\n\r\n"+BODY)
                except OSError: pass
    def udp_worker():
        while True:
            data,_,flags,source=udp.recvmsg(2048)
            if flags & socket.MSG_TRUNC or not 16<=len(data)<=1421 or source[0]!=address(family,True): continue
            digest=sha(data)
            record=dict(sha256=digest,size=len(data),family=family,source_family=ipaddress.ip_address(source[0]).version,ack_sent=False,echo_sent=False,echo_emsgsize=False,route_mtu=0)
            # Fresh connected response socket: no errors from an earlier UDP
            # datagram can be attributed to this nonce. This is fixture policy,
            # not the core's native DF/PMTU policy. Fixed source port is retained.
            with new_socket(socket.SOCK_DGRAM) as reply:
                reply.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1); reply.bind((server,PORT)); reply.connect(source)
                if family==4:
                    reply.setsockopt(socket.IPPROTO_IP,10,2) # IP_MTU_DISCOVER, IP_PMTUDISC_DO
                    record["route_mtu"]=reply.getsockopt(socket.IPPROTO_IP,14) # IP_MTU
                else:
                    reply.setsockopt(socket.IPPROTO_IPV6,23,2) # IPV6_MTU_DISCOVER, IPV6_PMTUDISC_DO
                    record["route_mtu"]=reply.getsockopt(socket.IPPROTO_IPV6,24) # IPV6_MTU
                try:
                    record["ack_sent"]=reply.send(ACK+hashlib.sha256(data).digest())==len(ACK)+32
                    record["echo_sent"]=reply.send(data)==len(data)
                except OSError as exc:
                    if exc.errno==errno.EMSGSIZE and record["ack_sent"]: record["echo_emsgsize"]=True
            with lock:
                if len(seen)>=64 or digest in seen: os._exit(2)
                seen[digest]=record
    for worker in (http_worker,udp_worker): threading.Thread(target=worker,daemon=True).start()
    while True:
        client,_=control.accept()
        with client,lock: client.sendall(json.dumps(seen,sort_keys=True).encode()+b"\n")


def child_arguments(args,root,kind,inner):
    return [sys.executable,str(Path(__file__).resolve()),f"--{kind}-child","--scratch",str(root),"--inner",str(inner),"--parent-pid",str(os.getpid()),"--parent-net",args.parent_net,"--parent-user",args.parent_user,"--parent-net-fd",args.parent_net_fd,"--parent-user-fd",args.parent_user_fd]


def client_child(args):
    namespace_authority(args,owner=False)
    data=sys.stdin.buffer.read(4097); wg.require(len(data)<=4096,"client_input_bound")
    request=json.loads(data)
    wg.require(isinstance(request,dict),"client_input")
    if request=={"kind":"http"}:
        return {"status":"PASS","bytes":http_bytes(int(args.inner))}
    wg.require(set(request)=={"kind","payload"} and request["kind"]=="udp" and isinstance(request["payload"],str),"client_input")
    payload=base64.b64decode(request["payload"],validate=True)
    wg.require(16<=len(payload)<=1421,"client_payload_bound")
    return {"status":"PASS",**udp_request(int(args.inner),payload)}


def transport_client(args,root,inner,payload=None):
    request={"kind":"http"} if payload is None else {"kind":"udp","payload":base64.b64encode(payload).decode()}
    with wg.log_handle(root/f"client-{time.monotonic_ns()}.log") as log:
        process=subprocess.Popen(wg.dropped(child_arguments(args,root,"client",inner)),stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=log,env=ENV,preexec_fn=wg.child_limit,pass_fds=(int(args.parent_net_fd),int(args.parent_user_fd)))
        try:
            data,_=process.communicate(json.dumps(request).encode(),timeout=8)
            wg.require(len(data)<=4096,"client_result_bound")
            result=json.loads(data)
            wg.require(process.returncode==0 and isinstance(result,dict) and result.get("status")=="PASS","transport_client_refused")
            if payload is None:
                wg.require(result=={"status":"PASS","bytes":65536},"http_exact_64k")
            else:
                wg.require(set(result)=={"status","ack","echo"} and type(result["ack"]) is bool and type(result["echo"]) is bool,"udp_result_shape")
            return result
        finally:
            wg.stop(process); process.stdin.close(); process.stdout.close()


def keys_and_config(root,flavor,outer,inner,mtu):
    keys={name:wg.command(["/usr/bin/wg","genkey"],"key_generation").stdout.strip() for name in ("server","client","wrong","header")}
    keys["psk"]=wg.command(["/usr/bin/wg","genpsk"],"key_generation").stdout.strip()
    public={name:wg.command(["/usr/bin/wg","pubkey"],"key_generation",data=keys[name]).stdout.strip() for name in ("server","client")}
    wg.require(keys["client"]!=keys["wrong"],"negative_key_identity")
    fields={} if flavor=="wireguard" else {**awg.FIELDS,"HeaderProtectionKey":keys["header"].decode()}
    if flavor=="3.1": fields.update(RandomTrailers="true",DisableCookies="true")
    prefix="32" if inner==4 else "128"; endpoint="127.0.0.1:51888" if outer==4 else "[::1]:51888"
    for name,key in (("positive",keys["client"]),("negative",keys["wrong"])):
        conf=b"[Interface]\nPrivateKey = "+key+f"\nAddress = {address(inner,True)}/{prefix}\nMTU = {mtu}\n".encode()
        conf+="".join(f"{field} = {value}\n" for field,value in fields.items()).encode()
        conf+=b"[Peer]\nPublicKey = "+public["server"]+b"\nPresharedKey = "+keys["psk"]+f"\nAllowedIPs = {address(inner)}/{prefix}\nEndpoint = {endpoint}\nPersistentKeepalive = 1\n".encode()
        wg.private_write(root/f"{name}.conf",conf)
    payload=b"set=1\nprivate_key="+base64.b64decode(keys["server"]).hex().encode()+b"\nlisten_port=51889\nreplace_peers=true\n"
    expected={}
    for name,value in fields.items():
        key=awg.UAPI_NAMES.get(name,name.lower()).encode(); rendered=base64.b64decode(value).hex().encode() if name=="HeaderProtectionKey" else value.encode()
        payload+=key+b"="+rendered+b"\n"; expected[key]=b"1" if rendered==b"true" else rendered
    payload+=b"public_key="+base64.b64decode(public["client"]).hex().encode()+b"\npreshared_key="+base64.b64decode(keys["psk"]).hex().encode()+f"\nallowed_ip={address(inner,True)}/{prefix}\npersistent_keepalive_interval=0\n\n".encode()
    return public["client"],payload,expected


def peer_stats(root,pid,public):
    values=awg.peer_get(root,pid)
    wg.require(values.get(b"public_key")==base64.b64decode(public).hex().encode(),"peer_identity")
    return tuple(int(values[key]) for key in (b"last_handshake_time_sec",b"rx_bytes",b"tx_bytes"))


def start_peer(root,args,outer,inner,mtu):
    fd=os.open("/dev/net/tun",os.O_RDWR|os.O_CLOEXEC)
    process=None
    try:
        # IFF_TUN|IFF_NO_PI only: deliberately no VNET_HDR, offload/GSO/GRO.
        fcntl.ioctl(fd,0x400454ca,struct.pack("16sH",b"wg-p4",0x0001|0x1000))
        prefix="32" if inner==4 else "128"; family_flag="-4" if inner==4 else "-6"
        commands=[("address","add",f"{address(inner)}/{prefix}","dev","wg-p4",*(("nodad",) if inner==6 else ())), ("link","set","wg-p4","mtu",str(mtu)),("link","set","wg-p4","up")]
        for command in commands: wg.command(["/usr/bin/ip",*command],"namespace_interface_setup")
        wg.command(["/usr/bin/ip",family_flag,"route","add",f"{address(inner,True)}/{prefix}","dev","wg-p4"],"namespace_return_route")
        links=json.loads(wg.command(["/usr/bin/ip","-j","link","show","dev","wg-p4"],"namespace_mtu_readback").stdout)
        wg.require(len(links)==1 and links[0]["mtu"]==mtu,"namespace_mtu_readback")
        env={**ENV,"P4_TUN_FD":str(fd),"P4_PARENT_net_FD":args.parent_net_fd,"P4_PARENT_user_FD":args.parent_user_fd,"GOMAXPROCS":"2"}
        with wg.log_handle(root/"peer.log") as log:
            process=subprocess.Popen(wg.dropped([args.peer,str(root),str(outer)]),stdin=subprocess.DEVNULL,stdout=log,stderr=log,env=env,preexec_fn=wg.child_limit,pass_fds=(fd,int(args.parent_net_fd),int(args.parent_user_fd)))
        readiness(process,root/"stats.sock"); wg.check_process(process,wg.namespace("net"))
        return process,fd
    except BaseException:
        wg.stop(process); os.close(fd); raise


def readiness(process,path):
    deadline=time.monotonic()+5
    while not path.exists():
        wg.require(process.poll() is None and time.monotonic()<deadline,"child_readiness"); time.sleep(.05)


def phase(root,args,flavor,outer,inner,mtu,name,attempt,peer,service,public):
    sock=root/"mihomo.sock"; wg.require(not sock.exists(),"controller_collision")
    child=None; before=peer_stats(root,peer.pid,public)
    try:
        with wg.log_handle(root/f"core-{attempt}.log") as log:
            child=wg.launch([args.core,"-d",root,"-f",root/f"{name}.yaml"],log)
        readiness(child,sock); wg.check_process(child,wg.namespace("net")); wg.controller(sock,child.pid); sock.chmod(0o600)
        service_before=read_json_socket(root/"service.sock",service.pid)
        if name=="negative":
            got_http=got_udp=False
            try: got_http=transport_client(args,root,inner)["bytes"]>0
            except (OSError,ValueError,wg.Refused): pass
            try:
                reply=transport_client(args,root,inner,fixed_payload(64)); got_udp=reply["ack"] or reply["echo"]
            except (OSError,ValueError,wg.Refused): pass
            wg.require(not got_http and not got_udp and peer_stats(root,peer.pid,public)==before and read_json_socket(root/"service.sock",service.pid)==service_before,"wrong_key_no_direct")
            return {"wrong_key_no_direct":True}
        got=transport_client(args,root,inner)["bytes"]; after=peer_stats(root,peer.pid,public)
        wg.require(after[0]>0 and after[1]>before[1] and after[2]>before[2],"handshake_transfer")
        if attempt=="recovery": return {"recovery_exact_http_bytes":got}
        results=[]
        for size in (92 if inner==4 else 112,mtu-1,mtu,mtu+1):
            payload=fixed_payload(payload_size(inner,size)); prior=observe(root,peer.pid)
            raw=transport_client(args,root,inner,payload); replies={key:raw[key] for key in ("ack","echo")}
            seen=read_json_socket(root/"service.sock",service.pid)
            wg.require(sha(payload) in seen,"udp_request_unreceived_inconclusive")
            results.append(assess_datagram(prior,observe(root,peer.pid),seen[sha(payload)],payload,replies,inner,size,mtu))
        final=observe(root,peer.pid)
        for direction in ("client","peer"):
            rows=[key.split(":") for key in final["outer"] if key.startswith(direction+":")]
            wg.require(rows and all(row[1]==str(outer) and row[2]=="loopback" for row in rows),"actual_outer_family")
        return {"http_bytes":got,"udp":results,"outer_observed":final["outer"],"inner_observed":final["flows"]}
    finally:
        wg.stop(child)
        if sock.exists(): wg.require(stat.S_ISSOCK(sock.lstat().st_mode),"controller_cleanup"); sock.unlink()


def namespace_run(args):
    root=namespace_authority(args,owner=True)
    wg.private_write(root/"namespace-identity.json",json.dumps({"net":wg.namespace("net"),"user":wg.namespace("user")}).encode())
    links=json.loads(wg.command(["/usr/bin/ip","-j","link","show"],"namespace_inventory").stdout)
    wg.require([row["ifname"] for row in links]==["lo"],"fresh_namespace_links")
    for family in ("-4","-6"):
        wg.require(json.loads(wg.command(["/usr/bin/ip",family,"-j","route","show","table","all"],"namespace_inventory").stdout)==[],"fresh_namespace_routes")
    wg.command(["/usr/bin/ip","link","set","lo","up"],"namespace_loopback")
    loopback=json.loads(wg.command(["/usr/bin/ip","-j","link","show","dev","lo"],"namespace_inventory").stdout)
    wg.require(len(loopback)==1 and loopback[0]["mtu"]==65536,"loopback_mtu_not_internet")
    records=[]
    for index,(flavor,outer,inner,mtu) in enumerate(CELLS):
        cell=root/f"cell-{index:02}"; cell.mkdir(mode=0o700)
        peer=service=None; tun_fd=None
        result={"flavor":flavor,"outer":outer,"inner":inner,"mtu":mtu,"engine":ENGINE,"status":"REFUSE"}
        try:
            public,payload,expected=keys_and_config(cell,flavor,outer,inner,mtu)
            peer,tun_fd=start_peer(cell,args,outer,inner,mtu)
            def reset():
                wg.require(awg.socket_payload(cell/"peer.sock",peer.pid,payload)==b"errno=0\n\n" and peer_stats(cell,peer.pid,public)==(0,0,0),"peer_reset")
                values=awg.peer_get(cell,peer.pid); wg.require(all(values.get(key)==value for key,value in expected.items()),"awg_field_readback")
            reset()
            for name in ("positive","negative"):
                result=wg.command(wg.dropped([args.renderer,str(cell),name,flavor,str(outer),str(inner),str(mtu)]),"private_render")
                wg.require(json.loads(result.stdout)=={"private_roundtrip":True,"flavor":flavor},"private_roundtrip")
            service_argv=child_arguments(args,cell,"service",inner)
            with wg.log_handle(cell/"service.log") as log: service=wg.launch(service_argv,log,pass_fds=(int(args.parent_net_fd),int(args.parent_user_fd)))
            readiness(service,cell/"service.sock"); wg.check_process(service,wg.namespace("net"))
            result.update(phase(cell,args,flavor,outer,inner,mtu,"positive","positive",peer,service,public))
            reset(); result.update(phase(cell,args,flavor,outer,inner,mtu,"negative","negative",peer,service,public))
            reset(); result.update(phase(cell,args,flavor,outer,inner,mtu,"positive","recovery",peer,service,public))
            result["status"]="MEASURED"
        except wg.Refused as exc:
            result["stage"]=exc.stage
        except (OSError,ValueError,TypeError,KeyError,AttributeError,subprocess.SubprocessError):
            result["stage"]="unexpected_fixed_failure"
        finally:
            wg.stop(service); wg.stop(peer)
            if tun_fd is not None: os.close(tun_fd)
            remaining=json.loads(wg.command(["/usr/bin/ip","-j","link","show"],"namespace_interface_cleanup").stdout)
            wg.require([row["ifname"] for row in remaining]==["lo"],"namespace_interface_cleanup")
        records.append(result)
    measured=sum(row["status"]=="MEASURED" for row in records)
    return {"status":"MEASURED" if measured==24 else "PARTIAL-NONPASS","cells":records,"cell_count":len(records),"measured_cells":measured,"refused_cells":24-measured,"engine":ENGINE,"loopback_outer_mtu":65536,"internet_pmtu_acceptance":False,"normal_activation_acceptance":False,"net_inode":wg.namespace("net"),"user_inode":wg.namespace("user")}


def outer(args):
    wg.require(args.acknowledge_disposable_vm and os.getuid()!=0,"disposable_vm_ack")
    wg.require(re.fullmatch("[a-f0-9]{40}",args.source_sha or ""),"source_identity")
    for tool in ("unshare","ip","wg","setpriv","getcap"):
        wg.require(Path(f"/usr/bin/{tool}").is_file(),"missing_prerequisite")
    cache=Path.home()/".cache"/"p4-matrix-review"; cache.mkdir(mode=0o700,exist_ok=True)
    wg.require(wg.private_directory(cache),"scratch_parent")
    root=Path(tempfile.mkdtemp(prefix="matrix-",dir=cache)); child=None; descriptors=[]; identities={}
    source_files=[Path(__file__).resolve(),Path(wg.__file__).resolve(),Path(awg.__file__).resolve()]
    hashes={str(path):wg.digest(path) for path in source_files}
    result=None
    try:
        for name in ("core","renderer","peer"):
            expected=getattr(args,f"expected_{name}_sha256"); wg.copy_binary(Path(getattr(args,name)),root/name,expected); identities[name]=expected
        descriptors=[os.open(f"/proc/self/ns/{name}",os.O_RDONLY|os.O_CLOEXEC) for name in ("net","user")]
        argv=["/usr/bin/unshare","--user","--map-root-user","--net",sys.executable,str(Path(__file__).resolve()),"--namespace-child","--scratch",str(root),"--parent-pid",str(os.getpid()),"--parent-net",str(wg.namespace("net")),"--parent-user",str(wg.namespace("user")),"--parent-net-fd",str(descriptors[0]),"--parent-user-fd",str(descriptors[1]),"--core",str(root/"core"),"--renderer",str(root/"renderer"),"--peer",str(root/"peer")]
        with wg.log_handle(root/"result.json") as log:
            child=subprocess.Popen(argv,stdin=subprocess.DEVNULL,stdout=log,stderr=log,env=ENV,start_new_session=True,pass_fds=descriptors,preexec_fn=wg.child_limit)
        deadline=time.monotonic()+600
        # Never reap/poll leader before process-group cleanup; WNOWAIT preserves
        # its PID as an ownership anchor even when it is already a zombie.
        while os.waitid(os.P_PID,child.pid,os.WEXITED|os.WNOHANG|os.WNOWAIT) is None:
            wg.require(time.monotonic()<deadline,"matrix_deadline"); time.sleep(.1)
        data=(root/"result.json").read_bytes(); wg.require(len(data)<=2*1024*1024,"result_bound")
        result=json.loads(data)
    finally:
        if child is not None and child.returncode is None:
            try: os.killpg(child.pid,signal.SIGKILL)
            except ProcessLookupError: pass
            child.wait(timeout=5)
        for fd in descriptors: os.close(fd)
        wg.require(all(wg.digest(Path(path))==digest for path,digest in hashes.items()),"source_unchanged")
        identity=root/"namespace-identity.json"
        if identity.exists():
            net=json.loads(identity.read_bytes())["net"]
            for entry in Path("/proc").iterdir():
                if entry.name.isdecimal():
                    try: active=wg.namespace("net",entry.name)
                    except OSError: continue
                    wg.require(active!=net,"namespace_process_cleanup")
        wg.require(root.parent==cache and wg.private_directory(root),"scratch_cleanup_target")
        shutil.rmtree(root); wg.require(not root.exists(),"scratch_cleanup")
    wg.require(child.returncode==0 and isinstance(result,dict) and result.get("status") in ("MEASURED","PARTIAL-NONPASS") and result.get("cell_count")==24,"matrix_incomplete")
    result.update(source_sha=args.source_sha,helper_sha256=hashes,binary_sha256=identities,scratch_cleanup=True,external_canonical_guard_required=True)
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--acknowledge-disposable-vm",action="store_true")
    parser.add_argument("--namespace-child",action="store_true",help=argparse.SUPPRESS)
    parser.add_argument("--service-child",action="store_true",help=argparse.SUPPRESS)
    parser.add_argument("--client-child",action="store_true",help=argparse.SUPPRESS)
    for name in ("core","renderer","peer","source-sha","scratch","parent-pid","parent-net","parent-user","parent-net-fd","parent-user-fd","inner"):
        parser.add_argument(f"--{name}")
    for name in ("core","renderer","peer"): parser.add_argument(f"--expected-{name}-sha256")
    args=parser.parse_args()
    wg.require(sum((args.service_child,args.client_child,args.namespace_child))<=1,"mode_policy")
    if args.service_child: service_child(args)
    elif args.client_child: print(json.dumps(client_child(args),sort_keys=True))
    elif args.namespace_child: print(json.dumps(namespace_run(args),sort_keys=True))
    else: print(json.dumps(outer(args),sort_keys=True))


if __name__=="__main__":
    try: main()
    except wg.Refused as exc:
        print(json.dumps({"status":"REFUSE","stage":exc.stage})); raise SystemExit(2)
    except (OSError,ValueError,TypeError,KeyError,AttributeError,subprocess.SubprocessError):
        print(json.dumps({"status":"REFUSE","stage":"unexpected_fixed_failure"})); raise SystemExit(2)
