#!/usr/bin/env python3
"""Offline review build of the exact experimental broker/core source pair.

No downloads, installation, privilege, or runtime activation. The output is a
private candidate directory, not an enrollment decision or release artifact.
"""
import argparse
import contextlib
import functools
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import sys
import subprocess
import tarfile
import tempfile

import stage


REPO = Path(__file__).resolve().parents[3]
PATCHES = REPO / "tests/core_dns_adapter"
MIHOMO = "ab405bad5beeeac8b003bb01f60f134f6df54471"
SING_TUN = "b50ae28a1409c7bce8e96e6c6966cf57d8ace754"  # v0.4.24
PATCH_SHA = {
    "mihomo-dns-broker.patch": "d5ebe9d6b37f6b76599fc3c2dd25adbfb774ca0121beeb79c5768a9a08d7ff37",
    "sing-tun-descriptor.patch": "2556c82aafbeb598a817d43042cf2069c6f209433c7a506395df581b4e31e2ab",
}
MAX_ARCHIVE = 128 * 1024 * 1024
GO_ARCH = {"x86_64": "amd64", "aarch64": "arm64"}


def qualified_tmpdir(environ=None):
    env = os.environ if environ is None else environ
    home = Path(env.get("HOME", ""))
    target = Path(env.get("TMPDIR", ""))
    # Caller supplies an already-private short HOME child. Never create,
    # remove, chmod or adopt the caller's directory; compiler/test children
    # have ordinary owned-temporary semantics. No long Unix-socket fallback.
    if (not home.is_absolute() or not target.is_absolute() or target.parent != home
            or target.name in ("", ".", "..") or len(os.fsencode(target)) > 32):
        raise stage.Refused("Qualified build needs a short private HOME TMPDIR.")
    hm, tm = home.lstat(), target.lstat()
    if (not stat.S_ISDIR(hm.st_mode) or hm.st_uid != os.getuid() or hm.st_mode & 0o7022
            or not stat.S_ISDIR(tm.st_mode) or tm.st_uid != os.getuid()
            or stat.S_IMODE(tm.st_mode) != 0o700):
        raise stage.Refused("Qualified compiler TMPDIR is unsafe.")
    return str(target)
CONDITIONAL_TESTS = {
    "github.com/metacubex/mihomo/hub/route": {
        "TestConditionalCloseExplicitReadiness", "TestConditionalCloseStrictRequest"},
    "github.com/metacubex/mihomo/tunnel/statistic": {
        "TestConditionalCloseReusedID", "TestConditionalCloseCannotReenroll",
        "TestConditionalCloseDelayedLeave", "TestConditionalCloseConcurrentConfirm",
        "TestConditionalCloseExhaustionAndFailure"},
}


def conditional_tests_completed(raw):
    if not raw or len(raw) > 1024 * 1024 or not raw.endswith(b"\n"):
        raise stage.Refused("Conditional test receipt is incomplete.")
    runs, passed, packages, started = set(), set(), set(), set()
    for line in raw.splitlines():
        try:
            event = json.loads(line, object_pairs_hook=stage.no_duplicate_keys)
        except (ValueError, UnicodeError) as error:
            raise stage.Refused("Conditional test receipt is invalid.") from error
        if not isinstance(event, dict) or event.get("Package") not in CONDITIONAL_TESTS:
            raise stage.Refused("Conditional test package is unexpected.")
        action, package, test = event.get("Action"), event["Package"], event.get("Test")
        if package in packages:
            raise stage.Refused("Conditional output followed package completion.")
        if action not in ("start", "run", "output", "pass"):
            raise stage.Refused("Conditional tests failed or skipped.")
        if test is not None and test not in CONDITIONAL_TESTS[package]:
            raise stage.Refused("Conditional test case is unexpected.")
        key = (package, test)
        if action == "start":
            if test is not None or package in started:
                raise stage.Refused("Conditional package start is invalid.")
            started.add(package)
        elif package not in started:
            raise stage.Refused("Conditional package did not start.")
        elif action == "run":
            if test is None or key in runs:
                raise stage.Refused("Conditional test invocation is invalid.")
            runs.add(key)
        elif action == "pass":
            target, member = (passed, key) if test is not None else (packages, package)
            if member in target or (test is not None and key not in runs):
                raise stage.Refused("Conditional test completion is invalid.")
            if test is None and {name for pkg, name in passed if pkg == package} != CONDITIONAL_TESTS[package]:
                raise stage.Refused("Conditional package passed before its cases.")
            target.add(member)
    expected = {(package, name) for package, names in CONDITIONAL_TESTS.items() for name in names}
    if runs != expected or passed != expected or packages != set(CONDITIONAL_TESTS):
        raise stage.Refused("Every conditional primitive must actually pass.")


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def source_phase(arguments):
    if "archive" in arguments: return "source-export"
    if "apply" in arguments: return "patch-check" if "--check" in arguments else "patch-apply"
    if "mod" in arguments:
        return "module-verify" if "verify" in arguments else "module-vendor" if "vendor" in arguments else "module-edit"
    if "test" in arguments: return "conditional-tests" if "^TestConditionalClose" in arguments else "dns-tests"
    if "build" in arguments: return "broker-build" if arguments[0] == "/usr/bin/cargo" else "core-build"
    return "source-step"


