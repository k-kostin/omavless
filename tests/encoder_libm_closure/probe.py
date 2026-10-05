#!/usr/bin/python3
"""Fixed encoder static provenance; candidates are data, never executables."""
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

STAGE = Path('/home/kdk_vm/.cache/t3-encoder-libm-closure-review-1')
CANDIDATE = '/usr/lib/libbrotlienc.so.1.2.0'
PINS = {'containment.py': '2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592',
        'owned.py': '3ff7973c23d750fc8f1706120116def70d4615ac1d45a75fd61d31373dd17a39',
        'helpers.py': 'cccc171213f4631f54d906652aeaf7954230949a7de40b2093c8ab587f86aa00',
        'copy-manifest.json': 'd6ca9cd599adb573f4419f49147677f681db08fcc54a06cb0f34ed950ce19c36'}
PACKAGE = '/var/lib/pacman/local/brotli-1.2.0-1/'
PACKAGE_PINS = {'desc': '974d3bdc717e12ac7f08e3f15afe3a67168d07aa854237269cf6528a05e1e0cc',
                'files': 'ff22c1aea2a86cca33a7b4028870948e6e76b292722bdcd49c9e0dac2465c3d4'}
LIBM = '/usr/lib/libm.so.6'
GLIBC_VERSION = '2.44+r24+g16be1518495f-1'
GLIBC_PACKAGE = '/var/lib/pacman/local/glibc-' + GLIBC_VERSION + '/'
GLIBC_PINS = {'desc': '2ffdc9e3ae2fc944730a6a9d0029b8cc103fd28eade7f35f5cf89ecd9de2abbc',
              'files': '4561378a0ee8d0288a52cbc6aef7305a6dc0073ee0a58bce639f95066ea8e3f5'}
PACKAGE_FILES = {**{PACKAGE+n: pin for n,pin in PACKAGE_PINS.items()},
                 **{GLIBC_PACKAGE+n: pin for n,pin in GLIBC_PINS.items()}}
READELF = '/usr/bin/readelf'
READELF_SHA = 'a72f12f3dd8c560554a3ba818bcecbda9178db5befbcc64d91bbb6a241fc21bc'
FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
PHASES = frozenset(('entry', 'input_ancestry', 'input_open', 'input_shape',
    'input_read', 'input_pin', 'input_load', 'manifest_boundary', 'package_desc',
    'package_files', 'package_parse', 'glibc_desc', 'glibc_files', 'glibc_parse', 'tool_admission', 'queue_boundary',
    'canonical_queue', 'queued_identity', 'candidate_admission', 'elf_header',
    'before_readelf', 'readelf_stderr', 'readelf_decode', 'canonical_recheck',
    'dependency_resolve', 'dependency_membership', 'interpreter_resolve',
    'interpreter_membership', 'final_aliases', 'final_edges', 'receipt',
    'source_scope', 'parent_open', 'parent_identity',
    'source_open', 'source_shape', 'source_read', 'source_hash',
    'source_path', 'source_pin', 'sources_recheck', 'recheck_file',
    'recheck_parent', 'output'))
EVENTS = None  # Inert imports have no observer; fixed main installs one.


class Events:
    def __init__(self):
        self.sealed = False
        self.reported = False
        self.last = 'entry'
        self.count = self.total = 0

    def before(self, phase):
        if self.sealed:
            raise RuntimeError('events_sealed')
        try:
            require(phase in PHASES and type(phase) is str)
            raw = ('T3_ENCODER_STATIC_BEFORE_V1 ' + phase + '\n').encode('ascii')
            require(self.count < 2048 and self.total + len(raw) <= 130816)
            self.last = phase
            self.count += 1
            self.total += len(raw)
            require(os.write(2, raw) == len(raw))
        except BaseException:
            self.sealed = True
            self.reported = True  # A failed event write is never retried.
            raise

    def failure(self):
        # Authorized finite output only, no post-failure observation or cleanup.
        self.sealed = True
        if self.reported:
            return
        self.reported = True
        raw = ('T3_ENCODER_STATIC_FAILED_AT_V1 ' + self.last + '\n').encode('ascii')
        require(os.write(2, raw) == len(raw))


def boundary(phase):
    if EVENTS is not None:
        EVENTS.before(phase)


