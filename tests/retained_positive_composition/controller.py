"""Developer-only fixed Unix controller witness, never production authority.

Only the sealed launcher may supply its retained Session/core objects. Private
targets never become IPC, user actions, log fields or package admission. First
uncertainty seals both controller and owner before propagating; no resend/cleanup.
"""
import json
import math
import os
import re
import socket
import stat
import struct
import time

DIRECTORY = '/home/core'
SOCKET = '/home/core/controller.sock'
FLAGS = os.O_PATH | os.O_NOFOLLOW | os.O_CLOEXEC
MAX_ROWS = 128
MAX_SNAPSHOT = 256 * 1024
MAX_REPLY = 16 * 1024
ID = re.compile(r'[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\Z')
TOKEN = re.compile(r'[1-9][0-9]{0,19}\Z')


class Refused(RuntimeError):
    def __init__(self):
        super().__init__('fixed_developer_controller_refused')


def require(value):
    if not value:
        raise Refused()


def identity(value):
    return (value.st_dev, value.st_ino, value.st_mode, value.st_uid,
            value.st_gid, value.st_nlink)


def remaining(deadline):
    now = time.monotonic()
    require(type(now) is float and math.isfinite(now)
            and type(deadline) is float and math.isfinite(deadline) and now < deadline)
    return deadline - now


def pairs(rows):
    result = {}
    for key, value in rows:
        require(key not in result)
        result[key] = value
    return result


def decode(raw, maximum):
    require(type(raw) is bytes and 0 < len(raw) <= maximum)
    def finite_float(text):
        value = float(text)
        require(math.isfinite(value))
        return value
    return json.loads(raw, object_pairs_hook=pairs,
                      parse_float=finite_float,
                      parse_constant=lambda _: require(False))


def targets(raw):
    value = decode(raw, MAX_SNAPSHOT)
    require(type(value) is dict and 'connections' in value)
    rows = value['connections']
    if rows is None:
        rows = []
    require(type(rows) is list and len(rows) <= MAX_ROWS)
    ids, tokens, result = set(), set(), []
    for row in rows:
        require(type(row) is dict and type(row.get('id')) is str
                and ID.fullmatch(row['id']) and type(row.get('omavlessCloseToken')) is str)
        token = row['omavlessCloseToken']
        require(TOKEN.fullmatch(token) and int(token) < 2**64
                and row['id'] not in ids and token not in tokens)
        ids.add(row['id']); tokens.add(token)
        result.append((row['id'], token))
    return result


def source_port(value):
    # Private synthetic stream correlation, never conditional-close authority.
    require(type(value) is int or type(value) is str
            and re.fullmatch(r'[1-9][0-9]{0,4}', value))
    number = int(value)
    require(0 < number <= 65535)
    return number


def targets_by_port(raw):
    identities = targets(raw)  # Preserve the exact private ID/token grammar.
    value = decode(raw, MAX_SNAPSHOT)
    rows = value['connections'] or []
    result = {}
    for row, identity in zip(rows, identities):
        require(type(row.get('metadata')) is dict)
        port = source_port(row['metadata'].get('sourcePort'))
        require(port not in result)
        result[port] = identity
    return result


def parse_http(raw):
    require(type(raw) is bytes and 0 < len(raw) <= MAX_SNAPSHOT)
    end = raw.find(b'\r\n\r\n')
    require(0 <= end <= 8192)
    head = raw[:end].decode('ascii', 'strict').split('\r\n')
    match = re.fullmatch(r'HTTP/1\.[01] ([1-5][0-9]{2})(?: [\x20-\x7e]*)?', head[0])
    require(match is not None)
    headers = {}
    for line in head[1:]:
        key, separator, value = line.partition(':')
        key = key.lower()
        require(separator and re.fullmatch(r'[a-z0-9-]+', key) and key not in headers
                and key != 'transfer-encoding' and all(32 <= ord(c) <= 126 for c in value))
        headers[key] = value.strip(' ')
    body = raw[end + 4:]
    if 'content-length' in headers:
        length = headers['content-length']
        require(re.fullmatch(r'0|[1-9][0-9]{0,5}', length) and int(length) == len(body))
    return int(match[1]), body


