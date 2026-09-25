#!/usr/bin/env python3
"""Explicit synthetic Rust FD-admission conformance. No host network effects."""
import array
import json
import os
from pathlib import Path
import socket
import stat
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'tests'))
import dns_core_namespace_probe as base
import dns_tun_namespace_probe as tun
import dns_tun_lease_probe as lease

FACTS = {'isolated', 'valid_accepted', 'regular_refused', 'socket_refused',
         'tap_refused', 'multi_queue_refused', 'persistent_refused',
         'wrong_name_refused', 'detached_refused', 'foreign_namespace_refused',
         'fixtures_cleaned'}


def check(executable, originals, fd, expected):
    result = subprocess.run([executable, *originals], stdin=fd, capture_output=True,
        timeout=5, check=False, env={'LANG': 'C', 'LC_ALL': 'C', 'PATH': '/usr/bin:/bin'})
    base.require(result.returncode == 0 and len(result.stdout) <= 64)
    # Executable output is fixed JSON, not raw diagnostic text.
    base.require(result.stdout.strip() == (b'{"accepted":true}' if expected else b'{"accepted":false}'))
    return True


def foreign(originals, fd_text):
    base.guard(*originals)
    base.require(fd_text.isascii() and fd_text.isdecimal() and 3 <= int(fd_text) <= 1024)
    with socket.socket(fileno=int(fd_text)) as channel, lease.device(name=b'Meta') as fd:
        base.require(channel.sendmsg([b'x'], [(socket.SOL_SOCKET, socket.SCM_RIGHTS,
                                              array.array('i', [fd]))]) == 1)


def foreign_check(executable, originals):
    recipient, sender = socket.socketpair(socket.AF_UNIX, socket.SOCK_DGRAM)
    with recipient, sender:
        recipient.settimeout(4)
        process = subprocess.Popen(['/usr/bin/unshare', '--net', '--', sys.executable,
            str(Path(__file__).resolve()), '--foreign', *originals, str(sender.fileno())],
            pass_fds=(sender.fileno(),), stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            env={'LANG': 'C', 'LC_ALL': 'C', 'PATH': '/usr/bin:/bin'})
        fd = None
        try:
            fd = lease.receive_fd(recipient)
            base.require(process.wait(timeout=4) == 0)
            return check(executable, originals, fd, False)
        finally:
            try:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=4)
            finally:
                if fd is not None:
                    os.close(fd)


def owner(originals, executable):
    base.guard(*originals)
    os.umask(0o077)
    facts = {'isolated': True}
    with lease.device(name=b'Meta') as fd:
        facts['valid_accepted'] = check(executable, originals, fd, True)
    with tempfile.TemporaryFile() as regular:
        facts['regular_refused'] = check(executable, originals, regular.fileno(), False)
    with socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM) as channel:
        facts['socket_refused'] = check(executable, originals, channel.fileno(), False)
    for label, arguments in [
        ('tap_refused', {'flags': lease.IFF_TAP | 0x1000}),
        ('multi_queue_refused', {'flags': tun.FLAGS | lease.IFF_MULTI_QUEUE}),
        ('persistent_refused', {'persistent': True}),
        ('wrong_name_refused', {'name': b'other'}),
        ('detached_refused', {'flags': tun.FLAGS | lease.IFF_MULTI_QUEUE, 'detach': True}),
    ]:
        base.require(lease.loopback_only())
        with lease.device(**{'name': b'Meta', **arguments}) as fd:
            facts[label] = check(executable, originals, fd, False)
    facts['foreign_namespace_refused'] = foreign_check(executable, originals)
    facts['fixtures_cleaned'] = lease.loopback_only()
    return facts


def main():
    try:
        if len(sys.argv) == 6 and sys.argv[1] == '--foreign':
            foreign(sys.argv[2:5], sys.argv[5])
            return 0
        if len(sys.argv) == 6 and sys.argv[1] == '--owner':
            facts = owner(sys.argv[2:5], sys.argv[5])
        elif len(sys.argv) == 2 and os.geteuid() != 0:
            path = Path(sys.argv[1])
            metadata = path.lstat()
            base.require(path.is_absolute() and stat.S_ISREG(metadata.st_mode)
                and metadata.st_uid in (0, os.getuid()) and not metadata.st_mode & 0o022
                and 0 < metadata.st_size <= 64 * 1024 * 1024)
            result = subprocess.run(['/usr/bin/unshare', '--user', '--map-root-user',
                '--net', '--pid', '--fork', '--kill-child=SIGKILL', '--', sys.executable,
                str(Path(__file__).resolve()), '--owner', tun.namespace('net'),
                tun.namespace('user'), tun.namespace('pid'), str(path)],
                stdin=subprocess.DEVNULL, capture_output=True, timeout=30, check=False)
            base.require(result.returncode == 0 and len(result.stdout) <= 2048)
            def unique(items):
                value = {}
                for key, item in items:
                    base.require(key not in value)
                    value[key] = item
                return value
            facts = json.loads(result.stdout, object_pairs_hook=unique)
            base.require(type(facts) is dict and set(facts) == FACTS
                         and all(type(value) is bool for value in facts.values()))
        else:
            print('{"unprivileged_parent_required":true}')
            return 2
        print(json.dumps(facts, sort_keys=True))
        return 0 if all(facts.values()) else 1
    except Exception:
        print('{"isolated_rust_tun_probe_failed":true}')
        return 1


if __name__ == '__main__':
    sys.exit(main())
