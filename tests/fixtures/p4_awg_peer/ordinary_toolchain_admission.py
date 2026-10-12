"""Read-only admission of one signed-package-pinned ordinary Arch Go tree.

No CLI, subprocess, extraction, environment selection or execution capability.
The archive digest is independently signature-verified by ROOT, not supplied by
the tree being admitted. Trust is ordinary administrator/package custody; these
sequential observations do not attest an atomic tree against hostile root.
"""
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import stat
import struct
import tarfile
import time

ARCHIVE = Path('/var/cache/pacman/pkg/go-2:1.27.0-1-x86_64.pkg.tar.zst')
ARCHIVE_SHA256 = '232a5228e0305cd42a2ed2517806c78ff34ec9b0e8ba879a20914a8f7a31f5a9'
ARCHIVE_SIZE = 44314959
GO = Path('/usr/bin/go')
GOROOT = Path('/usr/lib/go')
GO_SHA256 = '3144268876ba974458f06ab523581fabac9e967855e2a1bb9de9ba950167b542'
SIGNER = '2E36D8620221482FC45CB7F2A91764759326B440'
MAX_MEMBERS = 50000
MAX_FILE = 64 * 1024 * 1024
MAX_TREE = 512 * 1024 * 1024
GO_SYMLINK = '/usr/lib/go/bin/go'
DOC_LINK = 'usr/lib/go/doc'
DOC_TARGET = '/usr/share/doc/go'
REQUIRED_NATIVE_TOOLS = ('bin/go', 'pkg/tool/linux_amd64/compile',
                         'pkg/tool/linux_amd64/asm', 'pkg/tool/linux_amd64/link',
                         'pkg/tool/linux_amd64/test2json', 'pkg/tool/linux_amd64/vet')


def identity(m):
    return (m.st_dev, m.st_ino, m.st_uid, m.st_gid, m.st_mode,
            m.st_nlink, m.st_size, m.st_mtime_ns, m.st_ctime_ns)


def root_ancestry(path):
    for parent in path.parents:
        m = parent.lstat()
        if not stat.S_ISDIR(m.st_mode) or m.st_uid != 0 or m.st_gid != 0 or m.st_mode & 0o7022:
            raise ValueError('root_owned_nonwritable_ancestry_required')


def root_bytes(path, maximum, allow_empty=False):
    root_ancestry(path)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        before = os.fstat(fd)
        if (not stat.S_ISREG(before.st_mode) or before.st_uid != 0 or before.st_gid != 0
                or before.st_mode & 0o7022 or before.st_nlink != 1
                or not (0 if allow_empty else 1) <= before.st_size <= maximum or os.listxattr(fd)):
            raise ValueError('root_owned_package_member_required')
        data = bytearray()
        while len(data) < before.st_size:
            chunk = os.read(fd, min(65536, before.st_size - len(data)))
            if not chunk:
                raise ValueError('package_member_short_read')
            data.extend(chunk)
        if os.read(fd, 1) or identity(before) != identity(os.fstat(fd)) or identity(before) != identity(path.lstat()):
            raise ValueError('package_member_drift')
        return bytes(data), before
    finally:
        os.close(fd)


def static_elf(raw):
    """Inspect actual executable bytes; never invoke ldd or the executable.

    Every admitted executable must be Linux x86-64 ELF64 little-endian with
    bounded complete program headers and no interpreter or DT_NEEDED entry.
    Dynamic dependencies are a new separately reviewed closure, not guessed.
    """
    if len(raw) < 64 or raw[:7] != b'\x7fELF\x02\x01\x01':
        raise ValueError('ordinary_static_elf_required')
    kind, machine, version = struct.unpack_from('<HHI', raw, 16)
    offset = struct.unpack_from('<Q', raw, 32)[0]
    ehsize, phsize, count = struct.unpack_from('<HHH', raw, 52)
    if kind not in (2, 3) or machine != 62 or version != 1 or ehsize != 64 or phsize != 56 or not 1 <= count <= 128 or offset < 64 or offset + count * phsize > len(raw):
        raise ValueError('ordinary_elf_headers_required')
    for i in range(count):
        ptype, _, start, _, _, size, _, _ = struct.unpack_from('<IIQQQQQQ', raw, offset + i * phsize)
        if start + size > len(raw) or ptype == 3:
            raise ValueError('ordinary_elf_interpreter_or_segment_refused')
        if ptype == 2:
            if size % 16:
                raise ValueError('ordinary_elf_dynamic_shape')
            terminated = False
            for at in range(start, start + size, 16):
                tag, _ = struct.unpack_from('<qQ', raw, at)
                if tag == 0:
                    terminated = True
                    break
                if tag == 1:
                    raise ValueError('ordinary_elf_shared_dependency_refused')
            if not terminated:
                raise ValueError('ordinary_elf_dynamic_unterminated')


