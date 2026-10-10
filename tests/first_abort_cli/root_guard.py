#!/usr/bin/python3
"""Fixed disposable UID normal-CLI fixture. Never installed or production IPC."""
import errno
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import types
import time

ROOT = Path('/run/ov-t4-cli-guard-v8')
UID = 48049
NAME = 'ov-t4-abort-v6'
HOME = Path('/home/ov-t4-abort-v6')
RUNTIME = Path('/run/user/48049')
ARTIFACTS = HOME / '.t4-first-abort'
NATIVE_HEAD = 'bf650713ed6daa066b38bbe8c1ed608f8a36534c'
LIMIT = 8 * 1024 * 1024
ELF_LIMIT = 512 * 1024 * 1024
FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
ENV = {'HOME': str(HOME), 'PATH': '/usr/bin', 'LC_ALL': 'C', 'XDG_RUNTIME_DIR': str(RUNTIME)}
ROOT_ENV = {'HOME': '/root', 'PATH': '/usr/bin', 'LC_ALL': 'C'}
PRIMARY_ENV = {'HOME': '/home/kdk_vm', 'PATH': '/usr/bin', 'LC_ALL': 'C', 'XDG_RUNTIME_DIR': '/run/user/1000'}
PINNED = {
    'core.py': '3631492302b3e87beb0b2a801ded9ee5df1235a5c125117b0f4728596067049d',
    'support.py': '100428aae8271c9489454b5f0384ba3dc75287938c5b586c6f71ec3de8da286c',
    'startup_inventory.py': '39a6213751f334d13b510e64009b9420c48c7606ffbb5a1d1d1c6d7622fb20b8',
    'startup_followup.py': 'a94dc862698a04b61faa5ee4778edbfe1aa550cf411f97293ae86b90964140cf',
}
CODE = (*PINNED, 'lineage.py', 'root_guard.py')
CATALOGS = (
    ('startup_inventory.py', '/run/ov-t4-user-startup-inventory-v1',
     '8232be39e366bdc0540e42fe8988b90256a5c29851dd3fb2a4303eb065bc7361'),
    ('startup_followup.py', '/run/ov-t4-user-startup-followup-v1',
     '6e81df4ed1a1fd7aedb536eda1804ff5d480f1849da7cdbc090c678cef584921'),
)
ENTRIES = {'setup': 'setup_mixed_intent', 'launch': 'launch_normal_cli',
           'first': 'verify_first_abort', 'reentry': 'verify_abort_reentry',
           'diagnose': 'diagnose_stopped_admission'}
PREFIX = 'production_owner::first_abort::cli_vm_fixture::'
ADMISSION_PHASES = ('current_paths', 'existing_lock', 'proc_root', 'self_process',
    'self_arguments', 'proc_visibility', 'namespace_handles', 'manager_query',
    'manager_record', 'manager_process', 'manager_executable', 'manager_identity',
    'namespace_boundary', 'proc_visibility', 'manager_recheck', 'manager_query',
    'manager_record', 'manager_identity_recheck', 'legacy_unit_query', 'legacy_unit_record',
    'runtime_unit_query', 'runtime_unit_record', 'inventory', 'unix_table', 'unix_parser',
    'final_manager', 'final_self', 'final_namespaces', 'final_proc_visibility',
    'final_budget', 'final_lock')


def admission_trace(raw):
    # Only after exact known-zero child completion. Never parse failure into success.
    require(type(raw) is bytes and len(raw) <= 16384)
    expected = b''.join(('T4_STOPPED_BEFORE_V1 ' + phase + '\n').encode('ascii')
                        for phase in ADMISSION_PHASES)
    require(raw == expected + b'T4_STOPPED_READONLY_OBSERVATION_NOT_ADMISSION\n')

EXTRA_ROOT_UNITS = (
    'omavless-k1-typed-order-filter-fixture.service',
    'omavless-k1-manager-private-lifecycle.service',
    'omavless-k1-effective-config-reference.service',
    'omavless-k1-versioned-config-reference.service',
    'omavless-k1-configured-reference-phase.service',
    'omavless-k1-retained-activation-reference.service',
    'omavless-k1-private-lifecycle-admission.service',
)
PHASE = 'admission'
RETAINED = []
core = None
DEADLINE = float('inf')
TERMINAL = False


class Refused(Exception):
    pass


def require(value):
    if not value:
        if core is not None:
            core.UNCERTAIN = True
        raise Refused()


def available():
    require(time.monotonic() < DEADLINE and (core is None or not core.UNCERTAIN))


