#!/usr/bin/python3
"""Fixed private lifecycle observer. Unknown children/artifacts are retained."""
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import unicodedata

STAGE = Path('/run/omavless-k1-retained-private-lifecycle')
UNIT = 'omavless-k1-retained-private-lifecycle.service'
PARENT = Path('/run/systemd/system')
LINK = PARENT / UNIT
CGROUP = Path('/sys/fs/cgroup/system.slice') / UNIT
QUERY_SHA = '69d840b5a014f501b0246c78a4e9fa486ad31b86dea31f72eb8ad64e0bcb8f45'
UNIT_SHA = '198730a79751ccee045c6173d4cbb75db7ece33a784f5390f255cb65aa5e72b5'
PROBE_SHA = '0000000000000000000000000000000000000000000000000000000000000000'
NATIVE_SOURCE = '0000000000000000000000000000000000000000'
TEST = 'manager_retained_lifecycle::adapter::run_private_lifecycle'
MARKER = 'OBSERVED_CONFIGURED_FACTS_NOT_LIFECYCLE_ADMISSION'
PHASES = ('preflight', 'before-baseline', 'publish-link', 'native-helper',
          'validate-evidence', 'cleanup-admission', 'unlink-own-link',
          'daemon-reload', 'verify-not-found', 'after-baseline',
          'compare-baseline', 'publish-result')


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
    def __init__(self, path, mode, maximum, expected=None, empty=False):
        self.path, self.maximum = path, maximum
        self.fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK)
        self.meta = os.fstat(self.fd)
        require(stat.S_ISREG(self.meta.st_mode) and self.meta.st_uid == self.meta.st_gid == 0
                and stat.S_IMODE(self.meta.st_mode) == mode and self.meta.st_nlink == 1
                and (0 if empty else 1) <= self.meta.st_size <= maximum and not os.listxattr(self.fd))
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


class DirectoryPin:
    def __init__(self, path):
        self.path = path
        self.fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        self.meta = os.fstat(self.fd)
        require(stat.S_ISDIR(self.meta.st_mode) and self.meta.st_uid == self.meta.st_gid == 0
                and stat.S_IMODE(self.meta.st_mode) == 0o700)
        self.recheck()

    def recheck(self):
        require(directory_identity(self.meta) == directory_identity(os.fstat(self.fd))
                == directory_identity(self.path.lstat()))


def load_query(pin):
    require(pin.sha == QUERY_SHA)
    ns = {'__name__': 'config_reference_definitions_only', '__file__': str(pin.path)}
    exec(compile(pin.data(), '<pinned-readonly-snapshot>', 'exec'), ns)
    ns['STAGE'] = STAGE
    return ns


def validate_state(unit, service):
    require(type(unit) is dict and unit == {'Id': UNIT, 'LoadState': 'loaded', 'FragmentPath': str(LINK),
            'ActiveState': 'inactive', 'SubState': 'dead', 'DropInPaths': [],
            'Job': {'type': '(uo)', 'data': [0, '/']}})
    require(type(unit['Job']['data'][0]) is int)
    require(type(service) is dict and set(service) == {'StandardOutput', 'StandardError',
            'Type', 'User', 'Group', 'WatchdogUSec', 'ExecMainStartTimestampMonotonic',
            'MainPID', 'ControlPID', 'ExecMainPID', 'ControlGroup'})
    for name, expected in {'StandardOutput': 'append', 'StandardError': 'append',
                           'Type': 'oneshot', 'User': 'root', 'Group': 'root', 'ControlGroup': ''}.items():
        require(type(service[name]) is str and service[name] == expected)
    for name in ('MainPID', 'ControlPID', 'ExecMainPID', 'ExecMainStartTimestampMonotonic'):
        require(type(service[name]) is int and service[name] == 0)
    require(type(service['WatchdogUSec']) is int and service['WatchdogUSec'] == 2**64 - 1)


def validate_config(value):
    require(type(value) is dict and set(value) == {'schema', 'marker', 'unit', 'service', 'dump', 'permission_fields'}
            and type(value['schema']) is int and value['schema'] == 2 and value['marker'] == MARKER)
    validate_state(value['unit'], value['service'])
    validate_permissions(value['permission_fields'])
    dump = value['dump']
    require(type(dump) is dict and set(dump) == {'type', 'data'} and dump['type'] == 's'
            and type(dump['data']) is list and len(dump['data']) == 1
            and type(dump['data'][0]) is str and '\0' not in dump['data'][0]
            and 0 < len(dump['data'][0].encode()) <= 1024 * 1024)