def public_failure(arguments, stderr, stdout=b"", timeout=False):
    print("qualified_build_failed phase="+source_phase(arguments)+(" timeout" if timeout else ""),file=sys.stderr)
    budget=16384
    for label,raw in (("stderr",stderr or b""),("stdout",stdout or b"")):
        part=raw[:budget];budget-=len(part)
        text=part.decode("utf-8",errors="replace")
        text="".join(c if c in "\n\t" or c.isprintable() else "?" for c in text)
        if text:
            print("qualified_source_"+label,file=sys.stderr);sys.stderr.write(text)
            if not text.endswith("\n"):sys.stderr.write("\n")
        if len(raw)>len(part):print("qualified_source_diagnostic_truncated",file=sys.stderr)


def command(arguments, *, cwd=None, env=None, output=None, public_diagnostics=False):
    sink = output.open("wb") if output else subprocess.PIPE if public_diagnostics else subprocess.DEVNULL
    try:
        result = subprocess.run(arguments, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                stdout=sink, stderr=subprocess.PIPE, timeout=600,
                                check=False)
    except subprocess.TimeoutExpired as error:
        if public_diagnostics:public_failure(arguments,error.stderr,error.stdout,True)
        raise
    finally:
        if output:
            sink.close()
    if result.returncode != 0 or len(result.stderr) > 2 * 1024 * 1024:
        if public_diagnostics:
            # Only this closed offline SOURCE build supplies the flag. Never
            # private runtime/config/profile logs. Projection is16KiB, not a
            # hard bound on PIPE/kernel/compiler allocations or syscall time.
            public_failure(arguments,result.stderr,result.stdout if output is None else b"")
        raise stage.Refused("Offline reviewed build step failed.")
    return result.stderr


def git_value(repository, *arguments):
    result = subprocess.run(["/usr/bin/git", "-C", str(repository), *arguments],
                            stdin=subprocess.DEVNULL, capture_output=True, timeout=20,
                            env={"PATH": "/usr/bin:/bin", "LC_ALL": "C",
                                 "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"},
                            check=False)
    if result.returncode != 0 or len(result.stdout) > 4096:
        raise stage.Refused("Pinned source identity is unavailable.")
    return result.stdout.decode("ascii").strip()


def export_git(repository, revision, destination, public_diagnostics=False):
    if git_value(repository, "rev-parse", "--verify", f"{revision}^{{commit}}") != revision:
        raise stage.Refused("Pinned upstream commit is absent.")
    archive = destination.parent / (destination.name + ".tar")
    git_env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C",
               "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"}
    command(["/usr/bin/git", "-C", str(repository), "archive", "--format=tar",
             revision], env=git_env, output=archive, public_diagnostics=public_diagnostics)
    if archive.stat().st_size > MAX_ARCHIVE:
        raise stage.Refused("Pinned source archive exceeds its bound.")
    destination.mkdir()
    with tarfile.open(archive, "r:") as stream:
        stream.extractall(destination, filter="data")
    archive.unlink()


def reviewed_target(architecture, system, machine):
    if architecture not in GO_ARCH or system != "Linux" or machine != architecture:
        raise stage.Refused("A native Linux build for the selected architecture is required.")
    return GO_ARCH[architecture]


def reviewed_go(go, architecture):
    if architecture not in GO_ARCH:
        raise stage.Refused("A supported architecture is required.")
    path = Path(go)
    if (not path.is_absolute() or not path.is_file() or path.is_symlink()
            or not os.access(path, os.X_OK)):
        raise stage.Refused("An absolute local Go executable is required.")
    result = subprocess.run([str(path), "version"], stdin=subprocess.DEVNULL,
                            capture_output=True, timeout=10, check=False)
    if result.returncode != 0 or len(result.stdout) > 256:
        raise stage.Refused("Go toolchain identity is unavailable.")
    version = result.stdout.decode("ascii").strip()
    if (not version.startswith("go version go1.")
            or not version.endswith(" linux/" + GO_ARCH[architecture])):
        raise stage.Refused("The local Go toolchain does not match the selected architecture.")
    return version