def await_allowed(child, seconds, allowed):
    """First disallowed WNOWAIT result seals before any reap or other query."""
    try:
        available()
        require(type(allowed) is tuple and all(type(code) is int for code in allowed)
                and allowed in ((0,), (2,), (0, 1))
                and type(child.pid) is int and child.pid > 0)
        deadline = min(DEADLINE, time.monotonic() + seconds)
    except BaseException:
        core.quarantine(child)  # Even late entry retains the exact spawned owner.
    def owned_gate():
        if time.monotonic() >= deadline or core.UNCERTAIN:
            core.quarantine(child)
    try:
        if child.returncode is not None:
            core.quarantine(child)
        while time.monotonic() < deadline:
            owned_gate()
            seen = os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
            owned_gate()
            if seen is None:
                time.sleep(0.02)
                continue
            if not (type(seen.si_pid) is int and seen.si_pid == child.pid
                    and type(seen.si_code) is int and seen.si_code == os.CLD_EXITED
                    and type(seen.si_status) is int and seen.si_status in allowed):
                core.quarantine(child)
            owned_gate()
            pid, status = os.waitpid(child.pid, os.WNOHANG)
            if not (type(pid) is int and type(status) is int and pid == child.pid
                    and status == seen.si_status << 8):
                core.quarantine(child)
            owned_gate()
            child.returncode = seen.si_status
            return seen.si_status
        core.quarantine(child)
    except BaseException:
        if not core.UNCERTAIN:
            core.quarantine(child)
        raise


def pairs(items):
    out = {}
    for k, v in items:
        require(k not in out)
        out[k] = v
    return out


def decode(data):
    return json.loads(data, object_pairs_hook=pairs,
                      parse_constant=lambda _: (_ for _ in ()).throw(Refused()))


def identity(m):
    return (m.st_dev, m.st_ino, m.st_mode, m.st_uid, m.st_gid,
            m.st_nlink, m.st_size, m.st_mtime_ns, m.st_ctime_ns)


def dir_identity(m):
    return (m.st_dev, m.st_ino, m.st_mode, m.st_uid, m.st_gid)


class Parents:
    def __init__(self, path, uid):
        self.rows = []
        for path in reversed((path, *path.parents)):
            available()
            before = path.lstat()
            require(stat.S_ISDIR(before.st_mode) and before.st_uid in (0, uid)
                    and before.st_mode & 0o6022 == 0)
            fd = os.open(path, FLAGS | os.O_DIRECTORY)
            RETAINED.append(fd)
            require(dir_identity(before) == dir_identity(os.fstat(fd)))
            try:
                os.stat('.git', dir_fd=fd, follow_symlinks=False)
            except FileNotFoundError:
                pass
            else:
                require(False)
            self.rows.append((path, fd, before))
        self.recheck()

    def recheck(self):
        available()
        for path, fd, before in self.rows:
            require(dir_identity(before) == dir_identity(os.fstat(fd)) == dir_identity(path.lstat()))


class File:
    def __init__(self, path, uid, mode, maximum, expected=None):
        available()
        if type(mode) is tuple:
            # Only this fixed root-owned OS defaults file admits both safe
            # installed permission variants; all other file gates stay exact.
            require(path == Path('/etc/default/useradd') and type(uid) is int and uid == 0
                    and type(maximum) is int and maximum == 65536 and expected is None
                    and len(mode) == 2 and all(type(value) is int for value in mode)
                    and mode == (0o600, 0o644))
            modes = mode
        else:
            require(type(mode) is int)
            modes = (mode,)
        self.path, self.parents = path, Parents(path.parent, uid)
        self.fd = os.open(path.name, FLAGS, dir_fd=self.parents.rows[-1][1])
        RETAINED.append(self.fd)
        self.before = os.fstat(self.fd)
        require(stat.S_ISREG(self.before.st_mode) and self.before.st_uid == self.before.st_gid == uid
                and stat.S_IMODE(self.before.st_mode) in modes and self.before.st_nlink == 1
                and 0 < self.before.st_size <= maximum and not os.listxattr(self.fd))
        self.sha = self.hash()
        require(expected is None or expected == self.sha)
        self.recheck()

    def hash(self):
        result, offset = hashlib.sha256(), 0
        while offset < self.before.st_size:
            available()
            data = os.pread(self.fd, min(65536, self.before.st_size - offset), offset)
            require(data)
            result.update(data)
            offset += len(data)
        require(os.pread(self.fd, 1, offset) == b'')
        return result.hexdigest()

    def recheck(self):
        available()
        self.parents.recheck()
        require(identity(self.before) == identity(os.fstat(self.fd)) == identity(self.path.lstat())
                and not os.listxattr(self.fd) and self.hash() == self.sha)
        require(identity(self.before) == identity(os.fstat(self.fd)) == identity(self.path.lstat()))

    def bytes(self):
        self.recheck()
        require(self.before.st_size <= 48 * 1024 * 1024)
        data = os.pread(self.fd, self.before.st_size + 1, 0)
        require(len(data) == self.before.st_size)
        self.recheck()
        return data


def module(pin):
    loaded = types.ModuleType('sealed_' + pin.path.stem)
    loaded.__file__ = str(pin.path)
    exec(compile(pin.bytes(), '<reviewed-fixed-fixture>', 'exec'), loaded.__dict__)
    return loaded  # None of the copied modules' main functions run.


