#!/usr/bin/python3
"""Six fixed public candidates: static data, never candidate executables."""
import hashlib
import json
import os
import re
from pathlib import Path
import stat
import struct
import sys
import time
import types

STAGE = Path('/home/kdk_vm/.cache/t3-six-library-exact-package-review-1')
CANDIDATES = (
    '/usr/lib/libcrypto.so.3', '/usr/lib/libidn2.so.0.4.0',
    '/usr/lib/libssl.so.3', '/usr/lib/libunistring.so.5.2.1',
    '/usr/lib/libz.so.1.3.2', '/usr/lib/libzstd.so.1.5.7')
PACKAGES = {
    'openssl': (CANDIDATES[0], CANDIDATES[2]),
    'libidn2': (CANDIDATES[1],), 'libunistring': (CANDIDATES[3],),
    'zlib': (CANDIDATES[4],), 'zstd': (CANDIDATES[5],)}
CATALOG = Path('/var/lib/pacman/local')
PINS = {'containment.py': '2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592',
        'owned.py': 'fe33819270686b54fff5608cbc4db1a6dc7252f3e884c7d0b4e3f9cad123769a',
        'helpers.py': 'cccc171213f4631f54d906652aeaf7954230949a7de40b2093c8ab587f86aa00',
        'copy-manifest.json': 'b914dece6cb3c58f74bb4cdea8b19ade7b3b032e1b12d112d7524a2c47ca6c87'}
# Catalog names only are measured. No package version/hash is guessed.
def directory_version(name, package):
    require(type(name) is str and type(package) is str and package in PACKAGES
            and name.startswith(package + '-'))
    # libalpm splits local NAME-VERSION at the last two hyphens: NAME may
    # contain hyphens, pkgver may not. Never confuse openssl-1.1 with openssl.
    parts = name.rsplit('-', 2)
    require(len(parts) == 3 and parts[0] == package)
    version = parts[1] + '-' + parts[2]
    require(re.fullmatch(r'(?:[0-9]+:)?[A-Za-z0-9_+.~]+-[0-9]+(?:\.[0-9]+)*', version)
            and len(version) <= 160)
    return version

READELF = '/usr/bin/readelf'
READELF_SHA = 'a72f12f3dd8c560554a3ba818bcecbda9178db5befbcc64d91bbb6a241fc21bc'
FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
PHASES = frozenset(('entry', 'input_ancestry', 'input_open', 'input_shape',
    'input_read', 'input_pin', 'input_load', 'manifest_boundary', 'package_desc',
    'package_files', 'tool_admission', 'queue_boundary',
    'canonical_queue', 'queued_identity', 'candidate_admission', 'elf_header',
    'before_readelf', 'readelf_stderr', 'readelf_decode', 'canonical_recheck',
    'dependency_resolve', 'dependency_membership', 'interpreter_resolve',
    'interpreter_membership', 'final_aliases', 'final_edges', 'receipt',
    'source_scope', 'parent_open', 'parent_identity',
    'source_open', 'source_shape', 'source_read', 'source_hash',
    'source_path', 'source_pin', 'sources_recheck', 'recheck_file',
    'recheck_parent', 'output', 'catalog_open', 'catalog_names', 'catalog_recheck', 'package_membership'))
CATALOG_PHASES = frozenset(('catalog_iterator_open',
    'catalog_next_entry', 'catalog_entry_name', 'catalog_name_type',
    'catalog_name_cap', 'catalog_name_shape', 'catalog_name_duplicate',
    'catalog_iterator_close', 'catalog_sort'))
EXCEPTION_CLASSES = {
    PermissionError: 'PermissionError', FileNotFoundError: 'FileNotFoundError',
    NotADirectoryError: 'NotADirectoryError', OSError: 'OSError',
    RuntimeError: 'RuntimeError', TypeError: 'TypeError', ValueError: 'ValueError',
    OverflowError: 'OverflowError', MemoryError: 'MemoryError',
}
EVENTS = None  # Inert imports have no observer; fixed main installs one.
FINAL_SCOPE = None


