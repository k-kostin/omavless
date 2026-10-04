#!/usr/bin/python3
"""Fixed dedicated-VM observer. Never installed; failure retains everything."""
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import time

STAGE = Path('/run/omavless-k1-manager-private-lifecycle')
UNIT = 'omavless-k1-manager-private-lifecycle.service'
LINK = Path('/run/systemd/system') / UNIT
CGROUP = Path('/sys/fs/cgroup/system.slice') / UNIT
QUERY_SHA = '67e541ea5c764a05b669267b248d3df77ca3402569df5116bfff9346ff9ea0bd'
UNIT_SHA = '76f137c03314769af114b06ce4d9f4684e96893d1b12c6f15812a0082b24d8be'
PROBE_SHA = 'a3f224c7f9e0e288a2e93a64b428c65c4b03207388368cfc6e13572bc0cffa28'
NATIVE_SOURCE = '28bdba2ee398db2e3da75a8ab11a7465ba7beef8'
TEST = 'kernel_observer::creator_lifecycle::manager_private::manager_private_lifecycle'
BUS = ['/usr/bin/busctl', '--system', 'get-property', 'org.freedesktop.systemd1',
       '/org/freedesktop/systemd1/unit/omavless_2dk1_2dmanager_2dprivate_2dlifecycle_2eservice']
EMPTY_TYPED = ('ExecCondition', 'ExecStartPre', 'ExecStartPost', 'ExecReload',
               'ExecStop', 'ExecStopPost', 'ExecReloadPost', 'EnvironmentFiles', 'SystemCallFilter',
               'OpenFile', 'ExtraFileDescriptorNames', 'BindPaths', 'BindReadOnlyPaths',
               'TemporaryFileSystem', 'ExtensionDirectories', 'ExtensionImages', 'MountImages')
TYPED_BYTES = (('a(sasbttttuii) 0\n' * 7) + 'a(sb) 0\n(bas) false 0\n'
               'a(sst) 1 "/proc/1/ns/net" "k1-host-netns" 1\nas 0\n'
               'a(ssbt) 0\na(ssbt) 0\na(ss) 0\nas 0\na(sba(ss)) 0\na(ssba(ss)) 0\n').encode()
DEPENDENCIES = {b'as 2 "sysinit.target" "system.slice"\n',
                b'as 2 "system.slice" "sysinit.target"\n'}
FIXED = {
    'LoadState': 'loaded', 'FragmentPath': str(LINK), 'Conflicts': 'shutdown.target',
    'Type': 'oneshot', 'RemainAfterExit': 'yes', 'User': 'root', 'Group': 'root',
    'NoNewPrivileges': 'yes', 'Delegate': 'no', 'RestrictNamespaces': 'yes',
    'PrivateUsers': 'no', 'PrivatePIDs': 'no', 'PrivateNetwork': 'yes',
    'CapabilityBoundingSet': 'cap_net_admin', 'AmbientCapabilities': '',
    'Environment': 'OMAVLESS_K1_MANAGER_PRIVATE=1', 'UMask': '0077',
    'Restart': 'no', 'TimeoutStartUSec': 'infinity', 'TimeoutStopUSec': 'infinity',
    'RuntimeMaxUSec': 'infinity', 'KillMode': 'none', 'SendSIGKILL': 'no',
    'WatchdogUSec': '0', 'FailureAction': 'none', 'SuccessAction': 'none',
    'JobTimeoutUSec': 'infinity', 'JobRunningTimeoutUSec': 'infinity',
    'StandardInput': 'null', 'StandardOutput': 'append', 'StandardError': 'append',
    'StandardOutputFile': str(STAGE / 'native.stdout'),
    'StandardErrorFile': str(STAGE / 'native.stderr'),
    'FileDescriptorStoreMax': '0', 'NFileDescriptorStore': '0',
    'ProtectProc': 'default', 'ProcSubset': 'all',
    **{name: '' for name in ('DropInPaths', 'Wants', 'BindsTo', 'PartOf', 'Upholds',
        'OnFailure', 'OnSuccess', 'TriggeredBy', 'Requisite', 'PropagatesStopTo',
        'StopPropagatedFrom', 'PassEnvironment', 'SupplementaryGroups',
        'RootDirectory', 'RootImage', 'NetworkNamespacePath', 'UserNamespacePath',
        'JoinsNamespaceOf', 'Sockets')},
}
STATE_FIELDS = ('ActiveState', 'SubState', 'MainPID', 'ControlPID', 'ExecMainPID',
                'ExecMainCode', 'ExecMainStatus', 'Result', 'ControlGroup')


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


def exact_json(value, expected):
    # JSON booleans must not compare equal to integer evidence fields.
    require(type(value) is type(expected))
    if type(expected) is dict:
        require(value.keys() == expected.keys())
        for key in expected:
            exact_json(value[key], expected[key])
    elif type(expected) is list:
        require(len(value) == len(expected))
        for left, right in zip(value, expected):
            exact_json(left, right)
    else:
        require(value == expected)


def bounded_read(path, maximum):
    with path.open('rb') as stream:
        raw = stream.read(maximum + 1)
    require(len(raw) <= maximum)
    return raw