def require(value):
    if not value:
        raise RuntimeError('encoder_provenance_refused')


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
    boundary('input_ancestry')
    for parent in (STAGE, *STAGE.parents):
        s = parent.lstat()
        require(stat.S_ISDIR(s.st_mode) and s.st_uid in (0, 1000)
                and s.st_gid in (0, 1000) and not s.st_mode & 0o022)
    s = STAGE.lstat()
    require(s.st_uid == s.st_gid == 1000 and stat.S_IMODE(s.st_mode) == 0o700)
    for name, pin in PINS.items():
        boundary('input_open')
        fd = os.open(STAGE / name, FLAGS)
        try:
            boundary('input_shape')
            before = os.fstat(fd)
            require(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 1000
                    and before.st_nlink == 1 and 0 < before.st_size <= 131072
                    and stat.S_IMODE(before.st_mode) == (0o600 if name.endswith('.json') else 0o500)
                    and not os.listxattr(fd))
            boundary('input_read')
            data = os.pread(fd, 131073, 0)
            boundary('input_pin')
            require(len(data) == before.st_size and hashlib.sha256(data).hexdigest() == pin
                    and identity(before) == identity(os.fstat(fd)) == identity((STAGE / name).lstat()))
            raw[name] = data
        finally:
            os.close(fd)
    modules = []
    boundary('input_load')
    for name in ('containment.py', 'helpers.py', 'owned.py'):
        module = types.ModuleType('fixed_encoder_' + name[:-3])
        module.__file__ = str(STAGE / name)
        exec(compile(raw[name], module.__file__, 'exec'), module.__dict__)
        modules.append(module)
    return *modules, decode(raw['copy-manifest.json'])


class Sources:
    """Original public FDs and ancestor FDs retained; all errors terminal."""
    def __init__(self, known):
        self.known = known
        self.deadline = time.monotonic() + 120
        self.state, self.total = 'ready', 0
        self.parents, self.files = {}, {}
        self.packages_validated = False

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
            boundary('parent_open')
            fd = os.open(name, FLAGS | os.O_DIRECTORY, dir_fd=parent)
            self.parents[item] = (fd, None, parent, name)
            boundary('parent_identity')
            value = os.fstat(fd)
            require(stat.S_ISDIR(value.st_mode) and value.st_uid == value.st_gid == 0
                    and not value.st_mode & 0o022)
            self.parents[item] = (fd, value, parent, name)
        return self.parents[path.parent][0]

    def read(self, fd, before):
        boundary('source_read')
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
            boundary('source_scope')
            require(path in self.known or path in (CANDIDATE, LIBM, READELF) or path in PACKAGE_FILES)
            require(path not in (CANDIDATE, LIBM) or self.packages_validated)
            if path in self.files:
                return self.files[path]
            require(len(self.files) < 24)
            p = Path(path)
            parent = self.parent(p)
            boundary('source_open')
            fd = os.open(p.name, FLAGS, dir_fd=parent)
            self.files[path] = (fd, None, None, parent)
            boundary('source_shape')
            before = os.fstat(fd)
            maximum = 4 * 1024 * 1024 if path in PACKAGE_FILES else 32 * 1024 * 1024
            require(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 0
                    and not before.st_mode & 0o022 and before.st_nlink == 1
                    and 0 < before.st_size <= maximum and not os.listxattr(fd))
            self.total += before.st_size
            require(self.total <= 128 * 1024 * 1024)
            data = self.read(fd, before)
            boundary('source_hash')
            digest = hashlib.sha256(data).hexdigest()
            boundary('source_path')
            require(identity(before) == identity(os.stat(p.name, dir_fd=parent, follow_symlinks=False)))
            boundary('source_pin')
            if path in self.known:
                row = self.known[path]
                require(digest == row['sha256'] and all(getattr(before, 'st_' + field) == row[key]
                        for field, key in (('dev','device'), ('ino','inode'), ('mode','mode'),
                                          ('uid','uid'), ('gid','gid'), ('nlink','nlink'), ('size','size'))))
            elif path == READELF:
                require(digest == READELF_SHA and (before.st_dev, before.st_ino, before.st_size,
                        before.st_mode) == (31, 29149, 810072, 0o100755))
            elif path in PACKAGE_FILES:
                require(digest == PACKAGE_FILES[path])
            self.files[path] = (fd, before, data, parent)
            return self.files[path]
        except BaseException:
            self.state = 'refused'
            raise

    def recheck(self):
        self.available()
        try:
            boundary('sources_recheck')
            for path, (fd, before, data, parent) in self.files.items():
                boundary('recheck_file')
                require(self.read(fd, before) == data and not os.listxattr(fd)
                        and identity(before) == identity(os.stat(Path(path).name, dir_fd=parent, follow_symlinks=False)))
            for path, (fd, before, parent, name) in self.parents.items():
                self.available()
                boundary('recheck_parent')
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


