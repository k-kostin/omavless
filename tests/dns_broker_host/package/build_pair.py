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
import shutil
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


def git_value(repository, *arguments):
    result = subprocess.run(["/usr/bin/git", "-C", str(repository), *arguments],
                            stdin=subprocess.DEVNULL, capture_output=True, timeout=20,
                            env={"PATH": "/usr/bin:/bin", "LC_ALL": "C",
                                 "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"},
                            check=False)
    if result.returncode != 0 or len(result.stdout) > 4096:
        raise stage.Refused("Pinned source identity is unavailable.")
    return result.stdout.decode("ascii").strip()


def export_git(repository, revision, destination):
    if git_value(repository, "rev-parse", "--verify", f"{revision}^{{commit}}") != revision:
        raise stage.Refused("Pinned upstream commit is absent.")
    archive = destination.parent / (destination.name + ".tar")
    git_env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C",
               "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"}
    command(["/usr/bin/git", "-C", str(repository), "archive", "--format=tar",
             revision], env=git_env, output=archive)
    if archive.stat().st_size > MAX_ARCHIVE:
        raise stage.Refused("Pinned source archive exceeds its bound.")
    destination.mkdir()
    with tarfile.open(archive, "r:") as stream:
        stream.extractall(destination, filter="data")
    archive.unlink()


def reviewed_go(go):
    path = Path(go)
    if (not path.is_absolute() or not path.is_file() or path.is_symlink()
            or not os.access(path, os.X_OK)):
        raise stage.Refused("An absolute local Go executable is required.")
    result = subprocess.run([str(path), "version"], stdin=subprocess.DEVNULL,
                            capture_output=True, timeout=10, check=False)
    if result.returncode != 0 or len(result.stdout) > 256:
        raise stage.Refused("Go toolchain identity is unavailable.")
    version = result.stdout.decode("ascii").strip()
    if not version.startswith("go version go1.") or not version.endswith(" linux/amd64"):
        raise stage.Refused("This review builder currently requires Linux amd64 Go.")
    return version


def build(mihomo_git, sing_tun_git, go, output):
    output = stage.outside_git_destination(output)
    revision = git_value(REPO, "rev-parse", "HEAD")
    if len(revision) != 40 or git_value(REPO, "status", "--porcelain", "--untracked-files=no"):
        raise stage.Refused("The OmaVLESS source checkout must be committed and clean.")
    for filename, expected in PATCH_SHA.items():
        if digest(PATCHES / filename) != expected:
            raise stage.Refused("Reviewed patch identity changed.")
    go_version = reviewed_go(go)
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
            git_env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C",
                       "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"}
            command(["/usr/bin/git", "apply", "--check", str(patch)],
                    cwd=sources / directory, env=git_env)
            command(["/usr/bin/git", "apply", str(patch)],
                    cwd=sources / directory, env=git_env)
        go_env = {"PATH": "/usr/bin:/bin", "HOME": os.environ["HOME"],
                  "GOPROXY": "off", "GOSUMDB": "off", "GOTOOLCHAIN": "local",
                  "GOOS": "linux", "GOARCH": "amd64", "CGO_ENABLED": "0"}
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
        command(["/usr/bin/cargo", "build", "--release", "--locked", "--offline",
                 "-p", "omavless-dns-broker", "--bin", "omavless-dns-broker"],
                cwd=sources / "omavless", env=cargo_env)
        shutil.copy2(work / "cargo-target/release/omavless-dns-broker",
                     package / "omavless-dns-broker")
        stage.reviewed_binary(core, digest(core), "x86_64")
        stage.reviewed_binary(package / "omavless-dns-broker",
                              digest(package / "omavless-dns-broker"), "x86_64")
        with tarfile.open(package / "corresponding-source.tar.xz", "w:xz") as stream:
            for name in ("mihomo", "sing-tun", "omavless"):
                stream.add(sources / name, arcname=name, recursive=True)
        for name in ("mihomo", "sing-tun"):
            shutil.copy2(sources / name / "LICENSE", package / (name + ".LICENSE"))
        shutil.copy2(sources / "omavless/LICENSE", package / "omavless.LICENSE")
        receipt = {
            "schema": 1, "architecture": "x86_64", "omavless_commit": revision,
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
    args = parser.parse_args()
    try:
        receipt = build(args.mihomo_git, args.sing_tun_git, args.go, args.output)
    except (OSError, ValueError, subprocess.CalledProcessError,
            subprocess.TimeoutExpired, stage.Refused):
        parser.exit(1, "Offline DNS pair build refused; no package installed or activated.\n")
    print("Offline DNS pair candidate built for " + receipt["architecture"] +
          " at source " + receipt["omavless_commit"] + ".")


if __name__ == "__main__":
    main()
