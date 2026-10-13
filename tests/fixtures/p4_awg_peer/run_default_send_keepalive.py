#!/usr/bin/env python3
"""One fixed ordinary default send-keepalive SOURCE successor.

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
import stat
import subprocess
import tempfile
import types

BASE_RUNNER_SHA = "63fd931260d3baa45e66ea7945b0c5f28157a688518b0c55f40fff736fa6f6f6"

HERE = Path(__file__).resolve().parent
SUPERVISOR = HERE / "run_default_rekey_supervisor.py"
SUPERVISOR_SHA = "00fa64cacdf72d65fcdd772208eca1bc4ca40ca954ef94444ff11eec65b729ec"
HELPER_SHA = "55fa8597b183a5b8deb0b0aabf25740ee12dfc221054e04e62ac017d1b86cc6e"
RECEIPT_SHA = "5732b5f7029abcb589df583a8d44e21bb73310b924d8b132d8b4e936014cf431"
RECEIPT = HERE / "default_keepalive_receipt.py"
ADMISSION = HERE / "ordinary_toolchain_admission.py"
FORMAT_ADAPTER = HERE / "go127_residue_format.py"
OVERLAYS = {
    "default_elapsed_rekey_test.go": ("6722869be1b098603966eb0df564579789146a15c1340bc121e4ac96c5d2cb6f", "support.go"),
    "default_residue_support_test.go": ("7ad53ca96f4b98ae6cd03c37f50e8673ba9735a8e46e44263833f1f5bd2315e3", "residue-support.go"),
    "default_residue_cases_test.go": ("1a7b81da0684da3506914f3941c27df5173d89e004fa172d81ba366b6097dfae", "residue-cases.go"),
    "default_send_keepalive_test.go": ("ca46cf2e82ce9281588f0dae713220108454bfd63a3e7913719d3a7f353e1a6a", "keepalive.go"),
}

def object_bytes(path, maximum, allow_empty=False):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        before = os.fstat(fd)
        if not stat.S_ISREG(before.st_mode) or before.st_uid != os.getuid() or before.st_mode & 0o7022 or before.st_nlink != 1 or not (0 if allow_empty else 1) <= before.st_size <= maximum or os.listxattr(fd):
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
admission = types.ModuleType("p4_ordinary_toolchain")
ADMISSION_BYTES = object_bytes(ADMISSION, 65536)
exec(compile(ADMISSION_BYTES, "fixed-p4-ordinary-toolchain", "exec"), admission.__dict__)
formats = types.ModuleType("p4_go127_residue_format")
FORMAT_BYTES = object_bytes(FORMAT_ADAPTER, 65536)
exec(compile(FORMAT_BYTES, "fixed-p4-go127-residue-format", "exec"), formats.__dict__)


def inputs():
    value = {"runner.py": Path(__file__).read_bytes(), "supervisor.py": SUPERVISOR.read_bytes(),
             "export-helper.py": (HERE / "run_cookie_overlay.py").read_bytes(),
             "receipt-parser.py": RECEIPT.read_bytes(),
             "toolchain-admission.py": ADMISSION.read_bytes(),
             "go127-format-adapter.py": FORMAT_ADAPTER.read_bytes(),
             "baseline-runner.py": (HERE / "run_default_residue_ordinary.py").read_bytes()}
    for name, (digest, artifact) in OVERLAYS.items():
        raw = (HERE / "upstream-tests" / name).read_bytes()
        if hashlib.sha256(raw).hexdigest() != digest:
            raise ValueError("sealed_overlay_changed")
        value[artifact] = raw
    return value


def verify_telemetry(artifacts, home):
    # Both explicit tool override and default XDG resolution name this one
    # private OFF mode. Never inspect/mutate the user's real Go telemetry state.
    telemetry = artifacts / "configuration" / "go" / "telemetry"
    if owned.private_directory(telemetry, home) != telemetry:
        raise ValueError("fixed_private_telemetry_required")
    names = set(p.name for p in telemetry.iterdir())
    if "mode" not in names or not names <= {"mode", "local", "upload"}:
        raise ValueError("fixed_private_telemetry_members_required")
    for name in names - {"mode"}:
        directory = telemetry / name
        if owned.private_directory(directory, home) != directory or any(directory.iterdir()):
            raise ValueError("telemetry_counter_data_refused")
    if object_bytes(telemetry / "mode", 32) != b"off\n" or stat.S_IMODE((telemetry / "mode").lstat().st_mode) != 0o600:
        raise ValueError("fixed_telemetry_off_required")


def frozen_elf(path):
    raw = object_bytes(path, 256 * 1024 * 1024)
    if stat.S_IMODE(path.lstat().st_mode) != 0o700:
        raise ValueError("private_frozen_elf_mode_required")
    admission.static_elf(raw)
    return hashlib.sha256(raw).hexdigest()


def record_execution_completion(artifacts, expected, code, out, err):
    # Called only after the original supervisor returned its complete settled
    # captures/raw status. This is finite historical evidence, never permission
    # to retry, cleanup, bypass parsing or treat a later gate as accepted.
    if owned.UNSETTLED:
        raise owned.Unsettled("prior_owned_graph_unknown_preserve")
    if type(code) is not int or not -255 <= code <= 255 or type(out) is not bytes or type(err) is not bytes or len(out) + len(err) > 2 * 1024 * 1024:
        raise ValueError("finite_original_command_result_required")
    raw = json.dumps({
        "schema": "p4-original-execution-command-result-v1",
        "execution_identity": expected, "supervisor_returned_complete": True,
        "raw_exit_code": code, "stdout_bytes": len(out), "stderr_bytes": len(err),
        "stdout_sha256": hashlib.sha256(out).hexdigest(),
        "stderr_sha256": hashlib.sha256(err).hexdigest(),
        "engine_gate_verified": False, "retry_or_cleanup_authority": False}, sort_keys=True).encode()
    owned.save(artifacts, "execution-command-result.json", raw)
    return hashlib.sha256(raw).hexdigest()


def recheck(captured, fixture, toolchain, source, artifacts, home):
    # First unknown latch prohibits even later source/query effects.
    if owned.UNSETTLED:
        raise owned.Unsettled("prior_owned_graph_unknown_preserve")
    if inputs() != captured:
        raise ValueError("source_input_drift")
    if owned.helpers.git(HERE, "rev-parse", "HEAD").decode().strip() != fixture:
        raise ValueError("fixture_commit_drift")
    if admission.admit() != toolchain:
        raise ValueError("toolchain_drift")
    owned.helpers.verify_source(source)
    verify_telemetry(artifacts, home)


def limits():
    # Proposed caller/child resource envelope. No address-space or RSS claim:
    # Linux RSS rlimit is not an enforced heap cap and race reserves large VA.
    # Only this new runner process and its future children are constrained.
    for kind, bound in ((resource.RLIMIT_NOFILE,512), (resource.RLIMIT_NPROC,2048),
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
    parser.add_argument("--case", required=True, choices=tuple(receipts.SELECTORS))
    args = parser.parse_args()
    args.race = False  # Ordinary CGO0 only; no caller-selected compiler mode.
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
            or captured["receipt-parser.py"] != RECEIPT_BYTES
            or captured["toolchain-admission.py"] != ADMISSION_BYTES
            or captured["go127-format-adapter.py"] != FORMAT_BYTES
            or hashlib.sha256(captured["baseline-runner.py"]).hexdigest() != BASE_RUNNER_SHA):
        raise ValueError("sealed_helpers_changed")
    fixture = owned.helpers.git(HERE, "rev-parse", "HEAD").decode().strip()
    paths = [str(Path(__file__)), str(SUPERVISOR), str(HERE / "run_cookie_overlay.py"), str(RECEIPT),
             *(str(HERE / "upstream-tests" / name) for name in OVERLAYS),
             str(ADMISSION), str(FORMAT_ADAPTER), str(HERE / "run_default_residue_ordinary.py")]
    owned.helpers.git(HERE, "ls-files", "--error-unmatch", "--", *paths)
    owned.helpers.git(HERE, "diff", "--exit-code", "HEAD", "--", *paths)
    archive = owned.helpers.verify_source(source)
    telemetry = artifacts / "configuration" / "go" / "telemetry"
    env = dict(PATH="/usr/bin:/bin", HOME=str(home), LANG="C", LC_ALL="C", GOENV="off", GOTOOLCHAIN="local", GOROOT="/usr/lib/go", GOWORK="off", GOFLAGS="", GOPROXY="off", GOSUMDB="off", GOOS="linux", GOARCH="amd64", GOMAXPROCS="2", CGO_ENABLED="0", GOMODCACHE=str(modules), GOCACHE=str(cache), GOTMPDIR=str(scratch), TMPDIR=str(scratch), XDG_CONFIG_HOME=str(artifacts / "configuration"), XDG_CACHE_HOME=str(cache), TEST_TELEMETRY_DIR=str(telemetry))
    hashes = {name: hashlib.sha256(data).hexdigest() for name, data in captured.items()}
    binary = artifacts / "device.test"
    test2json = artifacts / "test2json"
    toolchain = admission.admit()
    tool_hash = toolchain["go_sha256"]
    expected = dict(fixture_commit=fixture, pin=owned.helpers.PIN, source_archive_sha256=owned.helpers.ARCHIVE_SHA256,
                    input_sha256=hashes, tool_sha256=tool_hash, toolchain=toolchain,
                    recipe="p4-default-send-keepalive-ordinary-arch-go-v1", race=False, cases=1,
                    output_format=formats.FORMAT,
                    test2json_build_package="cmd/test2json", telemetry_mode="off",
                    telemetry_mode_sha256=hashlib.sha256(b"off\n").hexdigest(),
                    xdg_configuration_relative="configuration",
                    telemetry_directory_relative="configuration/go/telemetry",
                    source_engine_only=True, network_transport=False, vm=False, defaults_unchanged=True,
                    test=test, case=args.case, body_limit_seconds=30,
                    go_timeout_seconds=40, supervisor_timeout_seconds=50,
                    resource_envelope={'nofile':512,'nproc':2048,'cpu_seconds':660,
                                       'file_bytes':256*1024*1024,'core_bytes':0},
                    intrinsic_engine_heap_cap=False, temporary_export_retained=True)
    if args.phase == "build":
        if any(artifacts.iterdir()):
            raise ValueError("fresh_build_artifacts_required")
        for directory in (artifacts / "configuration", artifacts / "configuration" / "go", telemetry):
            directory.mkdir(mode=0o700)
        owned.save(telemetry, "mode", b"off\n")
        for name, data in captured.items():
            owned.save(artifacts, name, data)
        root = Path(tempfile.mkdtemp(prefix="p4-default-send-keepalive-", dir=scratch))
        exported = root / "source"
        exported.mkdir(mode=0o700)
        proof = owned.helpers.export_source(archive, exported)
        replacements = {}
        for name, (_, artifact) in OVERLAYS.items():
            target = exported / "device" / name
            if target.exists():
                raise ValueError("add_only_overlay_required")
            owned.save(root, artifact, captured[artifact])
            replacements[str(target)] = str(root / artifact)
        owned.save(root, "overlay.json", json.dumps({"Replace": replacements}).encode())
        recheck(captured, fixture, toolchain, source, artifacts, home)
        code, out, err = owned.command(["/usr/bin/go", "mod", "verify"], exported, env, 30)
        owned.save(artifacts, "modules.stdout", out)
        owned.save(artifacts, "modules.stderr", err)
        if code != 0 or out.strip() != b"all modules verified" or err:
            raise ValueError("offline_module_verification_refused")
        # Explicit zero-case compiler phase. EXECUTE never uses `go tool` and
        # cannot lazily compile/regenerate a missing builtin tool.
        recheck(captured, fixture, toolchain, source, artifacts, home)
        code, out, err = owned.command(["/usr/bin/go", "build", "-p=2", "-trimpath", "-o", str(test2json), "cmd/test2json"], exported, env, 120)
        owned.save(artifacts, "test2json-build.stdout", out)
        owned.save(artifacts, "test2json-build.stderr", err)
        if code != 0 or out or err:
            raise ValueError("frozen_test2json_compile_refused_no_execution")
        os.chmod(test2json, 0o700)
        expected["test2json_sha256"] = frozen_elf(test2json)
        build = ["/usr/bin/go", "test", "-c", "-p=2", "-trimpath", "-mod=readonly", "-tags=p4_cookie_overlay,p4_default_residue_overlay,p4_default_keepalive_overlay", "-overlay", str(root / "overlay.json"), "-o", str(binary)]
        recheck(captured, fixture, toolchain, source, artifacts, home)
        code, out, err = owned.command([*build, "./device"], exported, env, 120)
        owned.save(artifacts, "build.stdout", out)
        owned.save(artifacts, "build.stderr", err)
        if code != 0 or out or err:
            raise ValueError("frozen_compile_refused_no_execution")
        os.chmod(binary, 0o700)
        expected["binary_sha256"] = frozen_elf(binary)
        if frozen_elf(test2json) != expected["test2json_sha256"]:
            raise ValueError("compiled_test2json_changed")
        owned.helpers.verify_export(exported, proof)
        if any((root / artifact).read_bytes() != captured[artifact] for _, artifact in OVERLAYS.values()):
            raise ValueError("compiled_overlay_changed")
        recheck(captured, fixture, toolchain, source, artifacts, home)
        # No cleanup path: preserve the exact exported source/overlay on every
        # positive, nonzero, drift, refusal or unknown constructor outcome.
        receipt_name = "build-receipt.json"
        receipt_bytes = json.dumps({**expected, "execution": False}, sort_keys=True).encode()
        result = "BUILT_NO_ENGINE_EXECUTION"
    else:
        for name, data in captured.items():
            if object_bytes(artifacts / name, 65536) != data:
                raise ValueError("frozen_inputs_differ")
        build_receipt = json.loads(object_bytes(artifacts / "build-receipt.json", 16384))
        expected["binary_sha256"] = frozen_elf(binary)
        expected["test2json_sha256"] = frozen_elf(test2json)
        if build_receipt != {**expected, "execution": False} or any(os.path.lexists(artifacts / name) for name in ("execution-attempt.json", "events.jsonl", "test.stderr", "execution-command-result.json", "execution-receipt.json")):
            raise ValueError("exact_unexecuted_build_receipt_required")
        recheck(captured, fixture, toolchain, source, artifacts, home)
        if frozen_elf(test2json) != expected["test2json_sha256"] or frozen_elf(binary) != expected["binary_sha256"]:
            raise ValueError("preexecution_frozen_elf_drift")
        # Consumes this artifact's one execution slot before child creation.
        # Timeout/unknown supervision cannot leave it apparently unattempted.
        owned.save(artifacts, "execution-attempt.json", json.dumps({**expected, "attempted": True}, sort_keys=True).encode())
        try:
            code, out, err = owned.command([str(test2json), "-p", PACKAGE, "-t", str(binary), "-test.v=test2json", f"-test.run=^{test}$", "-test.count=1", "-test.timeout=40s"], artifacts, env, 50)
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
        command_result_hash = record_execution_completion(artifacts, expected, code, out, err)
        if code != 0 or err:
            raise ValueError("actual_default_keepalive_nonpass_preserve")
        observation = receipts.verify_events(formats.normalize(out), args.case)
        if frozen_elf(binary) != expected["binary_sha256"] or frozen_elf(test2json) != expected["test2json_sha256"]:
            raise ValueError("executed_binary_changed")
        recheck(captured, fixture, toolchain, source, artifacts, home)
        if object_bytes(artifacts / "events.jsonl", 2 * 1024 * 1024) != out or object_bytes(artifacts / "test.stderr", 2 * 1024 * 1024, allow_empty=True) != err or hashlib.sha256(object_bytes(artifacts / "execution-command-result.json", 16384)).hexdigest() != command_result_hash:
            raise ValueError("retained_complete_capture_or_result_drift")
        receipt_name = "execution-receipt.json"
        receipt_bytes = json.dumps({**expected, "execution": True, "observation": observation,
                                    "command_result_sha256": command_result_hash,
                                    "raw_events_sha256": hashlib.sha256(out).hexdigest()}, sort_keys=True).encode()
        result = "PASS_CPU_SOURCE_ENGINE_ONLY"
    # A failed final output is NOT a receipt-bearing positive checkpoint.
    # Neither this line nor a saved receipt alone authorizes another command:
    # ROOT must separately verify the complete CLI's known-zero terminal.
    print("PROVISIONAL_" + result + " " + json.dumps(expected, sort_keys=True), flush=True)
    owned.save(artifacts, receipt_name, receipt_bytes)  # Final authorizing action.


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
        raise SystemExit("REFUSE default-send-keepalive: " + type(exc).__name__) from None
