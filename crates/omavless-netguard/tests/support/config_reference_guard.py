#!/usr/bin/python3
"""Fixed capture-only VM observer. Unknown children and artifacts are retained."""
import hashlib
import json
import os
from pathlib import Path
import stat
import sys

STAGE = Path('/run/omavless-k1-effective-config-reference')
UNIT = 'omavless-k1-effective-config-reference.service'
PARENT = Path('/run/systemd/system')
LINK = PARENT / UNIT
CGROUP = Path('/sys/fs/cgroup/system.slice') / UNIT
QUERY_SHA = '67e541ea5c764a05b669267b248d3df77ca3402569df5116bfff9346ff9ea0bd'
UNIT_SHA = '01464f072443481b5f45893e39c52ae801b3e60739201f58a307c0dec9e316f1'
PROBE_SHA = 'b98c1290d2a6d522e8ef5e47476c07ae8d7c2a23366850dc071fe254fe9efb07'
NATIVE_SOURCE = '4db6d601afb0130118498238c9b15559e803e286'
TEST = 'manager_config_reference_fixture::capture_effective_config'
OBJECT = '/org/freedesktop/systemd1/unit/omavless_2dk1_2deffective_2dconfig_2dreference_2eservice'
MARKER = 'OBSERVED_CONFIG_DATA_NOT_ADMISSION'


def require(value):
    if not value:
        raise RuntimeError()


def identity(m):
    return (m.st_dev, m.st_ino, m.st_uid, m.st_gid, m.st_mode, m.st_nlink,
            m.st_size, m.st_mtime_ns, m.st_ctime_ns)


def directory_identity(m):
    return (m.st_dev, m.st_ino, m.st_uid, m.st_gid, m.st_mode)


def pairs(items):
    value = {}
    for key, item in items:
        require(key not in value)
        value[key] = item
    return value


class Pin:
    def __init__(self, path, mode, maximum, expected=None):
        self.path, self.maximum = path, maximum
        self.fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK)
        self.meta = os.fstat(self.fd)
        require(stat.S_ISREG(self.meta.st_mode) and self.meta.st_uid == self.meta.st_gid == 0
                and stat.S_IMODE(self.meta.st_mode) == mode and self.meta.st_nlink == 1
                and 0 < self.meta.st_size <= maximum and not os.listxattr(self.fd))
        self.sha = self.hash()
        require(expected is None or self.sha == expected)
        self.recheck()

    def hash(self):
        result, offset = hashlib.sha256(), 0
        while offset < self.meta.st_size:
            block = os.pread(self.fd, min(65536, self.meta.st_size - offset), offset)
            require(block)
            result.update(block)
            offset += len(block)
        require(os.pread(self.fd, 1, offset) == b'')
        return result.hexdigest()

    def recheck(self):
        require(identity(self.meta) == identity(os.fstat(self.fd)) == identity(self.path.lstat()))
        require(self.hash() == self.sha)
        require(identity(self.meta) == identity(os.fstat(self.fd)) == identity(self.path.lstat()))

    def data(self):
        self.recheck()
        require(self.meta.st_size <= 2 * 1024 * 1024)
        raw = os.pread(self.fd, self.meta.st_size + 1, 0)
        require(len(raw) == self.meta.st_size)
        self.recheck()
        return raw


def load_query(pin):
    require(pin.sha == QUERY_SHA)
    ns = {'__name__': 'config_reference_definitions_only', '__file__': str(pin.path)}
    exec(compile(pin.data(), '<pinned-readonly-snapshot>', 'exec'), ns)
    ns['STAGE'] = STAGE
    return ns


def validate_capture(value, ack):
    require(type(value) is dict and set(value) == {'schema', 'marker', 'unit', 'service', 'dump'}
            and type(value['schema']) is int and value['schema'] == 1 and value['marker'] == MARKER)
    require(value['unit'] == {'Id': UNIT, 'LoadState': 'loaded', 'FragmentPath': str(LINK),
            'ActiveState': 'inactive', 'SubState': 'dead', 'DropInPaths': []})
    service = value['service']
    require(type(service) is dict and set(service) == {'StandardOutput', 'StandardError',
            'Type', 'User', 'Group', 'WatchdogUSec', 'ExecMainStartTimestampMonotonic',
            'MainPID', 'ControlPID', 'ExecMainPID'})
    for name, expected in {'StandardOutput': 'append', 'StandardError': 'append',
                           'Type': 'oneshot', 'User': 'root', 'Group': 'root'}.items():
        require(type(service[name]) is str and service[name] == expected)
    for name in ('MainPID', 'ControlPID', 'ExecMainPID', 'ExecMainStartTimestampMonotonic'):
        require(type(service[name]) is int and service[name] == 0)
    require(type(service['WatchdogUSec']) is int and 0 <= service['WatchdogUSec'] < 2**64)
    dump = value['dump']
    require(type(dump) is dict and set(dump) == {'type', 'data'} and dump['type'] == 's'
            and type(dump['data']) is list and len(dump['data']) == 1
            and type(dump['data'][0]) is str and '\0' not in dump['data'][0]
            and 0 < len(dump['data'][0].encode()) <= 1024 * 1024)
    require(type(ack) is dict and set(ack) == {'schema', 'unref_acknowledged', 'admission'}
            and type(ack['schema']) is int and ack['schema'] == 1
            and ack['unref_acknowledged'] is True and ack['admission'] is False)