def receipt(data):
    value = decode(data)
    require(type(value) is dict and set(value) == {'schema', 'native_head', 'guard_head', 'code', 'elfs'})
    require(value['schema'] == 't4-disposable-cli-delivery-v8' and value['native_head'] == NATIVE_HEAD
            and type(value['guard_head']) is str and re.fullmatch('[0-9a-f]{40}', value['guard_head']))
    require(type(value['code']) is dict and set(value['code']) == set(CODE)
            and type(value['elfs']) is dict and set(value['elfs']) == {'helper', 'omavless'})
    for name, digest in value['code'].items():
        require(type(digest) is str and re.fullmatch('[0-9a-f]{64}', digest)
                and (name not in PINNED or digest == PINNED[name]))
    for name, row in value['elfs'].items():
        require(type(row) is dict and set(row) == {'sha256', 'size', 'host_original', 'host_frozen', 'host_alias'})
        require(type(row['sha256']) is str and re.fullmatch('[0-9a-f]{64}', row['sha256'])
                and type(row['size']) is int and 0 < row['size'] <= ELF_LIMIT)
        for key in ('host_original', 'host_frozen'):
            meta = row[key]
            require(type(meta) is list and len(meta) == 9
                    and all(type(n) is int and 0 <= n < 2**64 for n in meta)
                    and stat.S_ISREG(meta[2]) and stat.S_IMODE(meta[2]) == (0o755 if key == 'host_original' else 0o500)
                    and meta[3] == meta[4] == 1000
                    and meta[5] == (2 if key == 'host_original' and name == 'omavless' else 1)
                    and meta[6] == row['size'])
        alias = row['host_alias']
        if name == 'helper':
            require(alias is None)
        else:
            require(type(alias) is dict and set(alias) == {'relative_path', 'identity', 'sha256'}
                    and alias['relative_path'] == 'debug/deps/omavless-3eaa741bede04cf2'
                    and type(alias['identity']) is list and len(alias['identity']) == 9
                    and all(type(n) is int for n in alias['identity'])
                    and alias['identity'] == row['host_original'] and alias['sha256'] == row['sha256'])
    return value


def fields(raw, expected):
    require(len(raw) <= 65536)
    result = pairs(line.split('=', 1) for line in raw.decode('utf-8').splitlines())
    require(set(result) == set(expected))
    return result


def mail_spool_disabled(raw):
    """Conservative /etc/default/useradd grammar; never login.defs -K."""
    require(type(raw) is bytes and 0 < len(raw) <= 65536
            and b'\0' not in raw and b'\r' not in raw)
    seen = 0
    for line in raw.split(b'\n'):
        require(len(line) <= 1024)
        if line.startswith(b'#') or not line:
            continue
        if b'CREATE_MAIL_SPOOL' in line:
            require(line == b'CREATE_MAIL_SPOOL=no')
            seen += 1
    require(seen == 1)


def reserve_elf_slots(pins):
    # Catalog admission retains hundreds of FDs. Reserve before that allocation,
    # using our original root ELF FDs, never an unrelated occupied descriptor.
    for number, name in ((198, 'helper'), (199, 'omavless')):
        available()
        try:
            os.fstat(number)
        except OSError as error:
            require(error.errno == errno.EBADF)
        else:
            require(False)
        pins[name].recheck()
        available()
        os.dup2(pins[name].fd, number, inheritable=False)
        require(identity(os.fstat(number)) == identity(pins[name].before))
        RETAINED.append(number)


def complete_publication(writer, original, name, source):
    """Transfer this completed created inode to readonly retention before exec."""
    available()
    require(name in ('helper', 'omavless') and writer in RETAINED)
    readonly = File(ARTIFACTS / name, UID, 0o500, ELF_LIMIT, source.sha)
    require(identity(original) == identity(os.fstat(writer)) == identity(readonly.before))
    readonly.recheck()
    source.recheck()
    available()
    os.fsync(writer)
    available()
    require(identity(original) == identity(os.fstat(writer))
            == identity(os.fstat(readonly.fd)) == identity(readonly.path.lstat())
            and not os.listxattr(writer))
    # A known successful close is the sole permitted release. A close error is
    # terminal, never retried; fchmod0500 alone does not clear FMODE_WRITE.
    available()
    os.close(writer)
    available()
    RETAINED.remove(writer)
    return readonly


class DeadlineEvidence:
    """This guard's fixed evidence sink; no change to historical support code."""
    def __init__(self, stage, support):
        stage.recheck()
        available()
        os.mkdir('evidence', mode=0o700, dir_fd=stage.fd)
        available()
        self.directory = support.Directory(stage.path / 'evidence', 0)
        self.count = 0

    def create(self, name):
        available()
        require(type(name) is str and re.fullmatch(r'[a-zA-Z0-9_.-]+', name) is not None)
        self.directory.recheck()
        available()
        fd = os.open(name, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                     0o600, dir_fd=self.directory.fd)
        RETAINED.append(fd)
        available()
        m = os.fstat(fd)
        require(stat.S_ISREG(m.st_mode) and m.st_uid == m.st_gid == 0
                and stat.S_IMODE(m.st_mode) == 0o600 and m.st_nlink == 1)
        return fd

    def write(self, name, value):
        fd = self.create(name)
        data = json.dumps(value, sort_keys=True).encode()
        require(len(data) <= LIMIT)
        available()
        count = os.write(fd, data)
        require(type(count) is int and count == len(data))
        available()
        os.fsync(fd)
        available()
        os.fsync(self.directory.fd)
        available()


