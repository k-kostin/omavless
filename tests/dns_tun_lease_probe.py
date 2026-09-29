#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Opt-in kernel-only FD lease hypothesis, never a host helper.

No routes, DNS, system bus, installed runtime or private data. Fresh user/net/PID
namespaces contain every device. This tests object admission/retention, not peer
authentication, resolved baseline ownership or resistance to administrators.
"""
import array
from contextlib import contextmanager
import errno
import fcntl
import json
import os
from pathlib import Path
import socket
import stat
import struct
import subprocess
import sys
import tempfile

import dns_core_namespace_probe as base
import dns_tun_namespace_probe as tun

TUNGETDEVNETNS = 0x54E3
TUNSETPERSIST = 0x400454CB
TUNSETQUEUE = 0x400454D9
IFF_TAP = 0x0002
IFF_MULTI_QUEUE = 0x0100
IFF_DETACH_QUEUE = 0x0400
IFF_PERSIST = 0x0800
FACTS = {'isolated', 'valid_fd_accepted', 'namespace_matches',
         'regular_refused', 'socket_refused', 'tap_refused',
         'multi_queue_refused', 'persistent_refused', 'wrong_name_refused',
         'detached_refused', 'foreign_namespace_refused',
         'creator_close_keeps_link', 'second_attach_refused',
         'holder_close_removes_link', 'all_fixtures_cleaned'}


class Rejected(Exception):
    """Expected admission refusal, never a raw kernel error."""


def identity(fd):
    info = os.fstat(fd)
    return info.st_dev, info.st_ino


def admit(fd, expected_namespace):
    """Borrow an FD; caller retains responsibility for closing it on all paths."""
    info = os.fstat(fd)
    if not stat.S_ISCHR(info.st_mode) or info.st_rdev != os.makedev(10, 200):
        raise Rejected()
    try:
        raw = fcntl.ioctl(fd, tun.TUNGETIFF, bytes(40))
    except OSError as error:
        if error.errno == errno.EBADFD:
            raise Rejected() from None
        raise  # Unsupported/permission errors cannot count as negative evidence.
    name, flags = struct.unpack_from('16sH', raw)
    if (name.split(b'\0', 1)[0] != tun.NAME or flags & 0xF != 1
            or not flags & 0x1000
            or flags & (IFF_MULTI_QUEUE | IFF_PERSIST | IFF_DETACH_QUEUE)):
        raise Rejected()
    namespace_fd = fcntl.ioctl(fd, TUNGETDEVNETNS)
    try:
        if identity(namespace_fd) != expected_namespace:
            raise Rejected()
    finally:
        os.close(namespace_fd)
    return True


def refused(fd, expected_namespace):
    try:
        admit(fd, expected_namespace)
    except Rejected:
        return True
    return False


@contextmanager
def device(name=tun.NAME, flags=tun.FLAGS, persistent=False, detach=False):
    fd = os.open('/dev/net/tun', os.O_RDWR | os.O_CLOEXEC)
    persisted = False
    try:
        fcntl.ioctl(fd, tun.TUNSETIFF, struct.pack('16sH22x', name, flags))
        if persistent:
            fcntl.ioctl(fd, TUNSETPERSIST, 1)
            persisted = True
        if detach:
            fcntl.ioctl(fd, TUNSETQUEUE, struct.pack('16sH22x', name, IFF_DETACH_QUEUE))
        yield fd
    finally:
        try:
            if persisted:
                fcntl.ioctl(fd, TUNSETPERSIST, 0)
        finally:
            os.close(fd)


def receive_fd(channel):
    received = []
    try:
        data, ancillary, flags, _ = channel.recvmsg(
            2, socket.CMSG_SPACE(4 * array.array('i').itemsize), socket.MSG_CMSG_CLOEXEC)
        unknown = False
        for level, kind, raw in ancillary:
            if level != socket.SOL_SOCKET or kind != socket.SCM_RIGHTS:
                unknown = True
                continue
            items = array.array('i')
            complete = len(raw) - len(raw) % items.itemsize
            items.frombytes(raw[:complete])
            received.extend(items)
            if complete != len(raw):
                unknown = True
        base.require(data == b'x' and not unknown and not flags & (socket.MSG_CTRUNC | socket.MSG_TRUNC)
                     and len(received) == 1)
        return received.pop()
    finally:
        for fd in received:
            os.close(fd)


def foreign_worker(original_net, original_user, original_pid, fd_text):
    base.guard(original_net, original_user, original_pid)
    base.require(fd_text.isascii() and fd_text.isdecimal() and 3 <= int(fd_text) <= 1024)
    with socket.socket(fileno=int(fd_text)) as channel, device() as fd:
        rights = array.array('i', [fd])
        base.require(channel.sendmsg([b'x'], [(socket.SOL_SOCKET, socket.SCM_RIGHTS, rights)]) == 1)


def foreign_case(original_net, original_user, original_pid, expected_namespace):
    recipient, sender = socket.socketpair(socket.AF_UNIX, socket.SOCK_DGRAM)
    with recipient, sender:
        recipient.settimeout(4)
        process = subprocess.Popen([
            '/usr/bin/unshare', '--net', '--', sys.executable, str(Path(__file__).resolve()),
            '--foreign-worker', original_net, original_user, original_pid, str(sender.fileno()),
        ], pass_fds=(sender.fileno(),), stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            env={'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C'})
        received = None
        try:
            received = receive_fd(recipient)
            base.require(process.wait(timeout=4) == 0)
            return refused(received, expected_namespace)
        finally:
            try:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=4)
            finally:
                if received is not None:
                    os.close(received)


def loopback_only():
    return {item['ifname'] for item in tun.links()} == {'lo'}


def retention(expected_namespace):
    holder = None
    try:
        with device() as creator:
            base.require(admit(creator, expected_namespace))
            holder = os.dup(creator)
            index = tun.tun_index()
        alive = admit(holder, expected_namespace) and tun.tun_index() == index
        try:
            with device():
                blocked = False
        except OSError as error:
            if error.errno != errno.EBUSY:
                raise
            blocked = True
    finally:
        if holder is not None:
            os.close(holder)
    return alive, blocked, loopback_only()


def owner(original_net, original_user, original_pid):
    base.guard(original_net, original_user, original_pid)
    os.umask(0o077)
    namespace_fd = os.open('/proc/self/ns/net', os.O_RDONLY | os.O_CLOEXEC)
    try:
        expected = identity(namespace_fd)
    finally:
        os.close(namespace_fd)
    facts = {'isolated': True}
    with device() as fd:
        facts['valid_fd_accepted'] = admit(fd, expected)
        facts['namespace_matches'] = facts['valid_fd_accepted']
    with tempfile.TemporaryFile() as regular:
        facts['regular_refused'] = refused(regular.fileno(), expected)
    with socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM) as channel:
        facts['socket_refused'] = refused(channel.fileno(), expected)
    for label, arguments in [
        ('tap_refused', {'flags': IFF_TAP | 0x1000}),
        ('multi_queue_refused', {'flags': tun.FLAGS | IFF_MULTI_QUEUE}),
        ('persistent_refused', {'persistent': True}),
        ('wrong_name_refused', {'name': b'ovdnsother0'}),
        ('detached_refused', {'flags': tun.FLAGS | IFF_MULTI_QUEUE, 'detach': True}),
    ]:
        base.require(loopback_only())
        with device(**arguments) as fd:
            facts[label] = refused(fd, expected)
    facts['foreign_namespace_refused'] = foreign_case(original_net, original_user, original_pid, expected)
    base.require(loopback_only())
    facts['creator_close_keeps_link'], facts['second_attach_refused'], facts['holder_close_removes_link'] = retention(expected)
    facts['all_fixtures_cleaned'] = loopback_only()
    return facts


def project(raw):
    base.require(len(raw) <= 2048)
    def unique(items):
        result = {}
        for key, value in items:
            base.require(key not in result)
            result[key] = value
        return result
    result = json.loads(raw, object_pairs_hook=unique)
    base.require(type(result) is dict and set(result) == FACTS)
    base.require(all(type(value) is bool for value in result.values()))
    return result


def main():
    try:
        if len(sys.argv) == 6 and sys.argv[1] == '--foreign-worker':
            foreign_worker(*sys.argv[2:])
            return 0
        if len(sys.argv) == 5 and sys.argv[1] == '--isolated-owner':
            facts = owner(*sys.argv[2:])
        elif len(sys.argv) == 1 and os.geteuid() != 0:
            result = subprocess.run([
                '/usr/bin/unshare', '--user', '--map-root-user', '--net', '--pid',
                '--fork', '--kill-child=SIGKILL', '--', sys.executable,
                str(Path(__file__).resolve()), '--isolated-owner',
                tun.namespace('net'), tun.namespace('user'), tun.namespace('pid'),
            ], stdin=subprocess.DEVNULL, capture_output=True, timeout=30, check=False)
            base.require(result.returncode in (0, 1))
            facts = project(result.stdout)
            base.require(result.returncode == 0 or not all(facts.values()))
        else:
            print('{"unprivileged_parent_required":true}')
            return 2
        print(json.dumps(facts, sort_keys=True))
        return 0 if all(facts.values()) else 1
    except Exception:
        print('{"isolated_tun_lease_probe_failed":true}')
        return 1


if __name__ == '__main__':
    sys.exit(main())