def bounded(path, maximum):
    with path.open('rb') as stream:
        raw = stream.read(maximum + 1)
    require(len(raw) <= maximum)
    return raw


def process_metadata(path):
    raw = bounded(path / 'stat', 8192).decode()
    require(') ' in raw and raw.split(' ', 1)[0] == path.name)
    fields = raw.rsplit(') ', 1)[1].split()
    require(len(fields) >= 20)
    rows = [row for row in bounded(path / 'status', 16384).decode().splitlines()
            if row.startswith('Uid:')]
    require(len(rows) == 1)
    uids = tuple(int(item) for item in rows[0].split()[1:])
    require(len(uids) == 4)
    return (path.name, fields[0], int(fields[6]), int(fields[19]), uids)


def exact_inode_absent(probe):
    """Point-in-time original executable inode absence for UID0/1000, not quiescence."""
    paths = sorted(path for path in Path('/proc').iterdir() if path.name.isdecimal())
    require(len(paths) <= 32768)
    target = (probe.meta.st_dev, probe.meta.st_ino)
    for path in paths:
        before = process_metadata(path)
        if not any(uid in (0, 1000) for uid in before[4]):
            require(before == process_metadata(path))
            continue
        try:
            fd = os.open(path / 'exe', os.O_PATH | os.O_CLOEXEC)
        except FileNotFoundError:
            # Linux v6.17 include/linux/sched.h PF_KTHREAD=0x00200000.
            # Only twice-stable root kernel threads or zombies have no userspace
            # executable here. All other absent/unreadable/racing cases refuse.
            require(before[1] == 'Z' or (before[4] == (0, 0, 0, 0)
                    and before[2] & 0x00200000 != 0))
            require(before == process_metadata(path))
            continue
        try:
            info = os.fstat(fd)
            require((info.st_dev, info.st_ino) != target and before == process_metadata(path))
            current = (path / 'exe').stat()
            require((current.st_dev, current.st_ino) == (info.st_dev, info.st_ino)
                    and before == process_metadata(path))
        finally:
            os.close(fd)