class Events:
    def __init__(self):
        self.sealed = False
        self.reported = False
        self.last = 'entry'
        self.count = self.total = 0
        self.catalog_phase = None

    def catalog_before(self, phase):
        # Private finite latch only: avoid multiplying the existing event cap
        # for every entry in the unchanged 4096-name catalog.
        require(not self.sealed and type(phase) is str and phase in CATALOG_PHASES)
        self.catalog_phase = phase

    def before(self, phase):
        if self.sealed:
            raise RuntimeError('events_sealed')
        try:
            require(phase in PHASES and type(phase) is str)
            raw = ('T3_SIX_LIBRARY_STATIC_BEFORE_V1 ' + phase + '\n').encode('ascii')
            require(self.count < 4096 and self.total + len(raw) <= 130816)
            self.last = phase
            self.count += 1
            self.total += len(raw)
            written = os.write(2, raw)
            require(type(written) is int and written == len(raw))
        except BaseException:
            self.sealed = True
            self.reported = True  # A failed event write is never retried.
            raise

    def failure(self, error=None):
        # Authorized finite output only, no post-failure observation or cleanup.
        self.sealed = True
        if self.reported:
            return
        self.reported = True
        raw = ('T3_SIX_LIBRARY_STATIC_FAILED_AT_V1 ' + self.last + '\n').encode('ascii')
        if error is not None and self.last == 'catalog_names' and self.catalog_phase is not None:
            # Only a literal category, never exception text, repr or class name.
            category = EXCEPTION_CLASSES.get(type(error), 'OtherBaseException')
            require(self.catalog_phase in CATALOG_PHASES)
            raw += ('T3_SIX_LIBRARY_STATIC_CATALOG_FAILED_AT_V1 ' + self.catalog_phase + '\n').encode('ascii')
            raw += ('T3_SIX_LIBRARY_STATIC_EXCEPTION_V1 ' + category + '\n').encode('ascii')
        require(len(raw) <= 256)
        if FINAL_SCOPE is not None:
            require(time.monotonic() < FINAL_SCOPE.deadline)
        written = os.write(2, raw)
        require(type(written) is int and written == len(raw))
        if FINAL_SCOPE is not None:
            require(time.monotonic() < FINAL_SCOPE.deadline)


def boundary(phase):
    if EVENTS is not None:
        EVENTS.before(phase)


def catalog_boundary(phase):
    if EVENTS is not None:
        EVENTS.catalog_before(phase)


