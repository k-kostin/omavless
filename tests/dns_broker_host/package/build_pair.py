#!/usr/bin/env python3
"""Offline review build of the exact experimental broker/core source pair.

No downloads, installation, privilege, or runtime activation. The output is a
private candidate directory, not an enrollment decision or release artifact.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import shutil
import signal
import subprocess
import tarfile
import tempfile
import time

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
MAX_GIT_VALUE = 4096
MAX_GIT_ERROR = 4096
GO_ARCH = {"x86_64": "amd64", "aarch64": "arm64"}


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def command(arguments, *, cwd=None, env=None, output=None):
    sink = output.open("wb") if output else subprocess.DEVNULL
    try:
        result = subprocess.run(arguments, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                stdout=sink, stderr=subprocess.PIPE, timeout=600,
                                check=False)
    finally:
        if output:
            sink.close()
    if result.returncode != 0 or len(result.stderr) > 2 * 1024 * 1024:
        raise stage.Refused("Offline reviewed build step failed.")
    return result.stderr


def git_environment():
    return {"PATH": "/usr/bin:/bin", "LC_ALL": "C",
            "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
            "GIT_ATTR_NOSYSTEM": "1", "GIT_NO_REPLACE_OBJECTS": "1",
            "GIT_NO_LAZY_FETCH": "1", "GIT_TERMINAL_PROMPT": "0",
            "GIT_OPTIONAL_LOCKS": "0"}


def git_command(arguments, *, env=None, output=None, timeout=20):
    """Bound Git pipes while running, without exposing private stderr/paths.

    Only an owned child process group is stopped on timeout/output overflow.
    The object-store commands below are read-only; hooks, fsmonitor and lazy
    downloads must not turn observation into a source mutation or network step.
    """
    arguments = ["/usr/bin/git", "-c", "core.fsmonitor=false", "-c",
                 "core.hooksPath=/dev/null", "-c", "core.attributesFile=/dev/null",
                 *arguments]
    captured = bytearray()
    size = error_size = 0
    sink = output.open("xb") if output is not None else None
    process = None
    waited = False
    poller = selectors.DefaultSelector()
    deadline = time.monotonic() + timeout
    try:
        process = subprocess.Popen(arguments, env=env or git_environment(),
                                   stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, start_new_session=True)
        poller.register(process.stdout, selectors.EVENT_READ, "output")
        poller.register(process.stderr, selectors.EVENT_READ, "error")
        while poller.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise stage.Refused("Offline Git source step exceeded its bound.")
            for key, _ in poller.select(min(remaining, 0.1)):
                block = os.read(key.fileobj.fileno(), 65536)
                if not block:
                    poller.unregister(key.fileobj)
                    continue
                if key.data == "error":
                    error_size += len(block)
                    if error_size > MAX_GIT_ERROR:
                        raise stage.Refused("Offline Git source step exceeded its bound.")
                else:
                    size += len(block)
                    if size > (MAX_ARCHIVE if sink is not None else MAX_GIT_VALUE):
                        raise stage.Refused("Offline Git source step exceeded its bound.")
                    if sink is not None:
                        sink.write(block)
                    else:
                        captured.extend(block)
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise stage.Refused("Offline Git source step exceeded its bound.")
        result = process.wait(timeout=remaining)
        waited = True
        if result != 0:
            raise stage.Refused("Offline Git source step failed.")
        return bytes(captured)
    finally:
        poller.close()
        try:
            if process is not None and not waited and process.returncode is None:
                # wait() may reap the leader before an interruption prevents
                # setting waited. Inspect its recorded result without polling
                # or reaping; only an unreaped leader protects group identity.
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=5)
        finally:
            if process is not None:
                process.stdout.close()
                process.stderr.close()
            if sink is not None:
                sink.close()


def git_value(repository, *arguments):
    return git_command(["-C", str(repository), *arguments]).decode("ascii").strip()


def export_git(repository, revision, destination):
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise stage.Refused("An exact source commit is required.")
    if git_value(repository, "rev-parse", "--verify", f"{revision}^{{commit}}") != revision:
        raise stage.Refused("Pinned upstream commit is absent.")
    objects = Path(git_value(repository, "rev-parse", "--path-format=absolute",
                             "--git-path", "objects"))
    if not objects.is_absolute() or not objects.is_dir():
        raise stage.Refused("Pinned source object directory is unavailable.")
    archive = destination.parent / (destination.name + ".tar")
    # archive honors source $GIT_DIR/info/attributes, even for an exact commit.
    # A fresh template-free repository sees only original object bytes, never
    # source config, replacement refs, info attributes, index or working files.
    with tempfile.TemporaryDirectory(prefix=".source-export-", dir=destination.parent) as name:
        isolated = Path(name)
        git_command(["init", "--quiet", "--bare", "--template=", str(isolated)])
        git_env = dict(git_environment(), GIT_OBJECT_DIRECTORY=str(objects))
        git_command(["--git-dir=" + str(isolated), "archive", "--format=tar", revision],
                    env=git_env, output=archive, timeout=60)
    destination.mkdir(mode=0o700)
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
    if flavor not in ("experimental", "release"):
        raise stage.Refused("A supported package flavor is required.")
    output = stage.outside_git_destination(output)
    host = os.uname()
    go_arch = reviewed_target(architecture, host.sysname, host.machine)
    revision = git_value(REPO, "rev-parse", "HEAD")
    if len(revision) != 40 or git_value(REPO, "status", "--porcelain", "--untracked-files=no"):
        raise stage.Refused("The OmaVLESS source checkout must be committed and clean.")
    for filename, expected in PATCH_SHA.items():
        if digest(PATCHES / filename) != expected:
            raise stage.Refused("Reviewed patch identity changed.")
    go_version = reviewed_go(go, architecture)
    with tempfile.TemporaryDirectory(prefix=".omavless-dns-build-", dir=output.parent) as scratch:
        work = Path(scratch)
        sources = work / "sources"
        sources.mkdir()
        export_git(mihomo_git, MIHOMO, sources / "mihomo")
        export_git(sing_tun_git, SING_TUN, sources / "sing-tun")
        export_git(REPO, revision, sources / "omavless")
        for name, directory in (("mihomo-dns-broker.patch", "mihomo"),
                                ("sing-tun-descriptor.patch", "sing-tun")):
            patch = PATCHES / name
            git_env = git_environment()
            command(["/usr/bin/git", "apply", "--check", str(patch)],
                    cwd=sources / directory, env=git_env)
            command(["/usr/bin/git", "apply", str(patch)],
                    cwd=sources / directory, env=git_env)
        go_env = {"PATH": "/usr/bin:/bin", "HOME": os.environ["HOME"],
                  "GOPROXY": "off", "GOSUMDB": "off", "GOTOOLCHAIN": "local",
                  "GOWORK": "off",
                  "GOOS": "linux", "GOARCH": go_arch, "CGO_ENABLED": "0"}
        command([go, "mod", "edit", "-replace=github.com/metacubex/sing-tun=../sing-tun"],
                cwd=sources / "mihomo", env=go_env)
        command([go, "mod", "vendor"], cwd=sources / "mihomo", env=go_env)
        if not (sources / "mihomo/vendor/modules.txt").is_file():
            raise stage.Refused("Offline vendored dependency source is incomplete.")
        command([go, "test", "-mod=vendor", "-tags=with_gvisor", "./listener/config",
                 "./config", "./listener/sing_tun", "-run", "TestSystemDNS", "-count=1"],
                cwd=sources / "mihomo", env=go_env)
        package = work / "payload"
        package.mkdir()
        package.chmod(0o700)
        core = package / "mihomo"
        command([go, "build", "-mod=vendor", "-tags=with_gvisor", "-trimpath",
                 "-ldflags=-s -w", "-o", str(core), "."],
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
        cargo_command = ["/usr/bin/cargo", "build", "--release", "--locked", "--offline",
                         "-p", "omavless-dns-broker", "--bin", "omavless-dns-broker"]
        if flavor == "release":
            cargo_command.extend(("--features", "release-package"))
        command(cargo_command,
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
            "schema": 1, "architecture": architecture, "omavless_commit": revision,
            "mihomo_commit": MIHOMO, "mihomo_tag": "v1.19.31",
            "sing_tun_commit": SING_TUN, "sing_tun_tag": "v0.4.24",
            "patch_sha256": PATCH_SHA, "go_version": go_version,
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
        if flavor == "release":
            receipt["package_flavor"] = "release"
            receipt["broker_feature"] = "release-package"
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
    parser.add_argument("--flavor", choices=("experimental", "release"), default="experimental")
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
