#!/usr/bin/env python3
"""TEST ONLY: bounded inline VLESS/TLS peer; never an outbound proxy.

Import is inert. Running requires a separately reviewed isolated VM setup.
No production runtime imports this module. Private inputs never enter argv/logs.
"""

import base64
from contextlib import ExitStack
import hmac
import os
import socket
import ssl
import stat
import struct
import sys
import time
from pathlib import Path

ROOT = Path('/run/omavless-k1-peer')
BIND = ('10.77.0.2', 24443)
CLIENT = '10.77.0.1'
HTTP_TARGET = (b'\xc0\x00\x02\x50', 80)  # 192.0.2.80
DOH_TARGET = (b'\x01\x01\x01\x01', 443)
MODES = ('success', 'doh-malformed', 'doh-close', 'redirect-https', 'redirect-http')
QUESTION = b'\x05probe\x02k1\x07invalid\x00'
BODY = b'K1 synthetic HTTP\n'


class Refused(Exception):
    """Finite protocol refusal; do not print input or exception detail."""


def exact(stream, count):
    data = bytearray()
    while len(data) < count:
        piece = stream.read(count - len(data))
        if not piece:
            raise Refused()
        data.extend(piece)
    return bytes(data)


def vless(stream, identity):
    if exact(stream, 1) != b'\x00':
        raise Refused()
    if not hmac.compare_digest(exact(stream, 16), identity):
        raise Refused()
    if exact(stream, 2) != b'\x00\x01':  # zero addons, TCP only
        raise Refused()
    port = struct.unpack('!H', exact(stream, 2))[0]
    if exact(stream, 1) != b'\x01':  # IPv4 only, no hostname resolution
        raise Refused()
    destination = (exact(stream, 4), port)
    if destination not in (HTTP_TARGET, DOH_TARGET):
        raise Refused()
    return destination


def request(stream):
    # No buffered overread into a second request or TLS frame.
    header = bytearray()
    while not header.endswith(b'\r\n\r\n'):
        if len(header) == 4096:
            raise Refused()
        header.extend(exact(stream, 1))
    lines = bytes(header[:-4]).split(b'\r\n')
    first = lines.pop(0).split(b' ')
    if len(first) != 3 or first[0] != b'GET' or first[2] != b'HTTP/1.1':
        raise Refused()
    headers = {}
    for line in lines:
        if b':' not in line:
            raise Refused()
        name, value = line.split(b':', 1)
        name = name.lower()
        if not name or any(c not in b'abcdefghijklmnopqrstuvwxyz-' for c in name):
            raise Refused()
        if name in headers or any(c < 32 or c > 126 for c in value):
            raise Refused()
        headers[name] = value.strip()
    if b'transfer-encoding' in headers or headers.get(b'content-length', b'0') != b'0':
        raise Refused()
    return first[1], headers


def dns_answer(encoded):
    if not encoded or len(encoded) > 512 or any(c not in b'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_' for c in encoded):
        raise Refused()
    try:
        query = base64.b64decode(encoded + b'=' * (-len(encoded) % 4), altchars=b'-_', validate=True)
    except ValueError:
        raise Refused() from None
    if base64.urlsafe_b64encode(query).rstrip(b'=') != encoded:
        raise Refused()
    # One exact A/IN question. No compression, EDNS, trailing data or responses.
    if len(query) != 12 + len(QUESTION) + 4:
        raise Refused()
    ident, flags, qd, an, ns, ar = struct.unpack('!6H', query[:12])
    if flags != 0x0100 or (qd, an, ns, ar) != (1, 0, 0, 0):
        raise Refused()
    if query[12:] != QUESTION + b'\x00\x01\x00\x01':
        raise Refused()
    return (struct.pack('!6H', ident, 0x8180, 1, 1, 0, 0) + query[12:]
            + b'\xc0\x0c\x00\x01\x00\x01\x00\x00\x00\x00\x00\x04'
            + HTTP_TARGET[0])


def response(body, content_type=b'text/plain', status=b'200 OK', location=None):
    result = (b'HTTP/1.1 ' + status + b'\r\nConnection: close\r\nContent-Type: '
              + content_type + b'\r\nContent-Length: ' + str(len(body)).encode() + b'\r\n')
    if location is not None:
        result += b'Location: ' + location + b'\r\n'
    return result + b'\r\n' + body


def answer_http(stream):
    target, headers = request(stream)
    if target != b'/k1' or headers.get(b'host') not in (b'192.0.2.80', b'192.0.2.80:80', b'probe.k1.invalid', b'probe.k1.invalid:80'):
        raise Refused()
    stream.write(response(BODY))


def answer_doh(stream, mode):
    target, headers = request(stream)
    if headers.get(b'host') not in (b'1.1.1.1', b'1.1.1.1:443') or not target.startswith(b'/dns-query?dns='):
        raise Refused()
    answer = dns_answer(target[len(b'/dns-query?dns='):])
    if mode == 'doh-close':
        return
    if mode == 'doh-malformed':
        answer = b'\x00'
    if mode in ('redirect-https', 'redirect-http'):
        location = (b'https://1.1.1.1/refused' if mode == 'redirect-https'
                    else b'http://1.1.1.1/refused')
        stream.write(response(b'', status=b'302 Found', location=location))
    else:
        stream.write(response(answer, b'application/dns-message'))


