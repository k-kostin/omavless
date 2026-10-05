#!/usr/bin/python3
"""Fixed decoder static provenance; candidates are data, never executables."""
import errno
import hashlib
import json
import os
from pathlib import Path
import stat
import struct
import sys
import time
import types

STAGE = Path('/home/kdk_vm/.cache/t3-brotli-decoder-provenance-review-1')
CANDIDATE = '/usr/lib/libbrotlidec.so.1.2.0'
PINS = {'containment.py': '2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592',
        'owned.py': '8d9504cff609d703870b4b7f8bec4e4b601d8763b5f69b7df62e3b5da9106258',
        'helpers.py': 'cccc171213f4631f54d906652aeaf7954230949a7de40b2093c8ab587f86aa00',
        'copy-manifest.json': '40a95c1e682f94ee379a8f1cf8c387e60cdbe08ac16516309ede5e0711a5f5fb'}
PACKAGE = '/var/lib/pacman/local/brotli-1.2.0-1/'
PACKAGE_PINS = {'desc': '974d3bdc717e12ac7f08e3f15afe3a67168d07aa854237269cf6528a05e1e0cc',
                'files': 'ff22c1aea2a86cca33a7b4028870948e6e76b292722bdcd49c9e0dac2465c3d4'}
READELF = '/usr/bin/readelf'
READELF_SHA = 'a72f12f3dd8c560554a3ba818bcecbda9178db5befbcc64d91bbb6a241fc21bc'
FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC


def require(value):
    if not value:
        raise RuntimeError('decoder_provenance_refused')


def identity(s):
    return (s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid, s.st_nlink,
            s.st_size, s.st_mtime_ns, s.st_ctime_ns)


def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result)
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=pairs,
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))


def inputs():
    raw = {}
    for parent in (STAGE, *STAGE.parents):
        s = parent.lstat()
        require(stat.S_ISDIR(s.st_mode) and s.st_uid in (0, 1000)
                and s.st_gid in (0, 1000) and not s.st_mode & 0o022)
    s = STAGE.lstat()
    require(s.st_uid == s.st_gid == 1000 and stat.S_IMODE(s.st_mode) == 0o700)
    for name, pin in PINS.items():
        fd = os.open(STAGE / name, FLAGS)
        try:
            before = os.fstat(fd)
            require(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 1000
                    and before.st_nlink == 1 and 0 < before.st_size <= 131072
                    and stat.S_IMODE(before.st_mode) == (0o600 if name.endswith('.json') else 0o500)
                    and not os.listxattr(fd))
            data = os.pread(fd, 131073, 0)
            require(len(data) == before.st_size and hashlib.sha256(data).hexdigest() == pin
                    and identity(before) == identity(os.fstat(fd)) == identity((STAGE / name).lstat()))
            raw[name] = data
        finally:
            os.close(fd)
    modules = []
    for name in ('containment.py', 'helpers.py', 'owned.py'):
        module = types.ModuleType('fixed_decoder_' + name[:-3])
        module.__file__ = str(STAGE / name)
        exec(compile(raw[name], module.__file__, 'exec'), module.__dict__)
        modules.append(module)
    return *modules, decode(raw['copy-manifest.json'])


