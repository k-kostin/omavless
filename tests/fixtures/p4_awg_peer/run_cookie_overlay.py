#!/usr/bin/env python3
"""Opt-in exact-upstream device tests: no source patch, peer build or network.

Only public source metadata and fixed PASS/refusal diagnostics are emitted.
All synthetic secrets live solely in the Go test's memory.
"""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import selectors
import signal
import subprocess
import tarfile
import tempfile
import time


PIN = "b5928efb6ca19f0153958460c3d141f04abc5c2e"
ARCHIVE_SHA256 = "716c0eec8a7557485555397f1217e0cac56317af8b41fefc287027d0c74ce00d"


def git(source, *args):
    return subprocess.check_output(["git", "-C", str(source), *args], stderr=subprocess.DEVNULL, timeout=15)


def verify_source(source):
    if git(source, "rev-parse", "HEAD").decode().strip() != PIN:
        raise ValueError("upstream pin mismatch")
    if git(source, "status", "--porcelain", "--untracked-files=all", "--ignored"):
        raise ValueError("upstream checkout must be entirely clean")
    archive = git(source, "archive", PIN)
    if len(archive) > 8 * 1024 * 1024 or hashlib.sha256(archive).hexdigest() != ARCHIVE_SHA256:
        raise ValueError("upstream source export mismatch")
    if (source / "device/p4_cookie_underload_test.go").exists():
        raise ValueError("virtual overlay destination already exists")
    return archive


def export_source(archive, destination):
    """Execute only hash-attested archive bytes, never filtered checkout bytes."""
    proof = {}
    total = 0
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as stream:
        for number, member in enumerate(stream):
            name = PurePosixPath(member.name)
            if number >= 2048 or name.is_absolute() or ".." in name.parts or not name.parts:
                raise ValueError("unsafe archive path/count")
            target = destination.joinpath(*name.parts)
            if member.isdir():
                target.mkdir(mode=0o700, parents=True, exist_ok=True)
                continue
            if not member.isfile() or member.size > 2 * 1024 * 1024 or str(name) in proof:
                raise ValueError("unsafe archive entry")
            total += member.size
            if total > 8 * 1024 * 1024:
                raise ValueError("archive byte bound")
            payload = stream.extractfile(member).read(member.size + 1)
            if len(payload) != member.size:
                raise ValueError("archive size mismatch")
            target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            with target.open("xb") as file:
                os.chmod(target, 0o600)
                file.write(payload)
            proof[str(name)] = hashlib.sha256(payload).hexdigest()
    verify_export(destination, proof)
    return proof


def verify_export(destination, proof):
    actual = {}
    for path in destination.rglob("*"):
        if path.is_symlink():
            raise ValueError("export symlink substitution")
        if path.is_file():
            if path.stat().st_size > 2 * 1024 * 1024:
                raise ValueError("export file bound")
            actual[str(path.relative_to(destination))] = hashlib.sha256(path.read_bytes()).hexdigest()
        elif not path.is_dir():
            raise ValueError("export special file substitution")
    if actual != proof:
        raise ValueError("executed export bytes changed")


def bounded_command(command, cwd, env, timeout):
    """Bound both captured streams; reap the owned process group on refusal."""
    process = subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    output = {"stdout": bytearray(), "stderr": bytearray()}
    deadline = time.monotonic() + timeout
    waited = False
    try:
        with selectors.DefaultSelector() as selector:
            for name in output:
                selector.register(getattr(process, name), selectors.EVENT_READ, name)
            while selector.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise ValueError("bounded command deadline")
                for key, _ in selector.select(min(remaining, 1)):
                    block = os.read(key.fileobj.fileno(), 65536)
                    if not block:
                        selector.unregister(key.fileobj)
                        continue
                    output[key.data].extend(block)
                    if sum(map(len, output.values())) > 2 * 1024 * 1024:
                        raise ValueError("bounded command output")
        code = process.wait(timeout=max(0.01, deadline - time.monotonic()))
        waited = True
        return code, bytes(output["stdout"]), bytes(output["stderr"])
    finally:
        try:
            # Never poll/reap the leader before timeout cleanup. Its unreaped
            # PID anchors our owned process group even if a helper holds stdio.
            # wait may itself reap just before interruption; returncode then
            # revokes group signalling, avoiding a recycled PID/group target.
            if not waited and process.returncode is None:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass  # ESRCH is the only benign missing-group result.
                finally:
                    process.wait(timeout=5)
        finally:
            process.stdout.close()
            process.stderr.close()