def require(value):
    if not value:
        raise RuntimeError('six_library_provenance_refused')


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
        self.package_files, self.catalog = {}, None

    def available(self):
        try:
            require(self.state == 'ready' and time.monotonic() < self.deadline)
        except BaseException:
            self.state = 'refused'
            raise

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

    def names(self, fd):
        self.available()
        boundary('catalog_names')
        names = []
        try:
            catalog_boundary('catalog_iterator_open')
            self.available()
            with os.scandir(fd) as entries:
                while True:
                    catalog_boundary('catalog_next_entry')
                    self.available()
                    try:
                        entry = next(entries)
                    except StopIteration:
                        break
                    catalog_boundary('catalog_entry_name')
                    self.available()
                    name = entry.name
                    catalog_boundary('catalog_name_type')
                    require(type(name) is str)
                    catalog_boundary('catalog_name_cap')
                    require(len(names) < 4096)
                    catalog_boundary('catalog_name_shape')
                    require(re.fullmatch(r'[A-Za-z0-9_+.@~:-]{1,320}', name)
                            and name not in ('.', '..'))
                    catalog_boundary('catalog_name_duplicate')
                    require(name not in names)
                    names.append(name)
                catalog_boundary('catalog_iterator_close')
            catalog_boundary('catalog_sort')
            self.available()
            return tuple(sorted(names))
        except BaseException:
            self.state = 'refused'
            raise

    def select_packages(self):
        self.available()
        try:
            require(self.catalog is None and not self.files and not self.package_files)
            parent = self.parent(CATALOG / 'catalog-name-only')
            # parent() retains CATALOG itself as an original directory FD.
            fd, before, _, _ = self.parents[CATALOG]
            boundary('catalog_open')
            require(parent == fd and not os.listxattr(fd))
            names = self.names(fd)
            selected = {}
            for package in PACKAGES:
                matches = [n for n in names if n.rsplit('-', 2)[0] == package]
                require(len(matches) == 1)
                selected[package] = (matches[0], directory_version(matches[0], package))
            self.catalog = (fd, before, names)
            self.package_files = {str(CATALOG / directory / leaf): (package, leaf)
                                  for package, (directory, _) in selected.items()
                                  for leaf in ('desc', 'files')}
            self.recheck_catalog()
            return selected
        except BaseException:
            self.state = 'refused'
            raise

    def recheck_catalog(self):
        self.available()
        try:
            boundary('catalog_recheck')
            fd, before, names = self.catalog
            _, _, parent, name = self.parents[CATALOG]
            require(self.names(fd) == names and not os.listxattr(fd)
                    and identity(before) == identity(os.fstat(fd))
                    == identity(os.stat(name, dir_fd=parent, follow_symlinks=False)))
        except BaseException:
            self.state = 'refused'
            raise

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
            require(path in self.known or path in (*CANDIDATES, READELF) or path in self.package_files)
            require(path not in CANDIDATES or self.packages_validated)
            if path in self.files:
                return self.files[path]
            require(len(self.files) < 36)
            p = Path(path)
            parent = self.parent(p)
            boundary('source_open')
            fd = os.open(p.name, FLAGS, dir_fd=parent)
            self.files[path] = (fd, None, None, parent)
            boundary('source_shape')
            before = os.fstat(fd)
            maximum = 4 * 1024 * 1024 if path in self.package_files else 32 * 1024 * 1024
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
            self.files[path] = (fd, before, data, parent)
            return self.files[path]
        except BaseException:
            self.state = 'refused'
            raise

    def recheck(self):
        self.available()
        try:
            boundary('sources_recheck')
            if self.catalog is not None:
                self.recheck_catalog()
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
        if FINAL_SCOPE is not None:
            FINAL_SCOPE.state = 'refused'
        if EVENTS is not None:
            EVENTS.sealed = True
        raise


