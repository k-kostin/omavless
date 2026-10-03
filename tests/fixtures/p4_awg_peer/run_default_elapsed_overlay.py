#!/usr/bin/env python3
"""Opt-in default elapsed retry; CPU source engine only, never networking."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import selectors
import shutil
import signal
import stat
import subprocess
import tempfile
import time

HERE = Path(__file__).resolve().parent
HELPER_BYTES = (HERE / "run_cookie_overlay.py").read_bytes()
spec = importlib.util.spec_from_file_location("p4_elapsed_export", HERE / "run_cookie_overlay.py")
helpers = importlib.util.module_from_spec(spec)
exec(compile(HELPER_BYTES, str(HERE / "run_cookie_overlay.py"), "exec"), helpers.__dict__)
PACKAGE = "github.com/amnezia-vpn/amneziawg-go/v3/device"
TEST = "TestP4DefaultElapsedRetryAndCancellation"
UNSETTLED = []


class Unsettled(RuntimeError):
    """Preserve scratch and the unreaped owned anchor; never infer cleanup."""


def members(leader):
    found = []
    for entry in Path("/proc").iterdir():
        if entry.name.isdecimal() and int(entry.name) != leader:
            try:
                fields = (entry / "stat").read_text().rsplit(")", 1)[1].split()
                if int(fields[2]) == leader:
                    found.append(int(entry.name))
            except FileNotFoundError:
                pass
    return found


def command(args, cwd, env, timeout):
    """Fixed trusted children; retain WNOWAIT anchor until group and EOF settle."""
    if UNSETTLED:
        raise Unsettled("prior_owned_group_unsettled")
    p = subprocess.Popen(args, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                         start_new_session=True, bufsize=0)
    selector = None
    output = {"out": bytearray(), "err": bytearray()}
    deadline = time.monotonic() + timeout
    exited_at = None
    quiescent = False
    try:
        # Setup is part of owned-child cancellation too.
        selector = selectors.DefaultSelector()
        for stream, label in ((p.stdout, "out"), (p.stderr, "err")):
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, label)
        while True:
            if time.monotonic() >= deadline:
                raise ValueError("fixed_command_timeout")
            for event, _ in selector.select(0.02):
                block = os.read(event.fileobj.fileno(), 65536)
                if not block:
                    selector.unregister(event.fileobj)
                else:
                    output[event.data].extend(block)
                    if sum(map(len, output.values())) > 2 * 1024 * 1024:
                        raise ValueError("fixed_command_output_bound")
            status = os.waitid(os.P_PID, p.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
            if status is not None:
                exited_at = exited_at or time.monotonic()
                remaining = members(p.pid)
                if not remaining and not selector.get_map():
                    quiescent = True
                    code = p.wait(timeout=1)
                    return code, bytes(output["out"]), bytes(output["err"])
                if time.monotonic() - exited_at >= 2:
                    raise ValueError("fixed_command_descendant_survives")
    except BaseException:
        try:
            os.waitid(os.P_PID, p.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        except ChildProcessError:
            # Already reaped: no authority to signal a possibly recycled PGID.
            if quiescent:
                raise
            UNSETTLED.append(p)
            raise Unsettled("owned_anchor_lost_preserve")
        except BaseException as exc:
            UNSETTLED.append(p)
            raise Unsettled("owned_anchor_state_unknown") from exc
        try:
            try:
                os.killpg(p.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            until = time.monotonic() + 5
            while members(p.pid) and time.monotonic() < until:
                time.sleep(0.02)
            if members(p.pid):
                raise Unsettled("cancelled_group_not_quiescent")
            p.wait(timeout=2)
        except BaseException as exc:
            UNSETTLED.append(p)
            raise Unsettled("owned_group_preserved") from exc
        raise
    finally:
        if selector is not None:
            selector.close()
        p.stdout.close()
        p.stderr.close()


def verify_events(raw, count):
    passes = package = receipts = 0
    for line in raw.splitlines():
        event = json.loads(line)
        if not isinstance(event, dict) or event.get("Package") != PACKAGE:
            raise ValueError("unexpected_event_package")
        action = event.get("Action")
        if not isinstance(action, str) or action not in {"start", "run", "pause", "cont", "output", "pass", "bench"}:
            raise ValueError("failed_skipped_or_malformed_action")
        name = event.get("Test")
        if "Test" in event and (not isinstance(name, str) or name != TEST):
            raise ValueError("unexpected_test")
        if action == "pass":
            if name is None:
                package += 1
            else:
                elapsed = event.get("Elapsed")
                if type(elapsed) not in (int, float) or not 0 < elapsed <= 18:
                    raise ValueError("actual_case_elapsed_bound")
                passes += 1
        if action == "output":
            payload = event.get("Output")
            if not isinstance(payload, str):
                raise ValueError("malformed_output")
            if "p4_elapsed_receipt" in payload:
                pattern = (r"    default_elapsed_retry_test\.go:\d+: p4_elapsed_receipt retry_target_ns=(\d+) observed_retry_ns=(\d+) quiet_window_ns=(\d+) malformed_worker_refused=true actual_session=true h1_initial=1 h1_retry=1 h1_after=0 h4_after=1 defaults_unchanged=true\n")
                match = re.fullmatch(pattern, payload)
                if name != TEST or not match:
                    raise ValueError("malformed_elapsed_receipt")
                target, observed, quiet = map(int, match.groups())
                if not 5_000_000_000 <= target < 5_334_000_000 or not 5_000_000_000 <= observed <= 8_000_000_000 or not 5_500_000_000 <= quiet <= 18_000_000_000:
                    raise ValueError("elapsed_observation_bounds")
                receipts += 1
            elif not (payload == f"=== RUN   {TEST}\n" or payload == "PASS\n" or re.fullmatch(r"--- PASS: " + TEST + r" \(\d+\.\d+s\)\n", payload)):
                raise ValueError("unexpected_output_or_timeout")
    if (passes, package, receipts) != (count, 1, count):
        raise ValueError("exact_nonempty_elapsed_counts_missing")


def private_directory(path, home):
    resolved = path.resolve(strict=True)
    if not resolved.is_relative_to(home) or resolved == home:
        raise ValueError("owned_home_subdirectory_required")
    for ancestor in (resolved, *resolved.parents):
        if ancestor == home:
            break
        m = ancestor.lstat()
        if not stat.S_ISDIR(m.st_mode) or m.st_uid != os.getuid() or m.st_mode & 0o7022 or (ancestor == resolved and stat.S_IMODE(m.st_mode) != 0o700):
            raise ValueError("private_owned_ancestors_required")
    return resolved


def save(directory, name, data, mode=0o600):
    fd = os.open(directory / name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
    try:
        os.fchmod(fd, mode)
        with os.fdopen(fd, "wb", closefd=False) as stream:
            stream.write(data)
            stream.flush()
            os.fsync(fd)
    finally:
        os.close(fd)


def main():
    runner = Path(__file__).read_bytes()
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source", "scratch", "cache", "module-cache", "artifacts"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--count", type=int, default=1)
    parser.add_argument("--race", action="store_true")
    args = parser.parse_args()
    if not 1 <= args.count <= 2:
        raise ValueError("one_or_two_bounded_cases_only")
    home = Path.home().resolve()
    source = args.source.resolve(strict=True)
    modules = args.module_cache.resolve(strict=True)
    for directory in (source, modules):
        if not directory.is_relative_to(home) or directory == home or not directory.is_dir() or directory.stat().st_uid != os.getuid() or directory.stat().st_mode & 0o022:
            raise ValueError("unsafe_owned_source_or_modules")
    scratch, cache, artifacts = [private_directory(p, home) for p in (args.scratch, args.cache, args.artifacts)]
    if any(artifacts.iterdir()):
        raise ValueError("fresh_artifacts_required")
    archive = helpers.verify_source(source)
    paths = [HERE / "upstream-tests/cookie_underload_test.go", HERE / "upstream-tests/default_elapsed_retry_test.go"]
    fixture_commit = helpers.git(HERE, "rev-parse", "HEAD").decode().strip()
    if not re.fullmatch(r"[0-9a-f]{40}", fixture_commit):
        raise ValueError("fixture_commit_required")
    tracked = [str(p) for p in [Path(__file__), HERE / "run_cookie_overlay.py", *paths]]
    helpers.git(HERE, "ls-files", "--error-unmatch", "--", *tracked)
    helpers.git(HERE, "diff", "--exit-code", "HEAD", "--", *tracked)
    inputs = [p.read_bytes() for p in paths]
    save(artifacts, "runner.py", runner)
    save(artifacts, "export-helper.py", HELPER_BYTES)
    for index, payload in enumerate(inputs):
        save(artifacts, f"overlay{index}.go", payload)
    env = dict(PATH="/usr/bin:/bin", HOME=str(home), LANG="C", LC_ALL="C", GOENV="off", GOTOOLCHAIN="local", GOWORK="off", GOFLAGS="", GOPROXY="off", GOSUMDB="off", GOOS="linux", GOARCH="amd64", CGO_ENABLED="1" if args.race else "0", CC="/usr/bin/gcc", CXX="/usr/bin/g++", GOMODCACHE=str(modules), GOCACHE=str(cache), GOTMPDIR=str(scratch), TMPDIR=str(scratch))
    root = Path(tempfile.mkdtemp(prefix="p4-default-elapsed-", dir=scratch))
    identity = root.stat().st_dev, root.stat().st_ino
    exported = root / "source"
    exported.mkdir(mode=0o700)
    try:
        proof = helpers.export_source(archive, exported)
        replacements = {}
        for index, payload in enumerate(inputs):
            backing = root / f"overlay{index}.go"
            save(root, backing.name, payload)
            destination = exported / "device" / ("p4_elapsed_support_test.go" if index == 0 else "default_elapsed_retry_test.go")
            if destination.exists():
                raise ValueError("add_only_overlay_required")
            replacements[str(destination)] = str(backing)
        save(root, "overlay.json", json.dumps({"Replace": replacements}).encode())
        code, out, err = command(["/usr/bin/go", "mod", "verify"], exported, env, 30)
        save(artifacts, "modules.stdout", out)
        save(artifacts, "modules.stderr", err)
        if code != 0 or out.strip() != b"all modules verified" or err:
            raise ValueError("offline_modules_refused")
        binary = artifacts / "device.test"
        build = ["/usr/bin/go", "test", "-c", "-trimpath", "-mod=readonly", "-tags=p4_cookie_overlay", "-overlay", str(root / "overlay.json"), "-o", str(binary)]
        if args.race:
            build.append("-race")
        code, out, err = command([*build, "./device"], exported, env, 120)
        save(artifacts, "build.stdout", out)
        save(artifacts, "build.stderr", err)
        if code != 0 or out or err:
            raise ValueError("frozen_test_build_refused")
        m = binary.lstat()
        if not stat.S_ISREG(m.st_mode) or m.st_uid != os.getuid() or m.st_nlink != 1 or m.st_mode & 0o7022 or os.listxattr(binary):
            raise ValueError("unsafe_frozen_binary")
        os.chmod(binary, 0o700)
        binary_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
        code, out, err = command(["/usr/bin/go", "tool", "test2json", "-p", PACKAGE, "-t", str(binary), "-test.v=test2json", f"-test.run=^{TEST}$", f"-test.count={args.count}", "-test.timeout=45s"], exported, env, 60)
        save(artifacts, "events.jsonl", out)
        save(artifacts, "stderr.log", err)
        if code != 0 or err:
            raise ValueError("actual_default_elapsed_test_nonpass")
        verify_events(out, args.count)
        if hashlib.sha256(binary.read_bytes()).hexdigest() != binary_hash:
            raise ValueError("executed_binary_changed")
        helpers.verify_export(exported, proof)
        for index, payload in enumerate(inputs):
            if (root / f"overlay{index}.go").read_bytes() != payload:
                raise ValueError("executed_overlay_changed")
        if [p.read_bytes() for p in paths] != inputs or Path(__file__).read_bytes() != runner or (HERE / "run_cookie_overlay.py").read_bytes() != HELPER_BYTES:
            raise ValueError("runner_inputs_changed")
        if helpers.git(HERE, "rev-parse", "HEAD").decode().strip() != fixture_commit:
            raise ValueError("fixture_commit_changed")
        receipt = dict(fixture_commit=fixture_commit, pin=helpers.PIN, source_archive_sha256=helpers.ARCHIVE_SHA256,
                       runner_sha256=hashlib.sha256(runner).hexdigest(), helper_sha256=hashlib.sha256(HELPER_BYTES).hexdigest(),
                       support_sha256=hashlib.sha256(inputs[0]).hexdigest(), overlay_sha256=hashlib.sha256(inputs[1]).hexdigest(),
                       binary_sha256=binary_hash, cases=args.count, race=args.race,
                       source_engine_only=True, transport=False, vm=False,
                       product_120s_rekey=False, defaults_unchanged=True)
        save(artifacts, "receipt.json", json.dumps(receipt, sort_keys=True).encode())
    finally:
        # Uncertain cancellation preserves the exact export/overlay; no cleanup
        # claim and no further child launch may hide an unsettled owned group.
        if not UNSETTLED:
            helpers.verify_source(source)
            if (root.stat().st_dev, root.stat().st_ino) != identity:
                raise ValueError("scratch_identity_changed_preserve")
            shutil.rmtree(root)
    print("PASS source-engine-default-elapsed-only " + json.dumps(receipt, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
        raise SystemExit("REFUSE default-elapsed-overlay: " + type(exc).__name__) from None
