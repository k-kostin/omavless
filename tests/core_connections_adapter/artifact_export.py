# SPDX-License-Identifier: MIT
"""Opt-in developer artifacts, deliberately not a release/package receipt."""
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import resource
import signal
import stat
import subprocess
import tarfile
import tempfile
import time

import dns_interop
import review

SCHEMA = "omavless-composed-developer-artifacts-v1"
MAX_SOURCE = 512 * 1024 * 1024
MAX_BINARY = 128 * 1024 * 1024
FILES = frozenset(("mihomo", "omavless-dns-broker", "channel-fixture", "wire-go.test",
                   "conditional.jsonl", "dns.jsonl", "wire.jsonl", "cases.json",
                   "corresponding-source.tar", "core-build.txt", "broker-build.txt",
                   "mihomo-dns-broker.patch", "sing-tun-descriptor.patch",
                   "mihomo-conditional-close.patch", "mihomo-dns-test-sockets.patch",
                   "mihomo-dns-interop-sockets.patch"))


def digest(data):
    return hashlib.sha256(data).hexdigest()


def toolchains(env):
    result = {}
    for tool, args in (("go", [review.GO, "version"]), ("rustc", ["/usr/bin/rustc", "--version"]),
                       ("cargo", ["/usr/bin/cargo", "--version"])):
        executable = Path(args[0]).resolve()
        expected = digest(read_object(executable, MAX_BINARY, root_owned=True))
        dns_interop.snapshot_fixture(executable, expected, maximum=MAX_BINARY)
        version = review.run(args, env=env).decode("ascii").strip()
        dns_interop.snapshot_fixture(executable, expected, maximum=MAX_BINARY)
        result[tool] = {"version": version, "sha256": expected}
    return result


def read_object(path, maximum=MAX_BINARY, *, root_owned=False):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        if (not stat.S_ISREG(before.st_mode) or before.st_uid not in ((0, os.getuid()) if root_owned else (os.getuid(),))
                or before.st_nlink != 1 or before.st_mode & 0o7022
                or not 0 < before.st_size <= maximum):
            raise RuntimeError("Developer input object refused")
        with os.fdopen(os.dup(fd), "rb") as stream:
            data = stream.read(maximum + 1)
        if (len(data) != before.st_size
                or dns_interop.identity(before) != dns_interop.identity(os.fstat(fd))):
            raise RuntimeError("Developer input changed")
        return data
    finally:
        os.close(fd)


def file_digest(path, maximum):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        if (not stat.S_ISREG(before.st_mode) or before.st_uid != os.getuid()
                or before.st_nlink != 1 or before.st_mode & 0o7022
                or not 0 < before.st_size <= maximum):
            raise RuntimeError("Developer artifact shape refused")
        value = hashlib.sha256()
        size = 0
        while True:
            chunk = os.read(fd, min(65536, maximum + 1 - size))
            if not chunk:
                break
            size += len(chunk)
            value.update(chunk)
            if size > maximum:
                raise RuntimeError("Developer artifact bound refused")
        if size != before.st_size or dns_interop.identity(before) != dns_interop.identity(os.fstat(fd)):
            raise RuntimeError("Developer artifact changed")
        return value.hexdigest()
    finally:
        os.close(fd)


