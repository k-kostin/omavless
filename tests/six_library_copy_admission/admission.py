"""Inert, developer-only admission proposal. No launcher, mounts or subprocesses."""
import hashlib
import json
import os
import stat
import time

MANIFEST_SHA256 = "3caa3d2bfdace10e97617b1e222192bf0c78f8ac194f51ee0a2f90f9defdd896"
MAX_FILE = 32 * 1024 * 1024
MAX_TOTAL = 128 * 1024 * 1024
FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC


class Refused(RuntimeError):
    pass


def require(ok, reason):
    if not ok:
        raise Refused(reason)


def identity(value):
    return (value.st_dev, value.st_ino, value.st_mode, value.st_uid,
            value.st_gid, value.st_nlink, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns)


def manifest(data):
    require(type(data) is bytes and len(data) <= 65536
            and hashlib.sha256(data).hexdigest() == MANIFEST_SHA256, "manifest_pin")
    # Exact bytes are the authority: no external observations extend this table.
    value = json.loads(data)
    require(len(value["elfs"]) == 26 and len(value["source_provenance"]) == 25,
            "manifest_count")
    require({row["resolved_path"] for row in value["elfs"].values()}
            == set(value["source_provenance"]), "manifest_closure")
    return value


def digest(fd, expected_size, deadline):
    require(0 < expected_size <= MAX_FILE, "source_bound")
    before = identity(os.fstat(fd))
    os.lseek(fd, 0, os.SEEK_SET)
    result, size = hashlib.sha256(), 0
    while True:
        require(time.monotonic() < deadline, "source_deadline")
        block = os.read(fd, min(65536, expected_size + 1 - size))
        if not block:
            break
        if size == 0:
            require(block.startswith(b"\x7fELF"), "source_not_elf")
        size += len(block)
        require(size <= expected_size, "source_growth")
        result.update(block)
    require(size == expected_size and identity(os.fstat(fd)) == before,
            "source_changed")
    return result.hexdigest()


class Sources:
    """Hold every canonical source and every ancestor FD until caller closes.

    Only for the already-reviewed private user namespace: installed root appears
    as 65534, whereas future tmpfs copies are owned by namespace uid/gid 0.
    Package hashes describe the reviewed capture, not a fresh ALPM observation.
    """

    def __init__(self, data, deadline):
        self._state = "open"
        self.table = manifest(data)["source_provenance"]
        self._parents, self._files = {}, {}
        try:
            self._parent("/")
            total = 0
            for path, row in sorted(self.table.items()):
                require(path.startswith(("/usr/lib/", "/usr/bin/"))
                        and all(p not in ("", ".", "..") for p in path.split("/")[1:]),
                        "source_path")
                directory, name = path.rsplit("/", 1)
                parent = self._parent(directory)[0]
                fd = os.open(name, FLAGS, dir_fd=parent)
                try:
                    self.files[path] = (fd, os.fstat(fd))
                except BaseException:
                    os.close(fd)
                    raise
                value = self.files[path][1]
                require(stat.S_ISREG(value.st_mode) and value.st_uid == value.st_gid == 65534
                        and value.st_dev == row["device"] and value.st_ino == row["inode"]
                        and value.st_size == row["size"] and value.st_mode == row["mode"]
                        and value.st_nlink == row["nlink"] == 1, "source_identity")
                total += value.st_size
                require(total <= MAX_TOTAL, "source_total")
                require(digest(fd, value.st_size, deadline) == row["sha256"], "source_hash")
            # All original FDs, hashes and parent chains pass before any caller
            # may start creating copies. This module itself cannot mount/launch.
            self.recheck(deadline)
        except BaseException:
            self.close()
            raise

    def _usable(self):
        require(self._state == "open", "sources_sealed")

    @property
    def files(self):
        self._usable()
        return self._files

    @property
    def parents(self):
        self._usable()
        return self._parents

    def _parent(self, path):
        self._usable()
        if path in self.parents:
            return self.parents[path]
        if path == "/":
            fd = os.open("/", FLAGS | os.O_DIRECTORY)
        else:
            parent, name = path.rsplit("/", 1)
            fd = os.open(name, FLAGS | os.O_DIRECTORY,
                         dir_fd=self._parent(parent or "/")[0])
        try:
            value = os.fstat(fd)
        except BaseException:
            os.close(fd)
            raise
        self.parents[path] = (fd, value)
        expected_owner = 0 if path == "/" else 65534
        require(stat.S_ISDIR(value.st_mode) and value.st_uid == value.st_gid == expected_owner
                and value.st_mode & 0o022 == 0, "source_parent_shape")
        return self.parents[path]

    def recheck(self, deadline):
        self._usable()
        try:
            self._recheck(deadline)
        except BaseException:
            # First uncertainty is terminal even if metadata/clock later recover.
            self._state = "refused"
            raise

    def _recheck(self, deadline):
        self._usable()
        for path, (fd, before) in self.files.items():
            parent, name = path.rsplit("/", 1)
            require(identity(before) == identity(os.fstat(fd))
                    == identity(os.stat(name, dir_fd=self.parents[parent][0],
                                        follow_symlinks=False)), "source_replaced")
            require(digest(fd, before.st_size, deadline) == self.table[path]["sha256"],
                    "source_rehash")
        for path, (fd, before) in self.parents.items():
            if path == "/":
                current = os.stat("/", follow_symlinks=False)
            else:
                parent, name = path.rsplit("/", 1)
                current = os.stat(name, dir_fd=self.parents[parent or "/"][0],
                                  follow_symlinks=False)
            require(identity(before) == identity(os.fstat(fd)) == identity(current),
                    "source_parent_replaced")

    def close(self):
        self._state = "closed"
        error = None
        for values in (self._files, self._parents):
            while values:
                _, (fd, _) = values.popitem()
                try:
                    os.close(fd)
                except BaseException as exc:
                    # Never retry an ambiguous close (the number may be reused).
                    if error is None:
                        error = exc
        if error is not None:
            raise error

    def __enter__(self):
        self._usable()
        return self

    def __exit__(self, *_):
        self.close()
