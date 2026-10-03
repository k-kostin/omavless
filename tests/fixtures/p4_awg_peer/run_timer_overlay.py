#!/usr/bin/env python3
"""Pinned CPU-only timer overlay; no peer binary, sockets, real TUN or VM."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("p4_timer_export_helpers", HERE / "run_cookie_overlay.py")
helpers = importlib.util.module_from_spec(spec)
HELPER_BYTES = (HERE / "run_cookie_overlay.py").read_bytes()
exec(compile(HELPER_BYTES, str(HERE / "run_cookie_overlay.py"), "exec"), helpers.__dict__)
CASES = ("defaults", "ranges", "retry_exhaustion", "elapsed_key_expiry")


def verify_events(output, count):
    names = {"TestP4TimerBoundaries", *(f"TestP4TimerBoundaries/{case}" for case in CASES)}
    passes = {name: 0 for name in names}
    package_pass = 0
    for line in output.splitlines():
        event = json.loads(line)
        if not isinstance(event, dict) or event.get("Package") != "github.com/amnezia-vpn/amneziawg-go/v3/device":
            raise ValueError("unexpected failed/skipped event or package")
        action = event.get("Action")
        if not isinstance(action, str) or action not in {"start", "run", "pause", "cont", "output", "pass", "bench"}:
            raise ValueError("missing/unsupported event action")
        name = event.get("Test")
        if "Test" in event and (not isinstance(name, str) or name not in names):
            raise ValueError("unexpected test execution")
        if action == "pass":
            if name is None:
                package_pass += 1
            else:
                passes[name] += 1
    if package_pass != 1 or any(n != count for n in passes.values()):
        raise ValueError("exact nonempty timer pass counts not observed")


def main():
    runner_bytes = Path(__file__).read_bytes()
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ("source", "scratch", "cache", "module-cache"):
        parser.add_argument(f"--{option}", type=Path, required=True)
    parser.add_argument("--count", type=int, default=20)
    parser.add_argument("--race", action="store_true")
    args = parser.parse_args()
    if not 1 <= args.count <= 50:
        raise ValueError("bounded repeat count required")
    home = Path.home().resolve()
    source, scratch, cache, modules = (p.resolve(strict=True) for p in (args.source, args.scratch, args.cache, args.module_cache))
    for directory in (source, scratch, cache, modules):
        if not directory.is_dir() or not directory.is_relative_to(home) or directory == home or directory.stat().st_uid != os.getuid():
            raise ValueError("explicit owned HOME subdirectory required")
    if scratch.stat().st_mode & 0o077 or cache.stat().st_mode & 0o077 or modules.stat().st_mode & 0o022:
        raise ValueError("unsafe scratch/cache modes")
    archive = helpers.verify_source(source)
    paths = [HERE / "upstream-tests/cookie_underload_test.go", HERE / "upstream-tests/timer_boundaries_test.go"]
    test_bytes = [p.read_bytes() for p in paths]
    hashes = [hashlib.sha256(payload).hexdigest() for payload in test_bytes]
    env = dict(PATH="/usr/bin:/bin", LANG="C", LC_ALL="C", GOENV="off", GOTOOLCHAIN="local", GOWORK="off", GOFLAGS="", GOPROXY="off", GOSUMDB="off", GOOS="linux", GOARCH="amd64", CGO_ENABLED="1" if args.race else "0", CC="/usr/bin/gcc", CXX="/usr/bin/g++", GOMODCACHE=str(modules), GOCACHE=str(cache), GOTMPDIR=str(scratch), TMPDIR=str(scratch))
    try:
        with tempfile.TemporaryDirectory(prefix="p4-timer-overlay-", dir=scratch) as directory:
            root = Path(directory)
            exported = root / "source"
            exported.mkdir(mode=0o700)
            proof = helpers.export_source(archive, exported)
            replacements = {}
            for i, payload in enumerate(test_bytes):
                backing = root / f"test{i}.go"
                with backing.open("xb") as file:
                    os.chmod(backing, 0o600)
                    file.write(payload)
                target = exported / "device" / f"p4_timer_overlay_{i}_test.go"
                if target.exists():
                    raise ValueError("overlay may add only absent files")
                replacements[str(target)] = str(backing)
            code, stdout, stderr = helpers.bounded_command(["/usr/bin/go", "mod", "verify"], exported, env, 30)
            if code != 0 or stdout.strip() != b"all modules verified" or stderr:
                raise ValueError("offline dependency verification refused")
            overlay = root / "overlay.json"
            overlay.write_text(json.dumps({"Replace": replacements}))
            command = ["/usr/bin/go", "test", "-json", "-mod=readonly", "-tags=p4_cookie_overlay", "-overlay", str(overlay), "-run", "^TestP4TimerBoundaries$", f"-count={args.count}", "-timeout=60s"]
            if args.race:
                command.append("-race")
            code, stdout, stderr = helpers.bounded_command([*command, "./device"], exported, env, 120)
            if code != 0 or stderr:
                raise ValueError("timer engine test refused; raw output withheld")
            verify_events(stdout, args.count)
            helpers.verify_export(exported, proof)
            for i, digest in enumerate(hashes):
                if hashlib.sha256((root / f"test{i}.go").read_bytes()).hexdigest() != digest:
                    raise ValueError("executed overlay changed")
    finally:
        helpers.verify_source(source)
    if [hashlib.sha256(p.read_bytes()).hexdigest() for p in paths] != hashes:
        raise ValueError("overlay input changed during execution")
    if (HERE / "run_cookie_overlay.py").read_bytes() != HELPER_BYTES or Path(__file__).read_bytes() != runner_bytes:
        raise ValueError("runner/helper input changed during execution")
    helper_hash = hashlib.sha256(HELPER_BYTES).hexdigest()
    runner_hash = hashlib.sha256(runner_bytes).hexdigest()
    print(f"PASS source-engine-only pin={helpers.PIN} export_sha256={helpers.ARCHIVE_SHA256} runner_sha256={runner_hash} helpers_sha256={helper_hash} support_sha256={hashes[0]} timer_sha256={hashes[1]} cases={args.count * len(CASES)} race={args.race} test_expiry_delay_ms=1 source_unchanged=true overlay_removed=true")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        raise SystemExit(f"REFUSE timer-overlay: {type(exc).__name__}") from None