class Bundle:
    """Reserve a new directory; publish completion last, never replace output."""
    def __init__(self, path):
        if (not isinstance(path, Path) or not path.is_absolute() or ".." in path.parts
                or path.name in ("", ".", "..")):
            raise RuntimeError("Developer destination refused")
        self.path = path
        parent = path.parent
        self.ancestry = []
        for ancestor in (parent, *parent.parents):
            value = ancestor.lstat()
            if (not stat.S_ISDIR(value.st_mode) or value.st_uid not in (0, os.getuid())
                    or value.st_mode & 0o022):
                raise RuntimeError("Developer destination ancestry refused")
            self.ancestry.append((ancestor, value.st_dev, value.st_ino, value.st_uid,
                                  stat.S_IMODE(value.st_mode)))
        value = parent.lstat()
        if value.st_uid != os.getuid() or stat.S_IMODE(value.st_mode) != 0o700:
            raise RuntimeError("Developer destination parent must be private")
        repo = Path(__file__).resolve().parents[2]
        if path.is_relative_to(repo):
            raise RuntimeError("Developer destination must be outside checkout")
        self.parent = os.open(parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        self.fd = None
        self.created = []
        self.identities = {}
        self.retained = {}
        self.hashes = {}
        self.completed = False
        try:
            os.mkdir(path.name, 0o700, dir_fd=self.parent)
            self.fd = os.open(path.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                              dir_fd=self.parent)
            self.identity = os.fstat(self.fd)
        except BaseException:
            os.close(self.parent)
            raise

    def check(self):
        for path, dev, ino, uid, mode in self.ancestry:
            now = path.lstat()
            if (not stat.S_ISDIR(now.st_mode)
                    or (now.st_dev, now.st_ino, now.st_uid, stat.S_IMODE(now.st_mode)) != (dev, ino, uid, mode)):
                raise RuntimeError("Developer destination ancestry changed")
        named = os.stat(self.path.name, dir_fd=self.parent, follow_symlinks=False)
        retained = os.fstat(self.fd)
        if ((named.st_dev, named.st_ino) != (self.identity.st_dev, self.identity.st_ino)
                or named.st_uid != os.getuid() or stat.S_IMODE(named.st_mode) != 0o700
                or (retained.st_dev, retained.st_ino) != (named.st_dev, named.st_ino)):
            raise RuntimeError("Developer destination changed")

    def put(self, name, data, mode=0o600):
        if name not in FILES | {"developer-manifest.json"} or name in self.created:
            raise RuntimeError("Developer member refused")
        self.check()
        fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                     mode, dir_fd=self.fd)
        self.created.append(name)
        value = os.fstat(fd)
        self.identities[name] = (value.st_dev, value.st_ino)
        self.retained[name] = os.dup(fd)
        with os.fdopen(fd, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        self.hashes[name] = digest(data)

    def finish(self, metadata):
        if set(self.hashes) != FILES:
            raise RuntimeError("Developer bundle incomplete")
        self.check()
        for name, expected in self.hashes.items():
            value = os.stat(name, dir_fd=self.fd, follow_symlinks=False)
            if (value.st_dev, value.st_ino) != self.identities[name]:
                raise RuntimeError("Developer exported member replaced")
            if file_digest(self.path / name, MAX_SOURCE) != expected:
                raise RuntimeError("Developer exported member changed")
            after = os.stat(name, dir_fd=self.fd, follow_symlinks=False)
            if dns_interop.identity(value) != dns_interop.identity(after):
                raise RuntimeError("Developer exported member changed during verification")
        document = dict(metadata, schema=SCHEMA, sha256=self.hashes.copy(),
                        broker_executed=False, installed_compatibility=False,
                        package_attestation=False, effect_authority=False)
        self.put("developer-manifest.json", (json.dumps(document, sort_keys=True, indent=2) + "\n").encode())
        os.fsync(self.fd)
        self.check()
        self.completed = True

    def put_archive(self, path):
        name = "corresponding-source.tar"
        source_fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
        try:
            before = os.fstat(source_fd)
            if (not stat.S_ISREG(before.st_mode) or before.st_uid != os.getuid()
                    or before.st_nlink != 1 or stat.S_IMODE(before.st_mode) != 0o600
                    or not 0 < before.st_size <= MAX_SOURCE):
                raise RuntimeError("Developer archive source refused")
            self.check()
            fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                         0o600, dir_fd=self.fd)
            self.created.append(name)
            value = os.fstat(fd)
            self.identities[name] = (value.st_dev, value.st_ino)
            self.retained[name] = os.dup(fd)
            expected = hashlib.sha256()
            with os.fdopen(fd, "wb") as output:
                total = 0
                while chunk := os.read(source_fd, 65536):
                    total += len(chunk)
                    if total > MAX_SOURCE:
                        raise RuntimeError("Developer archive copy exceeded bound")
                    output.write(chunk)
                    expected.update(chunk)
                output.flush()
                os.fsync(output.fileno())
            if (total != before.st_size
                    or dns_interop.identity(before) != dns_interop.identity(os.fstat(source_fd))
                    or dns_interop.identity(before) != dns_interop.identity(path.lstat())
                    or file_digest(self.path / name, MAX_SOURCE) != expected.hexdigest()):
                raise RuntimeError("Developer source archive changed")
            self.hashes[name] = expected.hexdigest()
        finally:
            os.close(source_fd)

    def close(self):
        try:
            if not self.completed:
                # Delete only names this instance exclusively created, through
                # its retained directory. Never traverse an arbitrary tree.
                substituted = False
                for name in reversed(self.created):
                    try:
                        value = os.stat(name, dir_fd=self.fd, follow_symlinks=False)
                    except FileNotFoundError:
                        continue
                    if (value.st_dev, value.st_ino) != self.identities[name]:
                        substituted = True
                        continue
                    os.unlink(name, dir_fd=self.fd)
                if substituted:
                    raise RuntimeError("Developer cleanup preserved substituted member")
                self.check()
                os.rmdir(self.path.name, dir_fd=self.parent)
        finally:
            for fd in self.retained.values():
                os.close(fd)
            os.close(self.fd)
            os.close(self.parent)