def verify_events(output, count):
    events = [json.loads(line) for line in output.splitlines()]
    allowed = {"TestP4CookieUnderload", *(f"TestP4CookieUnderload/trailers={trailers}/disable={disable}" for trailers in ("false", "true") for disable in ("false", "true"))}
    passed = {name: 0 for name in allowed}
    package_pass = 0
    for event in events:
        if not isinstance(event, dict):
            raise ValueError("unexpected JSON event shape")
        if event.get("Package") != "github.com/amnezia-vpn/amneziawg-go/v3/device" or event.get("Action") in ("skip", "fail"):
            raise ValueError("unexpected skipped/failed event or package")
        name = event.get("Test")
        if name is not None and name not in allowed:
            raise ValueError("unexpected test execution")
        if event.get("Action") == "pass":
            if name is None:
                package_pass += 1
            else:
                passed[name] += 1
    if package_pass != 1 or any(value != count for value in passed.values()):
        raise ValueError("exact test pass counts not observed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True, help="clean detached official checkout at the pinned commit")
    parser.add_argument("--scratch", type=Path, required=True, help="private existing HOME-only scratch directory")
    parser.add_argument("--cache", type=Path, required=True, help="private existing HOME-only Go build cache")
    parser.add_argument("--module-cache", type=Path, required=True, help="existing owned HOME-only pinned Go module cache")
    parser.add_argument("--count", type=int, default=20)
    parser.add_argument("--race", action="store_true")
    args = parser.parse_args()
    if not 1 <= args.count <= 50:
        raise ValueError("count outside bounded range")
    home = Path.home().resolve()
    source = args.source.resolve(strict=True)
    scratch, cache = (p.resolve(strict=True) for p in (args.scratch, args.cache))
    module_cache = args.module_cache.resolve(strict=True)
    for directory in (source, scratch, cache, module_cache):
        if not directory.is_dir() or not directory.is_relative_to(home) or directory == home:
            raise ValueError("only explicit HOME subdirectories are allowed")
    for directory in (scratch, cache):
        st = directory.stat()
        if st.st_uid != os.getuid() or st.st_mode & 0o077:
            raise ValueError("scratch/cache must be private and owned")
    if module_cache.stat().st_uid != os.getuid() or module_cache.stat().st_mode & 0o022:
        raise ValueError("module cache must be owned and not writable by others")
    archive = verify_source(source)
    backing = Path(__file__).resolve().parent / "upstream-tests/cookie_underload_test.go"
    test_bytes = backing.read_bytes()
    overlay_hash = hashlib.sha256(test_bytes).hexdigest()
    # No inherited GOENV/GOFLAGS/CC/test wrapper, preload or compiler flags.
    env = dict(PATH="/usr/bin:/bin", LANG="C", LC_ALL="C", GOENV="off", GOTOOLCHAIN="local", GOWORK="off", GOFLAGS="", GOPROXY="off", GOSUMDB="off", GOOS="linux", GOARCH="amd64", CGO_ENABLED="1" if args.race else "0", CC="/usr/bin/gcc", CXX="/usr/bin/g++", GOMODCACHE=str(module_cache), GOCACHE=str(cache), GOTMPDIR=str(scratch), TMPDIR=str(scratch))
    try:
        with tempfile.TemporaryDirectory(prefix="p4-cookie-overlay-", dir=scratch) as directory:
            exported = Path(directory) / "source"
            exported.mkdir(mode=0o700)
            proof = export_source(archive, exported)
            copied_test = Path(directory) / "test.go"
            copied_test.write_bytes(test_bytes)
            os.chmod(copied_test, 0o600)
            code, stdout, stderr = bounded_command(["/usr/bin/go", "mod", "verify"], exported, env, 30)
            if code != 0 or stdout.strip() != b"all modules verified" or stderr:
                raise ValueError("cached upstream dependency verification refused")
            overlay = Path(directory) / "overlay.json"
            overlay.write_text(json.dumps({"Replace": {str(exported / "device/p4_cookie_underload_test.go"): str(copied_test)}}))
            command = ["/usr/bin/go", "test", "-json", "-mod=readonly", "-tags=p4_cookie_overlay", "-overlay", str(overlay), "-run", "^TestP4CookieUnderload$", f"-count={args.count}", "-timeout=60s"]
            if args.race:
                command.append("-race")
            code, stdout, stderr = bounded_command([*command, "./device"], exported, env, 120)
            if code != 0 or stderr:
                raise ValueError("engine overlay test refused; raw subprocess output withheld")
            verify_events(stdout, args.count)
            verify_export(exported, proof)
            if hashlib.sha256(copied_test.read_bytes()).hexdigest() != overlay_hash:
                raise ValueError("executed overlay bytes changed")
    finally:
        # Overlay JSON is gone before any subsequent normal peer build. The
        # upstream disk tree is never changed, including on test refusal.
        verify_source(source)
    if hashlib.sha256(backing.read_bytes()).hexdigest() != overlay_hash:
        raise ValueError("overlay source changed during test")
    print(f"PASS source-engine-only pin={PIN} export_sha256={ARCHIVE_SHA256} overlay_sha256={overlay_hash} cases={args.count * 4} race={args.race} source_unchanged=true overlay_removed=true")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        # No raw engine output or source/key contents in a shareable refusal.
        raise SystemExit(f"REFUSE cookie-overlay: {type(exc).__name__}") from None