def catalogue(tar_bytes):
    """Pure bounded catalogue of package bytes; no filesystem extraction."""
    if type(tar_bytes) is not bytes or not 0 < len(tar_bytes) <= MAX_TREE:
        raise ValueError('package_tar_bound')
    found, names, native, total, link = {}, set(), set(), 0, None
    with tarfile.open(fileobj=io.BytesIO(tar_bytes), mode='r:') as archive:
        for number, member in enumerate(archive):
            name = member.name.rstrip('/')
            path = PurePosixPath(name)
            if (number >= MAX_MEMBERS or not name or str(path) != name
                    or path.is_absolute() or '..' in path.parts or name in names):
                raise ValueError('package_catalogue_path_or_count')
            names.add(name)
            if name == 'usr/bin/go':
                if not member.issym() or member.uid != 0 or member.gid != 0:
                    raise ValueError('fixed_go_symlink_required')
                link = member.linkname
                continue
            if name != 'usr/lib/go' and not name.startswith('usr/lib/go/'):
                continue
            # The signed Arch package places documentation outside lexical
            # GOROOT. Attest this one link itself; never open/traverse its target.
            # No compiler/stdlib path receives a symlink exception.
            if name == DOC_LINK:
                if (not member.issym() or member.uid != 0 or member.gid != 0
                        or member.mode != 0o777 or member.linkname != DOC_TARGET):
                    raise ValueError('fixed_package_documentation_link')
                found[name] = {'kind': 'documentation_symlink', 'mode': 0o777,
                               'target': DOC_TARGET}
                continue
            if member.uid != 0 or member.gid != 0 or member.mode & 0o7022:
                raise ValueError('package_tree_metadata')
            if member.isdir():
                record = {'kind': 'directory', 'mode': member.mode}
            elif member.isfile() and 0 <= member.size <= MAX_FILE:
                total += member.size
                if total > MAX_TREE:
                    raise ValueError('package_tree_byte_bound')
                stream = archive.extractfile(member)
                with stream:
                    raw = stream.read(member.size + 1)
                if len(raw) != member.size:
                    raise ValueError('package_payload_size')
                if raw.startswith(b'\x7fELF') and member.mode & 0o111:
                    static_elf(raw)
                    native.add(name.removeprefix('usr/lib/go/'))
                record = {'kind': 'file', 'mode': member.mode, 'size': member.size,
                          'sha256': hashlib.sha256(raw).hexdigest()}
            else:
                raise ValueError('package_tree_type')
            found[name] = record
    if (link != GO_SYMLINK
            or 'usr/lib/go' not in found or 'usr/lib/go/bin/go' not in found
            or found['usr/lib/go/bin/go'].get('sha256') != GO_SHA256):
        raise ValueError('fixed_package_go_identity')
    if not set(REQUIRED_NATIVE_TOOLS) <= native:
        raise ValueError('required_native_tool_missing')
    if found.get(DOC_LINK) != {'kind': 'documentation_symlink', 'mode': 0o777, 'target': DOC_TARGET}:
        raise ValueError('fixed_package_documentation_link_missing')
    # Explicit hierarchy prevents omitted directory metadata from being inferred.
    for name in found:
        for parent in PurePosixPath(name).parents:
            if str(parent) == 'usr/lib':
                break
            if found.get(str(parent), {}).get('kind') != 'directory':
                raise ValueError('package_tree_parent_missing')
    return found, link