class Pin:
    def __init__(self, path, mode, maximum, expected=None):
        self.path, self.maximum, self.expected = path, maximum, expected
        self.fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK)
        self.meta = os.fstat(self.fd)
        require(stat.S_ISREG(self.meta.st_mode) and self.meta.st_uid == self.meta.st_gid == 0
                and stat.S_IMODE(self.meta.st_mode) == mode and self.meta.st_nlink == 1
                and 0 <= self.meta.st_size <= maximum and os.listxattr(self.fd) == [])
        self.sha = self.hash()
        require(expected is None or self.sha == expected)
        self.recheck()

    def hash(self):
        result = hashlib.sha256()
        offset = 0
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
        require(self.meta.st_size <= 1024 * 1024)
        value = os.pread(self.fd, self.meta.st_size + 1, 0)
        self.recheck()
        return value


def load_query(pin):
    require(pin.sha == QUERY_SHA)
    namespace = {'__name__': 'manager_private_definitions_only', '__file__': str(pin.path)}
    exec(compile(pin.data(), '<pinned-readonly-snapshot>', 'exec'), namespace)
    namespace['STAGE'] = STAGE
    return namespace


class Observer:
    def __init__(self, namespace, pins, directory_fd):
        self.ns, self.pins, self.directory_fd = namespace, pins, directory_fd
        self.directory = os.fstat(directory_fd)
        self.sealed = False
        self.link_identity = None

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
        for pin in self.pins:
            pin.recheck()
        if linked:
            require(identity(LINK.lstat()) == self.link_identity
                    and LINK.is_symlink() and os.readlink(LINK) == str(STAGE / 'fixture.service'))

    def fixed_properties(self, before_start=False):
        require(self.properties(FIXED) == FIXED)
        require(self.call([*BUS, 'org.freedesktop.systemd1.Service', *EMPTY_TYPED]) == TYPED_BYTES)
        require(self.call([*BUS, 'org.freedesktop.systemd1.Unit', 'Requires']) in DEPENDENCIES)
        if before_start:
            probe = str(STAGE / 'probe')
            args = f'{probe} --exact {TEST} --ignored --nocapture --test-threads=1'
            expected = f'{{ path={probe} ; argv[]={args} ; ignore_errors=no ; start_time=[n/a] ; stop_time=[n/a] ; pid=0 ; code=(null) ; status=0/0 }}'
            require(self.properties(['ExecStart']) == {'ExecStart': expected})

    def empty_cgroup(self):
        require(not CGROUP.is_symlink())
        if CGROUP.exists():
            require(CGROUP.is_dir())
            raw = bounded_read(CGROUP / 'cgroup.events', 4096)
            require(len(raw) <= 4096 and b'\npopulated 0\n' in b'\n' + raw)
            require(bounded_read(CGROUP / 'cgroup.procs', 4096) == b'')

    def write(self, name, value):
        self.available()
        fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                     0o600, dir_fd=self.directory_fd)
        with os.fdopen(fd, 'w') as stream:
            json.dump(value, stream, sort_keys=True)
            stream.flush()
            os.fsync(stream.fileno())
        os.fsync(self.directory_fd)

    def wait_state(self, success, pending):
        deadline = time.monotonic() + 45
        while time.monotonic() < deadline:
            self.recheck(linked=True)
            state = self.properties(STATE_FIELDS)
            if (state['ActiveState'], state['SubState']) == success:
                require(state['MainPID'] == state['ControlPID'] == '0'
                        and state['Result'] == 'success'
                        and state['ExecMainCode'] == '1' and state['ExecMainStatus'] == '0'
                        and state['ExecMainPID'].isdecimal() and int(state['ExecMainPID']) > 1
                        and state['ControlGroup'] in ('', f'/system.slice/{UNIT}'))
                self.empty_cgroup()
                return state
            require((state['ActiveState'], state['SubState']) in pending
                    and state['Result'] == 'success')
            time.sleep(0.1)
        raise RuntimeError()  # Never stop/signal a timed-out unit.

    def native_evidence(self):
        receipt = Pin(STAGE / 'native-result.json', 0o600, 4096)
        exact_json(json.loads(receipt.data(), object_pairs_hook=pairs), {
            'schema': 1, 'synthetic_epoch': True, 'effects': 2, 'full_inventory': True,
            'second_socket_untrusted': True, 'absent': True})
        state = STAGE / 'state' / 'omavless-netguard'
        for directory in (STAGE / 'state', state):
            meta = directory.lstat()
            require(stat.S_ISDIR(meta.st_mode) and meta.st_uid == meta.st_gid == 0
                    and stat.S_IMODE(meta.st_mode) == 0o700)
        marker = Pin(state / 'armed-v1.json', 0o600, 4096)
        exact_json(json.loads(marker.data(), object_pairs_hook=pairs), {
            'version': 1, 'policy_version': 1, 'enrolled_uid': 1001,
            'generation': 7, 'armed': False, 'flags': 0})
        retired = Pin(state / 'table-receipt-v1.json', 0o600, 2048)
        value = json.loads(retired.data(), object_pairs_hook=pairs)
        require(set(value) == {'version', 'enrolled_uid', 'boot', 'host_netns_epoch',
            'netns_device', 'netns_inode', 'operation', 'phase', 'table_handle'})
        for field in ('version', 'enrolled_uid', 'operation', 'table_handle'):
            require(type(value[field]) is int)
        exact_json(value['boot'], [0x31] * 16)
        exact_json(value['host_netns_epoch'], [0x32] * 16)
        require(value['version'] == 1 and value['enrolled_uid'] == 1001
                and value['boot'] == [0x31] * 16 and value['host_netns_epoch'] == [0x32] * 16
                and type(value['netns_device']) is int and value['netns_device'] > 0
                and type(value['netns_inode']) is int and value['netns_inode'] > 0
                and value['operation'] == 2 and value['phase'] == 'retired' and value['table_handle'] == 0)
        self.pins.extend([receipt, marker, retired])

    def execute(self):
        self.available()
        try:
            self.once()
        except BaseException:
            self.sealed = True
            raise

    def once(self):
        self.recheck()
        require(self.call(['/usr/bin/systemd-detect-virt', '--vm']).strip() == b'kvm')
        require(not LINK.exists() and not LINK.is_symlink() and not CGROUP.exists() and not CGROUP.is_symlink())
        require(self.properties(['LoadState']) == {'LoadState': 'not-found'})
        require(set(os.listdir(self.directory_fd)) == {'guard.py', 'query-guard.py', 'fixture.service', 'probe'})
        self.write('invocation.json', {'schema': 1, 'native_source': NATIVE_SOURCE})
        before = self.ns['snapshot']()
        self.write('baseline-before.json', before)
        for name in ('native.stdout', 'native.stderr'):
            fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                         0o600, dir_fd=self.directory_fd)
            os.close(fd)
        self.recheck()
        os.symlink(str(STAGE / 'fixture.service'), LINK)
        self.link_identity = identity(LINK.lstat())
        self.call(['/usr/bin/systemctl', 'daemon-reload'])
        self.fixed_properties(before_start=True)
        require(self.properties(['ActiveState', 'SubState', 'MainPID', 'ControlPID']) == {
            'ActiveState': 'inactive', 'SubState': 'dead', 'MainPID': '0', 'ControlPID': '0'})
        self.recheck(linked=True)
        self.call(['/usr/bin/systemctl', 'start', '--no-block', UNIT])
        terminal = self.wait_state(('active', 'exited'), {('activating', 'start')})
        self.fixed_properties()
        self.native_evidence()
        self.recheck(linked=True)
        # Stop only the exact successful, zero-PID, empty-cgroup own unit. This
        # releases its retained anonymous namespace; never used on failure.
        self.call(['/usr/bin/systemctl', 'stop', '--no-block', UNIT])
        self.wait_state(('inactive', 'dead'), {('deactivating', 'stop'), ('active', 'exited')})
        self.recheck(linked=True)
        os.unlink(LINK)
        self.call(['/usr/bin/systemctl', 'daemon-reload'])
        require(self.properties(['LoadState']) == {'LoadState': 'not-found'})
        require(not CGROUP.exists() and not CGROUP.is_symlink())
        after = self.ns['snapshot']()
        self.write('baseline-after.json', after)
        require(self.ns['preserve'](before, after))
        self.recheck()
        self.write('result.json', {'schema': 1, 'native_source': NATIVE_SOURCE,
                                  'native_probe_sha256': PROBE_SHA, 'preserved': True,
                                  'synthetic_only': True, 'terminal': terminal})


