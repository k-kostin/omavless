"""Fixed private two-stream witness, developer-only and without entry point.

Only fresh PID1/root in the retained inner namespace may create these fixed
loopback sockets. No caller address, connection identifier or token is IPC.
All first uncertainty seals and retains sockets/thread; no failure close/join.
"""
import math
import os
import select
import socket
import stat
import threading

PROXY=('127.0.0.1',19090)
ECHO=('127.0.0.1',19092)
CONNECT=b'CONNECT 127.0.0.1:19092 HTTP/1.1\r\nHost: 127.0.0.1:19092\r\n\r\n'
PAYLOAD=b'retained-positive-two-stream-byte-witness'
FIELDS=('st_dev','st_ino','st_mode','st_uid','st_gid','st_nlink')


class Refused(RuntimeError):
    def __init__(self):super().__init__('fixed_positive_stream_refused')


def require(value):
    if not value:raise Refused()


def address(value, port=None):
    require(type(value) is tuple and len(value)==2 and type(value[0]) is str and value[0]=='127.0.0.1'
            and type(value[1]) is int and 0 < value[1] <= 65535 and (port is None or value[1]==port))
    return value[1]


class Streams:
    def __init__(self, owner, ownership, images, image_module, control, controller_module):
        self.owner,self.ownership,self.images,self.control=owner,ownership,images,control
        self.sealed=True;self.failed=self.stopping=self.worker_done=self.witness_done=self.finished=False
        self.exact_ack=self.selected_eof_verified=False
        self.held=[];self.identities={};self.clients=[];self.peers=[];self.accepted=[];self.peer_eof=set();self.transferred={}
        self.worker_thread=None
        try:
            require(type(owner) is ownership.Session and owner.kind=='inner' and owner.isolated
                    and os.getpid()==1 and os.geteuid()==os.getegid()==0
                    and type(images) is image_module.Images and images.owner is owner
                    and type(control) is controller_module.Controller and control.owner is owner
                    and control.core is owner.anchors['core']['child']
                    and type(control.core) is ownership.OwnedProcess
                    and owner.anchors['core']['state']=='mapped' and not control.sealed)
            owner.retained.append(self);owner.available()
            self.core=control.core;self.deadline=owner.local_deadline(8);self.sealed=False
            self.listener=self.created()
            self.call(self.listener.bind,ECHO)
            self.call(self.listener.listen,2)
            address(self.call(self.listener.getsockname),ECHO[1])
            self.call(self.listener.setblocking,False)
            self.worker_thread=threading.Thread(target=self.worker,name='fixed-private-echo',daemon=False)
            self.call(self.worker_thread.start)
            require(self.call(self.worker_thread.is_alive) is True)
        except BaseException:
            self.refuse();raise Refused() from None

    def refuse(self):
        self.sealed=self.owner.sealed=True

    def available(self):
        try:
            require(not self.sealed and not self.failed and not self.finished)
            self.owner.within(self.deadline)
            now=self.ownership.clock()
            require(type(now) is float and math.isfinite(now)
                    and type(self.deadline) is float and math.isfinite(self.deadline) and now < self.deadline)
            return self.deadline-now
        except BaseException:
            self.refuse();raise Refused() from None

    def call(self, operation, *args, **kwargs):
        try:
            self.available();value=operation(*args,**kwargs);self.available();return value
        except BaseException:
            self.refuse();raise Refused() from None

    def retained(self, stream):
        self.held.append(stream)  # Before any post-return deadline/metadata gate.
        self.available()
        fd=self.call(stream.fileno)
        require(type(fd) is int and fd >= 0)
        value=self.call(os.fstat,fd)
        require(all(type(getattr(value,key)) is int for key in FIELDS)
                and stat.S_ISSOCK(value.st_mode) and value.st_uid==value.st_gid==0
                and value.st_dev>=0 and value.st_ino>0 and value.st_nlink==1)
        self.identities[id(stream)]=(fd,tuple(getattr(value,key) for key in FIELDS))
        return stream

    def created(self):
        try:
            self.available()
            return self.retained(socket.socket(socket.AF_INET,socket.SOCK_STREAM))
        except BaseException:
            self.refuse();raise Refused() from None

    def check(self, stream):
        try:
            self.available()
            require(any(held is stream for held in self.held) and self.control.core is self.core
                    and self.owner.anchors['core']['child'] is self.core)
            fd,original=self.identities[id(stream)]
            require(self.call(stream.fileno)==fd)
            value=self.call(os.fstat,fd)
            require(all(type(getattr(value,key)) is int for key in FIELDS)
                    and tuple(getattr(value,key) for key in FIELDS)==original)
            self.call(self.owner.live,self.core)
            self.call(self.images.executable,self.core,'core',self.deadline)
        except BaseException:
            self.refuse();raise Refused() from None

    def write(self, stream, raw):
        try:
            require(type(raw) is bytes and 0 < len(raw) <= 1024)
            self.call(stream.settimeout,self.available())
            self.check(stream)  # Original socket/core/session immediately before every write.
            sent=self.call(stream.send,raw)
            require(type(sent) is int and sent==len(raw))
        except BaseException:
            self.refuse();raise Refused() from None

    def receive(self, stream, maximum):
        try:
            require(type(maximum) is int and 0 < maximum <= 4096)
            self.call(stream.settimeout,self.available())
            value=self.call(stream.recv,maximum)
            require(type(value) is bytes and len(value) <= maximum)
            return value
        except BaseException:
            self.refuse();raise Refused() from None

    def tunnel(self):
        try:
            require(len(self.clients)<2)
            stream=self.created();self.clients.append(stream)
            self.call(stream.settimeout,self.available())
            self.call(stream.connect,PROXY)
            address(self.call(stream.getpeername),PROXY[1])
            port=address(self.call(stream.getsockname))
            require(all(port != address(self.call(other.getsockname)) for other in self.clients if other is not stream))
            self.write(stream,CONNECT)
            header=b''
            while not header.endswith(b'\r\n\r\n'):
                require(len(header)<4096)
                part=self.receive(stream,1);require(part);header+=part
            require(header.startswith(b'HTTP/1.1 200 ') and b'\x00' not in header)
            self.check(stream)
            return stream
        except BaseException:
            self.refuse();raise Refused() from None

    def echo(self, stream):
        try:
            require(any(held is stream for held in self.clients))
            self.write(stream,PAYLOAD);raw=b''
            while len(raw)<len(PAYLOAD):
                part=self.receive(stream,len(PAYLOAD)-len(raw));require(part);raw+=part
            require(raw==PAYLOAD);self.check(stream)
        except BaseException:
            self.refuse();raise Refused() from None

    def worker(self):
        # No context manager/finally/implicit peer close. All accepted sockets
        # remain strongly retained after EOF, reset, timeout or any exception.
        try:
            while True:
                self.available()
                if self.stopping:
                    self.worker_done=True;return
                watched=[self.listener]+[peer for peer in self.peers if id(peer) not in self.peer_eof]
                ready,writable,exceptional=self.call(select.select,watched,[],[],min(0.05,self.available()))
                require(type(ready) is list and writable==exceptional==[] and len(ready)<=3
                        and all(any(item is held for held in watched) for item in ready)
                        and len({id(item) for item in ready})==len(ready))
                for item in ready:
                    if item is self.listener:
                        require(len(self.peers)<2)
                        self.available()
                        accepted=self.listener.accept()
                        self.accepted.append(accepted)  # Hold before even tuple-shape/late gates.
                        # Retain even a malformed/late accepted tuple's socket.
                        require(type(accepted) is tuple and len(accepted)==2)
                        peer,remote=accepted
                        self.retained(peer);self.peers.append(peer);address(remote)
                        address(self.call(peer.getsockname),ECHO[1])
                        self.transferred[id(peer)]=0
                    else:
                        raw=self.receive(item,1024)
                        if not raw:self.peer_eof.add(id(item));continue
                        self.transferred[id(item)]+=len(raw)
                        require(self.transferred[id(item)]<=4*len(PAYLOAD))
                        self.write(item,raw)
        except BaseException:
            self.failed=True;self.refuse();return  # Retained resources; no failure close/join/output.

    def selected_eof(self, stream):
        try:
            require(self.exact_ack is True and self.selected_eof_verified is False
                    and len(self.clients)==2 and stream is self.clients[0])
            self.check(stream);self.call(stream.settimeout,self.available());self.available()
            try:raw=stream.recv(1)
            except ConnectionResetError:raw=b''  # Only reached AFTER verified exact-target 204.
            self.available();require(type(raw) is bytes and raw==b'')
            self.selected_eof_verified=True
        except BaseException:
            self.refuse();raise Refused() from None

    def witness(self):
        try:
            self.available();require(not self.witness_done and not self.clients)
            first,second=self.tunnel(),self.tunnel()
            self.echo(first);self.echo(second)
            ports=tuple(address(self.call(stream.getsockname)) for stream in (first,second))
            selected,other=self.call(self.control.discover_for_ports,ports)
            require(self.call(self.control.close_witness,selected,wrong_token=True)=='changed')
            self.echo(first);self.echo(second)
            require(self.call(self.control.close_witness,selected)=='closed')
            self.exact_ack=True
            self.selected_eof(first);self.echo(second)
            require(self.call(self.control.close_witness,selected)=='missing')
            survivor=self.call(self.control.discover_for_ports,(ports[1],))
            require(type(survivor) is list and len(survivor)==1
                    and self.call(self.control.same_private_target,other,survivor[0]) is True)
            self.available();self.witness_done=True
            return {'wrong_token_changed':True,'exact_target_closed':True,'selected_eof_after_ack':True,
                    'unselected_byte_stream_survived':True,'replay_missing':True,'survivor_identity_unchanged':True}
        except BaseException:
            self.refuse();raise Refused() from None

    def finish_positive(self):
        try:
            self.available();require(self.witness_done and len(self.clients)==2 and not self.finished)
            for stream in self.clients:
                self.check(stream);require(self.call(stream.close) is None)
            self.available();self.stopping=True
            require(self.call(self.worker_thread.join,timeout=min(1.0,self.available())) is None)
            require(self.call(self.worker_thread.is_alive) is False and self.worker_done is True and not self.failed
                    and len(self.peers)==2)
            for stream in [*self.peers,self.listener]:
                self.check(stream);require(self.call(stream.close) is None)
            self.available();self.finished=True
            return {'positive_sockets_closed':True,'owned_echo_thread_returned_without_error':True}
        except BaseException:
            self.refuse();raise Refused() from None