def validate_capture(value, ack):
    # Retained pure metadata controls; the real lifecycle uses validate_config
    # and its distinct terminal schema, never an invented post-Unref snapshot.
    validate_config(value)
    require(type(ack) is dict and set(ack) == {'schema', 'unref_acknowledged', 'admission'}
            and type(ack['schema']) is int and ack['schema'] == 2
            and ack['unref_acknowledged'] is True and ack['admission'] is False)


def validate_version(value):
    require(type(value) is dict and set(value) == {'schema', 'marker', 'unique_owner', 'unit',
            'before_ref', 'while_ref_after_dump', 'admission'}
            and type(value['schema']) is int and value['schema'] == 2
            and value['marker'] == 'OBSERVED_VERSION_DATA_NOT_ADMISSION'
            and value['unit'] == UNIT and value['admission'] is False)
    owner = value['unique_owner']
    require(type(owner) is str and owner.startswith(':') and len(owner) <= 255
            and len(owner.split('.')) >= 2
            and all(part and all(c.isascii() and (c.isalnum() or c in '_-') for c in part)
                    for part in owner[1:].split('.')))
    for name in ('before_ref', 'while_ref_after_dump'):
        field = value[name]
        require(type(field) is dict and set(field) == {'type', 'data'} and field['type'] == 's'
                and type(field['data']) is list and len(field['data']) == 1)
        version = field['data'][0]
        require(type(version) is str and 0 < len(version.encode()) <= 256
                and not any(unicodedata.category(c) == 'Cc' for c in version))
    require(value['before_ref'] == value['while_ref_after_dump']
            and value['before_ref']['data'] == ['261.2-1-arch'])


def validate_post_state(value):
    require(type(value) is dict and set(value) == {'schema', 'phase', 'unit', 'service', 'admission', 'permission_fields'}
            and type(value['schema']) is int and value['schema'] == 2
            and value['phase'] == 'post-unref-single-getall' and value['admission'] is False)
    validate_state(value['unit'], value['service'])
    validate_permissions(value['permission_fields'])