@contextmanager
def destination(path):
    bundle = None if path is None else Bundle(path)
    try:
        yield bundle
    finally:
        if bundle is not None:
            bundle.close()


def build_command(args, cwd, env):
    """Bound compiler diagnostics without imposing wire's 4-MiB linker limit."""
    def limits():
        resource.setrlimit(resource.RLIMIT_FSIZE, (MAX_SOURCE,) * 2)
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    with tempfile.TemporaryFile(dir=env["TMPDIR"]) as output:
        child = subprocess.Popen(args, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                 stdout=output, stderr=output, start_new_session=True,
                                 preexec_fn=limits)
        try:
            deadline = time.monotonic() + 600
            while os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is None:
                if time.monotonic() >= deadline or output.tell() > 4 * 1024 * 1024:
                    raise RuntimeError("Developer build exceeded bound")
                time.sleep(0.02)
        finally:
            try:
                os.killpg(child.pid, signal.SIGKILL)
                deadline = time.monotonic() + 3
                while dns_interop.live_group_members(child.pid):
                    if time.monotonic() >= deadline:
                        raise RuntimeError("Developer build cleanup unconfirmed")
                    time.sleep(0.02)
            finally:
                child.wait(timeout=5)
        if child.returncode or output.tell() > 4 * 1024 * 1024:
            raise RuntimeError("Developer build refused")
        output.seek(0)
        return output.read(4 * 1024 * 1024 + 1)


def source_archive(root, directories):
    """Archive only corresponding source, never Cargo output or private fixtures."""
    target = root / "corresponding-source.tar"
    total = count = 0
    with tarfile.open(target, "x") as archive:
        for directory in directories:
            if not directory.is_dir() or directory.is_symlink():
                raise RuntimeError("Corresponding source root refused")
            for path in sorted(directory.rglob("*")):
                value = path.lstat()
                if stat.S_ISDIR(value.st_mode):
                    continue
                if not stat.S_ISREG(value.st_mode) or value.st_nlink != 1:
                    raise RuntimeError("Corresponding source type refused")
                total += value.st_size
                count += 1
                if total > MAX_SOURCE or count > 50000:
                    raise RuntimeError("Corresponding source exceeds bound")
                archive.add(path, arcname=str(path.relative_to(root)), recursive=False)
    target.chmod(0o600)
    if count == 0 or target.stat().st_size > MAX_SOURCE:
        raise RuntimeError("Corresponding source archive exceeds bound")
    return target


