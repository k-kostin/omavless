"""Developer-only positive-composition ownership protocol. No entry point or failure cleanup.

The fixed launcher owns this session. First uncertainty seals before propagation.
An unreaped direct child is the PID reuse anchor; original namespace FDs and
starttime bind the live proc observations. Namespace-init failure parks elsewhere.
"""
import functools
import math
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import tempfile
import time

STAGE_SCRATCH = '/home/kdk_vm/.cache/t3-retained-positive-composition-review-1/scratch'


class Refused(RuntimeError):
    pass


class OwnedProcess(subprocess.Popen):
    def __del__(self):
        pass

    def _internal_poll(self, *args, **kwargs):
        return self.returncode

    def poll(self):
        raise Refused('implicit_poll_forbidden')

    def wait(self, *args, **kwargs):
        raise Refused('implicit_wait_forbidden')


def guarded(method):
    @functools.wraps(method)
    def call(self, *args, **kwargs):
        self.available()
        try:
            result = method(self, *args, **kwargs)
            self.available()
            return result
        except BaseException:
            self.sealed = True
            raise
    return call


def require(value, reason):
    if not value:
        raise Refused(reason)


def clock():
    value = time.monotonic()
    require(type(value) is float and math.isfinite(value), 'typed_finite_clock')
    return value


def bounded(path, maximum, directory=None):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC,
                 dir_fd=directory)
    # Retaining or closing an owned read FD is not process cleanup.
    try:
        data = os.read(fd, maximum + 1)
        require(len(data) <= maximum, 'proc_bound')
        return data.decode('ascii')
    finally:
        os.close(fd)


def ns_identity(fd):
    value = os.fstat(fd)
    return value.st_dev, value.st_ino