def package_binding(desc, files, name, version, required):
    def sections(raw):
        require(type(raw) is bytes and 0 < len(raw) <= 4*1024*1024
                and b'\0' not in raw and b'\r' not in raw and raw.endswith(b'\n'))
        rows = raw.decode('utf-8', 'strict').split('\n')
        require(len(rows) <= 65536 and all(len(row.encode()) <= 4096 for row in rows))
        result, current = {}, None
        for row in rows:
            if row.startswith('%'):
                require(len(row) >= 3 and row.endswith('%') and row not in result
                        and all(c in 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_' for c in row[1:-1])
                        and len(result) < 128)
                result[row], current = [], row
            elif not row:
                current = None
            else:
                require(current is not None)
                result[current].append(row)
        return result
    description, listing = sections(desc), sections(files)
    require(description.get('%NAME%') == [name] and description.get('%VERSION%') == [version])
    entries = listing.get('%FILES%')
    require(type(entries) is list and 0 < len(entries) <= 65536 and len(entries) == len(set(entries)))
    require(all(not entry.startswith('/') and '..' not in entry.split('/')
                and '.' not in entry.split('/') and '//' not in entry for entry in entries))
    require(all(path in entries for path in required))


def capture(base, helpers, owned, manifest):
    if EVENTS is not None and EVENTS.sealed:
        raise RuntimeError('events_sealed')
    try:
        return capture_inner(base, helpers, owned, manifest)
    except BaseException:
        if EVENTS is not None:
            EVENTS.sealed = True
        raise


def capture_inner(base, helpers, owned, manifest):
    boundary('manifest_boundary')
    known = manifest['source_provenance']
    require(len(known) == 17 and CANDIDATE not in known and LIBM not in known)
    sources = Sources(known)
    # Fixed package rows only; never enumerate the installed package database.
    package = {}
    for name in PACKAGE_PINS:
        boundary('package_' + name)
        package[name] = sources.take(PACKAGE + name)[2]
    boundary('package_parse')
    package_binding(package['desc'], package['files'], 'brotli', '1.2.0-1', (CANDIDATE[1:],))
    glibc = {}
    for name in GLIBC_PINS:
        boundary('glibc_' + name)
        glibc[name] = sources.take(GLIBC_PACKAGE + name)[2]
    boundary('glibc_parse')
    package_binding(glibc['desc'], glibc['files'], 'glibc', GLIBC_VERSION,
                    (LIBM[1:], 'usr/lib/libc.so.6'))
    # All four original records are pinned, parsed and rechecked before either
    # newly scoped ELF may be opened. No global package index or third object.
    sources.recheck()
    sources.packages_validated = True
    boundary('tool_admission')
    tool_fd, tool_identity, _, _ = sources.take(READELF)
    queue, aliases, records = [(CANDIDATE, 0, None)], {}, {}
    steps = 0
    while queue:
        sources.available()
        boundary('queue_boundary')
        require(not base.UNSETTLED)
        path, depth, expected = queue.pop(0)
        steps += 1
        require(steps <= 128 and depth <= 8 and len(queue) <= 128)
        boundary('canonical_queue')
        resolved, links = helpers.canonical_public(path)
        boundary('queued_identity')
        require(path not in (CANDIDATE, LIBM) or (resolved == path and not links))
        require(expected is None or (resolved, links) == expected)
        require(resolved in (CANDIDATE, LIBM) or resolved in known)
        require(path not in aliases or aliases[path] == (resolved, links))
        aliases[path] = (resolved, links)
        if resolved in records:
            continue
        boundary('candidate_admission')
        fd, before, raw, _ = sources.take(resolved)
        boundary('elf_header')
        shape = header(raw)
        sources.recheck()  # ALL current original FDs before any tool execution.
        boundary('before_readelf')
        command = owned.command(base, [f'/proc/self/fd/{tool_fd}', '--wide', '--dynamic', '--program-headers',
                                f'/proc/self/fd/{fd}'], pass_fds=(tool_fd, fd),
                               env={'PATH':'/usr/bin', 'LANG':'C', 'LC_ALL':'C'})
        boundary('readelf_stderr')
        require(command.stderr == b'')
        boundary('readelf_decode')
        dynamic = helpers.decode_readelf(command.stdout)
        sources.recheck()
        boundary('canonical_recheck')
        require(helpers.canonical_public(path) == (resolved, links))
        dependencies = []
        for name in dynamic['needed']:
            boundary('dependency_resolve')
            logical, target, chain = helpers.resolve_needed(name)
            boundary('dependency_membership')
            require(target in known or target in (CANDIDATE, LIBM))  # No third unlisted object.
            dependencies.append(dict(name=name, logical_path=logical, resolved_path=target, links=chain))
            queue.append((logical, depth + 1, (target, chain)))
        if dynamic['interpreter']:
            boundary('interpreter_resolve')
            target, chain = helpers.canonical_public(dynamic['interpreter'])
            boundary('interpreter_membership')
            require(target in known or target in (CANDIDATE, LIBM))
            queue.append((dynamic['interpreter'], depth + 1, (target, chain)))
        records[resolved] = dict(path=resolved, identity=list(identity(before)), sha256=hashlib.sha256(raw).hexdigest(),
                                 header=shape, dynamic=dynamic, dependencies=dependencies,
                                 matches_reviewed_source=resolved in known)
    require(CANDIDATE in records and LIBM in records)
    boundary('final_aliases')
    for path, expected in aliases.items():
        require(helpers.canonical_public(path) == expected)
    boundary('final_edges')
    for row in records.values():
        for edge in row['dependencies']:
            require(aliases.get(edge['logical_path']) == (edge['resolved_path'], edge['links']))
    sources.recheck()
    boundary('receipt')
    require(len(sources.files) <= 24 and all(p in sources.files for p in PACKAGE_FILES))
    return dict(schema='fixed-encoder-libm-closure-v1', outcome='OBSERVED_STATIC_ONLY',
                candidate=CANDIDATE, manifest_sha256=PINS['copy-manifest.json'], readelf_sha256=READELF_SHA,
                package=dict(name='brotli', version='1.2.0-1', hashes=PACKAGE_PINS,
                             identities={name: list(identity(sources.files[PACKAGE + name][1])) for name in PACKAGE_PINS},
                             candidate_listed=True, global_package_owner_uniqueness_proven=False),
                glibc_package=dict(name='glibc', version=GLIBC_VERSION, hashes=GLIBC_PINS,
                    identities={name: list(identity(sources.files[GLIBC_PACKAGE+name][1])) for name in GLIBC_PINS},
                    libm_listed=True, known_libc_listed=True, global_package_owner_uniqueness_proven=False),
                explicit_additional_candidate=LIBM,
                readelf_identity=list(identity(tool_identity)),
                records=[records[p] for p in sorted(records)],
                aliases=[dict(path=p, resolved_path=row[0], links=row[1]) for p,row in sorted(aliases.items())],
                candidate_elf_executed=False, allowlist_adoption=False,
                loaded_elf_identity_proven=False, compatibility_acceptance=False)


def main():
    global EVENTS
    EVENTS = Events()
    boundary('entry')
    require(Path(__file__) == STAGE / 'probe.py' and sys.argv[1:] == ['--capture-fixed-encoder']
            and os.getuid() == os.geteuid() == 1000)
    base, helpers, owned, manifest = inputs()
    base.limits()
    result = capture(base, helpers, owned, manifest)
    boundary('output')
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    os.umask(0o077)
    try:
        main()
    except BaseException:
        try:
            if EVENTS is not None:
                EVENTS.failure()
        except BaseException:
            pass  # Unknown diagnostic write is terminal, never echo its context.
        raise SystemExit(1) from None
