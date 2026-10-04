#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Opt-in Linux authority experiment, NOT a privileged helper or host installer.

An ordinary parent creates fresh user/net/PID namespaces. Namespace-only owner
holds a nonpersistent TUN FD; a child loses all capabilities before using its
inherited FD. No system bus, runtime, private store, routes, DNS or sudo.
"""
import errno
import array
import fcntl
import json
import os
import platform
import socket
from pathlib import Path
import subprocess
import sys
import tempfile

import dns_core_namespace_probe as base
import dns_tun_namespace_probe as tun
import dns_tun_fd_policy as policy

FACTS = {
    'isolated', 'client_capabilities_empty', 'client_no_new_privileges',
    'client_has_attached_fd', 'client_cannot_create_tun',
    'client_cannot_change_owner', 'client_cannot_make_persistent',
    'client_cannot_delete_link', 'client_cannot_rename_link',
    'client_cannot_change_link_state', 'owner_fd_still_attached',
    'client_cannot_export_fd',
    'client_exit_keeps_owner_link', 'owner_close_removes_link',
}
WORKER_FACTS = {key for key in FACTS if key.startswith('client_')}
WORKER_FACTS.remove('client_exit_keeps_owner_link')
TUNSETOWNER = 0x400454CC
TUNSETPERSIST = 0x400454CB


def dropped_capabilities():
    with open('/proc/self/status', 'rb') as status:
        data = status.read(16385)
    base.require(len(data) <= 16384)
    fields = dict(line.split(':', 1) for line in data.decode('ascii').splitlines() if ':' in line)
    base.require(all(int(fields[name].strip(), 16) == 0
                     for name in ('CapInh', 'CapPrm', 'CapEff', 'CapBnd', 'CapAmb')))
    base.require(fields['NoNewPrivs'].strip() == '1')


def denied(operation):
    try:
        operation()
    except OSError as error:
        if error.errno == errno.EPERM:
            return True
        raise RuntimeError('unexpected_ioctl_outcome') from None
    return False


def link_denied(*arguments):
    # Only fixed synthetic test targets; stderr is never forwarded.
    result = subprocess.run(['/usr/bin/ip', 'link', *arguments],
                            stdin=subprocess.DEVNULL, capture_output=True,
                            timeout=3, check=False, env={'LANG': 'C', 'LC_ALL': 'C'})
    # iproute2 uses 1 for a refused sendmsg and 2 for a kernel netlink error.
    return result.returncode in (1, 2) and len(result.stderr) <= 1024 and b'Operation not permitted' in result.stderr


def cannot_export_fd(fd):
    # This is a real local ancillary transfer, not an ordinary byte send. A
    # caller able to send it to an unfiltered peer escapes an ioctl-only filter.
    sender, recipient = socket.socketpair(socket.AF_UNIX, socket.SOCK_DGRAM)
    with sender, recipient:
        rights = array.array('i', [fd])
        if denied(lambda: sender.sendmsg([b'x'], [(socket.SOL_SOCKET, socket.SCM_RIGHTS, rights)], socket.MSG_DONTWAIT)):
            return True
        data, ancillary, flags, _ = recipient.recvmsg(1, socket.CMSG_SPACE(rights.itemsize), socket.MSG_CMSG_CLOEXEC | socket.MSG_DONTWAIT)
        received = []
        try:
            for level, kind, raw in ancillary:
                base.require(level == socket.SOL_SOCKET and kind == socket.SCM_RIGHTS)
                items = array.array('i')
                items.frombytes(raw)
                received.extend(items)
            base.require(data == b'x' and not (flags & socket.MSG_CTRUNC) and len(received) == 1)
            base.require(tun.fd_attached(received[0]))
        finally:
            for item in received:
                os.close(item)
        return False


def worker(original_net, original_user, original_pid, fd_text):
    base.require(all(tun.namespace(kind) != original for kind, original in
                     [('net', original_net), ('user', original_user), ('pid', original_pid)]))
    # This must precede every ioctl and network mutation attempt.
    dropped_capabilities()
    base.require(fd_text.isascii() and fd_text.isdecimal() and 3 <= int(fd_text) <= 1024)
    fd = int(fd_text)
    base.require({item['ifname'] for item in tun.links()} == {'lo', tun.NAME.decode()})
    attached = tun.fd_attached(fd)
    # tun.open_tun() closes its attempted descriptor on failure.
    def create():
        other = os.open('/dev/net/tun', os.O_RDWR | os.O_CLOEXEC)
        try:
            import struct
            fcntl.ioctl(other, tun.TUNSETIFF, struct.pack('16sH22x', b'ovdnsother0', tun.FLAGS))
        finally:
            os.close(other)
    result = {
        'client_capabilities_empty': True, 'client_no_new_privileges': True,
        'client_has_attached_fd': attached,
        'client_cannot_create_tun': denied(create),
        'client_cannot_change_owner': denied(lambda: fcntl.ioctl(fd, TUNSETOWNER, os.getuid())),
        'client_cannot_make_persistent': denied(lambda: fcntl.ioctl(fd, TUNSETPERSIST, 1)),
        'client_cannot_export_fd': cannot_export_fd(fd),
        'client_cannot_delete_link': link_denied('delete', 'dev', tun.NAME.decode()),
        'client_cannot_rename_link': link_denied('set', 'dev', tun.NAME.decode(), 'name', 'ovdnsother0'),
        'client_cannot_change_link_state': link_denied('set', 'dev', tun.NAME.decode(), 'up'),
    }
    os.close(fd)
    return result


def project(raw, expected):
    base.require(len(raw) <= 2048)
    def unique(items):
        result = {}
        for key, value in items:
            base.require(key not in result)
            result[key] = value
        return result
    result = json.loads(raw, object_pairs_hook=unique)
    base.require(type(result) is dict and set(result) == expected)
    base.require(all(type(value) is bool for value in result.values()))
    return result


def one_case(original_net, original_user, original_pid, filter_path=None):
    base.guard(original_net, original_user, original_pid)
    fd = tun.open_tun()
    try:
        original_index = tun.tun_index()
        command = [
            '/usr/bin/setpriv', '--bounding-set=-all', '--inh-caps=-all',
            '--ambient-caps=-all', '--no-new-privs']
        if filter_path is not None:
            command += ['--seccomp-filter', str(filter_path)]
        command += ['--', sys.executable,
            str(Path(__file__).resolve()), '--worker',
            original_net, original_user, original_pid, str(fd)]
        result = subprocess.run(command, pass_fds=(fd,), stdin=subprocess.DEVNULL,
            capture_output=True, timeout=15, check=False,
            env={'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C'})
        base.require(result.returncode in (0, 1))
        facts = project(result.stdout, WORKER_FACTS)
        facts['isolated'] = True
        facts['owner_fd_still_attached'] = tun.fd_attached(fd)
        facts['client_exit_keeps_owner_link'] = tun.tun_index() == original_index
    finally:
        # The unfiltered negative control can make its inherited TUN persistent.
        # Only the owner's exact held FD is touched, never a name/index lookup.
        try:
            fcntl.ioctl(fd, TUNSETPERSIST, 0)
        finally:
            os.close(fd)
    facts['owner_close_removes_link'] = {item['ifname'] for item in tun.links()} == {'lo'}
    return facts


RESULT_FACTS = {'isolated', 'unfiltered_fd_can_change_owner',
                'unfiltered_fd_can_make_persistent', 'filtered_client_restricted',
                'unfiltered_fd_can_be_exported', 'both_cases_cleaned'}


def owner(original_net, original_user, original_pid):
    base.guard(original_net, original_user, original_pid)
    # Reject unsupported architectures before creating devices or temporary files.
    program = policy.encode(platform.machine())
    os.umask(0o077)
    with tempfile.TemporaryDirectory(prefix='omavless-tun-authority.') as directory:
        path = Path(directory) / 'ioctl-filter.bpf'
        path.write_bytes(program)
        unfiltered = one_case(original_net, original_user, original_pid)
        base.require(all(value for key, value in unfiltered.items()
                         if key not in ('client_cannot_change_owner', 'client_cannot_make_persistent', 'client_cannot_export_fd')))
        filtered = one_case(original_net, original_user, original_pid, path)
    return {
        'isolated': unfiltered['isolated'] and filtered['isolated'],
        'unfiltered_fd_can_change_owner': not unfiltered['client_cannot_change_owner'],
        'unfiltered_fd_can_make_persistent': not unfiltered['client_cannot_make_persistent'],
        'unfiltered_fd_can_be_exported': not unfiltered['client_cannot_export_fd'],
        'filtered_client_restricted': all(filtered.values()),
        'both_cases_cleaned': unfiltered['owner_close_removes_link'] and filtered['owner_close_removes_link'],
    }


def main():
    try:
        if len(sys.argv) == 6 and sys.argv[1] == '--worker':
            facts = worker(*sys.argv[2:])
        elif len(sys.argv) == 5 and sys.argv[1] == '--isolated-owner':
            facts = owner(*sys.argv[2:])
        elif len(sys.argv) == 1 and os.geteuid() != 0:
            result = subprocess.run([
                '/usr/bin/unshare', '--user', '--map-root-user', '--net', '--pid',
                '--fork', '--kill-child=SIGKILL', '--', sys.executable,
                str(Path(__file__).resolve()), '--isolated-owner',
                tun.namespace('net'), tun.namespace('user'), tun.namespace('pid'),
            ], stdin=subprocess.DEVNULL, capture_output=True, timeout=25, check=False)
            base.require(result.returncode in (0, 1))
            facts = project(result.stdout, RESULT_FACTS)
        else:
            print('{"unprivileged_parent_required":true}')
            return 2
        print(json.dumps(facts, sort_keys=True))
        return 0 if all(facts.values()) else 1
    except Exception:
        print('{"isolated_tun_authority_probe_failed":true}')
        return 1


if __name__ == '__main__':
    sys.exit(main())
