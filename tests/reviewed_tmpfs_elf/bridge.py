"""Developer-only private-copy integration; inert until called by a reviewed fixture.

The caller supplies the exact pinned containment/admission modules, never user
callbacks. This module has no entry point, source discovery, or subprocess API.
"""
import fcntl
import os
from pathlib import Path
import re
import stat
import time

STORE = "/elf-copy-store"
FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
PUBLIC = re.compile(r"/usr/(?:lib|bin)/[A-Za-z0-9_./+:-]+\Z")


class Refused(RuntimeError):
    pass


def require(value, reason):
    if not value:
        raise Refused(reason)


def bounded_text(path, limit):
    fd = os.open(path, FLAGS)
    try:
        data = b""
        while True:
            block = os.read(fd, min(65536, limit + 1 - len(data)))
            if not block:
                break
            data += block
            require(len(data) <= limit, "text_bound")
        return data.decode("ascii")
    finally:
        os.close(fd)


def mount_policy(path, readonly):
    rows = []
    for line in bounded_text("/proc/self/mountinfo", 1024 * 1024).splitlines():
        row = line.split()
        require(len(row) >= 10, "mount_shape")
        if row[4] == path:
            rows.append(row)
    require(len(rows) == 1, "mount_unique")
    row = rows[0]
    split = row.index("-")
    require(split >= 6 and len(row) == split + 4 and row[split + 1] == "tmpfs",
            "mount_tmpfs")
    flags, superflags = set(row[5].split(",")), set(row[split + 3].split(","))
    expected, opposite = ("ro", "rw") if readonly else ("rw", "ro")
    require({"nosuid", "nodev", expected} <= flags and "noexec" not in flags
            and opposite not in flags and expected in superflags and opposite not in superflags,
            "mount_policy")


def no_writable_fds(device):
    # Retain the enumeration FD itself: no blanket EBADF exception can conceal
    # an unknown descriptor. This fixture is single-threaded during admission.
    directory = os.open("/proc/self/fd", FLAGS | os.O_DIRECTORY)
    try:
        names = os.listdir(directory)
        require(len(names) <= 128 and all(n.isascii() and n.isdigit() for n in names),
                "fd_inventory_bound")
        for name in names:
            fd = int(name)
            value = os.fstat(fd)
            if value.st_dev == device:
                require(fcntl.fcntl(fd, fcntl.F_GETFL) & os.O_ACCMODE == os.O_RDONLY,
                        "copy_writable_fd")
    finally:
        os.close(directory)


def map_objects(text):
    require(type(text) is str and len(text) <= 1024 * 1024, "mapping_bound")
    objects = {}
    for line in text.splitlines():
        row = line.split(maxsplit=5)
        require(len(row) >= 5, "mapping_shape")
        if len(row) == 5 or row[5].startswith("["):
            continue
        path = row[5]
        require(len(path) <= 4096 and PUBLIC.fullmatch(path)
                and all(p not in ("", ".", "..") for p in path.split("/")[1:]),
                "mapping_nonpublic_or_deleted")
        major, minor = row[3].split(":")
        identity = os.makedev(int(major, 16), int(minor, 16)), int(row[4])
        require(identity[1] > 0 and (path not in objects or objects[path] == identity),
                "mapping_identity_conflict")
        objects[path] = identity
        require(len(objects) <= 64, "mapping_count")
    require(objects, "mapping_empty")
    return objects


