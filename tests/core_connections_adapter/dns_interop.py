# SPDX-License-Identifier: MIT
"""Explicit synthetic Rust/Go channel gate; no broker, DNS or TUN effects."""
from collections import Counter
import errno
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import stat

MAX_FIXTURE = 32 * 1024 * 1024
CORPUS_SHA256 = "51fef4516f3c56410118965ebe141d977412a463833473e4845458e604e2f8fc"
CORPUS_PATH = "crates/omavless-dns-channel/cases.json"
PACKAGE = "github.com/metacubex/mihomo/listener/sing_tun"
TEST = "TestSystemDNSRustChannelInterop"
SCENARIOS = ("success", "reject", "recovery_on_release", "loss_after_ready")
ACTIONS = frozenset(("start", "run", "pause", "cont", "pass", "output"))


def identity(value):
    return (value.st_dev, value.st_ino, value.st_mode, value.st_uid,
            value.st_gid, value.st_nlink, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns)


def snapshot_fixture(path, expected):
    """Consume checked descriptor bytes, never execute the original path.

    The caller-supplied digest identifies a separately reviewed test artifact;
    it is not compiler provenance or release/package authentication.
    """
    if (not isinstance(path, Path) or not path.is_absolute()
            or not isinstance(expected, str) or re.fullmatch("[0-9a-f]{64}", expected) is None):
        raise RuntimeError("Rust wire fixture identity refused")
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(descriptor)
        if (not stat.S_ISREG(before.st_mode) or before.st_uid not in (0, os.getuid())
                or before.st_nlink != 1 or before.st_mode & 0o7022
                or not before.st_mode & 0o100 or not 32 <= before.st_size <= MAX_FIXTURE):
            raise RuntimeError("Rust wire fixture object refused")
        try:
            capability = os.getxattr(descriptor, "security.capability")
        except OSError as error:
            if error.errno not in (errno.ENODATA, errno.ENOTSUP):
                raise
        else:
            if capability:
                raise RuntimeError("Rust wire fixture capability refused")
        chunks, remaining = [], MAX_FIXTURE + 1
        while remaining:
            chunk = os.read(descriptor, min(65536, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        data = b"".join(chunks)
        if (identity(before) != identity(os.fstat(descriptor)) or len(data) != before.st_size
                or hashlib.sha256(data).hexdigest() != expected):
            raise RuntimeError("Rust wire fixture changed or digest refused")
        machine = {"x86_64": 62, "aarch64": 183}.get(platform.machine())
        if (machine is None or data[:7] != b"\x7fELF\x02\x01\x01"
                or int.from_bytes(data[18:20], "little") != machine):
            raise RuntimeError("Rust wire fixture architecture refused")
        return data
    finally:
        os.close(descriptor)


def duplicate_free(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("Duplicate event field")
        result[key] = value
    return result


def snapshot_corpus(path):
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(descriptor)
        if (not stat.S_ISREG(before.st_mode) or before.st_uid != os.getuid()
                or before.st_nlink != 1 or stat.S_IMODE(before.st_mode) != 0o600
                or not 0 < before.st_size <= 4096):
            raise RuntimeError("Copied Rust wire corpus refused")
        data = os.read(descriptor, 4097)
        if (identity(before) != identity(os.fstat(descriptor)) or len(data) != before.st_size
                or hashlib.sha256(data).hexdigest() != CORPUS_SHA256):
            raise RuntimeError("Copied Rust wire corpus changed")
        return data
    finally:
        os.close(descriptor)


def receipt(raw):
    """Require parent AND all four actually executed named subcases, x20."""
    names = (TEST, *(TEST + "/" + item for item in SCENARIOS))
    wanted = Counter({name: 20 for name in names})
    runs, passes = Counter(), Counter()
    package_start = package_pass = 0
    active = set()
    try:
        if not raw or len(raw) > 4 * 1024 * 1024:
            raise ValueError("Receipt bound")
        for line in raw.splitlines():
            event = json.loads(line, object_pairs_hook=duplicate_free)
            if (not isinstance(event, dict) or event.get("Package") != PACKAGE
                    or event.get("Action") not in ACTIONS):
                raise ValueError("Event shape or failure")
            action = event["Action"]
            if "Test" not in event:
                if action == "start":
                    if package_start or package_pass:
                        raise ValueError("Repeated package start")
                    package_start += 1
                elif action == "pass":
                    if not package_start or package_pass or active:
                        raise ValueError("Premature package completion")
                    package_pass += 1
                elif action != "output":
                    raise ValueError("Package event")
                continue
            name = event["Test"]
            if not isinstance(name, str) or name not in wanted:
                raise ValueError("Unknown or malformed subcase")
            if action == "run":
                if (not package_start or package_pass or name in active
                        or (name == TEST and active)
                        or (name != TEST and active != {TEST})):
                    raise ValueError("Test execution order")
                active.add(name)
                runs[name] += 1
            elif action == "pass":
                if name not in active or (name == TEST and active != {TEST}):
                    raise ValueError("Unstarted or premature completion")
                active.remove(name)
                passes[name] += 1
            elif action not in ("output", "pause", "cont"):
                raise ValueError("Unexpected test event")
    except (ValueError, TypeError, KeyError):
        raise RuntimeError("Rust Go wire execution receipt refused") from None
    if runs != wanted or passes != wanted or package_start != 1 or package_pass != 1 or active:
        raise RuntimeError("Rust Go wire execution receipt refused")


def exercise(root, source, env, data, expected, corpus, run, go):
    if (not corpus or len(corpus) > 4096
            or hashlib.sha256(corpus).hexdigest() != CORPUS_SHA256):
        raise RuntimeError("Pinned Rust wire corpus refused")
    if len(os.fsencode(root / "di-XXXXXXXX" / "channel")) >= 108:
        raise RuntimeError("Rust wire socket parent exceeds bound")
    fixture = root / "dns-interop-fixture"
    corpus_file = root / "dns-interop-corpus.json"
    # Exclusive copies inherit neither executable xattrs/caps nor source mode.
    with fixture.open("xb") as output:
        output.write(data)
    fixture.chmod(0o700)
    with corpus_file.open("xb") as output:
        output.write(corpus)
    corpus_file.chmod(0o600)
    if snapshot_fixture(fixture, expected) != data or snapshot_corpus(corpus_file) != corpus:
        raise RuntimeError("Copied Rust wire fixture refused")
    output = run([go, "test", "-json", "-mod=vendor", "-tags=with_gvisor",
                  "./listener/sing_tun", "-run", "^" + TEST + "$", "-count=20"],
                 cwd=source, env=dict(env, OMAVLESS_DNS_INTEROP_SERVER=str(fixture),
                                      OMAVLESS_DNS_INTEROP_CORPUS=str(corpus_file)))
    receipt(output)
    if snapshot_fixture(fixture, expected) != data or snapshot_corpus(corpus_file) != corpus:
        raise RuntimeError("Rust wire inputs changed during execution")