def build(mihomo_git, sing_tun_git, go, architecture, output, flavor="experimental"):
    _, schema, patches = stage.pair_policy(flavor)
    close = flavor == "release-close"
    compiler_tmp = qualified_tmpdir() if close else None
    output = stage.outside_git_destination(output)
    host = os.uname()
    go_arch = reviewed_target(architecture, host.sysname, host.machine)
    revision = git_value(REPO, "rev-parse", "HEAD")
    if len(revision) != 40 or git_value(REPO, "status", "--porcelain", "--untracked-files=no"):
        raise stage.Refused("The OmaVLESS source checkout must be committed and clean.")
    for filename, expected in patches.items():
        if digest(PATCHES / filename) != expected:
            raise stage.Refused("Reviewed patch identity changed.")
    go_version = reviewed_go(go, architecture)
    # New qualified SOURCE producer retains scratch on every outcome. Its
    # compiler-only files are diagnostic evidence, never uncertain VM state.
    scratch_context = (contextlib.nullcontext(tempfile.mkdtemp(prefix=".omavless-dns-close-build-",dir=output.parent))
                       if close else tempfile.TemporaryDirectory(prefix=".omavless-dns-build-",dir=output.parent))
    run = functools.partial(command,public_diagnostics=close)
    with scratch_context as scratch:
        work = Path(scratch)
        if close:print("qualified_build_scratch="+str(work),file=sys.stderr)
        sources = work / "sources"
        sources.mkdir()
        export_git(mihomo_git, MIHOMO, sources / "mihomo",close)
        export_git(sing_tun_git, SING_TUN, sources / "sing-tun",close)
        export_git(REPO, revision, sources / "omavless",close)
        plan = [("mihomo-dns-broker.patch", "mihomo"),
                ("sing-tun-descriptor.patch", "sing-tun")]
        if close:
            plan.append(("mihomo-conditional-close.patch", "mihomo"))
        for name, directory in plan:
            patch = PATCHES / name
            git_env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C",
                       "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"}
            run(["/usr/bin/git", "apply", "--check", str(patch)],
                    cwd=sources / directory, env=git_env)
            run(["/usr/bin/git", "apply", str(patch)],
                    cwd=sources / directory, env=git_env)
        go_env = {"PATH": "/usr/bin:/bin", "HOME": os.environ["HOME"],
                  "GOPROXY": "off", "GOSUMDB": "off", "GOTOOLCHAIN": "local",
                  "GOWORK": "off",
                  "GOOS": "linux", "GOARCH": go_arch, "CGO_ENABLED": "0"}
        if close:
            go_env.update(GOENV="off", GOMAXPROCS="4", TMPDIR=compiler_tmp)
        run([go, "mod", "edit", "-replace=github.com/metacubex/sing-tun=../sing-tun"],
                cwd=sources / "mihomo", env=go_env)
        if close:
            run([go, "mod", "verify"], cwd=sources / "mihomo", env=go_env)
        run([go, "mod", "vendor"], cwd=sources / "mihomo", env=go_env)
        if not (sources / "mihomo/vendor/modules.txt").is_file():
            raise stage.Refused("Offline vendored dependency source is incomplete.")
        run([go, "test", "-mod=vendor", "-tags=with_gvisor", "./listener/config",
                 "./config", "./listener/sing_tun", "-run", "TestSystemDNS", "-count=1"],
                cwd=sources / "mihomo", env=go_env)
        if close:
            # Actual existing seven conditional primitives, not zero-test ABI
            # metadata. No race-instrumentation claim with this CGO0 build.
            receipt_path = work / "conditional-tests.json"
            run([go, "test", "-json", "-mod=vendor", "-tags=with_gvisor", "./tunnel/statistic",
                     "./hub/route", "-run", "^TestConditionalClose", "-count=1"],
                    cwd=sources / "mihomo", env=go_env, output=receipt_path)
            with receipt_path.open("rb") as result:
                conditional_tests_completed(result.read(1024 * 1024 + 1))
        package = work / "payload"
        package.mkdir()
        package.chmod(0o700)
        core = package / "mihomo"
        core_command = [go, "build", "-mod=vendor", "-tags=with_gvisor", "-trimpath"]
        if close:
            core_command.append("-buildvcs=false")
        run(core_command + ["-ldflags=-s -w", "-o", str(core), "."],
                cwd=sources / "mihomo", env=go_env)
        info = subprocess.run([go, "version", "-m", str(core)],
                              stdin=subprocess.DEVNULL, capture_output=True,
                              check=False, timeout=10)
        if (info.returncode != 0 or len(info.stdout) > 65536
                or b"path\tgithub.com/metacubex/mihomo" not in info.stdout
                or b"-tags=with_gvisor" not in info.stdout
                or b"CGO_ENABLED=0" not in info.stdout):
            raise stage.Refused("Reviewed core build identity is incomplete.")
        cargo_env = {"PATH": os.environ["PATH"], "HOME": os.environ["HOME"],
                     "CARGO_NET_OFFLINE": "true", "CARGO_TARGET_DIR": str(work / "cargo-target")}
        if close:
            cargo_env["CARGO_BUILD_JOBS"] = "4"
            cargo_env["TMPDIR"] = compiler_tmp
        cargo_command = ["/usr/bin/cargo", "build", "--release", "--locked", "--offline",
                         "-p", "omavless-dns-broker", "--bin", "omavless-dns-broker"]
        if flavor in ("release", "release-close"):
            cargo_command.extend(("--features", "release-package"))
        run(cargo_command,
                cwd=sources / "omavless", env=cargo_env)
        shutil.copy2(work / "cargo-target/release/omavless-dns-broker",
                     package / "omavless-dns-broker")
        stage.reviewed_binary(core, digest(core), architecture)
        stage.reviewed_binary(package / "omavless-dns-broker",
                              digest(package / "omavless-dns-broker"), architecture)
        with tarfile.open(package / "corresponding-source.tar.xz", "w:xz") as stream:
            for name in ("mihomo", "sing-tun", "omavless"):
                stream.add(sources / name, arcname=name, recursive=True)
        for name in ("mihomo", "sing-tun"):
            shutil.copy2(sources / name / "LICENSE", package / (name + ".LICENSE"))
        shutil.copy2(sources / "omavless/LICENSE", package / "omavless.LICENSE")
        receipt = {
            "schema": schema, "architecture": architecture, "omavless_commit": revision,
            "mihomo_commit": MIHOMO, "mihomo_tag": "v1.19.31",
            "sing_tun_commit": SING_TUN, "sing_tun_tag": "v0.4.24",
            "patch_sha256": patches, "go_version": go_version,
            "go_build_tags": "with_gvisor", "go_dependency_mode": "vendor",
            "go_binary_sha256": digest(Path(go)),
            "rustc_version": subprocess.run(
                ["/usr/bin/rustc", "--version"], stdin=subprocess.DEVNULL,
                capture_output=True, check=True, timeout=10).stdout.decode("ascii").strip(),
            "cargo_version": subprocess.run(
                ["/usr/bin/cargo", "--version"], stdin=subprocess.DEVNULL,
                capture_output=True, check=True, timeout=10).stdout.decode("ascii").strip(),
            "cargo_lock_sha256": digest(sources / "omavless/Cargo.lock"),
            "sha256": {name: digest(package / name) for name in
                       ("mihomo", "omavless-dns-broker", "corresponding-source.tar.xz",
                        "mihomo.LICENSE", "sing-tun.LICENSE", "omavless.LICENSE")},
        }
        if flavor in ("release", "release-close"):
            receipt["package_flavor"] = flavor
            receipt["broker_feature"] = "release-package"
        if close:
            receipt.update(conditional_close_abi=1, go_cgo=False, go_buildvcs=False)
        (package / "source-receipt.json").write_text(
            json.dumps(receipt, sort_keys=True, indent=2) + "\n", encoding="utf-8")
        if output.exists() or output.is_symlink():
            raise stage.Refused("The output destination already exists.")
        package.rename(output)
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("mihomo-git", "sing-tun-git", "go", "output"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--arch", choices=tuple(GO_ARCH), required=True)
    parser.add_argument("--flavor", choices=("experimental", "release", "release-close"), default="experimental")
    args = parser.parse_args()
    try:
        receipt = build(args.mihomo_git, args.sing_tun_git, args.go,
                        args.arch, args.output, args.flavor)
    except (OSError, ValueError, subprocess.CalledProcessError,
            subprocess.TimeoutExpired, stage.Refused):
        parser.exit(1, "Offline DNS pair build refused; no package installed or activated.\n")
    print("Offline DNS pair candidate built for " + receipt["architecture"] +
          " at source " + receipt["omavless_commit"] + ".")


if __name__ == "__main__":
    main()