class Bridge:
    def __init__(self, base, admission, manifest_bytes):
        self.base, self.admission = base, admission
        self.data = manifest_bytes
        self.manifest = admission.manifest(manifest_bytes)
        self.state = "new"
        self.fds, self.records = {}, {}
        self.store_fd, self.store_identity = None, None

    def _require_state(self, expected):
        require(self.state == expected, "bridge_sealed")
        if self.base.UNSETTLED:
            self.state = "refused"
            raise Refused("owned_state_quarantined")

    def _command(self, argv):
        require(not self.base.UNSETTLED, "owned_state_quarantined")
        self.base.command(argv)
        require(not self.base.UNSETTLED, "owned_state_quarantined")

    def _boundary(self, original):
        require(os.geteuid() == os.getegid() == 0 and set(original) == set(self.base.NS)
                and all(self.base.namespace(n) != original[n] for n in self.base.NS),
                "private_namespace_boundary")
        self.base.validate_maps(bounded_text("/proc/self/uid_map", 4096),
                                bounded_text("/proc/self/gid_map", 4096),
                                bounded_text("/proc/self/setgroups", 128))
        require(os.readlink("/proc/self") == str(os.getpid()) and os.stat("/").st_uid == 0,
                "private_proc_root")
        mount_policy("/", False)

    def _copy(self, sources, path, name, store_fd, deadline):
        source, original = sources.files[path]
        row = sources.table[path]
        fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW
                     | os.O_CLOEXEC, 0o600, dir_fd=store_fd)
        try:
            os.lseek(source, 0, os.SEEK_SET)
            count = 0
            while count < original.st_size:
                require(time.monotonic() < deadline, "copy_deadline")
                block = os.read(source, min(65536, original.st_size - count))
                require(block and os.write(fd, block) == len(block), "copy_short_io")
                count += len(block)
            require(os.read(source, 1) == b"", "copy_source_growth")
            os.fchmod(fd, row["mode"] & 0o777)
            os.fsync(fd)
            value = os.fstat(fd)
            require(stat.S_ISREG(value.st_mode) and value.st_uid == value.st_gid == 0
                    and value.st_nlink == 1 and value.st_size == original.st_size
                    and value.st_mode == row["mode"]
                    and value.st_dev == os.fstat(store_fd).st_dev, "copy_shape")
        finally:
            os.close(fd)
        retained = os.open(name, FLAGS, dir_fd=store_fd)
        self.fds[path] = retained
        require(self.admission.identity(os.fstat(retained)) == self.admission.identity(value)
                and self.admission.digest(retained, value.st_size, deadline) == row["sha256"],
                "copy_changed")
        self.records[path] = {"device": value.st_dev, "inode": value.st_ino,
                              "size": value.st_size, "mode": value.st_mode,
                              "uid": 0, "gid": 0, "nlink": 1, "sha256": row["sha256"],
                              "source_device": original.st_dev, "source_inode": original.st_ino}

    def prepare(self, original):
        self._require_state("new")
        self.state = "refused"  # No retry even if any subsequent operation fails.
        self._boundary(original)
        os.mkdir(STORE, 0o755)
        self._command(["/usr/bin/mount", "-t", "tmpfs", "-o",
                       "size=128m,mode=0755,nosuid,nodev", "tmpfs", STORE])
        mount_policy(STORE, False)
        store_fd = os.open(STORE, FLAGS | os.O_DIRECTORY)
        self.store_fd = store_fd
        try:
            store = os.fstat(store_fd)
            require(stat.S_ISDIR(store.st_mode) and store.st_uid == store.st_gid == 0
                    and stat.S_IMODE(store.st_mode) == 0o755, "store_identity")
            deadline = time.monotonic() + 15
            with self.admission.Sources(self.data, deadline) as sources:
                # Construction admits ALL sixteen sources and ALL ancestors.
                paths = sorted(sources.files)
                require(len(paths) == 16, "source_count")
                for index, path in enumerate(paths):
                    self._copy(sources, path, str(index), store_fd, deadline)
                require(len(self.records) == len(self.fds) == 16, "copy_count")
                self.store_identity = self.admission.identity(os.fstat(store_fd))
                sources.recheck(deadline)
                no_writable_fds(os.fstat(store_fd).st_dev)
                self._command(["/usr/bin/mount", "-o", "remount,ro,nosuid,nodev", STORE])
                mount_policy(STORE, True)
                require(os.fstatvfs(store_fd).f_flag & os.ST_RDONLY, "store_writable")
                sources.recheck(deadline)  # Last full original-path check BEFORE overlays.
                self._verify_store()
                # Verify all published names against retained copies before the
                # first bind, not merely afterward when a bad bind is detectable.
                for index, path in enumerate(paths):
                    retained = self.fds[path]
                    require(self.admission.identity(os.stat(str(index), dir_fd=store_fd,
                                                            follow_symlinks=False))
                            == self.admission.identity(os.fstat(retained)), "staged_copy_replaced")
                    require(self.admission.digest(retained, self.records[path]["size"], deadline)
                            == self.records[path]["sha256"], "staged_copy_hash")
            for index, path in enumerate(paths):
                self._verify_store()
                self._command(["/usr/bin/mount", "--bind", STORE + "/" + str(index), path])
                self._command(["/usr/bin/mount", "-o", "remount,bind,ro,nosuid,nodev", path])
            for alias, expected in self.manifest["elfs"].items():
                require(str(Path(alias).resolve(strict=True)) == expected["resolved_path"],
                        "copy_alias_drift")
            self._verify_all(deadline)
        except BaseException:
            # Caller may close owned FDs, but cannot retry, publish or observe.
            self.state = "refused"
            raise
        self.state = "ready"

    def _verify_store(self):
        require(self.store_fd is not None and self.store_identity is not None, "store_missing")
        require(self.store_identity == self.admission.identity(os.fstat(self.store_fd))
                == self.admission.identity(os.stat(STORE, follow_symlinks=False)), "store_replaced")
        require(os.fstatvfs(self.store_fd).f_flag & os.ST_RDONLY, "store_writable")
        mount_policy(STORE, True)

    def _verify_target(self, path, deadline):
        record, retained = self.records[path], self.fds[path]
        fd = os.open(path, FLAGS)
        try:
            value = os.fstat(fd)
            require(self.admission.identity(value) == self.admission.identity(os.fstat(retained))
                    and (value.st_dev, value.st_ino, value.st_size, value.st_mode,
                         value.st_uid, value.st_gid, value.st_nlink)
                    == tuple(record[k] for k in ("device", "inode", "size", "mode", "uid", "gid", "nlink")),
                    "copy_target_identity")
            require(os.fstatvfs(fd).f_flag & os.ST_RDONLY, "copy_target_writable")
            mount_policy(path, True)
            require(self.admission.digest(fd, value.st_size, deadline) == record["sha256"],
                    "copy_target_hash")
        finally:
            os.close(fd)

    def _verify_all(self, deadline):
        require(len(self.records) == len(self.fds) == 16, "copy_count")
        self._verify_store()
        no_writable_fds(os.fstat(self.store_fd).st_dev)
        for path in sorted(self.records):
            self._verify_target(path, deadline)

    def verify(self, deadline):
        self._require_state("ready")
        self.state = "refused"
        self._verify_all(deadline)
        self.state = "ready"

    def inventory(self, child, deadline):
        self._require_state("ready")
        self.state = "refused"
        require(self.base.child_status(child) is None, "mapping_child_not_live")
        before = map_objects(bounded_text(f"/proc/{child.pid}/maps", 1024 * 1024))
        result = []
        for path, identity in sorted(before.items()):
            require(path in self.records, "uncopied_loaded_object")
            row = self.records[path]
            require(identity == (row["device"], row["inode"]), "mapped_copy_identity")
            # Device/inode refusal precedes opening or hashing any target.
            self._verify_target(path, deadline)
            result.append({"path": path, "device": row["device"], "inode": row["inode"],
                           "size": row["size"], "sha256": row["sha256"]})
        after = map_objects(bounded_text(f"/proc/{child.pid}/maps", 1024 * 1024))
        require(before == after and self.base.child_status(child) is None, "mapping_changed")
        self.state = "ready"
        return result

    def close(self):
        self.state = "closed"
        error = None
        while self.fds:
            _, fd = self.fds.popitem()
            try:
                os.close(fd)
            except BaseException as exc:
                if error is None:
                    error = exc
        if self.store_fd is not None:
            fd, self.store_fd = self.store_fd, None
            try:
                os.close(fd)
            except BaseException as exc:
                if error is None:
                    error = exc
        if error is not None:
            raise error