class Observer:
    def __init__(self, ns, pins, directory_fd, parent_fd):
        self.ns, self.pins, self.directory_fd, self.parent_fd = ns, pins, directory_fd, parent_fd
        self.directory, self.parent = os.fstat(directory_fd), os.fstat(parent_fd)
        self.sealed, self.link_identity = False, None

    def available(self):
        require(not self.sealed and not self.ns['UNCERTAIN'])

    def call(self, argv):
        self.available()
        try:
            return self.ns['command'](argv)
        except BaseException:
            self.sealed = True
            raise

    def properties(self, names):
        raw = self.call(['/usr/bin/systemctl', '--system', '--no-pager', 'show', UNIT,
                         *[arg for name in names for arg in ('-p', name)]])
        require(len(raw) <= 65536)
        rows = pairs(line.split('=', 1) for line in raw.decode().splitlines())
        require(set(rows) == set(names))
        return rows

    def recheck(self, linked=False):
        self.available()
        require(directory_identity(self.directory) == directory_identity(os.fstat(self.directory_fd))
                == directory_identity(STAGE.lstat()))
        require(directory_identity(self.parent) == directory_identity(os.fstat(self.parent_fd))
                == directory_identity(PARENT.lstat()))
        for pin in self.pins:
            pin.recheck()
        if linked:
            meta = os.stat(UNIT, dir_fd=self.parent_fd, follow_symlinks=False)
            require(identity(meta) == self.link_identity and stat.S_ISLNK(meta.st_mode)
                    and os.readlink(UNIT, dir_fd=self.parent_fd) == str(STAGE / 'fixture.service'))

    def write(self, name, value):
        self.available()
        fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                     0o600, dir_fd=self.directory_fd)
        with os.fdopen(fd, 'w') as stream:
            json.dump(value, stream, sort_keys=True)
            stream.flush()
            os.fsync(stream.fileno())
        os.fsync(self.directory_fd)

    def helper(self):
        self.available()
        probe = self.pins[2]
        logs = []
        for name in ('helper.stdout', 'helper.stderr'):
            fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                         0o600, dir_fd=self.directory_fd)
            logs.append(fd)
        try:
            child = self.ns['spawn']([str(STAGE / 'probe'), '--exact', TEST, '--ignored',
                    '--nocapture', '--test-threads=1'], executable=f'/proc/self/fd/{probe.fd}',
                    pass_fds=(probe.fd,), stdin=-3, stdout=logs[0], stderr=logs[1],
                    user=0, group=0, extra_groups=[], close_fds=True,
                    env={'PATH': '/usr/bin', 'LC_ALL': 'C', 'OMAVLESS_K1_CONFIG_REFERENCE': '1'})
            code = self.ns['await_child'](child, seconds=45)
            require(code == 0)
            require(all(os.fstat(fd).st_size <= 65536 for fd in logs))
            for fd in logs:
                os.fsync(fd)
        finally:
            # Child retains its own descriptors on unknown exit; no deletion or
            # process query occurs here, including no Popen destructor polling.
            for fd in logs:
                os.close(fd)

    def evidence(self):
        data = Pin(STAGE / 'reference-config-data.json', 0o600, 2 * 1024 * 1024)
        ack = Pin(STAGE / 'reference-unref-ack.json', 0o600, 4096)
        validate_capture(json.loads(data.data(), object_pairs_hook=pairs),
                         json.loads(ack.data(), object_pairs_hook=pairs))
        self.pins.extend([data, ack])

    def cleanup_known_success(self):
        self.recheck(linked=True)
        expected = {'LoadState': 'loaded', 'FragmentPath': str(LINK), 'ActiveState': 'inactive',
                    'SubState': 'dead', 'MainPID': '0', 'ControlPID': '0', 'ExecMainPID': '0',
                    'ExecMainStartTimestampMonotonic': '0', 'ControlGroup': ''}
        require(self.properties(expected) == expected)
        require(self.call(['/usr/bin/busctl', '--system', 'get-property', 'org.freedesktop.systemd1',
                          OBJECT, 'org.freedesktop.systemd1.Unit', 'Job']) == b'(uo) 0 "/"\n')
        require(not CGROUP.exists() and not CGROUP.is_symlink())
        exact_inode_absent(self.pins[2])
        self.recheck(linked=True)
        os.unlink(UNIT, dir_fd=self.parent_fd)
        os.fsync(self.parent_fd)
        self.call(['/usr/bin/systemctl', 'daemon-reload'])
        require(self.properties(['LoadState']) == {'LoadState': 'not-found'})
        require(not LINK.exists() and not LINK.is_symlink()
                and not CGROUP.exists() and not CGROUP.is_symlink())

    def once(self):
        self.recheck()
        require(self.call(['/usr/bin/systemd-detect-virt', '--vm']).strip() == b'kvm')
        require(not LINK.exists() and not LINK.is_symlink() and not CGROUP.exists() and not CGROUP.is_symlink())
        require(self.properties(['LoadState']) == {'LoadState': 'not-found'})
        require(set(os.listdir(self.directory_fd)) == {'guard.py', 'query-guard.py', 'fixture.service', 'probe'})
        self.write('invocation.json', {'schema': 1, 'native_source': NATIVE_SOURCE, 'admission': False})
        before = self.ns['snapshot']()
        self.write('baseline-before.json', before)
        self.recheck()
        os.symlink(str(STAGE / 'fixture.service'), UNIT, dir_fd=self.parent_fd)
        self.link_identity = identity(os.stat(UNIT, dir_fd=self.parent_fd, follow_symlinks=False))
        os.fsync(self.parent_fd)
        self.recheck(linked=True)
        # RefUnit is the first load after publication. No initial daemon-reload.
        self.helper()
        self.evidence()
        self.cleanup_known_success()
        after = self.ns['snapshot']()
        self.write('baseline-after.json', after)
        require(self.ns['preserve'](before, after))
        self.recheck()
        self.write('result.json', {'schema': 1, 'marker': MARKER, 'preserved': True,
                                  'native_source': NATIVE_SOURCE, 'probe_sha256': PROBE_SHA,
                                  'unref_acknowledged': True, 'admission': False})

    def execute(self):
        self.available()
        try:
            self.once()
        except BaseException:
            self.sealed = True
            raise


def main():
    require(os.getuid() == os.geteuid() == os.getgid() == os.getegid() == 0
            and len(sys.argv) == 1 and os.environ.get('OMAVLESS_K1_CONFIG_REFERENCE_GUARD') == '1')
    for path in (Path('/run'), Path('/run/systemd'), PARENT):
        meta = path.lstat()
        require(stat.S_ISDIR(meta.st_mode) and meta.st_uid == meta.st_gid == 0 and meta.st_mode & 0o022 == 0)
    fd = os.open(STAGE, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    meta = os.fstat(fd)
    require(meta.st_uid == meta.st_gid == 0 and stat.S_IMODE(meta.st_mode) == 0o700
            and directory_identity(meta) == directory_identity(STAGE.lstat()))
    parent_fd = os.open(PARENT, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    query = Pin(STAGE / 'query-guard.py', 0o600, 256 * 1024, QUERY_SHA)
    pins = [query, Pin(STAGE / 'fixture.service', 0o600, 16384, UNIT_SHA),
            Pin(STAGE / 'probe', 0o500, 128 * 1024 * 1024, PROBE_SHA),
            Pin(STAGE / 'guard.py', 0o500, 256 * 1024)]
    require(Path(__file__) == STAGE / 'guard.py')
    Observer(load_query(query), pins, fd, parent_fd).execute()
    print('K1_CONFIG_REFERENCE_CAPTURE_PRESERVED_NOT_ADMISSION')


if __name__ == '__main__':
    os.umask(0o077)
    try:
        main()
    except BaseException:
        print('K1_CONFIG_REFERENCE_NONPASS_RETAINED', file=sys.stderr)
        sys.exit(2)
