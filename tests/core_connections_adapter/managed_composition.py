#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Offline review of conditional close plus the exact managed-DNS core patches.

This compiles only a disposable core, not a broker/package/release. No installed
core, DNS, TUN, runtime, provider or credentials are accessed or modified.
"""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile

import loopback
import review

DNS_REVISION = "c4e800425243c1b02165f82153e4bf418fe465e6"
SING_TUN = "b50ae28a1409c7bce8e96e6c6966cf57d8ace754"
PATCHES = {
    "mihomo-dns-broker.patch": "d5ebe9d6b37f6b76599fc3c2dd25adbfb774ca0121beeb79c5768a9a08d7ff37",
    "sing-tun-descriptor.patch": "2556c82aafbeb598a817d43042cf2069c6f209433c7a506395df581b4e31e2ab",
}
MAX_ARCHIVE = 128 * 1024 * 1024
MAX_PATCH = 1024 * 1024
SOCKET_TEST_PATCH = Path(__file__).with_name("mihomo-dns-test-sockets.patch")
SOCKET_TEST_SHA256 = "38eeedf8ac00cf387138a50107f87a392f17ef65607710a3cc6e34d320e0c169"


def git_environment():
    return {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "GIT_NO_REPLACE_OBJECTS": "1",
            "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"}


def validate_paths(paths, scratch):
    for path in (*paths, scratch):
        if not path.is_absolute() or path.is_symlink() or not path.is_dir():
            raise RuntimeError("Composition requires existing absolute directories")
    st = scratch.stat()
    if st.st_uid != os.getuid() or st.st_mode & 0o077:
        raise RuntimeError("Composition scratch must be private")


def dns_patches(repository):
    result = {}
    for name, expected in PATCHES.items():
        data = review.run(["/usr/bin/git", "-C", str(repository), "show",
                           DNS_REVISION + ":tests/core_dns_adapter/" + name], env=git_environment())
        if not data or len(data) > MAX_PATCH or hashlib.sha256(data).hexdigest() != expected:
            raise RuntimeError("Exact managed-DNS patch identity refused")
        result[name] = data
    return result


def export(repository, revision, destination):
    identity = review.run(["/usr/bin/git", "-C", str(repository), "rev-parse",
                           revision + "^{commit}"], env=git_environment()).strip()
    if identity != revision.encode("ascii"):
        raise RuntimeError("Composition upstream identity refused")
    # archive consults $GIT_DIR/info/attributes even without --worktree-attributes.
    # Use a fresh repository with only the original read-only object directory;
    # local export-ignore/export-subst rules and hooks are never consulted.
    objects = Path(os.fsdecode(review.run(
        ["/usr/bin/git", "-C", str(repository), "rev-parse", "--path-format=absolute",
         "--git-path", "objects"], env=git_environment()).rstrip(b"\n")))
    if not objects.is_absolute() or not objects.is_dir():
        raise RuntimeError("Composition object directory refused")
    isolated = destination.parent / (destination.name + "-export.git")
    review.run(["/usr/bin/git", "init", "--quiet", "--bare", "--template=", str(isolated)],
               env=git_environment())
    archive_env = dict(git_environment(), GIT_OBJECT_DIRECTORY=str(objects))
    archive = destination.parent / (destination.name + ".tar")
    with archive.open("xb") as output:
        result = subprocess.run(["/usr/bin/git", "--git-dir=" + str(isolated), "archive", revision],
                                stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.DEVNULL,
                                timeout=60, env=archive_env, check=False)
    if result.returncode or archive.stat().st_size > MAX_ARCHIVE:
        raise RuntimeError("Composition archive refused")
    destination.mkdir(mode=0o700)
    with tarfile.open(archive) as members:
        members.extractall(destination, filter="data")
    archive.unlink()


def apply(path, patch, *, reverse=False):
    for arguments in (["--check"], []):
        review.run(["/usr/bin/git", "apply", *(["--reverse"] if reverse else []),
                    *arguments, str(patch)], cwd=path, env=git_environment())


def exercise(mihomo, sing_tun, repository, scratch):
    validate_paths((mihomo, sing_tun, repository), scratch)
    patches = dns_patches(repository)  # Refuse unknown patch bytes before compilation.
    if hashlib.sha256(SOCKET_TEST_PATCH.read_bytes()).hexdigest() != SOCKET_TEST_SHA256:
        raise RuntimeError("Composition test overlay identity refused")
    with tempfile.TemporaryDirectory(prefix="managed-close-", dir=scratch) as name:
        root = Path(name)
        export(mihomo, review.PIN, root / "mihomo")
        export(sing_tun, SING_TUN, root / "sing-tun")
        for filename, destination in (("mihomo-dns-broker.patch", "mihomo"),
                                      ("sing-tun-descriptor.patch", "sing-tun")):
            patch = root / filename
            patch.write_bytes(patches[filename])
            apply(root / destination, patch)
        apply(root / "mihomo", review.PATCH)
        # Separate test-only overlay: Go's TempDir includes full subtest names,
        # exceeding Linux sun_path with HOME scratch. Production code and the
        # two exact managed-DNS patch blobs remain unchanged and hash-pinned.
        apply(root / "mihomo", SOCKET_TEST_PATCH)
        # No dependency downloads, installed modules, Go workspaces or implicit
        # toolchain acquisition; private copied dependency replaces only sing-tun.
        env = {"PATH": os.environ["PATH"], "HOME": os.environ["HOME"],
               "TMPDIR": str(root), "GOPROXY": "off", "GOSUMDB": "off",
               "GOWORK": "off", "GOTOOLCHAIN": "local", "GOFLAGS": "", "CGO_ENABLED": "0"}
        source = root / "mihomo"
        review.run(["go", "mod", "edit", "-replace=github.com/metacubex/sing-tun=../sing-tun"], cwd=source, env=env)
        review.run(["go", "mod", "vendor"], cwd=source, env=env)
        # Race instrumentation requires cgo; it is never used in the core build.
        review.run(["go", "test", "-race", "-mod=vendor", "-tags=with_gvisor",
                    "./tunnel/statistic", "./hub/route", "-run", "TestConditional", "-count=20"],
                   cwd=source, env=dict(env, CGO_ENABLED="1"))
        review.run(["go", "test", "-mod=vendor", "-tags=with_gvisor", "./listener/config",
                    "./config", "./listener/sing_tun", "-run", "TestSystemDNS", "-count=20"], cwd=source, env=env)
        # Restore the exact production composition before compiling its binary.
        # Test source is not linked by Go build, but keeping the two identities
        # visibly separate avoids treating a modified test tree as a package.
        apply(source, SOCKET_TEST_PATCH, reverse=True)
        core = root / "combined-core"
        review.run(["go", "build", "-mod=vendor", "-tags=with_gvisor", "-trimpath",
                    "-buildvcs=false", "-ldflags=-s -w", "-o", str(core), "."], cwd=source, env=env)
        metadata = review.run(["go", "version", "-m", str(core)], env=env)
        if b"-tags=with_gvisor" not in metadata or b"CGO_ENABLED=0" not in metadata:
            raise RuntimeError("Composition production-tag identity refused")
        loopback.exercise(core, root)
        return hashlib.sha256(core.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for field in ("mihomo-source", "sing-tun-source", "dns-repository", "scratch-parent"):
        parser.add_argument("--" + field, type=Path, required=True)
    args = parser.parse_args()
    sha = exercise(args.mihomo_source, args.sing_tun_source, args.dns_repository, args.scratch_parent)
    print("managed_conditional_composition: passed; offline; synthetic loopback; no installation")
    print("mihomo_sha=" + review.PIN + " sing_tun_sha=" + SING_TUN)
    print("dns_adapter_sha=" + DNS_REVISION)
    print("conditional_patch_sha256=" + hashlib.sha256(review.PATCH.read_bytes()).hexdigest())
    print("socket_test_patch_sha256=" + hashlib.sha256(SOCKET_TEST_PATCH.read_bytes()).hexdigest())
    print("go_toolchain=" + review.run(["go", "version"]).decode("ascii").strip())
    print("core_sha256=" + sha)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError, tarfile.TarError):
        raise SystemExit("managed_conditional_composition: refused; no installation") from None
