#!/usr/bin/python3
"""Trusted-stdin create-only delivery. Never executes delivered code or ELF."""
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import time

SOURCE = Path('/home/kdk_vm/.cache/t4-first-abort-cli-delivery-v6')
DESTINATION = Path('/run/ov-t4-cli-guard-v6')
CODE = ('core.py', 'support.py', 'startup_inventory.py', 'startup_followup.py',
        'lineage.py', 'root_guard.py')
ELFS = ('helper', 'omavless')
FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
HELD = []
DEADLINE = float('inf')
TERMINAL = False
HEADROOM = 512 * 1024 * 1024


def need(value):
    if not value or time.monotonic() >= DEADLINE:
        raise RuntimeError('fixed_delivery_refused')


def identity(s):
    return (s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid,
            s.st_nlink, s.st_size, s.st_mtime_ns, s.st_ctime_ns)


def directory_identity(s):
    return (s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid)


def unique(items):
    out = {}
    for name, value in items:
        need(name not in out)
        out[name] = value
    return out


def parents(path, uid):
    rows = []
    for part in reversed((path, *path.parents)):
        need(True)
        parent = rows[-1][1] if rows else None
        fd = os.open(part.name if parent is not None else '/', FLAGS | os.O_DIRECTORY, dir_fd=parent)
        HELD.append(fd)
        s = os.fstat(fd)
        need(stat.S_ISDIR(s.st_mode) and s.st_uid == s.st_gid and s.st_uid in (0, uid)
             and not s.st_mode & 0o6022)
        try:
            os.stat('.git', dir_fd=fd, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            need(False)
        rows.append((part, fd, s))
    return rows


def recheck_parents(rows):
    for path, fd, s in rows:
        need(directory_identity(s) == directory_identity(os.fstat(fd)) == directory_identity(path.lstat()))


def digest(fd, size):
    out, offset = hashlib.sha256(), 0
    while offset < size:
        need(True)
        data = os.pread(fd, min(65536, size - offset), offset)
        need(data)
        out.update(data)
        offset += len(data)
    need(os.pread(fd, 1, offset) == b'')
    return out.hexdigest()


def admit(parent, name, mode, maximum, expected):
    need(True)
    fd = os.open(name, FLAGS, dir_fd=parent)
    HELD.append(fd)
    s = os.fstat(fd)
    need(stat.S_ISREG(s.st_mode) and s.st_uid == s.st_gid == 1000 and s.st_nlink == 1
         and stat.S_IMODE(s.st_mode) == mode and 0 < s.st_size <= maximum and not os.listxattr(fd))
    row = (fd, s, expected)
    recheck(parent, name, row)
    return row


def recheck(parent, name, row):
    fd, s, expected = row
    need(identity(s) == identity(os.fstat(fd))
         == identity(os.stat(name, dir_fd=parent, follow_symlinks=False)) and not os.listxattr(fd))
    need(digest(fd, s.st_size) == expected)
    need(identity(s) == identity(os.fstat(fd))
         == identity(os.stat(name, dir_fd=parent, follow_symlinks=False)))


def remaining_capacity(run_fd, home_fd, root_copy, home_copy):
    """Source is already allocated. Reserve remaining copies, never tmpfs resize."""
    need(type(root_copy) is int and type(home_copy) is int and root_copy > 0 and home_copy > 0)
    available, required = {}, {}
    for fd, count in ((run_fd, root_copy), (home_fd, home_copy)):
        need(True)
        device = os.fstat(fd).st_dev
        filesystem = os.fstatvfs(fd)
        free = filesystem.f_bavail * filesystem.f_frsize
        need(type(free) is int and free >= 0)
        available[device] = min(available.get(device, free), free)
        required[device] = required.get(device, 0) + count + HEADROOM
    need(all(available[device] >= count for device, count in required.items()))


def copy_file(source, target, size, mode):
    need(type(size) is int and size > 0 and type(mode) is int and mode in (0o500, 0o600))
    offset = 0
    while offset < size:
        need(True)
        block = os.pread(source, min(65536, size - offset), offset)
        need(block)
        written = os.write(target, block)
        need(type(written) is int and written == len(block))
        offset += len(block)
    need(True)
    os.fchmod(target, mode)
    need(True)
    os.fsync(target)
    need(True)


def deliver(expected):
    need(type(expected) is str and re.fullmatch('[0-9a-f]{64}', expected))
    source = parents(SOURCE, 1000)
    parent = source[-1][1]
    need(source[-1][2].st_uid == 1000 and stat.S_IMODE(source[-1][2].st_mode) == 0o700)
    entries = []
    with os.scandir(parent) as it:
        for item in it:
            entries.append(item.name)
            need(len(entries) <= 9)
    need(set(entries) == {*CODE, *ELFS, 'receipt.json'})
    pins = {'receipt.json': admit(parent, 'receipt.json', 0o600, 32768, expected)}
    fd, s, _ = pins['receipt.json']
    value = json.loads(os.pread(fd, s.st_size + 1, 0), object_pairs_hook=unique,
                       parse_constant=lambda _: need(False))
    need(type(value) is dict and set(value) == {'schema', 'native_head', 'guard_head', 'code', 'elfs'}
         and value['schema'] == 't4-disposable-cli-delivery-v6'
         and value['native_head'] == '285233049bc6c5e1356de0ffcc167185c9e1761d'
         and type(value['guard_head']) is str and re.fullmatch('[0-9a-f]{40}', value['guard_head'])
         and type(value['code']) is dict and set(value['code']) == set(CODE)
         and type(value['elfs']) is dict and set(value['elfs']) == set(ELFS))
    for name in CODE:
        sha = value['code'][name]
        need(type(sha) is str and re.fullmatch('[0-9a-f]{64}', sha))
        pins[name] = admit(parent, name, 0o500, 8 * 1024 * 1024, sha)
    for name in ELFS:
        row = value['elfs'][name]
        need(type(row) is dict and set(row) == {'sha256', 'size', 'host_original', 'host_frozen', 'host_alias'}
             and type(row['sha256']) is str and re.fullmatch('[0-9a-f]{64}', row['sha256'])
             and type(row['size']) is int and 0 < row['size'] <= 512 * 1024 * 1024)
        for key, mode in (('host_original', 0o755), ('host_frozen', 0o500)):
            m = row[key]
            need(type(m) is list and len(m) == 9 and all(type(v) is int and 0 <= v < 2**64 for v in m)
                 and stat.S_ISREG(m[2]) and stat.S_IMODE(m[2]) == mode
                 and m[3] == m[4] == 1000 and m[5] == (2 if key == 'host_original' and name == 'omavless' else 1)
                 and m[6] == row['size'])
        alias = row['host_alias']
        if name == 'helper':
            need(alias is None)
        else:
            need(type(alias) is dict and set(alias) == {'relative_path', 'identity', 'sha256'}
                 and alias['relative_path'] == 'debug/deps/omavless-3eaa741bede04cf2'
                 and type(alias['identity']) is list and len(alias['identity']) == 9
                 and all(type(n) is int for n in alias['identity'])
                 and alias['identity'] == row['host_original'] and alias['sha256'] == row['sha256'])
        pins[name] = admit(parent, name, 0o500, 512 * 1024 * 1024, row['sha256'])
        need(pins[name][1].st_size == row['size'] and os.pread(pins[name][0], 4, 0) == b'\x7fELF')
    # No publication until every original source FD and all metadata are admitted.
    destination = parents(DESTINATION.parent, 0)
    home = parents(Path('/home'), 0)
    for name, row in pins.items():
        recheck(parent, name, row)
    recheck_parents(source)
    recheck_parents(destination)
    remaining_capacity(destination[-1][1], home[-1][1],
                       sum(row[1].st_size for row in pins.values()),
                       sum(pins[name][1].st_size for name in ELFS))
    recheck_parents(home)
    recheck_parents(destination)
    recheck_parents(source)
    need(True)
    os.mkdir(DESTINATION.name, 0o700, dir_fd=destination[-1][1])
    need(True)
    out = os.open(DESTINATION.name, FLAGS | os.O_DIRECTORY, dir_fd=destination[-1][1])
    HELD.append(out)
    original = os.fstat(out)
    need(original.st_uid == original.st_gid == 0 and stat.S_IMODE(original.st_mode) == 0o700)
    published = []
    for name, (fd, s, sha) in pins.items():
        need(True)
        recheck(parent, name, pins[name])
        need(True)
        target = os.open(name, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                         0o600, dir_fd=out)
        HELD.append(target)
        copy_file(fd, target, s.st_size, 0o600 if name == 'receipt.json' else 0o500)
        m = os.fstat(target)
        need(m.st_uid == m.st_gid == 0 and m.st_nlink == 1 and not os.listxattr(target)
             and digest(target, s.st_size) == sha
             and identity(m) == identity(os.fstat(target))
             == identity(os.stat(name, dir_fd=out, follow_symlinks=False)))
        published.append((name, target, m, sha))
        recheck(parent, name, pins[name])
    for name, row in pins.items():
        recheck(parent, name, row)
    for name, fd, m, sha in published:
        need(identity(m) == identity(os.fstat(fd))
             == identity(os.stat(name, dir_fd=out, follow_symlinks=False))
             and not os.listxattr(fd) and digest(fd, m.st_size) == sha)
    recheck_parents(source)
    recheck_parents(destination)
    need(directory_identity(original) == directory_identity(os.fstat(out)) == directory_identity(DESTINATION.lstat()))
    need(True)
    os.fsync(out)
    need(True)
    os.fsync(destination[-1][1])
    need(True)


def emit_terminal(raw, stream):
    global TERMINAL
    try:
        need(not TERMINAL and type(raw) is bytes and len(raw) <= 8192)
        written = stream.write(raw)
        need(type(written) is int and written == len(raw))
        stream.flush()
        need(True)
    finally:
        TERMINAL = True  # Success or uncertainty: never a second output attempt.


if __name__ == '__main__':
    os.umask(0o077)
    DEADLINE = time.monotonic() + 180
    try:
        need(__file__ == '<stdin>' and sys.flags.isolated == 1 and sys.dont_write_bytecode
             and os.getresuid() == os.getresgid() == (0, 0, 0)
             and len(sys.argv) == 2)
        deliver(sys.argv[1])
        emit_terminal(b'T4_CLI_V2_CREATE_ONLY_DELIVERY_NOT_EXECUTED\n', sys.stdout.buffer)
    except BaseException:
        if not TERMINAL:
            try:
                emit_terminal(b'T4_CLI_DELIVERY_NONPASS_RETAINED\n', sys.stderr.buffer)
            except BaseException:
                pass  # A failed terminal output never authorizes another attempt.
        sys.exit(2)
