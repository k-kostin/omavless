#!/usr/bin/env python3
"""Build only the opt-in tagged developer peer from a pinned private export.

No network/namespace/key generation or application package action occurs here.
The official engine is an exact source export, not a patched module cache.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

import run_cookie_overlay as base


def verify_tests(output, package, expected, count):
    passes = {name: 0 for name in expected}
    package_pass = 0
    for line in output.splitlines():
        event = json.loads(line)
        if not isinstance(event, dict) or event.get("Package") != package or event.get("Action") in ("skip", "fail"):
            raise ValueError("test event refusal")
        name = event.get("Test")
        if name is not None and name not in passes:
            raise ValueError("unexpected test")
        if event.get("Action") == "pass":
            if name is None: package_pass += 1
            else: passes[name] += 1
    if package_pass != 1 or any(value != count for value in passes.values()):
        raise ValueError("exact named test counts missing")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source", "scratch", "cache", "module-cache", "output"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    parser.add_argument("--count", type=int, default=20)
    parser.add_argument("--race", action="store_true", help="race-enable pure tests only; peer remains CGO=0")
    args = parser.parse_args()
    if not 1 <= args.count <= 50: raise ValueError("bounded repeat count required")
    home = Path.home().resolve()
    source, scratch, cache, modules = (path.resolve(strict=True) for path in (args.source, args.scratch, args.cache, args.module_cache))
    for directory in (source, scratch, cache, modules):
        if not directory.is_dir() or directory == home or not directory.is_relative_to(home): raise ValueError("HOME subdirectory required")
    for directory in (scratch, cache):
        st = directory.stat()
        if st.st_uid != os.getuid() or st.st_mode & 0o077: raise ValueError("private owned scratch/cache required")
    if modules.stat().st_uid != os.getuid() or modules.stat().st_mode & 0o022: raise ValueError("owned non-shared-writable module cache required")
    output = args.output.parent.resolve(strict=True) / args.output.name
    st = output.parent.stat()
    if not output.parent.is_relative_to(home) or output.parent == home or st.st_uid != os.getuid() or st.st_mode & 0o077 or output.exists() or output.is_symlink() or output.with_suffix(".receipt.json").exists(): raise ValueError("new output inside explicit private HOME parent required")
    archive = base.verify_source(source)
    fixture = Path(__file__).resolve().parent
    snapshots = {path.name: path.read_bytes() for path in sorted(fixture.glob("*.go"))}
    overlay_files = {name: (fixture / "upstream-tests" / name).read_bytes() for name in ("cookie_transport_hooks.go", "cookie_underload_test.go", "cookie_transport_test.go")}
    hashes = {name: hashlib.sha256(payload).hexdigest() for name, payload in {**snapshots, **overlay_files}.items()}
    tools = {str(Path(__file__).resolve()): Path(__file__).read_bytes(), str(Path(base.__file__).resolve()): Path(base.__file__).read_bytes()}
    env = dict(PATH="/usr/bin:/bin", LANG="C", LC_ALL="C", GOENV="off", GOTOOLCHAIN="local", GOWORK="off", GOFLAGS="", GOPROXY="off", GOSUMDB="off", GOOS="linux", GOARCH="amd64", CGO_ENABLED="0", CC="/usr/bin/gcc", CXX="/usr/bin/g++", GOMODCACHE=str(modules), GOCACHE=str(cache), GOTMPDIR=str(scratch), TMPDIR=str(scratch))
    receipt = dict(upstream_sha=base.PIN, export_sha256=base.ARCHIVE_SHA256, fixture_sha256=hashes, builder_sha256=hashlib.sha256(tools[str(Path(__file__).resolve())]).hexdigest(), exporter_sha256=hashlib.sha256(tools[str(Path(base.__file__).resolve())]).hexdigest(), source_tests_only=True, namespace_run=False, count=args.count, race_tests=args.race)
    success = False
    try:
        with tempfile.TemporaryDirectory(prefix="p4-cookie-peer-", dir=scratch) as directory:
            root = Path(directory); exported = root / "source"; exported.mkdir(mode=0o700)
            proof = base.export_source(archive, exported)
            peer = exported / "p4-peer"; peer.mkdir(mode=0o700)
            for name, payload in snapshots.items():
                (peer / name).write_bytes(payload); os.chmod(peer / name, 0o600)
                proof[f"p4-peer/{name}"] = hashes[name]
            replacements = {}
            for name, payload in overlay_files.items():
                backing = root / name; backing.write_bytes(payload); os.chmod(backing, 0o600)
                replacements[str(exported / "device" / f"p4_{name}")] = str(backing)
            overlay = root / "overlay.json"; overlay.write_text(json.dumps({"Replace": replacements}))
            code, stdout, stderr = base.bounded_command(["/usr/bin/go", "mod", "verify"], exported, env, 30)
            if code != 0 or stdout.strip() != b"all modules verified" or stderr: raise ValueError("cached modules refused")
            version = base.bounded_command(["/usr/bin/go", "version"], exported, env, 10)
            if version[0] != 0 or version[2]: raise ValueError("toolchain refused")
            receipt["toolchain"] = version[1].decode().strip()
            test_env = {**env, "CGO_ENABLED": "1" if args.race else "0"}
            command = ["/usr/bin/go", "test", "-json", "-mod=readonly", "-tags=p4_cookie_transport,p4_cookie_overlay", "-overlay", str(overlay), f"-count={args.count}", "-timeout=60s"]
            if args.race: command.append("-race")
            code, stdout, stderr = base.bounded_command([*command, "-run", "^TestP4Cookie(Underload|TransportHooks)$", "./device"], exported, test_env, 120)
            if code != 0 or stderr: raise ValueError("actual worker/hook tests refused")
            expected = ["TestP4CookieUnderload", "TestP4CookieTransportHooks", *(f"TestP4CookieUnderload/trailers={trailers}/disable={disable}" for trailers in ("false", "true") for disable in ("false", "true")), *(f"TestP4CookieTransportHooks/trailers={trailers}" for trailers in ("false", "true"))]
            verify_tests(stdout, "github.com/amnezia-vpn/amneziawg-go/v3/device", expected, args.count)
            code, stdout, stderr = base.bounded_command([*command, "./p4-peer"], exported, test_env, 120)
            if code != 0 or stderr: raise ValueError("tagged wrapper pure tests refused")
            verify_tests(stdout, "github.com/amnezia-vpn/amneziawg-go/v3/p4-peer", ["TestByteCountIsNotPacketSliceBound", "TestFailedWriteDoesNotClaimPaddingEvidence", "TestPaddingMatchesAreBoundedByBothObservations", "TestCookieModeInput"], args.count)
            code, _, stderr = base.bounded_command(["/usr/bin/go", "build", "-trimpath", "-mod=readonly", "-buildvcs=false", "-tags=p4_cookie_transport", "-overlay", str(overlay), "-o", str(output), "./p4-peer"], exported, env, 120)
            if code != 0 or stderr: raise ValueError("tagged developer peer build refused")
            receipt["binary_sha256"] = hashlib.sha256(output.read_bytes()).hexdigest()
            code, stdout, stderr = base.bounded_command(["/usr/bin/go", "version", "-m", str(output)], exported, env, 10)
            if code != 0 or stderr: raise ValueError("binary attestation refused")
            receipt["binary_attestation"] = stdout.decode().splitlines()[1:]
            # The instrumented binary is deliberately a main module from the
            # exact exported engine, not falsely a cached versioned dependency.
            overlay.unlink()
            normal = root / "normal-peer"
            code, _, stderr = base.bounded_command(["/usr/bin/go", "build", "-trimpath", "-mod=readonly", "-buildvcs=false", "-o", str(normal), "./p4-peer"], exported, env, 120)
            if code != 0 or stderr: raise ValueError("normal untagged peer build refused")
            code, stdout, stderr = base.bounded_command(["/usr/bin/go", "tool", "nm", str(normal)], exported, env, 10)
            if code != 0 or stderr or b"P4Fixture" in stdout or b"cookieObserver" in stdout: raise ValueError("tagged hooks leaked into normal peer")
            receipt["normal_peer_has_no_tagged_hooks"] = True
            base.verify_export(exported, proof)
            for name, payload in overlay_files.items():
                if (root / name).read_bytes() != payload: raise ValueError("overlay bytes changed")
        if base.verify_source(source) != archive: raise ValueError("source export changed")
        if any((fixture / name).read_bytes() != payload for name, payload in snapshots.items()) or any((fixture / "upstream-tests" / name).read_bytes() != payload for name, payload in overlay_files.items()): raise ValueError("fixture bytes changed")
        if any(Path(name).read_bytes() != payload for name, payload in tools.items()): raise ValueError("builder/exporter changed")
        receipt.update(source_unchanged=True, export_overlay_removed=True, engine_subcases=args.count*6, wrapper_tests=args.count*4)
        path = output.with_suffix(".receipt.json")
        with path.open("x") as file:
            os.chmod(path, 0o600); json.dump(receipt, file, sort_keys=True)
        success = True
        print(json.dumps({"status":"PASS", "binary_sha256":receipt["binary_sha256"], "upstream_sha":base.PIN, "engine_subcases":args.count*6, "wrapper_tests":args.count*4, "race":args.race, "normal_peer_has_no_tagged_hooks":True, "export_overlay_removed":True}, sort_keys=True))
    finally:
        base.verify_source(source)
        if not success and output.is_file(): output.unlink() # Exact new owned public artifact only, never an installed binary.


if __name__ == "__main__":
    try: main()
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        raise SystemExit(f"REFUSE cookie-peer-build: {type(exc).__name__}") from None