def admit():
    """Observe the fixed installed tree against independently pinned package."""
    started = time.monotonic()
    def budget():
        if time.monotonic() - started > 60:
            raise ValueError('toolchain_observation_budget')
    package, metadata = root_bytes(ARCHIVE, ARCHIVE_SIZE)
    if metadata.st_size != ARCHIVE_SIZE or stat.S_IMODE(metadata.st_mode) != 0o644 or hashlib.sha256(package).hexdigest() != ARCHIVE_SHA256:
        raise ValueError('independent_signed_package_pin')
    # Python 3.14 standard-library decoder; never run an archive utility.
    from compression import zstd
    with zstd.ZstdFile(io.BytesIO(package)) as stream:
        unpacked = stream.read(MAX_TREE + 1)
    wanted, target = catalogue(unpacked)
    root_ancestry(GO)
    before_link = GO.lstat()
    if (not stat.S_ISLNK(before_link.st_mode) or before_link.st_uid != 0
            or before_link.st_gid != 0 or before_link.st_nlink != 1
            or os.listxattr(GO, follow_symlinks=False) or os.readlink(GO) != target):
        raise ValueError('installed_fixed_go_symlink')
    actual, directory_records, documentation_links = {}, {}, {}
    pending = [GOROOT]
    total = 0
    while pending:
        budget()
        path = pending.pop()
        name = path.as_posix().lstrip('/')
        if len(actual) >= MAX_MEMBERS or name not in wanted:
            raise ValueError('installed_tree_membership')
        m = path.lstat()
        record = wanted[name]
        root_ancestry(path)
        if record['kind'] == 'documentation_symlink':
            if (name != DOC_LINK or record != {'kind': 'documentation_symlink', 'mode': 0o777, 'target': DOC_TARGET}
                    or not stat.S_ISLNK(m.st_mode) or m.st_uid != 0 or m.st_gid != 0
                    or stat.S_IMODE(m.st_mode) != 0o777 or m.st_nlink != 1
                    or os.listxattr(path, follow_symlinks=False) or os.readlink(path) != DOC_TARGET):
                raise ValueError('installed_fixed_documentation_link')
            documentation_links[path] = identity(m)
            actual[name] = record
            continue  # No target ancestry, enumeration, content read or adoption.
        if (m.st_uid != 0 or m.st_gid != 0 or m.st_mode & 0o7022
                or stat.S_IMODE(m.st_mode) != record['mode'] or os.listxattr(path, follow_symlinks=False)):
            raise ValueError('installed_tree_metadata')
        if record['kind'] == 'directory' and stat.S_ISDIR(m.st_mode):
            children = tuple(sorted(path.iterdir()))
            directory_records[path] = (identity(m), children)
            pending.extend(children)
            actual[name] = record
        elif record['kind'] == 'file' and stat.S_ISREG(m.st_mode):
            raw, _ = root_bytes(path, MAX_FILE, allow_empty=True)
            total += len(raw)
            if total > MAX_TREE or len(raw) != record['size'] or hashlib.sha256(raw).hexdigest() != record['sha256']:
                raise ValueError('installed_package_bytes')
            if raw.startswith(b'\x7fELF') and record['mode'] & 0o111:
                static_elf(raw)
            actual[name] = record
        else:
            raise ValueError('installed_tree_type')
    if actual != wanted:
        raise ValueError('installed_tree_incomplete')
    for path, (metadata, children) in directory_records.items():
        budget()
        if identity(path.lstat()) != metadata or tuple(sorted(path.iterdir())) != children:
            raise ValueError('installed_tree_directory_drift')
    for path, metadata in documentation_links.items():
        budget()
        if identity(path.lstat()) != metadata or os.readlink(path) != DOC_TARGET:
            raise ValueError('installed_documentation_link_drift')
    if identity(GO.lstat()) != identity(before_link) or os.readlink(GO) != target:
        raise ValueError('installed_go_link_drift')
    budget()
    return {'schema': 'p4-ordinary-arch-go-admission-v2', 'package_sha256': ARCHIVE_SHA256,
            'package_signer_fingerprint': SIGNER, 'go_sha256': GO_SHA256,
            'tree_sha256': hashlib.sha256(json.dumps(wanted, sort_keys=True, separators=(',', ':')).encode()).hexdigest(),
            'members': len(wanted), 'go_symlink_target': target,
            'goroot': str(GOROOT), 'static_executable_closure': True,
            'tree_scope': 'lexical-goroot', 'documentation_target_traversed': False,
            'administrator_package_trust': True, 'hostile_root_atomic_attestation': False}