def expected_permissions():
    # Fixed public values, independently spelled against the v261 typed getters.
    # These are prerequisites for this inert capture, not namespace/write proof.
    unit, service = {}, {}
    def add(where, signature, mapping):
        for key, value in mapping.items():
            require(key not in where)
            where[key] = {'signature': signature, 'value': value}
    add(unit, 's', dict(Id=UNIT, LoadState='loaded', ActiveState='inactive', SubState='dead',
        FailureAction='none', SuccessAction='none', FragmentPath=str(LINK), Following=''))
    add(unit, 'as', {key: [] for key in ('DropInPaths', 'Wants', 'BindsTo', 'PartOf', 'Upholds',
        'OnFailure', 'OnSuccess', 'TriggeredBy', 'Requisite', 'PropagatesStopTo',
        'StopPropagatedFrom', 'JoinsNamespaceOf')})
    add(unit, 'as', dict(Conflicts=['shutdown.target'], Requires=['sysinit.target', 'system.slice'], Names=[UNIT]))
    add(unit, 'b', dict(RefuseManualStart=False, RefuseManualStop=False))
    add(unit, 't', dict(JobTimeoutUSec=2**64-1, JobRunningTimeoutUSec=2**64-1))
    add(service, 's', dict(Type='oneshot', User='root', Group='root', PrivatePIDs='no', Restart='no',
        KillMode='none', StandardInput='null', StandardOutput='append', StandardError='append',
        ProtectProc='default', ProcSubset='all', RootDirectory='', RootImage='',
        NetworkNamespacePath='', UserNamespacePath='', ControlGroup=''))
    add(service, 'b', dict(RemainAfterExit=True, NoNewPrivileges=True, PrivateNetwork=True,
        PrivateUsers=False, Delegate=False, SendSIGKILL=False))
    add(service, 't', dict(CapabilityBoundingSet=0x1000, AmbientCapabilities=0, RestrictNamespaces=0,
        TimeoutStartUSec=2**64-1, TimeoutStopUSec=2**64-1, RuntimeMaxUSec=2**64-1,
        WatchdogUSec=2**64-1, ExecMainStartTimestampMonotonic=0))
    add(service, 'u', dict(MainPID=0, ControlPID=0, ExecMainPID=0, FileDescriptorStoreMax=0,
        NFileDescriptorStore=0, UMask=0o77))
    add(service, 'as', {key: [] for key in ('PassEnvironment', 'UnsetEnvironment', 'SupplementaryGroups',
        'ExtraFileDescriptorNames', 'ExtensionDirectories')})
    add(service, 'as', dict(Environment=['OMAVLESS_K1_RETAINED_LIFECYCLE_WRITER=1']))
    add(service, 'a(sb)', dict(EnvironmentFiles=[]))
    add(service, '(bas)', dict(SystemCallFilter=[False, []]))
    add(service, 'a(sst)', dict(OpenFile=[['/proc/1/ns/net', 'k1-host-netns', 1]]))
    add(service, 'a(ssbt)', dict(BindPaths=[], BindReadOnlyPaths=[]))
    add(service, 'a(ss)', dict(TemporaryFileSystem=[]))
    add(service, 'a(sba(ss))', dict(ExtensionImages=[]))
    add(service, 'a(ssba(ss))', dict(MountImages=[]))
    add(service, 'a(sasbttttuii)', {key: [] for key in ('ExecCondition', 'ExecStartPre', 'ExecStartPost',
        'ExecReload', 'ExecReloadPost', 'ExecStop', 'ExecStopPost')})
    probe = str(STAGE / 'probe')
    writer = 'kernel_observer::creator_lifecycle::response_diagnostic::manager_private_lifecycle'
    add(service, 'a(sasbttttuii)', dict(ExecStart=[[probe,
        [probe, '--exact', writer, '--ignored', '--nocapture', '--test-threads=1'],
        False, 0, 0, 0, 0, 0, 0, 0]]))
    return {'unit': unit, 'service': service}


def strict_equal(actual, expected):
    require(type(actual) is type(expected))
    if type(expected) is dict:
        require(set(actual) == set(expected))
        for key in expected:
            strict_equal(actual[key], expected[key])
    elif type(expected) is list:
        require(len(actual) == len(expected))
        for left, right in zip(actual, expected):
            strict_equal(left, right)
    else:
        require(actual == expected)


def validate_permissions(value):
    expected = expected_permissions()
    require(type(value) is dict and type(value.get('unit')) is dict)
    requires = value['unit'].get('Requires')
    require(type(requires) is dict and set(requires) == {'signature', 'value'}
            and requires['signature'] == 'as')
    if requires['value'] == ['system.slice', 'sysinit.target']:
        expected['unit']['Requires']['value'].reverse()
    strict_equal(value, expected)


def validate_rpc(index, before, response, version, data):
    strict_equal(before, {'schema': 1, 'diagnostic': True, 'boundary': 'before-rpc', 'admission': False})
    require(type(response) is dict and set(response) == {'schema', 'diagnostic', 'admission', 'validation_passed', 'facts'})
    strict_equal({key: response[key] for key in response if key != 'facts'},
                 {'schema': 1, 'diagnostic': True, 'admission': False, 'validation_passed': True})
    facts = response['facts']
    def selected(side, row):
        require(type(row) is dict and set(row) == {'selected', 'mismatch_fields'} and row['mismatch_fields'] == [])
        require(type(row['mismatch_fields']) is list and type(row['selected']) is dict)
        expected = expected_permissions()[side]
        if side == 'unit':
            expected['Job'] = {'signature': '(uo)', 'value': [0, '/']}
        require(set(row['selected']) == set(expected))
        actual = {}
        for key, entry in row['selected'].items():
            require(type(entry) is dict and set(entry) == {'present', 'variant'} and entry['present'] is True)
            actual[key] = entry['variant']
        if side == 'unit' and actual['Requires'] == {'signature': 'as', 'value': ['system.slice', 'sysinit.target']}:
            expected['Requires']['value'].reverse()
        strict_equal(actual, expected)
    if index == 0:
        strict_equal(facts, {'owner': version['unique_owner']})
    elif index in (1, 6):
        strict_equal(facts, {'version': '261.2-1-arch'})
    elif index in (2, 7):
        strict_equal(facts, {'empty_ack': True})
    elif index in (3, 4):
        selected('unit' if index == 3 else 'service', facts)
    elif index == 5:
        strict_equal(facts, {'dump': data['dump']['data'][0], 'mismatch_fields': []})
    else:
        require(index == 8 and type(facts) is dict and set(facts) == {'unit', 'service'})
        selected('unit', facts['unit']); selected('service', facts['service'])


