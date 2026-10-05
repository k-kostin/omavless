#!/usr/bin/env python3
"""Proposed separate offline build/one-shot execute for real-default residue.

Developer-only channel engine experiment. No native/VM invocation is authorized
by adding this source. ROOT and independent FULL recipe review are prerequisites.
"""
import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import resource
import shutil
import stat
import subprocess
import tempfile
import types

HERE = Path(__file__).resolve().parent
SUPERVISOR = HERE / "run_default_rekey_supervisor.py"
SUPERVISOR_SHA = "00fa64cacdf72d65fcdd772208eca1bc4ca40ca954ef94444ff11eec65b729ec"
HELPER_SHA = "55fa8597b183a5b8deb0b0aabf25740ee12dfc221054e04e62ac017d1b86cc6e"
RECEIPT_SHA = "e47cf22a9886c1a60577f91b23fd58e9c27f649e27c4c22b3d08bc5a0fb63906"
RECEIPT = HERE / "default_residue_receipt.py"
OVERLAYS = {
    "default_elapsed_rekey_test.go": ("6722869be1b098603966eb0df564579789146a15c1340bc121e4ac96c5d2cb6f", "support.go"),
    "default_residue_support_test.go": ("7ad53ca96f4b98ae6cd03c37f50e8673ba9735a8e46e44263833f1f5bd2315e3", "residue-support.go"),
    "default_residue_cases_test.go": ("1a7b81da0684da3506914f3941c27df5173d89e004fa172d81ba366b6097dfae", "residue-cases.go"),
}

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


def pinned(path, digest):
    raw = object_bytes(path, 65536)
    if hashlib.sha256(raw).hexdigest() != digest:
        raise ValueError("sealed_source_changed")
    return raw


def definitions(raw, filename):
    """Use exact pinned supervisor bodies, not its unpinned path-reload prefix.

    No old source is patched. All five command/save/directory functions and the
    ownership class/containers are selected verbatim from its complete attested
    source. Bind only an independently attested helper module before use.
    """
    if type(raw) is not bytes or hashlib.sha256(raw).hexdigest() != SUPERVISOR_SHA:
        raise ValueError("fixed_supervisor_source_required")
    selected = []
    functions = set()
    classes = set()
    containers = set()
    wanted = {'members', 'reap', 'command', 'private_directory', 'save'}
    for node in ast.parse(raw).body:
        if isinstance(node, (ast.Import, ast.ImportFrom)):
            selected.append(node)
        elif isinstance(node, ast.FunctionDef) and node.name in wanted:
            functions.add(node.name)
            selected.append(node)
        elif isinstance(node, ast.ClassDef) and node.name == 'Unsettled':
            classes.add(node.name)
            selected.append(node)
        elif isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name):
            name = node.targets[0].id
            if name in ('UNSETTLED', 'HELD_GRAPHS'):
                containers.add(name)
                selected.append(node)
    if functions != wanted or classes != {'Unsettled'} or containers != {'UNSETTLED', 'HELD_GRAPHS'}:
        raise ValueError("fixed_supervisor_definitions_required")
    return compile(ast.Module(body=selected, type_ignores=[]), filename, 'exec')


SUPERVISOR_BYTES = pinned(SUPERVISOR, SUPERVISOR_SHA)
HELPER_BYTES = pinned(HERE / 'run_cookie_overlay.py', HELPER_SHA)
RECEIPT_BYTES = pinned(RECEIPT, RECEIPT_SHA)
helpers = types.ModuleType('p4_residue_export')
exec(compile(HELPER_BYTES, 'fixed-p4-export-helper', 'exec'), helpers.__dict__)
owned = types.ModuleType('p4_residue_owned_commands')
exec(definitions(SUPERVISOR_BYTES, 'fixed-p4-supervisor'), owned.__dict__)
owned.helpers, owned.HELPER_BYTES = helpers, HELPER_BYTES
receipts = types.ModuleType('p4_residue_receipts')
exec(compile(RECEIPT_BYTES, 'fixed-p4-residue-parser', 'exec'), receipts.__dict__)
PACKAGE = receipts.PACKAGE


def inputs():
    value = {"runner.py": Path(__file__).read_bytes(), "supervisor.py": SUPERVISOR.read_bytes(),
             "export-helper.py": (HERE / "run_cookie_overlay.py").read_bytes(),
             "receipt-parser.py": RECEIPT.read_bytes()}
    for name, (digest, artifact) in OVERLAYS.items():
        raw = (HERE / "upstream-tests" / name).read_bytes()
        if hashlib.sha256(raw).hexdigest() != digest:
            raise ValueError("sealed_overlay_changed")
        value[artifact] = raw
    return value


