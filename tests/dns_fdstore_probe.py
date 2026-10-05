#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Explicit disposable user-service FD-store conformance; ordinary file only.

Never touches installed VPN units, DNS, TUN, system manager, sudo or private data.
No persistent unit files are written. Every generated transient unit is stopped
and its own descriptor store explicitly cleaned, including error paths.
"""
import array
import json
import os
from pathlib import Path
import re
import select
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import time
import uuid

UNIT_RE = re.compile(r'omavless-dns-fdstore-probe-[0-9a-f]{32}\.service\Z')
NAME = 'synthetic-lease'
FACTS = {'user_only', 'barrier_completed', 'store_confirmed',
         'crash_restart_same_object', 'explicit_restart_same_object',
         'restart_policy_stop_releases', 'yes_policy_stop_retains',
         'yes_policy_start_recovers', 'explicit_remove_releases',
         'barrier_without_capacity_not_acceptance', 'cleanup_complete'}


def require(condition):
    if not condition:
        raise RuntimeError('fdstore_probe_refused')


def unit_name(value):
    require(type(value) is str and UNIT_RE.fullmatch(value) is not None)
    return value


def command(arguments, check=True):
    result = subprocess.run(arguments, stdin=subprocess.DEVNULL, capture_output=True,
                            timeout=9, check=False)
    require(len(result.stdout) <= 4096 and len(result.stderr) <= 4096)
    if check:
        require(result.returncode == 0)
    return result


def control(unit, action, *options, check=True):
    unit_name(unit)
    require(action in ('show', 'stop', 'restart', 'start', 'kill', 'clean', 'reset-failed'))
    return command(['/usr/bin/systemctl', '--user', action, *options, unit], check=check)


def count(unit):
    result = control(unit, 'show', '--property=LoadState', '--property=NFileDescriptorStore', check=False)
    fields = {}
    for line in result.stdout.decode('ascii').splitlines():
        key, value = line.split('=', 1)
        require(key not in fields and key in ('LoadState', 'NFileDescriptorStore'))
        fields[key] = value
    if fields.get('LoadState') == 'not-found':
        return 0
    require(result.returncode == 0 and fields.get('LoadState') == 'loaded')
    value = fields.get('NFileDescriptorStore', '')
    require(value.isascii() and value.isdecimal() and int(value) in (0, 1))
    return int(value)


def notify(channel, payload, descriptors=()):
    require(payload in (b'FDSTORE=1\nFDNAME=synthetic-lease\nFDPOLL=0',
                        b'FDSTOREREMOVE=1\nFDNAME=synthetic-lease',
                        b'BARRIER=1', b'READY=1'))
    ancillary = []
    if descriptors:
        ancillary = [(socket.SOL_SOCKET, socket.SCM_RIGHTS, array.array('i', descriptors))]
    require(channel.sendmsg([payload], ancillary) == len(payload))


def barrier(channel):
    reader, writer = os.pipe2(os.O_CLOEXEC)
    try:
        notify(channel, b'BARRIER=1', (writer,))
        os.close(writer)
        writer = None
        ready, _, _ = select.select([reader], [], [], 3)
        require(ready == [reader] and os.read(reader, 1) == b'')
    finally:
        if writer is not None:
            os.close(writer)
        os.close(reader)


def record(root, value):
    data = json.dumps(value, sort_keys=True).encode('ascii')
    require(len(data) <= 512)
    # Private synthetic fixture only; atomic reader-visible checkpoint.
    path = root / 'record.pending'
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC, 0o600)
    try:
        require(os.write(fd, data) == len(data))
    finally:
        os.close(fd)
    os.replace(path, root / 'record')


def read_record(root):
    with (root / 'record').open('rb') as stream:
        data = stream.read(513)
    require(len(data) <= 512)
    value = json.loads(data)
    require(type(value) is dict and set(value) == {'generation', 'device', 'inode', 'same', 'barrier', 'removed'})
    require(type(value['generation']) is int and 1 <= value['generation'] <= 4)
    require(type(value['device']) is int and type(value['inode']) is int)
    require(all(type(value[key]) is bool for key in ('same', 'barrier', 'removed')))
    return value


def worker(root_text):
    root = Path(root_text)
    require(os.geteuid() != 0 and root.is_absolute() and root.name.startswith('omavless-fdstore.'))
    metadata = root.lstat()
    require(stat.S_ISDIR(metadata.st_mode) and metadata.st_uid == os.getuid()
            and stat.S_IMODE(metadata.st_mode) == 0o700)
    address = os.environ.get('NOTIFY_SOCKET', '')
    require(1 < len(address) <= 107 and address[0] in ('/', '@'))
    if address.startswith('@'):
        address = '\0' + address[1:]
    previous = read_record(root) if (root / 'record').exists() else None
    inherited = os.environ.get('LISTEN_FDS', '0')
    require(inherited in ('0', '1'))
    with socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM) as channel:
        channel.settimeout(3)
        channel.connect(address)
        if inherited == '1':
            require(os.environ.get('LISTEN_PID') == str(os.getpid())
                    and os.environ.get('LISTEN_FDNAMES') == NAME and previous is not None)
            with os.fdopen(3, 'rb', closefd=True) as stored:
                metadata = os.fstat(stored.fileno())
                require(stat.S_ISREG(metadata.st_mode))
                require(os.pread(stored.fileno(), 32, 0) == b'ordinary-file-proof')
                same = (metadata.st_dev, metadata.st_ino) == (previous['device'], previous['inode'])
            require(same)
        else:
            require(previous is None)
            with tempfile.TemporaryFile() as stored:
                stored.write(b'ordinary-file-proof')
                stored.flush()
                metadata = os.fstat(stored.fileno())
                notify(channel, b'FDSTORE=1\nFDNAME=synthetic-lease\nFDPOLL=0', (stored.fileno(),))
                barrier(channel)
            same = True
        value = {'generation': 1 if previous is None else previous['generation'] + 1,
                 'device': metadata.st_dev, 'inode': metadata.st_ino,
                 'same': same, 'barrier': True, 'removed': False}
        record(root, value)
        def remove(_signal, _frame):
            notify(channel, b'FDSTOREREMOVE=1\nFDNAME=synthetic-lease')
            barrier(channel)
            value['removed'] = True
            record(root, value)
        signal.signal(signal.SIGUSR1, remove)
        notify(channel, b'READY=1')
        while True:
            signal.pause()


def await_record(root, generation, removed=False):
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        try:
            value = read_record(root)
            if value['generation'] == generation and value['removed'] == removed:
                return value
        except FileNotFoundError:
            pass
        time.sleep(0.05)
    raise RuntimeError('fdstore_probe_refused')


def cleanup(unit):
    # Every target is a freshly generated, validated test unit, never a glob.
    control(unit, 'stop', check=False)
    control(unit, 'clean', '--what=fdstore', check=False)
    control(unit, 'reset-failed', check=False)
    require(count(unit) == 0)
    result = control(unit, 'show', '--property=LoadState', '--property=ActiveState',
                     '--property=MainPID', check=False)
    fields = {}
    for line in result.stdout.decode('ascii').splitlines():
        key, value = line.split('=', 1)
        require(key not in fields and key in ('LoadState', 'ActiveState', 'MainPID'))
        fields[key] = value
    require(fields.get('LoadState') == 'not-found' or
            (result.returncode == 0 and fields.get('LoadState') == 'loaded'
             and fields.get('ActiveState') == 'inactive' and fields.get('MainPID') == '0'))


def one_case(preserve, capacity):
    require(preserve in ('restart', 'yes') and capacity in (0, 1))
    unit = unit_name('omavless-dns-fdstore-probe-' + uuid.uuid4().hex + '.service')
    with tempfile.TemporaryDirectory(prefix='omavless-fdstore.') as directory:
        root = Path(directory)
        try:
            command(['/usr/bin/systemd-run', '--user', '--quiet', '--unit=' + unit,
                '--property=Type=notify', '--property=NotifyAccess=main',
                '--property=FileDescriptorStoreMax=' + str(capacity),
                '--property=FileDescriptorStorePreserve=' + preserve,
                '--property=Restart=on-failure', '--property=RestartSec=100ms',
                '--property=StartLimitIntervalSec=2min', '--property=StartLimitBurst=5',
                '--property=RuntimeMaxSec=25s', '--property=TimeoutStartSec=5s',
                '--property=TimeoutStopSec=3s', '--property=NoNewPrivileges=yes',
                '--property=StandardOutput=null', '--property=StandardError=null',
                '--', sys.executable, str(Path(__file__).resolve()), '--worker', str(root)])
            initial = await_record(root, 1)
            require(initial['barrier'] and count(unit) == capacity)
            if capacity == 0:
                return {'barrier_without_capacity_not_acceptance': True}
            facts = {'barrier_completed': True, 'store_confirmed': True}
            control(unit, 'kill', '--kill-whom=main', '--signal=SIGKILL')
            facts['crash_restart_same_object'] = await_record(root, 2)['same'] and count(unit) == 1
            control(unit, 'restart')
            facts['explicit_restart_same_object'] = await_record(root, 3)['same'] and count(unit) == 1
            control(unit, 'stop')
            if preserve == 'restart':
                facts['restart_policy_stop_releases'] = count(unit) == 0
            else:
                facts['yes_policy_stop_retains'] = count(unit) == 1
                control(unit, 'start')
                facts['yes_policy_start_recovers'] = await_record(root, 4)['same'] and count(unit) == 1
                control(unit, 'kill', '--kill-whom=main', '--signal=SIGUSR1')
                require(await_record(root, 4, removed=True)['removed'])
                facts['explicit_remove_releases'] = count(unit) == 0
            return facts
        finally:
            cleanup(unit)


def main():
    try:
        if len(sys.argv) == 3 and sys.argv[1] == '--worker':
            worker(sys.argv[2])
            return 0
        if len(sys.argv) != 1 or os.geteuid() == 0:
            print('{"unprivileged_parent_required":true}')
            return 2
        facts = {'user_only': True}
        for preserve, capacity in [('restart', 1), ('yes', 1), ('yes', 0)]:
            current = one_case(preserve, capacity)
            for key, value in current.items():
                facts[key] = facts.get(key, True) and value
        facts['cleanup_complete'] = True
        require(set(facts) == FACTS and all(type(value) is bool for value in facts.values()))
        print(json.dumps(facts, sort_keys=True))
        return 0 if all(facts.values()) else 1
    except Exception:
        print('{"user_fdstore_probe_failed":true}')
        return 1


if __name__ == '__main__':
    sys.exit(main())