def main():
    require(os.getuid() == os.geteuid() == os.getgid() == os.getegid() == 0
            and len(sys.argv) == 1 and os.environ.get('OMAVLESS_K1_MANAGER_PRIVATE_GUARD') == '1')
    run = Path('/run').lstat()
    require(stat.S_ISDIR(run.st_mode) and run.st_uid == run.st_gid == 0 and run.st_mode & 0o022 == 0)
    fd = os.open(STAGE, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    meta = os.fstat(fd)
    require(meta.st_uid == meta.st_gid == 0 and stat.S_IMODE(meta.st_mode) == 0o700
            and directory_identity(meta) == directory_identity(STAGE.lstat()))
    query = Pin(STAGE / 'query-guard.py', 0o600, 256 * 1024, QUERY_SHA)
    pins = [query, Pin(STAGE / 'fixture.service', 0o600, 16384, UNIT_SHA),
            Pin(STAGE / 'probe', 0o500, 1024**3, PROBE_SHA),
            Pin(STAGE / 'guard.py', 0o500, 256 * 1024)]
    require(Path(__file__) == STAGE / 'guard.py')
    Observer(load_query(query), pins, fd).execute()
    print('K1_MANAGER_PRIVATE_GUARDED_PASS_NOT_PRODUCTION')


if __name__ == '__main__':
    os.umask(0o077)
    try:
        main()
    except BaseException:
        print('K1_MANAGER_PRIVATE_NONPASS_RETAINED', file=sys.stderr)
        sys.exit(2)
