"""Developer-only owned process protocol. No entry point or failure cleanup.

The fixed launcher owns this session. First uncertainty seals before propagation.
An unreaped direct child is the PID reuse anchor; original namespace FDs and
starttime bind the live proc observations. Namespace-init failure parks elsewhere.
"""
import functools
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import tempfile
import time


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
            return method(self, *args, **kwargs)
        except BaseException:
            self.sealed = True
            raise
    return call


def require(value, reason):
    if not value:
        raise Refused(reason)


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
    def __init__(self):
        self.sealed = False
        self.children = []
        self.anchors = {}
        self.retained = []

    def available(self):
        require(not self.sealed, 'session_sealed')

    @guarded
    def perform(self, operation, *args):
        # Only fixed pinned launcher functions may be supplied, never IPC input.
        return operation(*args)

    @guarded
    def spawn(self, argv, **kwargs):
        require(len(self.children) < 96, 'child_count')
        child = OwnedProcess(argv, **kwargs)
        self.children.append(child)
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
        deadline = time.monotonic() + seconds
        while True:
            require(time.monotonic() < deadline, 'owned_deadline')
            if self.observation(child) is None:
                time.sleep(0.02)
                continue
            pid, status = os.waitpid(child.pid, os.WNOHANG)
            require(type(pid) is int and type(status) is int and pid == child.pid
                    and os.WIFEXITED(status) and os.WEXITSTATUS(status) == 0,
                    'exact_zero_reap_unknown')
            child.returncode = 0
            return 0

    @guarded
    def command(self, argv, **kwargs):
        require(type(argv) is list and argv and argv[0] in ('/usr/bin/mount', '/usr/bin/ip')
                and not kwargs, 'fixed_bootstrap_utility')
        with tempfile.TemporaryFile(dir='/tmp') as output, tempfile.TemporaryFile(dir='/tmp') as error:
            child = self.spawn(argv, stdin=subprocess.DEVNULL, stdout=output, stderr=error,
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
        require(name in ('bus', 'resolved') and name not in self.anchors, 'fixed_daemon')
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
        deadline = time.monotonic() + 8
        while True:
            require(time.monotonic() < deadline, 'readiness_deadline')
            for item in self.anchors.values():
                self.live(item['child'])
            try:
                info = os.stat(socket, follow_symlinks=False)
            except FileNotFoundError:
                time.sleep(0.02)
                continue
            require(stat.S_ISSOCK(info.st_mode), 'readiness_socket')
            row['state'] = 'ready'
            return

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
        if name == 'bus':
            require(self.anchors['resolved']['state'] == 'zero-reaped', 'shutdown_order')
        child = row['child']
        self.live(child)
        copies.verify(time.monotonic() + 5)
        require(copies.inventory(child, time.monotonic() + 5) == row['maps'], 'shutdown_maps')
        if name == 'resolved':
            base.verify_child(child, 974, base.RESOLVER_CAPS)
        self.live(child)
        row['state'] = 'shutdown-authorized'
        # Only this positive path authorizes a single signal to the unreaped PID.
        # ESRCH or any error is uncertainty, never permission for another query.
        os.kill(child.pid, signal.SIGTERM)
        row['state'] = 'term-sent'
        self.settle_zero(child, 6)
        # No proc or namespace read after exit. Retained FDs close only at normal
        # interpreter exit, after the complete successful inventory receipt.
        row['state'] = 'zero-reaped'
        return {'pid': child.pid, 'starttime': row['starttime'],
                'namespaces': {k: list(v) for k, v in row['namespaces'].items()},
                'signal': 'SIGTERM', 'signal_count': 1, 'exit_code': 0,
                'state': 'zero-reaped'}