class Sources:
    """Original public FDs and ancestor FDs retained; all errors terminal."""
    def __init__(self, known):
        self.known = known
        self.deadline = time.monotonic() + 50
        self.state, self.total = 'ready', 0
        self.parents, self.files = {}, {}

    def available(self):
        if self.state != 'ready' or time.monotonic() >= self.deadline:
            self.state = 'refused'
            raise RuntimeError('sources_sealed')

    def parent(self, path):
        for item in (*reversed(path.parent.parents), path.parent):
            if item in self.parents:
                continue
            parent = None if item == Path('/') else self.parents[item.parent][0]
            name = '/' if parent is None else item.name
            fd = os.open(name, FLAGS | os.O_DIRECTORY, dir_fd=parent)
            self.parents[item] = (fd, None, parent, name)
            value = os.fstat(fd)
            require(stat.S_ISDIR(value.st_mode) and value.st_uid == value.st_gid == 0
                    and not value.st_mode & 0o022)
            self.parents[item] = (fd, value, parent, name)
        return self.parents[path.parent][0]

    def read(self, fd, before):
        data = bytearray()
        while len(data) <= before.st_size:
            self.available()
            block = os.pread(fd, min(65536, before.st_size + 1 - len(data)), len(data))
            if not block:
                break
            data.extend(block)
        require(len(data) == before.st_size and identity(before) == identity(os.fstat(fd)))
        return bytes(data)

    def take(self, path):
        self.available()
        try:
            require(path in self.known or path in (CANDIDATE, READELF)
                    or path in tuple(PACKAGE + n for n in PACKAGE_PINS))
            if path in self.files:
                return self.files[path]
            require(len(self.files) < 20)
            p = Path(path)
            parent = self.parent(p)
            fd = os.open(p.name, FLAGS, dir_fd=parent)
            self.files[path] = (fd, None, None, parent)
            before = os.fstat(fd)
            maximum = 4 * 1024 * 1024 if path.startswith(PACKAGE) else 32 * 1024 * 1024
            require(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 0
                    and not before.st_mode & 0o022 and before.st_nlink == 1
                    and 0 < before.st_size <= maximum and not os.listxattr(fd))
            self.total += before.st_size
            require(self.total <= 128 * 1024 * 1024)
            data = self.read(fd, before)
            digest = hashlib.sha256(data).hexdigest()
            require(identity(before) == identity(os.stat(p.name, dir_fd=parent, follow_symlinks=False)))
            if path in self.known:
                row = self.known[path]
                require(digest == row['sha256'] and all(getattr(before, 'st_' + field) == row[key]
                        for field, key in (('dev','device'), ('ino','inode'), ('mode','mode'),
                                          ('uid','uid'), ('gid','gid'), ('nlink','nlink'), ('size','size'))))
            elif path == READELF:
                require(digest == READELF_SHA and (before.st_dev, before.st_ino, before.st_size,
                        before.st_mode) == (31, 29149, 810072, 0o100755))
            elif path.startswith(PACKAGE):
                require(digest == PACKAGE_PINS[p.name])
            self.files[path] = (fd, before, data, parent)
            return self.files[path]
        except BaseException:
            self.state = 'refused'
            raise

    def recheck(self):
        self.available()
        try:
            for path, (fd, before, data, parent) in self.files.items():
                require(self.read(fd, before) == data and not os.listxattr(fd)
                        and identity(before) == identity(os.stat(Path(path).name, dir_fd=parent, follow_symlinks=False)))
            for path, (fd, before, parent, name) in self.parents.items():
                self.available()
                require(identity(before) == identity(os.fstat(fd))
                        == identity(os.stat(name, dir_fd=parent, follow_symlinks=False)))
        except BaseException:
            self.state = 'refused'
            raise


def header(raw):
    require(len(raw) >= 64 and raw[:7] == b'\x7fELF\x02\x01\x01')
    fields = struct.unpack('<16sHHIQQQIHHHHHH', raw[:64])
    require(fields[1] == 3 and fields[2] == 62 and fields[3] == 1 and fields[8] == 64)
    return dict(elf_class=2, data_encoding=1, version=1, object_type=fields[1],
                machine=fields[2], header_size=fields[8], program_header_count=fields[10])


