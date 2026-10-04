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
MAP_LINE = re.compile(r"([0-9a-f]+)-([0-9a-f]+) ([r-][w-][x-][ps]) ([0-9a-f]+) "
                      r"([0-9a-f]{2,}):([0-9a-f]{2,}) ([0-9]+)(?: +(.*))?\Z")
PHASES = frozenset(("before_isolate", "before_prepare", "before_store_create",
                   "before_store_mount", "before_source_admission", "before_copy",
                   "before_source_recheck", "before_fd_inventory", "before_store_freeze",
                   "before_bind", "before_verify_copies", "before_bus_config",
                   "before_bus_start", "before_resolved_start", "before_initial_maps",
                   "before_final_maps", "before_resolved_stop", "before_bus_stop"))
_breadcrumb_count = 0
_breadcrumb_refused = False


class Refused(RuntimeError):
    pass


class UnknownMapping(Refused):
    def __init__(self, path):
        super().__init__("unknown_public_mapping")
        self.diagnostic = {"schema": "unknown-public-mapping-v1", "path": path,
                           "loaded_elf_identity_proven": False, "allowlist_adoption": False}


def require(value, reason):
    if not value:
        raise Refused(reason)


def breadcrumb(phase):
    """Before-effect finite label only; not success, cleanup or process evidence.

    FD 2 is the already-owned private child.stderr stream. Never inspect it,
    open another output, stringify an exception or issue a post-error query.
    """
    global _breadcrumb_count, _breadcrumb_refused
    require(not _breadcrumb_refused, "breadcrumb_sealed")
    _breadcrumb_refused = True
    require(type(phase) is str and phase in PHASES and _breadcrumb_count < 128,
            "breadcrumb_shape")
    data = ("T3_LIVE_FD_PHASE_V1 " + phase + "\n").encode("ascii")
    require(os.write(2, data) == len(data), "breadcrumb_short_write")
    _breadcrumb_count += 1
    _breadcrumb_refused = False


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
    # CPython scandir retains its duplicated DIR FD until EOF/context exit.
    # Check each entry WHILE the stream is live, never after list(stream).
    # listdir(fd) instead closes its duplicate before returning its own name.
    # This fixture is single-threaded; any genuine unknown FD remains fatal.
    directory = os.open("/proc/self/fd", FLAGS | os.O_DIRECTORY)
    try:
        seen = set()
        with os.scandir(directory) as stream:
            for entry in stream:
                name = entry.name
                require(type(name) is str and re.fullmatch(r"0|[1-9][0-9]{0,9}", name)
                        and int(name) <= 2**31 - 1 and name not in seen
                        and len(seen) < 128, "fd_inventory_bound")
                seen.add(name)
                fd = int(name)
                value = os.fstat(fd)
                if value.st_dev == device:
                    require(fcntl.fcntl(fd, fcntl.F_GETFL) & os.O_ACCMODE == os.O_RDONLY,
                            "copy_writable_fd")
        require(str(directory) in seen, "fd_inventory_missing_directory")
    finally:
        os.close(directory)


def map_objects(text):
    require(type(text) is str and len(text) <= 1024 * 1024, "mapping_bound")
    objects = {}
    previous_end = 0
    for line in text.splitlines():
        match = MAP_LINE.fullmatch(line)
        require(match is not None, "mapping_shape")
        start, end, _, offset, major, minor, inode, path = match.groups()
        start, end, offset = int(start, 16), int(end, 16), int(offset, 16)
        require(previous_end <= start < end <= 2**64 - 1 and offset <= 2**64 - 1,
                "mapping_range")
        previous_end = end
        identity = os.makedev(int(major, 16), int(minor, 16)), int(inode)
        if path is None or path == "" or path.startswith("["):
            require(identity == (0, 0) and offset == 0
                    and (not path or re.fullmatch(r"\[[A-Za-z0-9_:.-]+\]", path)),
                    "anonymous_mapping_shape")
            continue
        require(len(path) <= 4096 and PUBLIC.fullmatch(path)
                and all(p not in ("", ".", "..") for p in path.split("/")[1:]),
                "mapping_nonpublic_or_deleted")
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
        breadcrumb("before_store_create")
        os.mkdir(STORE, 0o755)
        breadcrumb("before_store_mount")
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
            breadcrumb("before_source_admission")
            with self.admission.Sources(self.data, deadline) as sources:
                # Construction admits ALL sixteen sources and ALL ancestors.
                paths = sorted(sources.files)
                require(len(paths) == 16, "source_count")
                for index, path in enumerate(paths):
                    breadcrumb("before_copy")
                    self._copy(sources, path, str(index), store_fd, deadline)
                require(len(self.records) == len(self.fds) == 16, "copy_count")
                self.store_identity = self.admission.identity(os.fstat(store_fd))
                breadcrumb("before_source_recheck")
                sources.recheck(deadline)
                breadcrumb("before_fd_inventory")
                no_writable_fds(os.fstat(store_fd).st_dev)
                breadcrumb("before_store_freeze")
                self._command(["/usr/bin/mount", "-o", "remount,ro,nosuid,nodev", STORE])
                mount_policy(STORE, True)
                require(os.fstatvfs(store_fd).f_flag & os.ST_RDONLY, "store_writable")
                breadcrumb("before_source_recheck")
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
                breadcrumb("before_bind")
                self._command(["/usr/bin/mount", "--bind", STORE + "/" + str(index), path])
                breadcrumb("before_bind")
                self._command(["/usr/bin/mount", "-o", "remount,bind,ro,nosuid,nodev", path])
            for alias, expected in self.manifest["elfs"].items():
                require(str(Path(alias).resolve(strict=True)) == expected["resolved_path"],
                        "copy_alias_drift")
            breadcrumb("before_verify_copies")
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
            require(self.admission.identity(value) == self.admission.identity(os.fstat(fd))
                    == self.admission.identity(os.stat(path, follow_symlinks=False)),
                    "copy_target_replaced_during_hash")
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

    def _child_binding(self, child):
        require(type(child.pid) is int and child.pid > 0
                and os.readlink("/proc/self") == str(os.getpid())
                and os.readlink(f"/proc/{child.pid}/ns/pid") == os.readlink("/proc/self/ns/pid"),
                "mapping_pid_namespace")
        fields = bounded_text(f"/proc/{child.pid}/stat", 4096).rsplit(")", 1)
        require(len(fields) == 2 and fields[0].split(" (", 1)[0] == str(child.pid), "mapping_pid_shape")
        state = fields[1].split()
        require(len(state) >= 20 and state[0] not in ("Z", "X")
                and int(state[1]) == os.getpid(), "mapping_direct_child")

    def inventory(self, child, deadline):
        self._require_state("ready")
        self.state = "refused"
        require(self.base.child_status(child) is None, "mapping_child_not_live")
        self._child_binding(child)
        before = map_objects(bounded_text(f"/proc/{child.pid}/maps", 1024 * 1024))
        result = []
        for path, identity in sorted(before.items()):
            if path not in self.records:
                raise UnknownMapping(path)  # Grammar-checked public path, no object open/hash.
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
