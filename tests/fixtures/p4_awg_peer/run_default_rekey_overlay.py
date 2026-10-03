#!/usr/bin/env python3
"""Explicit build/execute phases for one CPU-only real-default120s rekey case."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
SUPERVISOR = HERE / "run_default_elapsed_overlay.py"
SUPERVISOR_BYTES = SUPERVISOR.read_bytes()
SUPERVISOR_SHA = "dcec0ed6b5bbde5fc6015f8a66dd1a3255cdfa2f3c2d6b8802d046edacfde165"
if hashlib.sha256(SUPERVISOR_BYTES).hexdigest() != SUPERVISOR_SHA:
    raise ValueError("sealed_supervisor_changed")
spec = importlib.util.spec_from_file_location("p4_rekey_owned_commands", SUPERVISOR)
owned = importlib.util.module_from_spec(spec)
exec(compile(SUPERVISOR_BYTES, str(SUPERVISOR), "exec"), owned.__dict__)
TEST = "TestP4DefaultElapsedRekey"
DIAGNOSTIC = "TestP4RekeyReadEpochOrdering"
PACKAGE = "github.com/amnezia-vpn/amneziawg-go/v3/device"
OVERLAY = HERE / "upstream-tests/default_elapsed_rekey_test.go"


def verify_events(raw, diagnostic=False):
    test = DIAGNOSTIC if diagnostic else TEST
    cases = package = observations = 0
    for line in raw.splitlines():
        event = json.loads(line)
        if not isinstance(event, dict) or event.get("Package") != PACKAGE:
            raise ValueError("unexpected_event_package")
        action, name = event.get("Action"), event.get("Test")
        if not isinstance(action, str) or action not in {"start", "run", "pause", "cont", "output", "pass"}:
            raise ValueError("failed_skipped_or_malformed_action")
        if "Test" in event and (not isinstance(name, str) or name != test):
            raise ValueError("unexpected_test")
        if action == "pass":
            if name is None:
                package += 1
            else:
                elapsed = event.get("Elapsed")
                if type(elapsed) not in (int, float) or not ((0 <= elapsed <= 5) if diagnostic else (120 <= elapsed <= 150)):
                    raise ValueError("actual_case_elapsed_bound")
                cases += 1
        if action == "output":
            text = event.get("Output")
            if not isinstance(text, str):
                raise ValueError("malformed_output")
            if diagnostic and "p4_rekey_read_receipt" in text:
                if name != test or not re.fullmatch(r"    default_elapsed_rekey_test\.go:\d+: p4_rekey_read_receipt first_offset=16 configured_padding=48 pending_offset=16 next_offset=64 empty_reads=1 emitted_packets=0 actual_worker=true network_fds=false\n", text):
                    raise ValueError("malformed_actual_read_epoch_receipt")
                observations += 1
            elif not diagnostic and "p4_rekey_receipt" in text:
                pattern = (r"    default_elapsed_rekey_test\.go:\d+: p4_rekey_receipt pre_age_ns=(\d+) trigger_data_age_ns=(\d+) rekey_h1_age_ns=(\d+) observed_body_ns=(\d+) baseline_bidir=true pre_no_rekey=true echo_cancels_idle=true real_receive_aead=true new_indexed_sessions=true post_bidir=true client_h1=2 server_h1=0 server_h2=2 defaults_unchanged=true network_fds=false\n")
                match = re.fullmatch(pattern, text)
                if name != test or match is None:
                    raise ValueError("malformed_causal_rekey_receipt")
                pre, data, initiation, body = map(int, match.groups())
                if not (117_000_000_000 <= pre < 120_000_000_000 <= data <= initiation <= body <= 150_000_000_000):
                    raise ValueError("actual_causal_age_bounds")
                observations += 1
            elif not (text == f"=== RUN   {test}\n" or text == "PASS\n" or re.fullmatch(r"--- PASS: " + test + r" \(\d+\.\d+s\)\n", text)):
                raise ValueError("unexpected_output_or_timeout")
    if (cases, package, observations) != (1, 1, 1):
        raise ValueError("exact_single_rekey_receipt_missing")


def object_bytes(path, maximum):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        before = os.fstat(fd)
        if not stat.S_ISREG(before.st_mode) or before.st_uid != os.getuid() or before.st_mode & 0o7022 or before.st_nlink != 1 or not 0 < before.st_size <= maximum or os.listxattr(fd):
            raise ValueError("unsafe_frozen_member")
        data = bytearray()
        while len(data) <= maximum:
            chunk = os.read(fd, min(65536, maximum + 1 - len(data)))
            if not chunk:
                break
            data.extend(chunk)
        key = lambda m: (m.st_dev, m.st_ino, m.st_uid, m.st_gid, m.st_mode, m.st_nlink, m.st_size, m.st_mtime_ns, m.st_ctime_ns)
        if key(before) != key(os.fstat(fd)) or key(before) != key(path.lstat()) or len(data) != before.st_size:
            raise ValueError("frozen_member_drift")
        return bytes(data)
    finally:
        os.close(fd)


def inputs():
    return {"runner.py": Path(__file__).read_bytes(), "supervisor.py": SUPERVISOR.read_bytes(),
            "export-helper.py": (HERE / "run_cookie_overlay.py").read_bytes(),
            "overlay.go": OVERLAY.read_bytes()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase", required=True, choices=("build", "execute"))
    for name in ("source", "scratch", "cache", "module-cache", "artifacts"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--race", action="store_true")
    parser.add_argument("--diagnostic", action="store_true")
    args = parser.parse_args()
    if args.diagnostic and args.race:
        raise ValueError("diagnostic_is_fixed_ordinary_only")
    test = DIAGNOSTIC if args.diagnostic else TEST
    home = Path.home().resolve()
    source, modules = args.source.resolve(strict=True), args.module_cache.resolve(strict=True)
    for path in (source, modules):
        if path == home or not path.is_relative_to(home) or not path.is_dir() or path.stat().st_uid != os.getuid() or path.stat().st_mode & 0o022:
            raise ValueError("unsafe_owned_source_or_modules")
    scratch, cache, artifacts = [owned.private_directory(p, home) for p in (args.scratch, args.cache, args.artifacts)]
    captured = inputs()
    if captured["supervisor.py"] != SUPERVISOR_BYTES or captured["export-helper.py"] != owned.HELPER_BYTES:
        raise ValueError("sealed_helpers_changed")
    fixture = owned.helpers.git(HERE, "rev-parse", "HEAD").decode().strip()
    paths = [str(Path(__file__)), str(SUPERVISOR), str(HERE / "run_cookie_overlay.py"), str(OVERLAY)]
    owned.helpers.git(HERE, "ls-files", "--error-unmatch", "--", *paths)
    owned.helpers.git(HERE, "diff", "--exit-code", "HEAD", "--", *paths)
    archive = owned.helpers.verify_source(source)
    env = dict(PATH="/usr/bin:/bin", HOME=str(home), LANG="C", LC_ALL="C", GOENV="off", GOTOOLCHAIN="local", GOWORK="off", GOFLAGS="", GOPROXY="off", GOSUMDB="off", GOOS="linux", GOARCH="amd64", GOMAXPROCS="2", CGO_ENABLED="1" if args.race else "0", CC="/usr/bin/gcc", CXX="/usr/bin/g++", GOMODCACHE=str(modules), GOCACHE=str(cache), GOTMPDIR=str(scratch), TMPDIR=str(scratch))
    hashes = {name: hashlib.sha256(data).hexdigest() for name, data in captured.items()}
    binary = artifacts / "device.test"
    tool_hash = hashlib.sha256(Path("/usr/bin/go").read_bytes()).hexdigest()
    expected = dict(fixture_commit=fixture, pin=owned.helpers.PIN, source_archive_sha256=owned.helpers.ARCHIVE_SHA256,
                    input_sha256=hashes, tool_sha256=tool_hash, race=args.race, cases=1,
                    source_engine_only=True, network_transport=False, vm=False, default_rekey_seconds=120,
                    test=test, diagnostic=args.diagnostic, elapsed_rekey_case=not args.diagnostic)
    if args.phase == "build":
        if any(artifacts.iterdir()):
            raise ValueError("fresh_build_artifacts_required")
        for name, data in captured.items():
            owned.save(artifacts, name, data)
        root = Path(tempfile.mkdtemp(prefix="p4-default-rekey-", dir=scratch))
        identity = root.stat().st_dev, root.stat().st_ino
        exported = root / "source"
        exported.mkdir(mode=0o700)
        try:
            proof = owned.helpers.export_source(archive, exported)
            target = exported / "device/default_elapsed_rekey_test.go"
            if target.exists():
                raise ValueError("add_only_overlay_required")
            owned.save(root, "overlay.go", captured["overlay.go"])
            owned.save(root, "overlay.json", json.dumps({"Replace": {str(target): str(root / "overlay.go")}}).encode())
            code, out, err = owned.command(["/usr/bin/go", "mod", "verify"], exported, env, 30)
            owned.save(artifacts, "modules.stdout", out)
            owned.save(artifacts, "modules.stderr", err)
            if code != 0 or out.strip() != b"all modules verified" or err:
                raise ValueError("offline_module_verification_refused")
            build = ["/usr/bin/go", "test", "-c", "-p=2", "-trimpath", "-mod=readonly", "-tags=p4_cookie_overlay", "-overlay", str(root / "overlay.json"), "-o", str(binary)]
            if args.race:
                build.append("-race")
            code, out, err = owned.command([*build, "./device"], exported, env, 120)
            owned.save(artifacts, "build.stdout", out)
            owned.save(artifacts, "build.stderr", err)
            if code != 0 or out or err:
                raise ValueError("frozen_compile_refused_no_execution")
            os.chmod(binary, 0o700)
            expected["binary_sha256"] = hashlib.sha256(object_bytes(binary, 256 * 1024 * 1024)).hexdigest()
            owned.helpers.verify_export(exported, proof)
            if (root / "overlay.go").read_bytes() != captured["overlay.go"]:
                raise ValueError("compiled_overlay_changed")
            owned.save(artifacts, "build-receipt.json", json.dumps({**expected, "execution": False}, sort_keys=True).encode())
        finally:
            if not owned.UNSETTLED:
                owned.helpers.verify_source(source)
                if (root.stat().st_dev, root.stat().st_ino) != identity:
                    raise ValueError("scratch_identity_changed_preserve")
                shutil.rmtree(root)
        result = "BUILT_NO_ENGINE_EXECUTION"
    else:
        for name, data in captured.items():
            if object_bytes(artifacts / name, 65536) != data:
                raise ValueError("frozen_inputs_differ")
        build_receipt = json.loads(object_bytes(artifacts / "build-receipt.json", 16384))
        expected["binary_sha256"] = hashlib.sha256(object_bytes(binary, 256 * 1024 * 1024)).hexdigest()
        if build_receipt != {**expected, "execution": False} or any(os.path.lexists(artifacts / name) for name in ("execution-attempt.json", "events.jsonl", "test.stderr", "execution-receipt.json")):
            raise ValueError("exact_unexecuted_build_receipt_required")
        # Consumes this artifact's one execution slot before child creation.
        # Timeout/unknown supervision cannot leave it apparently unattempted.
        owned.save(artifacts, "execution-attempt.json", json.dumps({**expected, "attempted": True}, sort_keys=True).encode())
        try:
            code, out, err = owned.command(["/usr/bin/go", "tool", "test2json", "-p", PACKAGE, "-t", str(binary), "-test.v=test2json", f"-test.run=^{test}$", "-test.count=1", "-test.timeout=10s" if args.diagnostic else "-test.timeout=180s"], artifacts, env, 15 if args.diagnostic else 200)
        except BaseException as exc:
            # The sealed supervisor returns output only when settled; never
            # misrepresent partial live output as a retained complete receipt.
            owned.save(artifacts, "supervision-refusal.json", json.dumps({
                "exception_type": type(exc).__name__, "attempted": True,
                "complete_output_retained": False,
                "unsettled_anchors": len(owned.UNSETTLED),
                "pass": False}, sort_keys=True).encode())
            raise
        owned.save(artifacts, "events.jsonl", out)
        owned.save(artifacts, "test.stderr", err)
        if code != 0 or err:
            raise ValueError("actual_default_rekey_nonpass_preserve")
        verify_events(out, args.diagnostic)
        if hashlib.sha256(object_bytes(binary, 256 * 1024 * 1024)).hexdigest() != expected["binary_sha256"]:
            raise ValueError("executed_binary_changed")
        owned.helpers.verify_source(source)
        owned.save(artifacts, "execution-receipt.json", json.dumps({**expected, "execution": True}, sort_keys=True).encode())
        result = "PASS_CPU_SOURCE_ENGINE_ONLY"
    if inputs() != captured or owned.helpers.git(HERE, "rev-parse", "HEAD").decode().strip() != fixture or hashlib.sha256(Path("/usr/bin/go").read_bytes()).hexdigest() != tool_hash:
        raise ValueError("source_or_tool_drift")
    print(result + " " + json.dumps(expected, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
        raise SystemExit("REFUSE default-rekey-overlay: " + type(exc).__name__) from None