def copy_elf(pin, fd):
    offset = 0
    while offset < pin.before.st_size:
        available()
        data = os.pread(pin.fd, min(65536, pin.before.st_size - offset), offset)
        require(data)
        available()
        written = os.write(fd, data)
        require(type(written) is int and written == len(data))
        available()
        offset += len(data)
    available()
    os.fchown(fd, UID, UID)
    available()
    os.fchmod(fd, 0o500)
    available()
    os.fsync(fd)
    available()


class Guard:
    def __init__(self, value, pins, support, lineage):
        self.value, self.pins, self.support, self.lineage = value, pins, support, lineage
        self.stage = support.Directory(ROOT, 0)
        self.evidence = DeadlineEvidence(self.stage, support)
        self.count, self.log_bytes, self.catalogs, self.artifacts = 0, 0, [], {}

    def run_child(self, argv, uid, tag, *, executable=None, pass_fds=(), stdin=subprocess.DEVNULL,
                  env=None, seconds=15, allowed=(0,)):
        available()
        self.count += 1
        require(self.count <= 128 and re.fullmatch('[a-z0-9-]+', tag))
        out = self.evidence.create(f'{self.count}-{tag}.stdout')
        err = self.evidence.create(f'{self.count}-{tag}.stderr')
        child = core.spawn(argv, executable=executable, pass_fds=pass_fds,
            user=uid, group=uid, extra_groups=(), umask=0o077, close_fds=True,
            env=env if env is not None else dict(ROOT_ENV if uid == 0 else PRIMARY_ENV if uid == 1000 else ENV),
            stdin=stdin, stdout=out, stderr=err)
        try:
            available()
            budget = min(seconds, max(0, DEADLINE - time.monotonic()))
        except BaseException:
            core.quarantine(child)  # A late spawn must not lose its retained Popen.
        code = await_allowed(child, budget, allowed)
        require(type(code) is int and code in allowed)  # No observation after failure/unknown.
        available()
        require(os.fstat(out).st_size <= LIMIT and os.fstat(err).st_size <= LIMIT)
        self.log_bytes += os.fstat(out).st_size + os.fstat(err).st_size
        require(self.log_bytes <= 64 * 1024 * 1024)
        available()
        os.fsync(out)
        available()
        os.fsync(err)
        available()
        return code, os.pread(out, LIMIT + 1, 0), os.pread(err, LIMIT + 1, 0)

    def baseline_command(self, evidence, argv, allowed=(0,)):
        require(evidence is self.evidence and tuple(argv) in self.support.allowed_commands())
        require(allowed == ((0, 1) if argv == ['/usr/bin/pgrep', '-x', 'mihomo'] else (0,)))
        code, out, _ = self.run_child(argv, 1000, 'primary-query', allowed=allowed)
        return code, out

    def source_admission(self):
        self.mail_defaults = File(Path('/etc/default/useradd'), 0, (0o600, 0o644), 65536)
        mail_spool_disabled(self.mail_defaults.bytes())
        # Old catalogs retain original48044 observations. Explicitly cover the
        # new instance names without relabelling that old catalog as new proof.
        self.fresh_instance_parents = []
        for root in ('/etc/systemd/system', '/run/systemd/system',
                     '/usr/local/lib/systemd/system', '/usr/lib/systemd/system',
                     '/etc/systemd/system.control', '/run/systemd/system.control',
                     '/run/systemd/transient', '/run/systemd/generator.early',
                     '/run/systemd/generator', '/run/systemd/generator.late',
                     '/etc/systemd/system.attached', '/run/systemd/system.attached'):
            path = Path(root)
            missing = None
            while True:
                try:
                    path.lstat()
                except FileNotFoundError:
                    missing = path.name
                    path = path.parent
                else:
                    break
            parent = Parents(path, 0)
            self.fresh_instance_parents.append((parent, missing))
            self.check_fresh_instances(parent, missing)
        for name, directory, digest in CATALOGS:
            available()
            pin = File(Path(directory) / 'inventory.json', 0, 0o600, 48 * 1024 * 1024, digest)
            original = decode(pin.bytes())
            mod = module(self.pins[name])
            current = mod.Inventory(mod.ROOTS, mod.TARGETS)
            if name == 'startup_followup.py':
                current.package_inputs()
            value = current.capture()
            if name == 'startup_followup.py':
                value['package_bindings'] = current.package_bindings()
            require(decode(json.dumps(value).encode()) == original)
            # Initial capture retained its own45s budget. Matched original FDs
            # remain held under the one whole-invocation deadline thereafter.
            current.deadline = DEADLINE
            self.catalogs.append((pin, current))

    def check_fresh_instances(self, parent, missing):
        parent.recheck()
        names = ((missing,) if missing is not None else
                 ('user@48049.service', 'user@48049.service.d',
                  'user-runtime-dir@48049.service', 'user-runtime-dir@48049.service.d'))
        for name in names:
            try:
                os.stat(name, dir_fd=parent.rows[-1][1], follow_symlinks=False)
            except FileNotFoundError:
                pass
            else:
                require(False)
        parent.recheck()

    def source_recheck(self):
        available()
        self.mail_defaults.recheck()
        for parent, missing in self.fresh_instance_parents:
            self.check_fresh_instances(parent, missing)
        for pin, current in self.catalogs:
            pin.recheck()
            current.recheck()
        for pin in self.pins.values():
            pin.recheck()

    def absent_account(self):
        for database in ('passwd', 'group'):
            for key in (NAME, str(UID)):
                _, out, err = self.run_child(['/usr/bin/getent', database, key], 0, 'account-absent', allowed=(2,))
                require(out == err == b'')
        for path in (HOME, RUNTIME, Path('/var/lib/systemd/linger') / NAME):
            try:
                path.lstat()
            except FileNotFoundError:
                pass
            else:
                require(False)

    def remaining_capacity(self):
        """Both deliveries already exist; only the HOME ELF copy remains."""
        available()
        run, home = Parents(Path('/run'), 0), Parents(HOME.parent, 0)
        required, free = {}, {}
        copies = sum(self.pins[name].before.st_size for name in ('helper', 'omavless'))
        for parent, count in ((run, 0), (home, copies)):
            available()
            fd = parent.rows[-1][1]
            device = os.fstat(fd).st_dev
            filesystem = os.fstatvfs(fd)
            value = filesystem.f_bavail * filesystem.f_frsize
            require(type(value) is int and value >= 0)
            free[device] = min(free.get(device, value), value)
            required[device] = required.get(device, 0) + count + 512 * 1024 * 1024
            parent.recheck()
        require(all(free[device] >= count for device, count in required.items()))
        run.recheck(); home.recheck()

    def account_snapshot(self, created):
        output = {}
        for name in ('passwd', 'group', 'shadow', 'gshadow', 'subuid', 'subgid'):
            available()
            path = Path('/etc') / name
            fd = os.open(path, FLAGS)
            RETAINED.append(fd)
            before = os.fstat(fd)
            require(stat.S_ISREG(before.st_mode) and before.st_uid == 0 and before.st_size <= LIMIT)
            raw = os.pread(fd, LIMIT + 1, 0)
            require(len(raw) == before.st_size and identity(before) == identity(os.fstat(fd)) == identity(path.lstat()))
            rows = raw.splitlines(keepends=True)
            own = [row for row in rows if row.startswith(NAME.encode() + b':')]
            require(len(own) == (1 if created and name in ('passwd', 'group', 'shadow', 'gshadow') else 0))
            if own:
                parts = own[0].rstrip(b'\n').split(b':')
                if name == 'passwd':
                    require(len(parts) == 7 and parts[2:4] == [b'48049', b'48049']
                            and parts[5:] == [str(HOME).encode(), b'/usr/bin/nologin'])
                elif name == 'group':
                    require(len(parts) == 4 and parts[2:] == [b'48049', b''])
                elif name == 'shadow':
                    require(len(parts) == 9 and parts[1].startswith((b'!', b'*')))
                else:
                    require(len(parts) == 4 and parts[2:] == [b'', b''])
            output[name] = {'other_rows_sha256': hashlib.sha256(b''.join(row for row in rows if row not in own)).hexdigest(),
                            'own_row_sha256': hashlib.sha256(own[0]).hexdigest() if own else None}
        return output

    def create_account(self):
        self.source_recheck()
        self.run_child(['/usr/bin/groupadd', '--gid', '48049', NAME], 0, 'group-create')
        self.source_recheck()
        self.run_child(['/usr/bin/useradd', '--uid', '48049', '--gid', '48049', '--no-user-group',
            '--no-create-home', '--home-dir', str(HOME), '--shell', '/usr/bin/nologin', '--no-log-init',
            '--key', 'SUB_UID_COUNT=0', '--key', 'SUB_GID_COUNT=0', NAME],
            0, 'account-create')
        available()
        parent = Parents(HOME.parent, 0)
        available()
        os.mkdir(HOME.name, 0o700, dir_fd=parent.rows[-1][1])
        available()
        fd = os.open(HOME.name, FLAGS | os.O_DIRECTORY, dir_fd=parent.rows[-1][1])
        RETAINED.append(fd)
        available()
        os.fchown(fd, UID, UID)
        available()
        os.fsync(fd)
        available()
        os.fsync(parent.rows[-1][1])
        available()
        self.home_pin = Parents(HOME, UID)
        require(HOME.lstat().st_uid == HOME.lstat().st_gid == UID and stat.S_IMODE(HOME.lstat().st_mode) == 0o700)
        require(os.listdir(fd) == [])

    def manager_start(self):
        self.source_recheck()
        self.run_child(['/usr/bin/systemctl', '--system', '--no-pager', 'start', 'user-runtime-dir@48049.service'],
                       0, 'runtime-dir-start', seconds=45)
        self.source_recheck()
        self.home_pin.recheck()
        self.run_child(['/usr/bin/systemctl', '--system', '--no-pager', 'start', 'user@48049.service'],
                       0, 'manager-start', seconds=45)
        self.runtime_pin = Parents(RUNTIME, UID)
        require(RUNTIME.lstat().st_uid == RUNTIME.lstat().st_gid == UID
                and stat.S_IMODE(RUNTIME.lstat().st_mode) == 0o700)
        self.manager_stopped_app()

    def manager_stopped_app(self):
        self.home_pin.recheck()
        args = [arg for key in core.FIELDS for arg in ('-p', key)]
        _, out, _ = self.run_child(['/usr/bin/systemctl', '--system', '--no-pager', 'show', 'user@48049.service', *args],
                                   0, 'manager-observe')
        data = fields(out, core.FIELDS)
        require(data['LoadState'] == 'loaded' and data['ActiveState'] == 'active'
                and data['SubState'] == 'running' and data['ControlPID'] == '0'
                and re.fullmatch('[1-9][0-9]{0,9}', data['MainPID']))
        pid = int(data['MainPID'])
        proc = Path('/proc') / str(pid)
        before_start = self.support.proc_fields(proc)
        require(before_start[0] not in (b'Z', b'X'))
        status = (proc / 'status').read_bytes()
        require(len(status) <= 65536)
        values = pairs(line.split(b':', 1) for line in status.splitlines() if b':' in line)
        require(values[b'Uid'].split() == values[b'Gid'].split() == [b'48049'] * 4)
        loaded = os.open(proc / 'exe', FLAGS & ~os.O_NOFOLLOW)
        RETAINED.append(loaded)
        installed = self.catalogs[1][1].nodes[Path('/usr/lib/systemd/systemd')][2]
        require(identity(os.fstat(loaded)) == identity(os.fstat(installed)))
        commandline = (proc / 'cmdline').read_bytes()
        require(len(commandline) <= 4096 and commandline == b'/usr/lib/systemd/systemd\0--user\0'
                and self.support.proc_fields(proc) == before_start)
        token = (pid, before_start[1], os.fstat(loaded).st_dev, os.fstat(loaded).st_ino)
        require(not hasattr(self, 'manager_token') or self.manager_token == token)
        self.manager_token = token
        self.manager_pid = pid
        for unit in ('omavless.service', 'omavless-runtime.service', 'omavless-login-prepare.service'):
            _, out, _ = self.run_child(['/usr/bin/systemctl', '--user', '--no-pager', 'show', unit, *args], UID, 'owner-inactive')
            data = fields(out, core.FIELDS)
            require(data['LoadState'] in ('loaded', 'not-found') and data['ActiveState'] == 'inactive'
                    and data['SubState'] == 'dead' and data['MainPID'] == data['ControlPID'] == '0')

    def publish_elfs(self):
        self.home_pin.recheck()
        available()
        os.mkdir(ARTIFACTS, 0o700)
        available()
        directory = os.open(ARTIFACTS, FLAGS | os.O_DIRECTORY)
        RETAINED.append(directory)
        writers = {}
        for name in ('helper', 'omavless'):
            pin = self.pins[name]
            pin.recheck()
            available()
            fd = os.open(name, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                         0o600, dir_fd=directory)
            RETAINED.append(fd)
            copy_elf(pin, fd)
            pin.recheck()
            writers[name] = (fd, os.fstat(fd))
        available()
        os.fchown(directory, UID, UID)
        available()
        os.fsync(directory)
        available()
        os.fsync(self.home_pin.rows[-1][1])
        available()
        self.artifacts_pin = Parents(ARTIFACTS, UID)
        for name in ('helper', 'omavless'):
            self.artifacts[name] = complete_publication(*writers[name], name, self.pins[name])
        for number, name in ((198, 'helper'), (199, 'omavless')):
            # Explicit owned root-source alias -> owned fixture-copy transition.
            require(identity(os.fstat(number)) == identity(self.pins[name].before))
            available()
            os.dup2(self.artifacts[name].fd, number, inheritable=False)
            require(identity(os.fstat(number)) == identity(self.artifacts[name].before))
        self.namespace_pins = {}
        for name in ('pid', 'user', 'mnt', 'net'):
            fd = os.open('/proc/self/ns/' + name, os.O_RDONLY | os.O_CLOEXEC)
            RETAINED.append(fd)
            meta = os.fstat(fd)
            self.namespace_pins[name] = (fd, meta.st_dev, meta.st_ino)

    def native(self, entry, tag, request=None):
        self.source_recheck()
        self.artifacts_pin.recheck()
        for pin in self.artifacts.values():
            pin.recheck()
        env = dict(ENV, OV_T4_HELPER_SHA256=self.artifacts['helper'].sha,
                   OV_T4_CLI_SHA256=self.artifacts['omavless'].sha)
        for name, (fd, dev, ino) in self.namespace_pins.items():
            require((os.fstat(fd).st_dev, os.fstat(fd).st_ino) == (dev, ino)
                    == (os.stat('/proc/self/ns/' + name).st_dev, os.stat('/proc/self/ns/' + name).st_ino))
            env['OV_T4_NS_' + name.upper()] = f'{dev}:{ino}'
        if request is not None:
            available()
            os.lseek(request, 0, os.SEEK_SET)
        _, out, err = self.run_child([str(ARTIFACTS / 'helper'), '--exact', PREFIX + ENTRIES[entry],
            '--ignored', '--nocapture', '--test-threads=1', '--quiet'], UID, tag,
            executable='/proc/self/fd/198', pass_fds=(198, 199), env=env,
            stdin=subprocess.DEVNULL if request is None else request, seconds=180)
        if entry == 'diagnose':
            admission_trace(err)
        else:
            require(err == b'')
        if entry == 'launch':
            require(out == b'\nrunning 1 test\nOLD restored; recovery fence remains. Normal startup is still blocked.\n')
        else:
            require(re.fullmatch(rb'\nrunning 1 test\n\.\ntest result: ok\. 1 passed; 0 failed; 0 ignored; '
                                 rb'0 measured; [0-9]+ filtered out; finished in [0-9]+\.[0-9]+s\n\n', out))
        for pin in self.artifacts.values():
            pin.recheck()

    def typed_case(self, name, state):
        pin = File(ARTIFACTS / name, UID, 0o600, 4096)
        value = decode(pin.bytes())
        require(type(value) is dict and set(value) == {'schema', 'state', 'synthetic', 'product_pass'}
                and type(value['schema']) is int and value['schema'] == 1 and value['state'] == state
                and value['synthetic'] is True and value['product_pass'] is False)
        return pin

    def normal_cli_case(self):
        self.native('setup', 'setup')
        self.typed_case('setup.json', 'MIXED_FIRST_INTENT')
        chain = self.lineage.Lineage()
        self.chain = chain
        request = chain.files[ARTIFACTS / 'request.json'][0]
        chain.before_first()
        self.evidence.write('lineage-before.json', chain.mixed_receipt())
        self.native('diagnose', 'stopped-admission-diagnostic')
        chain.before_first()
        self.native('launch', 'normal-cli-first', request)
        self.native('first', 'verify-first')
        first = self.typed_case('after-first.json', 'ABORTED_OLD_PAIR_STILL_FENCED')
        chain.first_completed()
        self.evidence.write('lineage-first.json', chain.receipt())
        chain.reentry_boundary()
        self.native('launch', 'normal-cli-reentry', request)
        self.native('reentry', 'verify-reentry')
        second = self.typed_case('after-reentry.json', 'ABORTED_OLD_PAIR_STILL_FENCED')
        chain.reentry_boundary()
        first.recheck()
        second.recheck()
        self.evidence.write('lineage-reentry.json', chain.receipt())

    def exact_fixture_absence(self):
        available()
        targets = {(p.before.st_dev, p.before.st_ino) for p in self.artifacts.values()}
        checked = 0
        enumerated = 0
        with os.scandir('/proc') as entries:
            for entry in entries:
                available()
                enumerated += 1
                require(enumerated <= 32768)
                if not entry.name.isascii() or not entry.name.isdecimal():
                    continue
                proc = Path('/proc') / entry.name
                before_stat = self.support.proc_fields(proc)
                status = (proc / 'status').read_bytes()
                require(len(status) <= 65536)
                values = pairs(line.split(b':', 1) for line in status.splitlines() if b':' in line)
                ids = values[b'Uid'].split()
                require(len(ids) == 4 and all(v.isdigit() for v in ids))
                if b'48049' not in ids:
                    continue
                require(before_stat[0] not in (b'Z', b'X'))
                fd = os.open(proc / 'exe', os.O_RDONLY | os.O_CLOEXEC)
                RETAINED.append(fd)
                meta = os.fstat(fd)
                after_status = (proc / 'status').read_bytes()
                require(len(after_status) <= 65536)
                after_values = pairs(line.split(b':', 1) for line in after_status.splitlines() if b':' in line)
                require(before_stat == self.support.proc_fields(proc)
                        and ids == after_values[b'Uid'].split()
                        and identity(meta) == identity(os.fstat(fd)) == identity((proc / 'exe').stat())
                        and (meta.st_dev, meta.st_ino) not in targets)
                checked += 1
        return checked  # Sampled original inode absence, not atomic descendants.

    def execute(self):
        global PHASE
        core.command = self.baseline_command
        PHASE = 'source-admission'
        self.source_admission()
        require(self.run_child(['/usr/bin/systemd-detect-virt', '--vm'], 1000, 'vm-check')[1].strip() == b'kvm')
        PHASE = 'before-baseline'
        before = core.snapshot(self.evidence)
        accounts = self.account_snapshot(False)
        self.evidence.write('baseline-before.json', before)
        self.evidence.write('accounts-before.json', accounts)
        self.absent_account()
        PHASE = 'remaining-capacity'
        self.remaining_capacity()
        PHASE = 'account-create'
        self.create_account()
        PHASE = 'manager-start'
        self.manager_start()
        PHASE = 'artifact-publication'
        self.publish_elfs()
        PHASE = 'normal-cli'
        self.normal_cli_case()
        PHASE = 'final-known-observation'
        self.manager_stopped_app()
        observed = self.exact_fixture_absence()
        after_accounts = self.account_snapshot(True)
        require(all(accounts[name]['other_rows_sha256'] == after_accounts[name]['other_rows_sha256'] for name in accounts))
        self.evidence.write('accounts-after.json', after_accounts)
        after = core.snapshot(self.evidence)
        self.evidence.write('baseline-after.json', after)
        require(before.keys() == after.keys() and all(before[k] == after[k] for k in before if k != 'network')
                and core.network_equal(before['network'], after['network']))
        self.source_recheck()
        self.chain.reentry_boundary()
        PHASE = 'result'
        self.evidence.write('result.json', {'schema': 't4-disposable-normal-cli-v1',
            'native_head': NATIVE_HEAD, 'guard_head': self.value['guard_head'], 'normal_cli_known_zero_calls': 2,
            'uid': UID, 'manager_pid': self.manager_pid, 'sampled_uid_executables': observed,
            'exact_fixture_inodes_absent_at_observation': True, 'atomic_descendants_claimed': False,
            'baseline_preserved': True, 'fence_retired': False, 'account_manager_retained': True,
            'outcome': 'NORMAL_CLI_ABORT_AND_REENTRY_SYNTHETIC_INTENT_STILL_FENCED'})