def capture_inner(base, helpers, owned, manifest):
    global FINAL_SCOPE
    boundary('manifest_boundary')
    known = manifest['source_provenance']
    require(len(known) == 19 and all(p not in known for p in CANDIDATES))
    sources = Sources(known)
    FINAL_SCOPE = sources
    selected = sources.select_packages()
    packages = []
    # Ten finite selected records; exact names/versions/memberships before ELF opens.
    for name, (directory, version) in selected.items():
        raw = {}
        for leaf in ('desc', 'files'):
            boundary('package_' + leaf)
            raw[leaf] = sources.take(str(CATALOG / directory / leaf))[2]
        boundary('package_membership')
        package_binding(raw['desc'], raw['files'], name, version,
                        tuple(p[1:] for p in PACKAGES[name]))
        packages.append(dict(name=name, version=version, directory=directory,
            hashes={leaf: hashlib.sha256(raw[leaf]).hexdigest() for leaf in raw},
            identities={leaf: list(identity(sources.files[str(CATALOG/directory/leaf)][1])) for leaf in raw},
            listed_candidates=list(PACKAGES[name]),
            global_package_owner_uniqueness_proven=False, package_signature_verified=False))
    sources.recheck()
    sources.packages_validated = True
    boundary('tool_admission')
    tool_fd, tool_identity, _, _ = sources.take(READELF)
    queue, aliases, records = [(p, 0, None) for p in CANDIDATES], {}, {}
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
        require(path not in CANDIDATES or (resolved == path and not links))
        require(expected is None or (resolved, links) == expected)
        require(resolved in CANDIDATES or resolved in known)
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
        sources.available()  # Final event/filesystem work cannot consume the next spawn budget.
        command = owned.command(base, [f'/proc/self/fd/{tool_fd}', '--wide', '--dynamic', '--program-headers',
                                f'/proc/self/fd/{fd}'], pass_fds=(tool_fd, fd),
                               env={'PATH':'/usr/bin', 'LANG':'C', 'LC_ALL':'C'},
                               deadline=sources.deadline)
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
            require(target in known or target in CANDIDATES)  # No seventh unlisted object.
            dependencies.append(dict(name=name, logical_path=logical, resolved_path=target, links=chain))
            queue.append((logical, depth + 1, (target, chain)))
        if dynamic['interpreter']:
            boundary('interpreter_resolve')
            target, chain = helpers.canonical_public(dynamic['interpreter'])
            boundary('interpreter_membership')
            require(target in known or target in CANDIDATES)
            queue.append((dynamic['interpreter'], depth + 1, (target, chain)))
        records[resolved] = dict(path=resolved, identity=list(identity(before)), sha256=hashlib.sha256(raw).hexdigest(),
                                 header=shape, dynamic=dynamic, dependencies=dependencies,
                                 matches_reviewed_source=resolved in known)
    require(all(p in records for p in CANDIDATES))
    boundary('final_aliases')
    for path, expected in aliases.items():
        require(helpers.canonical_public(path) == expected)
    boundary('final_edges')
    for row in records.values():
        for edge in row['dependencies']:
            require(aliases.get(edge['logical_path']) == (edge['resolved_path'], edge['links']))
    sources.recheck()
    boundary('receipt')
    require(len(sources.files) <= 36 and all(p in sources.files for p in sources.package_files))
    fd, before, names = sources.catalog
    return dict(schema='fixed-six-library-static-closure-v1', outcome='OBSERVED_STATIC_ONLY',
                candidates=list(CANDIDATES), manifest_sha256=PINS['copy-manifest.json'],
                readelf_sha256=READELF_SHA, packages=packages,
                catalog=dict(name_count=len(names),
                             names_sha256=hashlib.sha256(('\n'.join(names)+'\n').encode('ascii')).hexdigest(),
                             identity=list(identity(before))),
                readelf_identity=list(identity(tool_identity)),
                records=[records[p] for p in sorted(records)],
                aliases=[dict(path=p, resolved_path=row[0], links=row[1]) for p,row in sorted(aliases.items())],
                candidate_elf_executed=False, allowlist_adoption=False,
                loaded_elf_identity_proven=False, compatibility_acceptance=False)


def emit_result(result, sources):
    """Typed final write/flush/deadline result is a permanent scope disposition."""
    try:
        boundary('output')
        raw = (json.dumps(result, sort_keys=True) + '\n').encode('ascii')
        require(len(raw) <= 262144)
        sources.available()
        written = sys.stdout.buffer.write(raw)
        require(type(written) is int and written == len(raw))
        sources.available()
        sys.stdout.buffer.flush()
        sources.available()
        sources.state = 'sealed'
        if EVENTS is not None:
            EVENTS.sealed = True
    except BaseException:
        sources.state = 'refused'
        if EVENTS is not None:
            EVENTS.sealed = True
        raise


def main():
    global EVENTS
    EVENTS = Events()
    boundary('entry')
    require(Path(__file__) == STAGE / 'probe.py' and sys.argv[1:] == ['--capture-fixed-six-libraries']
            and os.getuid() == os.geteuid() == 1000)
    base, helpers, owned, manifest = inputs()
    base.limits()
    result = capture(base, helpers, owned, manifest)
    emit_result(result, FINAL_SCOPE)


if __name__ == '__main__':
    os.umask(0o077)
    try:
        main()
    except BaseException as error:
        try:
            if EVENTS is not None:
                EVENTS.failure(error)
        except BaseException:
            pass  # Unknown diagnostic write is terminal, never echo its context.
        if FINAL_SCOPE is not None:
            FINAL_SCOPE.state = 'refused'
        raise SystemExit(1) from None
