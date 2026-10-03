#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Offline, add-only peer export/build. CPU guards are not transport acceptance."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
HELPER = HERE.parent / "p4_awg_peer/run_cookie_overlay.py"
HELPER_BYTES = HELPER.read_bytes()
base = importlib.util.module_from_spec(importlib.util.spec_from_file_location("p4_matrix_export", HELPER))
exec(compile(HELPER_BYTES, str(HELPER), "exec"), base.__dict__)
CASES = ("TestIPShapes", "TestFragments", "TestMalformedShapes", "TestByteCountAndFailure", "TestUDPFlowIdentity")
PACKAGE = "github.com/amnezia-vpn/amneziawg-go/v3/p4-matrix-peer"


def verify_events(output, count):
    passes = dict.fromkeys(CASES, 0)
    package_pass = 0
    for line in output.splitlines():
        event = json.loads(line)
        if not isinstance(event, dict) or event.get("Package") != PACKAGE:
            raise ValueError("unexpected event/package")
        action = event.get("Action")
        if not isinstance(action, str) or action not in {"start", "run", "pause", "cont", "output", "pass", "bench"}:
            raise ValueError("missing/unsupported action")
        name = event.get("Test")
        if "Test" in event and (not isinstance(name, str) or name not in passes):
            raise ValueError("unexpected test")
        if action == "pass":
            if name is None: package_pass += 1
            else: passes[name] += 1
    if package_pass != 1 or any(value != count for value in passes.values()):
        raise ValueError("exact named test counts absent")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source", "scratch", "cache", "module-cache", "output"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    parser.add_argument("--count", type=int, default=20)
    parser.add_argument("--race", action="store_true")
    args = parser.parse_args()
    if not 1 <= args.count <= 50: raise ValueError("bounded count required")
    home = Path.home().resolve()
    source, scratch, cache, modules = (p.resolve(strict=True) for p in (args.source, args.scratch, args.cache, args.module_cache))
    for path in (source, scratch, cache, modules):
        if not path.is_dir() or path == home or not path.is_relative_to(home) or path.stat().st_uid != os.getuid(): raise ValueError("owned HOME subdirectory required")
    if scratch.stat().st_mode & 0o077 or cache.stat().st_mode & 0o077 or modules.stat().st_mode & 0o022: raise ValueError("unsafe cache mode")
    output = args.output.parent.resolve(strict=True) / args.output.name
    st = output.parent.stat()
    if not output.parent.is_relative_to(home) or output.parent == home or st.st_uid != os.getuid() or st.st_mode & 0o077 or output.exists() or output.is_symlink() or output.with_suffix(".receipt.json").exists(): raise ValueError("new private output required")
    archive = base.verify_source(source)
    files = {p.name: p.read_bytes() for p in sorted(HERE.glob("*.go"))}
    hashes = {name: hashlib.sha256(data).hexdigest() for name, data in files.items()}
    builder = Path(__file__).read_bytes()
    env = dict(PATH="/usr/bin:/bin", LANG="C", LC_ALL="C", GOENV="off", GOTOOLCHAIN="local", GOWORK="off", GOFLAGS="", GOPROXY="off", GOSUMDB="off", GOOS="linux", GOARCH="amd64", CGO_ENABLED="0", CC="/usr/bin/gcc", CXX="/usr/bin/g++", GOMODCACHE=str(modules), GOCACHE=str(cache), GOTMPDIR=str(scratch), TMPDIR=str(scratch))
    receipt = dict(upstream_sha=base.PIN, export_sha256=base.ARCHIVE_SHA256, fixture_sha256=hashes, builder_sha256=hashlib.sha256(builder).hexdigest(), exporter_sha256=hashlib.sha256(HELPER_BYTES).hexdigest(), source_tests_only=True, namespace_run=False, engine="official-amneziawg-go-v3-including-standard-WG-mode", count=args.count, race_tests=args.race)
    success = False
    try:
        with tempfile.TemporaryDirectory(prefix="p4-matrix-peer-", dir=scratch) as directory:
            root = Path(directory); exported = root / "source"; exported.mkdir(mode=0o700)
            proof = base.export_source(archive, exported)
            peer = exported / "p4-matrix-peer"; peer.mkdir(mode=0o700)
            for name, data in files.items():
                with (peer / name).open("xb") as file:
                    os.chmod(peer / name, 0o600); file.write(data)
                proof[f"p4-matrix-peer/{name}"] = hashes[name]
            code, stdout, stderr = base.bounded_command(["/usr/bin/go", "mod", "verify"], exported, env, 30)
            if code or stdout.strip() != b"all modules verified" or stderr: raise ValueError("offline modules refused")
            code, stdout, stderr = base.bounded_command(["/usr/bin/go", "version"], exported, env, 10)
            if code or stderr: raise ValueError("toolchain refused")
            receipt["toolchain"] = stdout.decode().strip()
            command = ["/usr/bin/go", "test", "-json", "-mod=readonly", f"-count={args.count}", "-timeout=60s"]
            if args.race: command.append("-race")
            code, stdout, stderr = base.bounded_command([*command, "./p4-matrix-peer"], exported, {**env,"CGO_ENABLED":"1" if args.race else "0"}, 120)
            if code or stderr: raise ValueError("CPU tests refused; raw output withheld")
            verify_events(stdout, args.count)
            code, _, stderr = base.bounded_command(["/usr/bin/go", "build", "-trimpath", "-mod=readonly", "-buildvcs=false", "-o", str(output), "./p4-matrix-peer"], exported, env, 120)
            if code or stderr: raise ValueError("developer build refused; raw output withheld")
            receipt["binary_sha256"] = hashlib.sha256(output.read_bytes()).hexdigest()
            code, stdout, stderr = base.bounded_command(["/usr/bin/go", "version", "-m", str(output)], exported, env, 10)
            if code or stderr: raise ValueError("binary attestation refused")
            receipt["binary_attestation"] = stdout.decode().splitlines()[1:]
            base.verify_export(exported, proof)
        if base.verify_source(source) != archive or any((HERE/name).read_bytes()!=data for name,data in files.items()) or Path(__file__).read_bytes()!=builder or HELPER.read_bytes()!=HELPER_BYTES: raise ValueError("inputs changed")
        receipt.update(source_unchanged=True, wrapper_tests=args.count*len(CASES), upstream_edits=False)
        with output.with_suffix(".receipt.json").open("x") as file:
            os.chmod(file.name,0o600); json.dump(receipt,file,sort_keys=True)
        success = True
        print(json.dumps({"status":"PASS", "binary_sha256":receipt["binary_sha256"], "wrapper_tests":receipt["wrapper_tests"], "race":args.race, "namespace_run":False},sort_keys=True))
    finally:
        base.verify_source(source)
        if not success and output.is_file(): output.unlink() # Exact new artifact only.


if __name__ == "__main__":
    try: main()
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        raise SystemExit(f"REFUSE matrix-peer-build: {type(exc).__name__}") from None