def emit_terminal(raw, stream, *, success):
    global TERMINAL
    try:
        require(not TERMINAL and type(raw) is bytes and len(raw) <= 8192)
        require(time.monotonic() < DEADLINE)
        if success:
            available()
        written = stream.write(raw)
        require(type(written) is int and written == len(raw))
        require(time.monotonic() < DEADLINE)
        if success:
            available()
        stream.flush()
        require(time.monotonic() < DEADLINE)
        if success:
            available()
    finally:
        TERMINAL = True


def main():
    global core, DEADLINE
    DEADLINE = time.monotonic() + 600
    require(os.getresuid() == os.getresgid() == (0, 0, 0) and sys.flags.isolated == 1
            and sys.dont_write_bytecode and Path(__file__) == ROOT / 'root_guard.py'
            and len(sys.argv) == 3 and sys.argv[1] == '--run-t4-first-abort-cli'
            and re.fullmatch('[0-9a-f]{64}', sys.argv[2]))
    status = Path('/proc/self/status').read_bytes()
    require(len(status) <= 65536 and pairs(line.split(b':', 1) for line in status.splitlines() if b':' in line)[b'NoNewPrivs'].strip() == b'1')
    delivery = File(ROOT / 'receipt.json', 0, 0o600, 32768, sys.argv[2])
    value = receipt(delivery.bytes())
    pins = {'receipt.json': delivery}
    for name in CODE:
        pins[name] = File(ROOT / name, 0, 0o500, LIMIT, value['code'][name])
    for name in ('helper', 'omavless'):
        pins[name] = File(ROOT / name, 0, 0o500, ELF_LIMIT, value['elfs'][name]['sha256'])
        require(pins[name].before.st_size == value['elfs'][name]['size'] and os.pread(pins[name].fd, 4, 0) == b'\x7fELF')
    core = module(pins['core.py'])
    core.available = available
    core.ROOT_UNITS = (*core.ROOT_UNITS, *EXTRA_ROOT_UNITS)
    reserve_elf_slots(pins)
    support = module(pins['support.py'])
    support.core = core
    lineage = module(pins['lineage.py'])
    lineage.GATE = available
    Guard(value, pins, support, lineage).execute()
    emit_terminal(b'T4_NORMAL_CLI_ABORT_REENTRY_STILL_FENCED_NOT_FULL_PRODUCT_PASS\n',
                  sys.stdout.buffer, success=True)


if __name__ == '__main__':
    os.umask(0o077)
    try:
        main()
    except BaseException:
        if core is not None:
            core.UNCERTAIN = True
        if not TERMINAL:
            try:
                emit_terminal(('T4_NORMAL_CLI_NONPASS_RETAINED_' + PHASE + '\n').encode(),
                              sys.stderr.buffer, success=False)
            except BaseException:
                pass
        sys.exit(2)