def capture(base, helpers, owned, manifest):
    known = manifest['source_provenance']
    require(len(known) == 16 and CANDIDATE not in known)
    sources = Sources(known)
    # Fixed package rows only; never enumerate the installed package database.
    package = {name: sources.take(PACKAGE + name)[2] for name in PACKAGE_PINS}
    sections = package['desc'].decode('utf-8', 'strict').splitlines()
    for key, value in (('%NAME%', 'brotli'), ('%VERSION%', '1.2.0-1')):
        require(sections.count(key) == 1 and sections[sections.index(key) + 1] == value)
    rows = package['files'].decode('utf-8', 'strict').splitlines()
    require(rows.count('%FILES%') == 1)
    entries = []
    for row in rows[rows.index('%FILES%') + 1:]:
        if not row or row.startswith('%'):
            break
        require(len(row) <= 4096 and not row.startswith('/') and '..' not in Path(row).parts)
        entries.append(row)
    require(len(entries) <= 16384 and len(entries) == len(set(entries)) and CANDIDATE[1:] in entries)
    tool_fd, tool_identity, _, _ = sources.take(READELF)
    queue, aliases, records = [(CANDIDATE, 0, None)], {}, {}
    steps = 0
    while queue:
        sources.available()
        require(not base.UNSETTLED)
        path, depth, expected = queue.pop(0)
        steps += 1
        require(steps <= 128 and depth <= 8 and len(queue) <= 128)
        resolved, links = helpers.canonical_public(path)
        require(path != CANDIDATE or (resolved == CANDIDATE and not links))
        require(expected is None or (resolved, links) == expected)
        require(resolved == CANDIDATE or resolved in known)
        require(path not in aliases or aliases[path] == (resolved, links))
        aliases[path] = (resolved, links)
        if resolved in records:
            continue
        fd, before, raw, _ = sources.take(resolved)
        shape = header(raw)
        sources.recheck()  # ALL current original FDs before any tool execution.
        command = owned.command(base, [f'/proc/self/fd/{tool_fd}', '--wide', '--dynamic', '--program-headers',
                                f'/proc/self/fd/{fd}'], pass_fds=(tool_fd, fd),
                               env={'PATH':'/usr/bin', 'LANG':'C', 'LC_ALL':'C'})
        require(command.stderr == b'')
        dynamic = helpers.decode_readelf(command.stdout)
        sources.recheck()
        require(helpers.canonical_public(path) == (resolved, links))
        dependencies = []
        for name in dynamic['needed']:
            logical, target, chain = helpers.resolve_needed(name)
            require(target in known)  # Never discover or admit another object.
            dependencies.append(dict(name=name, logical_path=logical, resolved_path=target, links=chain))
            queue.append((logical, depth + 1, (target, chain)))
        if dynamic['interpreter']:
            target, chain = helpers.canonical_public(dynamic['interpreter'])
            require(target in known)
            queue.append((dynamic['interpreter'], depth + 1, (target, chain)))
        records[resolved] = dict(path=resolved, identity=list(identity(before)), sha256=hashlib.sha256(raw).hexdigest(),
                                 header=shape, dynamic=dynamic, dependencies=dependencies,
                                 matches_reviewed_source=resolved in known)
    for path, expected in aliases.items():
        require(helpers.canonical_public(path) == expected)
    for row in records.values():
        for edge in row['dependencies']:
            require(aliases.get(edge['logical_path']) == (edge['resolved_path'], edge['links']))
    sources.recheck()
    return dict(schema='fixed-brotli-decoder-provenance-v1', outcome='OBSERVED_STATIC_ONLY',
                candidate=CANDIDATE, manifest_sha256=PINS['copy-manifest.json'], readelf_sha256=READELF_SHA,
                package=dict(name='brotli', version='1.2.0-1', hashes=PACKAGE_PINS,
                             identities={name: list(identity(sources.files[PACKAGE + name][1])) for name in PACKAGE_PINS},
                             candidate_listed=True, global_package_owner_uniqueness_proven=False),
                readelf_identity=list(identity(tool_identity)),
                records=[records[p] for p in sorted(records)],
                aliases=[dict(path=p, resolved_path=row[0], links=row[1]) for p,row in sorted(aliases.items())],
                candidate_elf_executed=False, allowlist_adoption=False,
                loaded_elf_identity_proven=False, compatibility_acceptance=False)


def main():
    require(Path(__file__) == STAGE / 'probe.py' and sys.argv[1:] == ['--capture-fixed-decoder']
            and os.getuid() == os.geteuid() == 1000)
    base, helpers, owned, manifest = inputs()
    base.limits()
    result = capture(base, helpers, owned, manifest)
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    os.umask(0o077)
    try:
        main()
    except BaseException:
        os.write(2, b'FIXED_DECODER_CAPTURE_NONPASS\n')
        raise SystemExit(1) from None