def validate_lifecycle(value, native, stopped, owner):
    require(type(value) is dict and set(value) == {'schema', 'unit', 'unique_owner',
        'start_job', 'stop_job', 'execution', 'effects', 'closed_retired', 'absent',
        'stopped', 'unref_acknowledged', 'synthetic_epoch', 'production_admission'})
    strict_equal({k: v for k, v in value.items() if k not in ('start_job', 'stop_job', 'execution')},
        {'schema': 1, 'unit': UNIT, 'unique_owner': owner, 'effects': 2, 'closed_retired': True,
         'absent': True, 'stopped': True, 'unref_acknowledged': True,
         'synthetic_epoch': True, 'production_admission': False})
    for key in ('start_job', 'stop_job'):
        job = value[key]
        require(type(job) is dict and set(job) == {'id', 'path'} and type(job['id']) is int
                and 0 < job['id'] <= 2**32-1
                and job['path'] == f'/org/freedesktop/systemd1/job/{job["id"]}')
    require(value['start_job'] != value['stop_job'])
    execution = value['execution']
    require(type(execution) is dict and set(execution) ==
            {'invocation', 'pid', 'start', 'exit', 'command_start', 'command_exit'})
    invocation = execution['invocation']
    require(type(invocation) is list and len(invocation) == 16
            and all(type(v) is int and 0 <= v <= 255 for v in invocation) and any(invocation))
    require(type(execution['pid']) is int and 1 < execution['pid'] < 2**32)
    for key in ('start', 'exit', 'command_start', 'command_exit'):
        require(type(execution[key]) is int and 0 < execution[key] < 2**64-1)
    require(execution['exit'] >= execution['start']
            and execution['command_exit'] >= execution['command_start'])
    strict_equal(native, {'schema': 1, 'execution': execution, 'effects': 2,
        'closed_retired': True, 'absent': True, 'synthetic_epoch': True})
    strict_equal(stopped, {'schema': 1, 'execution': execution, 'inactive_dead': True,
        'zero_pids': True, 'no_job': True, 'empty_cgroup': True})