class Wire:
    """One socket, fixed total lifetime; no outbound connection capability."""
    def __init__(self, sock, deadline):
        self.sock, self.deadline, self.received, self.sent = sock, deadline, 0, 0

    def limit(self):
        remaining = self.deadline - time.monotonic()
        if remaining <= 0:
            raise Refused()
        self.sock.settimeout(min(3.0, remaining))

    def read(self, count):
        self.limit()
        data = self.sock.recv(min(count, 16384))
        self.received += len(data)
        if self.received > 65536:
            raise Refused()
        return data

    def write(self, data):
        self.limit()
        self.sent += len(data)
        if self.sent > 65536:
            raise Refused()
        self.sock.sendall(data)


class InnerTLS:
    """TLS inside VLESS via bounded MemoryBIO, not a forwarding socket."""
    def __init__(self, wire, context):
        self.wire, self.incoming, self.outgoing = wire, ssl.MemoryBIO(), ssl.MemoryBIO()
        self.deadline = wire.deadline
        self.tls = context.wrap_bio(self.incoming, self.outgoing, server_side=True)
        self.perform(self.tls.do_handshake)

    def flush(self):
        while self.outgoing.pending:
            self.wire.write(self.outgoing.read(16384))

    def perform(self, operation):
        while True:
            if time.monotonic() >= self.deadline:
                raise Refused()
            try:
                result = operation()
                self.flush()
                return result
            except ssl.SSLWantReadError:
                self.flush()
                self.incoming.write(exact(self.wire, 1))
            except ssl.SSLWantWriteError:
                self.flush()

    def read(self, count):
        return self.perform(lambda: self.tls.read(count))

    def write(self, data):
        while data:
            count = self.perform(lambda: self.tls.write(data))
            if count <= 0:
                raise Refused()
            data = data[count:]


def private_input(stack, directory, name, limit):
    # Fixed basenames only. OpenSSL subsequently reads the original held FD.
    fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK, dir_fd=directory)
    source = stack.enter_context(os.fdopen(fd, 'rb'))
    before = os.fstat(source.fileno())
    if (not stat.S_ISREG(before.st_mode) or before.st_uid != os.getuid()
            or stat.S_IMODE(before.st_mode) != 0o400 or before.st_nlink != 1
            or before.st_size > limit):
        raise Refused()
    value = source.read(limit + 1)
    after = os.fstat(source.fileno())
    named = os.stat(name, dir_fd=directory, follow_symlinks=False)
    keys = ('st_dev', 'st_ino', 'st_mode', 'st_uid', 'st_gid', 'st_nlink', 'st_size', 'st_mtime_ns', 'st_ctime_ns')
    if len(value) != before.st_size or any(getattr(before, key) != getattr(after, key) or getattr(before, key) != getattr(named, key) for key in keys):
        raise Refused()
    return source, value


def serve(mode):
    if mode not in MODES or os.geteuid() == 0 or os.getuid() != os.geteuid():
        raise Refused()
    with ExitStack() as stack:
        directory = os.open(ROOT, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        stack.callback(os.close, directory)
        info = os.fstat(directory)
        if info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
            raise Refused()
        _, identity = private_input(stack, directory, 'identity.bin', 16)
        if len(identity) != 16:
            raise Refused()
        cert, _ = private_input(stack, directory, 'peer.pem', 16384)
        key, _ = private_input(stack, directory, 'peer.key', 16384)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.minimum_version = ssl.TLSVersion.TLSv1_2
        context.set_alpn_protocols(['http/1.1'])
        context.load_cert_chain(f'/proc/self/fd/{cert.fileno()}', f'/proc/self/fd/{key.fileno()}')
    deadline = time.monotonic() + 60
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.bind(BIND)
        listener.listen(4)
        print('K1_PEER_READY', flush=True)
        for _ in range(16):
            listener.settimeout(max(0.001, deadline - time.monotonic()))
            raw, address = listener.accept()
            with raw:
                try:
                    if address[0] != CLIENT or time.monotonic() >= deadline:
                        raise Refused()
                    wire = InnerTLS(Wire(raw, min(deadline, time.monotonic() + 8)), context)
                    destination = vless(wire, identity)
                    wire.write(b'\x00\x00')
                    if destination == HTTP_TARGET:
                        answer_http(wire)
                        print('K1_PEER_HTTP_SENT', flush=True)
                    else:
                        answer_doh(InnerTLS(wire, context), mode)
                        print('K1_PEER_DOH_CASE_HANDLED', flush=True)
                except (Refused, OSError, ssl.SSLError):
                    print('K1_PEER_CONNECTION_REFUSED', flush=True)
        print('K1_PEER_BUDGET_END', flush=True)


def main():
    try:
        if len(sys.argv) != 2:
            raise Refused()
        serve(sys.argv[1])
        return 0
    except (Refused, OSError, ssl.SSLError, ValueError):
        print('K1_PEER_STOPPED', flush=True)
        return 1


if __name__ == '__main__':
    sys.exit(main())