class Session:
    def __init__(self, kind):
        self.sealed = True
        self.kind = kind
        self.children = []
        self.roles = {}
        self.zero_reaped = {}
        self.anchors = {}
        self.retained = []
        self.isolated = False
        require(kind in ('inner', 'outer'), 'fixed_owner_kind')
        start = clock()
        require(type(start) is float and math.isfinite(start), 'initial_clock')
        self.deadline = start + 90.0
        require(math.isfinite(self.deadline) and start < self.deadline, 'initial_clock')
        self.sealed = False

    def available(self):
        try:
            now = clock()
            require(not self.sealed and type(now) is float and math.isfinite(now)
                    and type(self.deadline) is float and math.isfinite(self.deadline)
                    and now < self.deadline, 'session_sealed')
        except BaseException:
            self.sealed = True
            raise

    @guarded
    def local_deadline(self, seconds):
        require(type(seconds) is int and seconds in (5, 6, 8, 65), 'fixed_deadline')
        now = clock()
        computed = now + seconds
        require(type(computed) is float and math.isfinite(computed) and now < computed,
                'typed_finite_deadline')
        deadline = min(computed, self.deadline)
        self.within(deadline)
        return deadline

    @guarded
    def within(self, deadline):
        require(not self.sealed and type(deadline) is float and math.isfinite(deadline)
                and type(self.deadline) is float and math.isfinite(self.deadline)
                and clock() < min(deadline, self.deadline), 'owned_deadline')

    @guarded
    def perform(self, operation, *args):
        # Only fixed pinned launcher functions may be supplied, never IPC input.
        return operation(*args)

    @guarded
    def spawn(self, argv, *, role, **kwargs):
        require(len(self.children) < 96 and role in ('utility','namespace','bus','resolved','core','broker','host'), 'child_count')
        require((self.kind == 'outer' and role == 'namespace' and not self.children)
                or (self.kind == 'inner' and role != 'namespace'), 'fixed_role_scope')
        require(role == 'utility' or role not in self.roles.values(), 'duplicate_role')
        child = OwnedProcess(argv, **kwargs)
        self.children.append(child)
        self.roles[id(child)] = role
        return child

    @guarded
    def observation(self, child):
        require(any(c is child for c in self.children) and child.returncode is None
                and type(child.pid) is int and child.pid > 0, 'owned_child')
        seen = os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        if seen is None:
            return None
        require(type(seen.si_pid) is int and seen.si_pid == child.pid
                and type(seen.si_code) is int and seen.si_code == os.CLD_EXITED
                and type(seen.si_status) is int and seen.si_status == 0,
                'nonzero_or_unknown_terminal')
        return 0

    @guarded
    def settle_zero(self, child, seconds):
        require(type(seconds) is int and seconds in (5, 6, 65), 'fixed_deadline')
        deadline = self.local_deadline(seconds)
        while True:
            self.within(deadline)
            observed = self.observation(child)
            self.within(deadline)
            if observed is None:
                time.sleep(0.02)
                self.within(deadline)
                continue
            self.within(deadline)
            pid, status = os.waitpid(child.pid, os.WNOHANG)
            require(type(pid) is int and type(status) is int and pid == child.pid
                    and os.WIFEXITED(status) and os.WEXITSTATUS(status) == 0,
                    'exact_zero_reap_unknown')
            child.returncode = 0
            self.zero_reaped[id(child)] = (pid, status)
            self.within(deadline)
            return 0

    @guarded
    def command(self, argv, **kwargs):
        require(type(argv) is list and argv and argv[0] in ('/usr/bin/mount', '/usr/bin/ip')
                and not kwargs, 'fixed_bootstrap_utility')
        scratch = '/tmp' if self.isolated else STAGE_SCRATCH
        require(self.isolated or os.environ.get('TMPDIR') == scratch, 'fixed_bootstrap_scratch')
        with tempfile.TemporaryFile(dir=scratch) as output, tempfile.TemporaryFile(dir=scratch) as error:
            child = self.spawn(argv, role='utility', stdin=subprocess.DEVNULL, stdout=output, stderr=error,
                               env={'PATH': '/usr/bin', 'LANG': 'C', 'LC_ALL': 'C'})
            self.settle_zero(child, 5)
            output.seek(0)
            error.seek(0)
            stdout, stderr = output.read(131073), error.read(131073)
            require(len(stdout) <= 131072 and len(stderr) <= 131072, 'utility_output_bound')
            return subprocess.CompletedProcess(argv, 0, stdout, stderr)

    @guarded
    def no_directory_fds(self):
        directory = os.open('/proc/self/fd', os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        try:
            identity = ns_identity(directory)
            seen = set()
            with os.scandir(directory) as entries:
                for entry in entries:
                    require(re.fullmatch(r'0|[1-9][0-9]{0,9}', entry.name)
                            and int(entry.name) <= 2**31 - 1 and entry.name not in seen
                            and len(seen) < 128, 'fd_inventory_shape')
                    seen.add(entry.name)
                    value = os.fstat(int(entry.name))
                    require(not stat.S_ISDIR(value.st_mode)
                            or (value.st_dev, value.st_ino) == identity, 'outside_directory_fd')
            require(str(directory) in seen, 'fd_inventory_incomplete')
            self.isolated = True
        finally:
            os.close(directory)

    def _proc_identity(self, child, row):
        require(os.readlink('/proc/self') == str(os.getpid()), 'proc_topology')
        current = os.stat(f'/proc/{child.pid}', follow_symlinks=False)
        require(stat.S_ISDIR(current.st_mode) and ns_identity(row['proc_fd'])
                == row['proc_identity'] == (current.st_dev, current.st_ino), 'proc_anchor')
        raw = bounded('stat', 4096, row['proc_fd'])
        prefix, fields = raw.rsplit(')', 1)
        require(prefix.split(' (', 1)[0] == str(child.pid), 'proc_pid')
        fields = fields.split()
        require(len(fields) >= 20 and fields[0] not in ('Z', 'X')
                and fields[1] == str(os.getpid()) and fields[19].isdigit(), 'proc_direct_live')
        return int(fields[19])

    @guarded
    def anchor(self, name, child):
        require(self.kind == 'inner' and name in ('bus','resolved','core','broker','host')
                and name not in self.anchors and self.roles.get(id(child)) == name, 'fixed_daemon')
        require(self.observation(child) is None, 'early_exit')
        record = {'child': child, 'state': 'spawned', 'fds': {}, 'namespaces': {}}
        self.anchors[name] = record
        record['proc_fd'] = os.open(f'/proc/{child.pid}', os.O_RDONLY | os.O_DIRECTORY
                                   | os.O_NOFOLLOW | os.O_CLOEXEC)
        record['proc_identity'] = ns_identity(record['proc_fd'])
        record['starttime'] = self._proc_identity(child, record)
        for kind in ('pid', 'net'):
            fd = os.open('ns/' + kind, os.O_RDONLY | os.O_CLOEXEC, dir_fd=record['proc_fd'])
            record['fds'][kind] = fd
            identity = ns_identity(fd)
            own = os.stat('/proc/self/ns/' + kind)
            require(identity == (own.st_dev, own.st_ino), 'daemon_namespace')
            record['namespaces'][kind] = identity
        self.live(child)
        return record

    @guarded
    def live(self, child):
        require(self.observation(child) is None, 'daemon_early_exit')
        records = [row for row in self.anchors.values() if row['child'] is child]
        require(len(records) == 1, 'missing_anchor')
        row = records[0]
        require(row['state'] in ('spawned', 'ready', 'mapped'), 'daemon_phase')
        require(self._proc_identity(child, row) == row['starttime'], 'daemon_starttime')
        for kind, fd in row['fds'].items():
            current = os.stat('ns/' + kind, dir_fd=row['proc_fd'])
            own = os.stat('/proc/self/ns/' + kind)
            require(ns_identity(fd) == row['namespaces'][kind]
                    == (current.st_dev, current.st_ino) == (own.st_dev, own.st_ino),
                    'daemon_namespace_changed')
        require(self.observation(child) is None, 'daemon_early_exit')
        return None

    @guarded
    def ready(self, name):
        row = self.anchors[name]
        require(row['state'] == 'spawned', 'ready_phase')
        socket = '/run/dbus/system_bus_socket' if name == 'bus' else '/run/systemd/resolve/io.systemd.Resolve'
        deadline = self.local_deadline(8)
        while True:
            self.within(deadline)
            for item in self.anchors.values():
                self.live(item['child'])
                self.within(deadline)
            try:
                info = os.stat(socket, follow_symlinks=False)
            except FileNotFoundError:
                self.within(deadline)
                time.sleep(0.02)
                self.within(deadline)
                continue
            self.within(deadline)
            require(stat.S_ISSOCK(info.st_mode), 'readiness_socket')
            row['state'] = 'ready'
            return

    @guarded
    def native_ready(self, name):
        require(name in ('core','broker','host'), 'fixed_native_role')
        row = self.anchors[name]
        require(row['state'] == 'spawned', 'native_ready_phase')
        self.live(row['child'])
        row['state'] = 'ready'

    @guarded
    def mapped(self, name, initial, final):
        row = self.anchors[name]
        require(row['state'] == 'ready' and initial == final and initial, 'mapped_phase')
        self.live(row['child'])
        row['maps'] = final
        row['state'] = 'mapped'

    @guarded
    def shutdown(self, name, copies, base):
        row = self.anchors[name]
        require(row['state'] == 'mapped', 'shutdown_phase')
        require(name in ('core','broker','resolved','bus'), 'fixed_shutdown_role')
        required = {'core': (), 'broker': ('core',),
                    'resolved': ('core','broker','host'),
                    'bus': ('core','broker','host','resolved')}[name]
        require(all(self.anchors[item]['state'] == 'zero-reaped' for item in required),
                'positive_shutdown_order')
        child = row['child']
        self.live(child)
        deadline = self.local_deadline(5)
        copies.verify(deadline)
        self.within(deadline)
        require(copies.inventory(child, deadline) == row['maps'], 'shutdown_maps')
        self.within(deadline)
        if name == 'resolved':
            base.verify_child(child, 974, base.RESOLVER_CAPS)
            self.within(deadline)
        self.live(child)
        self.within(deadline)
        row['state'] = 'shutdown-authorized'
        # Only this positive path authorizes a single signal to the unreaped PID.
        # ESRCH or any error is uncertainty, never permission for another query.
        os.kill(child.pid, signal.SIGTERM)
        self.within(deadline)
        row['state'] = 'term-sent'
        self.settle_zero(child, 6)
        # No proc or namespace read after exit. Retained FDs close only at normal
        # interpreter exit, after the complete successful inventory receipt.
        row['state'] = 'zero-reaped'
        return {'pid': child.pid, 'starttime': row['starttime'],
                'namespaces': {k: list(v) for k, v in row['namespaces'].items()},
                'signal': 'SIGTERM', 'signal_count': 1, 'exit_code': 0,
                'state': 'zero-reaped'}

    @guarded
    def host_finished_zero(self):
        row = self.anchors['host']
        require(row['state'] == 'mapped' and self.anchors['core']['state'] == 'zero-reaped'
                and self.anchors['broker']['state'] == 'zero-reaped', 'host_finish_order')
        self.settle_zero(row['child'], 6)
        row['state'] = 'zero-reaped'
        return {'state': 'zero-reaped', 'exit_code': 0, 'signal_count': 0}

    @guarded
    def complete(self):
        expected = {'namespace'} if self.kind == 'outer' else {'bus','resolved','core','broker','host'}
        actual = set(self.roles.values()) - {'utility'}
        require(actual == expected and len(self.roles) == len(self.children), 'complete_roles')
        require(set(self.zero_reaped) == set(self.roles), 'complete_zero_reap_ledger')
        require(all(type(child.returncode) is int and child.returncode == 0
                    for child in self.children), 'complete_exact_zero')
        require(all(self.zero_reaped[id(child)] == (child.pid, 0) for child in self.children),
                'complete_original_raw_status')
        if self.kind == 'inner':
            require(set(self.anchors) == expected
                    and all(row['state'] == 'zero-reaped' for row in self.anchors.values()),
                    'complete_anchors')
        return {'schema': 'retained-positive-zero-ledger-v1', 'scope': self.kind,
                'roles': sorted(actual), 'owned_child_count': len(self.children),
                'utility_count': list(self.roles.values()).count('utility'),
                'all_owned_direct_children_exact_zero_reaped': True,
                'global_shared_argv_or_uid_absence_claimed': False,
                'production_effect_authority': False}