class Observer:
    def __init__(self, ns, pins, directory_fd, parent_fd):
        self.ns, self.pins, self.directory_fd, self.parent_fd = ns, pins, directory_fd, parent_fd
        self.directory, self.parent = os.fstat(directory_fd), os.fstat(parent_fd)
        self.sealed, self.link_identity = False, None
        self.lifecycle_validated = False
        self.helper_known_zero = False
        self.phase = 'not-entered'

    def available(self):
        require(not self.sealed and not self.ns['UNCERTAIN'] and not self.ns['ACTIVATION_REFUSED'])

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
        self.ns['retained_activation']()
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
        self.persist(name, value)

    def persist(self, name, value):
        # The sole failure-path operation is fixed private evidence publication
        # through this retained root-owned FD. No manager/process/path query.
        require(directory_identity(self.directory) == directory_identity(os.fstat(self.directory_fd)))
        fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                     0o600, dir_fd=self.directory_fd)
        with os.fdopen(fd, 'w') as stream:
            json.dump(value, stream, sort_keys=True)
            stream.flush()
            os.fsync(stream.fileno())
        os.fsync(self.directory_fd)

    def before(self, phase):
        self.available()
        index = PHASES.index(phase)
        require(index == (0 if self.phase == 'not-entered' else PHASES.index(self.phase) + 1))
        self.phase = phase
        self.write(f'phase-{index:02d}-{phase}.json',
                   {'schema': 1, 'phase': phase, 'boundary': 'before', 'admission': False})

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
                    env={'PATH': '/usr/bin', 'LC_ALL': 'C', 'OMAVLESS_K1_RETAINED_LIFECYCLE': '1'})
            code = self.ns['await_child'](child, seconds=180)
            require(type(code) is int and code == 0)
            self.helper_known_zero = True
            require(all(os.fstat(fd).st_size <= 65536 for fd in logs))
            for fd in logs:
                os.fsync(fd)
        finally:
            # Child retains its own descriptors on unknown exit; no deletion or
            # process query occurs here, including no Popen destructor polling.
            for fd in logs:
                os.close(fd)

    def evidence(self):
        version = Pin(STAGE / 'private-admission-manager-version.json', 0o600, 4096)
        data = Pin(STAGE / 'private-admission-config-data.json', 0o600, 2 * 1024 * 1024)
        version_value = json.loads(version.data(), object_pairs_hook=pairs)
        data_value = json.loads(data.data(), object_pairs_hook=pairs)
        validate_version(version_value)
        validate_config(data_value)
        self.pins.extend([version, data])
        for index in range(7):
            before = Pin(STAGE / f'rpc-{index:02}-before.json', 0o600, 4096)
            response = Pin(STAGE / f'rpc-{index:02}-response.json', 0o600, 2 * 1024 * 1024)
            validate_rpc(index, json.loads(before.data(), object_pairs_hook=pairs),
                         json.loads(response.data(), object_pairs_hook=pairs), version_value, data_value)
            self.pins.extend([before, response])
        proofs = [Pin(STAGE / name, 0o600, 16384) for name in
                  ('lifecycle-result.json', 'lifecycle-native-proof.json', 'lifecycle-stopped-proof.json')]
        validate_lifecycle(*(json.loads(pin.data(), object_pairs_hook=pairs) for pin in proofs),
                           version_value['unique_owner'])
        self.pins.extend(proofs)
        self.native_evidence()
        self.lifecycle_validated = True

    def native_evidence(self):
        # Active lifecycle evidence, not the inert capture's never-started proof.
        witness_pin = Pin(STAGE / 'host-negative-witness.json', 0o600, 2048)
        witness = json.loads(witness_pin.data(), object_pairs_hook=pairs)
        stage_meta = os.fstat(self.directory_fd)
        require(type(witness) is dict and set(witness) == {'schema', 'fixture_unit',
            'stage_device', 'stage_inode', 'namespace_device', 'namespace_inode',
            'negative_witness_only', 'canonical_authority'})
        for key in ('namespace_device', 'namespace_inode'):
            require(type(witness[key]) is int and 0 < witness[key] < 2**64)
        strict_equal({k: v for k, v in witness.items() if k not in ('namespace_device', 'namespace_inode')},
            {'schema': 1, 'fixture_unit': UNIT, 'stage_device': stage_meta.st_dev,
             'stage_inode': stage_meta.st_ino, 'negative_witness_only': True, 'canonical_authority': False})
        self.pins.append(witness_pin)
        for path in (STAGE / 'state', STAGE / 'state/omavless-netguard'):
            self.pins.append(DirectoryPin(path))
        specifications = (
            ('native-result.json', {'schema': 1, 'synthetic_epoch': True, 'effects': 2,
                'full_inventory': True, 'second_socket_untrusted': True, 'absent': True}),
            ('state/omavless-netguard/armed-v1.json', {'version': 1, 'policy_version': 1,
                'enrolled_uid': 1001, 'generation': 7, 'armed': False, 'flags': 0}))
        for name, expected in specifications:
            pin = Pin(STAGE / name, 0o600, 16384)
            strict_equal(json.loads(pin.data(), object_pairs_hook=pairs), expected)
            self.pins.append(pin)
        pin = Pin(STAGE / 'state/omavless-netguard/table-receipt-v1.json', 0o600, 16384)
        receipt = json.loads(pin.data(), object_pairs_hook=pairs)
        require(type(receipt) is dict and set(receipt) == {'version', 'enrolled_uid', 'boot',
            'host_netns_epoch', 'netns_device', 'netns_inode', 'operation', 'phase', 'table_handle'})
        for key in ('netns_device', 'netns_inode'):
            require(type(receipt[key]) is int and 0 < receipt[key] < 2**64)
        require((receipt['netns_device'], receipt['netns_inode']) !=
                (witness['namespace_device'], witness['namespace_inode']))
        strict_equal({k: v for k, v in receipt.items() if k not in ('netns_device', 'netns_inode')},
            {'version': 1, 'enrolled_uid': 1001, 'boot': [0x31]*16, 'host_netns_epoch': [0x32]*16,
             'operation': 2, 'phase': 'retired', 'table_handle': 0})
        self.pins.append(pin)
        for name in ('native.stdout', 'native.stderr'):
            self.pins.append(Pin(STAGE / name, 0o600, 65536, empty=True))

    def cleanup_known_success(self):
        self.before('cleanup-admission')
        require(self.lifecycle_validated and self.helper_known_zero)
        self.recheck(linked=True)
        # The same retained helper completed native proof and exact Stop before
        # Unref. No post-Unref manager query can heal an incomplete lifecycle.
        require(not CGROUP.exists() and not CGROUP.is_symlink())
        self.recheck(linked=True)
        self.before('unlink-own-link')
        os.unlink(UNIT, dir_fd=self.parent_fd)
        os.fsync(self.parent_fd)
        self.before('daemon-reload')
        self.call(['/usr/bin/systemctl', 'daemon-reload'])
        self.before('verify-not-found')
        require(self.properties(['LoadState']) == {'LoadState': 'not-found'})
        require(not LINK.exists() and not LINK.is_symlink()
                and not CGROUP.exists() and not CGROUP.is_symlink())

    def once(self):
        require(set(os.listdir(self.directory_fd)) == {'guard.py', 'query-guard.py', 'fixture.service', 'probe'})
        self.before('preflight')
        self.recheck()
        require(self.call(['/usr/bin/systemd-detect-virt', '--vm']).strip() == b'kvm')
        require(not LINK.exists() and not LINK.is_symlink() and not CGROUP.exists() and not CGROUP.is_symlink())
        require(self.properties(['LoadState']) == {'LoadState': 'not-found'})
        self.write('invocation.json', {'schema': 2, 'native_source': NATIVE_SOURCE, 'admission': False})
        self.before('before-baseline')
        before = self.ns['snapshot']()
        self.write('baseline-before.json', before)
        self.recheck()
        self.before('publish-link')
        os.symlink(str(STAGE / 'fixture.service'), UNIT, dir_fd=self.parent_fd)
        self.link_identity = identity(os.stat(UNIT, dir_fd=self.parent_fd, follow_symlinks=False))
        os.fsync(self.parent_fd)
        self.recheck(linked=True)
        # RefUnit is the first load after publication. No initial daemon-reload.
        self.before('native-helper')
        self.helper()
        self.before('validate-evidence')
        self.evidence()
        self.cleanup_known_success()
        self.before('after-baseline')
        after = self.ns['snapshot']()
        self.write('baseline-after.json', after)
        self.before('compare-baseline')
        require(self.ns['preserve'](before, after))
        self.recheck()
        self.before('publish-result')
        self.write('result.json', {'schema': 3, 'marker': 'PRIVATE_LIFECYCLE_PRESERVED_NOT_PRODUCTION', 'preserved': True,
                                  'native_source': NATIVE_SOURCE, 'probe_sha256': PROBE_SHA,
                                  'unref_acknowledged': True, 'native_closed_retired': True,
                                  'private_lifecycle': True, 'admission': False})

    def execute(self):
        self.available()
        try:
            self.once()
        except BaseException:
            self.sealed = True
            try:
                self.persist('refusal.json', {'schema': 1, 'phase': self.phase,
                    'reason': 'BOUNDARY_REFUSED_NO_RETRY',
                    'helper_known_zero': self.helper_known_zero,
                    'cleanup_authorized': False, 'admission': False})
            except BaseException:
                # Unknown evidence write is terminal too; never retry it.
                pass
            raise


def main():
    require(os.getuid() == os.geteuid() == os.getgid() == os.getegid() == 0
            and len(sys.argv) == 1 and os.environ.get('OMAVLESS_K1_RETAINED_LIFECYCLE_GUARD') == '1')
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
    print('K1_RETAINED_PRIVATE_LIFECYCLE_PRESERVED_NOT_PRODUCTION')


if __name__ == '__main__':
    os.umask(0o077)
    try:
        main()
    except BaseException:
        print('K1_RETAINED_LIFECYCLE_NONPASS_RETAINED', file=sys.stderr)
        sys.exit(2)