def limits():
    # Proposed caller/child resource envelope. No address-space or RSS claim:
    # Linux RSS rlimit is not an enforced heap cap and race reserves large VA.
    # Only this new runner process and its future children are constrained.
    for kind, bound in ((resource.RLIMIT_NOFILE,512), (resource.RLIMIT_NPROC,1024),
                        (resource.RLIMIT_CPU,660), (resource.RLIMIT_FSIZE,256*1024*1024),
                        (resource.RLIMIT_CORE,0)):
        soft, hard = resource.getrlimit(kind)
        requested = bound if soft == resource.RLIM_INFINITY else min(soft,bound)
        resource.setrlimit(kind,(requested,hard))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase", required=True, choices=("build", "execute"))
    for name in ("source", "scratch", "cache", "module-cache", "artifacts"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--race", action="store_true")
    parser.add_argument("--case", required=True, choices=tuple(receipts.SELECTORS))
    args = parser.parse_args()
    limits()
    test = receipts.SELECTORS[args.case]
    home = Path.home().resolve()
    source = args.source.resolve(strict=True)
    modules = owned.private_directory(args.module_cache,home)
    for path in (source, modules):
        if path == home or not path.is_relative_to(home) or not path.is_dir() or path.stat().st_uid != os.getuid() or path.stat().st_mode & 0o022:
            raise ValueError("unsafe_owned_source_or_modules")
    scratch, cache, artifacts = [owned.private_directory(p, home) for p in (args.scratch, args.cache, args.artifacts)]
    captured = inputs()
    if (captured["supervisor.py"] != SUPERVISOR_BYTES or captured["export-helper.py"] != owned.HELPER_BYTES
            or captured["receipt-parser.py"] != RECEIPT_BYTES):
        raise ValueError("sealed_helpers_changed")
    fixture = owned.helpers.git(HERE, "rev-parse", "HEAD").decode().strip()
    paths = [str(Path(__file__)), str(SUPERVISOR), str(HERE / "run_cookie_overlay.py"), str(RECEIPT),
             *(str(HERE / "upstream-tests" / name) for name in OVERLAYS)]
    owned.helpers.git(HERE, "ls-files", "--error-unmatch", "--", *paths)
    owned.helpers.git(HERE, "diff", "--exit-code", "HEAD", "--", *paths)
    archive = owned.helpers.verify_source(source)
    env = dict(PATH="/usr/bin:/bin", HOME=str(home), LANG="C", LC_ALL="C", GOENV="off", GOTOOLCHAIN="local", GOWORK="off", GOFLAGS="", GOPROXY="off", GOSUMDB="off", GOOS="linux", GOARCH="amd64", GOMAXPROCS="2", CGO_ENABLED="1" if args.race else "0", CC="/usr/bin/gcc", CXX="/usr/bin/g++", GOMODCACHE=str(modules), GOCACHE=str(cache), GOTMPDIR=str(scratch), TMPDIR=str(scratch))
    hashes = {name: hashlib.sha256(data).hexdigest() for name, data in captured.items()}
    binary = artifacts / "device.test"
    tool_hash = hashlib.sha256(Path("/usr/bin/go").read_bytes()).hexdigest()
    expected = dict(fixture_commit=fixture, pin=owned.helpers.PIN, source_archive_sha256=owned.helpers.ARCHIVE_SHA256,
                    input_sha256=hashes, tool_sha256=tool_hash, race=args.race, cases=1,
                    source_engine_only=True, network_transport=False, vm=False, defaults_unchanged=True,
                    test=test, case=args.case, body_limit_seconds=570,
                    go_timeout_seconds=600, supervisor_timeout_seconds=620,
                    resource_envelope={'nofile':512,'nproc':1024,'cpu_seconds':660,
                                       'file_bytes':256*1024*1024,'core_bytes':0},
                    intrinsic_engine_heap_cap=False)
    if args.phase == "build":
        if any(artifacts.iterdir()):
            raise ValueError("fresh_build_artifacts_required")
        for name, data in captured.items():
            owned.save(artifacts, name, data)
        root = Path(tempfile.mkdtemp(prefix="p4-default-residue-", dir=scratch))
        identity = root.stat().st_dev, root.stat().st_ino
        exported = root / "source"
        exported.mkdir(mode=0o700)
        try:
            proof = owned.helpers.export_source(archive, exported)
            replacements = {}
            for name, (_, artifact) in OVERLAYS.items():
                target = exported / "device" / name
                if target.exists():
                    raise ValueError("add_only_overlay_required")
                owned.save(root, artifact, captured[artifact])
                replacements[str(target)] = str(root / artifact)
            owned.save(root, "overlay.json", json.dumps({"Replace": replacements}).encode())
            code, out, err = owned.command(["/usr/bin/go", "mod", "verify"], exported, env, 30)
            owned.save(artifacts, "modules.stdout", out)
            owned.save(artifacts, "modules.stderr", err)
            if code != 0 or out.strip() != b"all modules verified" or err:
                raise ValueError("offline_module_verification_refused")
            build = ["/usr/bin/go", "test", "-c", "-p=2", "-trimpath", "-mod=readonly", "-tags=p4_cookie_overlay,p4_default_residue_overlay", "-overlay", str(root / "overlay.json"), "-o", str(binary)]
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
            if any((root / artifact).read_bytes() != captured[artifact] for _, artifact in OVERLAYS.values()):
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
            code, out, err = owned.command(["/usr/bin/go", "tool", "test2json", "-p", PACKAGE, "-t", str(binary), "-test.v=test2json", f"-test.run=^{test}$", "-test.count=1", "-test.timeout=600s"], artifacts, env, 620)
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
            raise ValueError("actual_default_residue_nonpass_preserve")
        observation = receipts.verify_events(out, args.case)
        if hashlib.sha256(object_bytes(binary, 256 * 1024 * 1024)).hexdigest() != expected["binary_sha256"]:
            raise ValueError("executed_binary_changed")
        owned.helpers.verify_source(source)
        owned.save(artifacts, "execution-receipt.json", json.dumps({**expected, "execution": True, "observation": observation}, sort_keys=True).encode())
        result = "PASS_CPU_SOURCE_ENGINE_ONLY"
    if inputs() != captured or owned.helpers.git(HERE, "rev-parse", "HEAD").decode().strip() != fixture or hashlib.sha256(Path("/usr/bin/go").read_bytes()).hexdigest() != tool_hash:
        raise ValueError("source_or_tool_drift")
    print(result + " " + json.dumps(expected, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
        raise SystemExit("REFUSE default-residue-overlay: " + type(exc).__name__) from None