def complete(bundle, root, repository, env, core_sha, conditional, dns, wire, fixture_sha, core_metadata, tools):
    # Import here to avoid an import cycle and keep the default gate unchanged.
    import managed_composition as composition
    review.matrix_receipt(conditional, composition.CONDITIONAL_TESTS)
    review.matrix_receipt(dns, composition.DNS_TESTS, (composition.DNS_INTEROP_SKIP,))
    dns_interop.receipt(wire)
    own_repo = Path(__file__).resolve().parents[2]
    if review.run(["/usr/bin/git", "-C", str(own_repo), "status", "--porcelain"], env=review.git_environment()):
        raise RuntimeError("Developer builder source must be committed and clean")
    revision = review.run(["/usr/bin/git", "-C", str(own_repo), "rev-parse", "HEAD"], env=review.git_environment()).decode().strip()
    # The actual broker recipe is the already existing release-package build;
    # compiling that feature here does not issue its release receipt or execute it.
    review.export(repository, composition.DNS_REVISION, root / "omavless")
    review.export(own_repo, revision, root / "developer-builder")
    cargo_env = dict(env, CARGO_NET_OFFLINE="true", CARGO_TARGET_DIR=str(root / "cargo-target"),
                     CARGO_BUILD_JOBS="2")
    command = ["/usr/bin/cargo", "build", "--release", "--locked", "--offline", "-p",
               "omavless-dns-broker", "--bin", "omavless-dns-broker", "--features", "release-package"]
    log = build_command(command, root / "omavless", cargo_env)
    broker = root / "cargo-target/release/omavless-dns-broker"
    broker.chmod(0o700)
    broker_data = read_object(broker)
    dns_interop.snapshot_fixture(broker, digest(broker_data), maximum=MAX_BINARY)
    core = root / "combined-core"
    core_data = dns_interop.snapshot_fixture(core, core_sha, maximum=MAX_BINARY)
    fixture_data = dns_interop.snapshot_fixture(root / "dns-interop-fixture", fixture_sha)
    wire_test = root / "dns-interop-go.test"
    test_sha = file_digest(wire_test, MAX_BINARY)
    test_data = dns_interop.snapshot_fixture(wire_test, test_sha, maximum=MAX_BINARY)
    for name, data in (("mihomo", core_data), ("omavless-dns-broker", broker_data),
                       ("channel-fixture", fixture_data), ("wire-go.test", test_data)):
        bundle.put(name, data, 0o700)
    for name, data in (("conditional.jsonl", conditional), ("dns.jsonl", dns), ("wire.jsonl", wire),
                       ("cases.json", dns_interop.snapshot_corpus(root / "dns-interop-corpus.json")),
                       ("core-build.txt", core_metadata), ("broker-build.txt", log)):
        bundle.put(name, data)
    for name in composition.PATCHES:
        bundle.put(name, read_object(root / name, composition.MAX_PATCH))
    for path in (review.PATCH, composition.SOCKET_TEST_PATCH, composition.INTEROP_TEST_PATCH):
        bundle.put(path.name, read_object(path, composition.MAX_PATCH))
    bundle.put_archive(source_archive(root, [root / name for name in
                       ("mihomo", "sing-tun", "omavless", "developer-builder")]))
    if toolchains(env) != tools:
        raise RuntimeError("Developer toolchains changed during build")
    bundle.finish({"builder_source": revision, "dns_source": composition.DNS_REVISION,
                   "mihomo_source": review.PIN, "sing_tun_source": composition.SING_TUN,
                   "architecture": os.uname().machine, "toolchains": tools,
                   "cargo_lock_sha256": file_digest(root / "omavless/Cargo.lock", 4 * 1024 * 1024),
                   "broker_build_argv": command, "broker_feature": "release-package",
                   "core_build": {"tags": "with_gvisor", "cgo": False, "buildvcs": False,
                                  "dependency_mode": "vendor", "trimpath": True, "ldflags": "-s -w"},
                   "wire": {"kind": "regular-file-synthetic-not-broker", "scenarios": 4,
                            "repetitions": 20, "fixture_sha256": fixture_sha},
                   "tcp_loopback": "passed", "udp_loopback_repetitions": 20})