class Target:
    __slots__ = ('_id', '_token', '_session', '_core')

    def __init__(self, identifier, token, session, core):
        self._id, self._token, self._session, self._core = identifier, token, session, core

    def __repr__(self):
        return 'DeveloperBoundTarget([private])'


class Controller:
    def __init__(self, owner, core, ownership):
        self.sealed = True
        self.owner, self.core = owner, core
        self._identity = object()
        self.held, self.streams = [], []
        try:
            start = time.monotonic()
            require(type(start) is float and math.isfinite(start))
            self.deadline = start + 40.0
            deadline = start + 3.0
            remaining(deadline)
            # ownership is the launcher's exact pinned loaded module, never
            # IPC input. Require its concrete retained classes, not duck anchors.
            require(type(owner) is ownership.Session and type(core) is ownership.OwnedProcess
                    and owner.kind == 'inner' and owner.roles.get(id(core)) == 'core'
                    and any(child is core for child in owner.children))
            self.io(deadline, owner.available)
            require(owner.anchors.get('core', {}).get('child') is core)
            self.io(deadline, owner.live, core)
            remaining(deadline)
            self.directory = os.open(DIRECTORY, FLAGS | os.O_DIRECTORY)
            self.held.append(self.directory)
            remaining(deadline)
            self.socket = os.open('controller.sock', FLAGS, dir_fd=self.directory)
            self.held.append(self.socket)
            remaining(deadline)
            self.directory_identity = identity(self.io(deadline, os.fstat, self.directory))
            self.socket_identity = identity(self.io(deadline, os.fstat, self.socket))
            self.sealed = False
            self.check(deadline=deadline)
            remaining(deadline)
        except BaseException:
            self.sealed = owner.sealed = True
            raise Refused() from None

    def io(self, deadline, operation, *args, **kwargs):
        remaining(deadline)
        value = operation(*args, **kwargs)
        remaining(deadline)
        return value

    def check(self, stream=None, *, deadline=None):
        try:
            self._check(stream, self.deadline if deadline is None else min(deadline, self.deadline))
        except BaseException:
            self.sealed = self.owner.sealed = True
            raise Refused() from None

    def _check(self, stream, deadline):
        require(not self.sealed)
        self.io(deadline, self.owner.available)
        self.io(deadline, self.owner.live, self.core)
        directory = self.io(deadline, os.fstat, self.directory)
        controller = self.io(deadline, os.fstat, self.socket)
        require(stat.S_ISDIR(directory.st_mode) and directory.st_uid == directory.st_gid == 1000
                and stat.S_IMODE(directory.st_mode) == 0o700
                and identity(directory) == self.directory_identity
                == identity(self.io(deadline, os.stat, DIRECTORY, follow_symlinks=False)))
        require(stat.S_ISSOCK(controller.st_mode) and controller.st_uid == controller.st_gid == 1000
                and stat.S_IMODE(controller.st_mode) == 0o600 and controller.st_nlink == 1
                and identity(controller) == self.socket_identity
                == identity(self.io(deadline, os.stat, 'controller.sock', dir_fd=self.directory, follow_symlinks=False))
                == identity(self.io(deadline, os.stat, SOCKET, follow_symlinks=False)))
        require(self.owner.anchors.get('core', {}).get('child') is self.core)
        if stream is not None:
            raw = self.io(deadline, stream.getsockopt, socket.SOL_SOCKET,
                          socket.SO_PEERCRED, struct.calcsize('3i'))
            pid, uid, gid = struct.unpack('3i', raw)
            require(pid == self.core.pid and uid == gid == 1000)
        self.io(deadline, self.owner.live, self.core)

    def exchange(self, kind, selected=None, wrong_token=False):
        """Fixed semantic reads or developer-selected conditional-close witness."""
        try:
            require(kind in ('capabilities', 'snapshot', 'conditional_close')
                    and type(wrong_token) is bool)
            start = time.monotonic()
            require(type(start) is float and math.isfinite(start))
            deadline = min(start + 3.0, self.deadline)
            remaining(deadline)
            self.check(deadline=deadline)
            if kind == 'conditional_close':
                require(type(selected) is Target and selected._session is self._identity
                        and selected._core is self.core and ID.fullmatch(selected._id)
                        and TOKEN.fullmatch(selected._token) and int(selected._token) < 2**64)
                token = str(1 if int(selected._token) == 2**64 - 1 else int(selected._token) + 1) if wrong_token else selected._token
                request = (f'POST /connections/{selected._id}/close-conditional HTTP/1.0\r\n'
                           f'Host: localhost\r\nIf-Match: "{token}"\r\nContent-Length: 0\r\n'
                           'Connection: close\r\n\r\n').encode('ascii')
            else:
                require(selected is None and wrong_token is False)
                path = '/connections/conditional-capabilities' if kind == 'capabilities' else '/connections'
                request = f'GET {path} HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n'.encode('ascii')
            remaining(deadline)
            stream = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.streams.append(stream)  # Retain on any unknown; no failure close.
            remaining(deadline)
            stream.settimeout(remaining(deadline))
            self.check(deadline=deadline)
            remaining(deadline)
            stream.connect(SOCKET)
            remaining(deadline)
            self.check(stream, deadline=deadline)
            stream.settimeout(remaining(deadline))
            self.check(stream, deadline=deadline)  # Exact peer/core/session before EVERY write.
            remaining(deadline)
            # One write attempt; short sends are unknown, never continuation/resend.
            sent = stream.send(request)
            require(type(sent) is int and sent == len(request))
            remaining(deadline)
            cap = MAX_REPLY if kind == 'conditional_close' else MAX_SNAPSHOT
            raw = b''
            while True:
                stream.settimeout(remaining(deadline))
                remaining(deadline)
                chunk = stream.recv(min(8192, cap + 1 - len(raw)))
                remaining(deadline)
                require(type(chunk) is bytes and len(raw) + len(chunk) <= cap)
                if not chunk:
                    break
                raw += chunk
            self.check(stream, deadline=deadline)
            remaining(deadline)
            result = parse_http(raw)
            remaining(deadline)
            stream.close()  # Only successful complete bounded read closes own socket.
            remaining(deadline)
            return result
        except BaseException:
            self.sealed = self.owner.sealed = True
            raise Refused() from None

    def ready(self):
        try:
            code, body = self.exchange('capabilities')
            value = decode(body, 1024)
            require(code == 200 and type(value) is dict and set(value) == {'abi', 'ready'}
                    and type(value['abi']) is int and value['abi'] == 1 and value['ready'] is True)
        except BaseException:
            self.sealed = self.owner.sealed = True
            raise Refused() from None

    def discover(self):
        try:
            self.ready()
            code, body = self.exchange('snapshot')
            require(code == 200)
            return [Target(identifier, token, self._identity, self.core) for identifier, token in targets(body)]
        except BaseException:
            self.sealed = self.owner.sealed = True
            raise Refused() from None

    def discover_for_ports(self, ports):
        """Select one bounded snapshot's private targets for our actual streams."""
        try:
            require(type(ports) is tuple and len(ports) in (1, 2)
                    and all(type(port) is int and 0 < port <= 65535 for port in ports)
                    and len(set(ports)) == len(ports))
            self.ready()
            code, body = self.exchange('snapshot')
            require(code == 200)
            selected = targets_by_port(body)
            require(set(selected) == set(ports))
            return [Target(*selected[port], self._identity, self.core) for port in ports]
        except BaseException:
            self.sealed = self.owner.sealed = True
            raise Refused() from None

    def same_private_target(self, selected, observed):
        try:
            require(not self.sealed)
            self.owner.available()
            require(all(type(target) is Target and target._session is self._identity
                        and target._core is self.core and type(target._id) is str and ID.fullmatch(target._id)
                        and type(target._token) is str and TOKEN.fullmatch(target._token)
                        and int(target._token) < 2**64 for target in (selected, observed)))
            return (selected._id, selected._token) == (observed._id, observed._token)
        except BaseException:
            self.sealed = self.owner.sealed = True
            raise Refused() from None

    def close_witness(self, selected, *, wrong_token=False):
        try:
            require(type(selected) is Target and selected._session is self._identity and selected._core is self.core)
            self.ready()
            code, body = self.exchange('conditional_close', selected, wrong_token)
            require(not body and code in (204, 404, 409))
            return {204:'closed', 404:'missing', 409:'changed'}[code]
        except BaseException:
            self.sealed = self.owner.sealed = True
            raise Refused() from None
