"""Fixed private modeled-manager helper protocol, not privileged generic IPC.

Only the actual retained host role's original Popen pipes are eligible. One
checked full write per snapshot/finish; all first uncertainty is permanent.
No effect retry, failure close, signal, reap or diagnostic query occurs here.
"""
import io
import math
import os
import select
import stat

MAX_LINE = 16384
MAX_REQUESTS = 32


class Refused(RuntimeError):
    def __init__(self):
        super().__init__('fixed_manager_helper_refused')


def require(value):
    if not value:
        raise Refused()


def identity(value):
    return value.st_dev, value.st_ino, value.st_mode, value.st_uid, value.st_gid, value.st_nlink


class Helper:
    def __init__(self, owner, child, ownership, images, image_module, protocol):
        self.owner, self.child, self.ownership, self.images, self.protocol = owner, child, ownership, images, protocol
        self.sealed = True
        self.started = self.finished = self.eof = False
        self.requests = 0
        try:
            require(type(owner) is ownership.Session and owner.kind == 'inner'
                    and type(child) is ownership.OwnedProcess
                    and type(images) is image_module.Images and images.owner is owner
                    and owner.roles.get(id(child)) == 'host'
                    and owner.anchors['host']['child'] is child)
            owner.retained.append(self)
            owner.available()
            owner.live(child)
            owner.available()
            self.input, self.output = child.stdin, child.stdout
            require(type(self.input) is io.FileIO and type(self.output) is io.FileIO)
            self.input_fd, self.output_fd = self.input.fileno(), self.output.fileno()
            require(self.input_fd != self.output_fd)
            self.input_identity = identity(os.fstat(self.input_fd))
            owner.available()
            self.output_identity = identity(os.fstat(self.output_fd))
            owner.available()
            self.sealed = False
            self.check(owner.local_deadline(5))
        except BaseException:
            self.refuse()
            raise Refused() from None

    def refuse(self):
        self.sealed = self.owner.sealed = True

    def available(self, deadline):
        try:
            require(not self.sealed)
            self.owner.within(deadline)
            now = self.ownership.clock()
            require(type(now) is float and math.isfinite(now)
                    and type(deadline) is float and math.isfinite(deadline) and now < deadline)
            return deadline - now
        except BaseException:
            self.refuse()
            raise Refused() from None

    def call(self, deadline, operation, *args, **kwargs):
        try:
            self.available(deadline)
            value = operation(*args, **kwargs)
            self.available(deadline)
            return value
        except BaseException:
            self.refuse()
            raise Refused() from None

    def check(self, deadline):
        try:
            self.available(deadline)
            require(not self.eof and self.child.stdin is self.input and self.child.stdout is self.output
                    and self.input.fileno() == self.input_fd and self.output.fileno() == self.output_fd
                    and self.owner.roles.get(id(self.child)) == 'host'
                    and self.owner.anchors['host']['child'] is self.child)
            self.call(deadline, self.owner.live, self.child)
            for fd, original in ((self.input_fd,self.input_identity),(self.output_fd,self.output_identity)):
                value = self.call(deadline, os.fstat, fd)
                require(stat.S_ISFIFO(value.st_mode) and value.st_uid == value.st_gid == 0
                        and value.st_nlink == 1 and identity(value) == original)
            if self.started:
                self.call(deadline, self.images.executable, self.child, 'host', deadline)
            self.available(deadline)
        except BaseException:
            self.refuse()
            raise Refused() from None

    def line(self, deadline):
        try:
            self.check(deadline)
            data = b''
            while b'\n' not in data:
                self.check(deadline)
                ready, writable, exceptional = self.call(deadline, select.select, [self.output_fd], [], [],
                                                         self.available(deadline))
                require(ready == [self.output_fd] and writable == exceptional == [])
                part = self.call(deadline, os.read, self.output_fd, MAX_LINE + 1 - len(data))
                require(type(part) is bytes and part)
                data += part
                require(len(data) <= MAX_LINE)
            value, remainder = data.split(b'\n', 1)
            require(not remainder and b'\r' not in value)
            self.check(deadline)
            return value + b'\n'
        except BaseException:
            self.refuse()
            raise Refused() from None

    def ready(self, deadline):
        try:
            require(not self.started and not self.finished)
            require(self.line(deadline) == b'fixture-ready\n')
            self.call(deadline, self.images.executable, self.child, 'host', deadline)
            self.started = True
            self.check(deadline)
        except BaseException:
            self.refuse()
            raise Refused() from None

    def snapshot(self, deadline, *, final=False):
        try:
            self.available(deadline)
            require(type(final) is bool and self.started and not self.finished and self.requests < MAX_REQUESTS)
            if final:
                for name in ('core','broker'):
                    row = self.owner.anchors[name]
                    child = row['child']
                    zero = self.owner.zero_reaped.get(id(child))
                    require(row['state'] == 'zero-reaped' and type(child.returncode) is int and child.returncode == 0
                            and type(child.pid) is int and child.pid > 0
                            and type(zero) is tuple and len(zero) == 2
                            and all(type(field) is int for field in zero) and zero == (child.pid,0))
            self.check(deadline)  # Exact live helper/image/pipe immediately BEFORE write.
            command = b'finish\n' if final else b'snapshot\n'
            self.requests += 1  # One attempt is consumed even if the result is unknown.
            written = self.call(deadline, os.write, self.input_fd, command)
            require(type(written) is int and written == len(command))
            raw = self.line(deadline)
            value = self.protocol.decode(raw, MAX_LINE)
            require(type(value) is dict and value.get('notify_alive') is True
                    and value.get('observer_finalized') is final and value.get('monitor_alive') is (not final))
            self.check(deadline)
            self.finished = final
            return value  # Private namespace data; the launcher validates/projects, never raw logs.
        except BaseException:
            self.refuse()
            raise Refused() from None

    def positive_eof(self, deadline):
        try:
            self.available(deadline)
            require(self.finished and not self.eof)
            self.check(deadline)
            # Exact unbuffered FileIO only: no hidden flush, retry or failure close.
            result = self.call(deadline, self.input.close)
            require(result is None)
            self.eof = True
        except BaseException:
            self.refuse()
            raise Refused() from None
