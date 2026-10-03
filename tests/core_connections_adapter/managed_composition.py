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
import udp_loopback
import dns_interop

DNS_REVISION = "c4e800425243c1b02165f82153e4bf418fe465e6"
SING_TUN = "b50ae28a1409c7bce8e96e6c6966cf57d8ace754"
PATCHES = {
    "mihomo-dns-broker.patch": "d5ebe9d6b37f6b76599fc3c2dd25adbfb774ca0121beeb79c5768a9a08d7ff37",
    "sing-tun-descriptor.patch": "2556c82aafbeb598a817d43042cf2069c6f209433c7a506395df581b4e31e2ab",
}
MAX_PATCH = 1024 * 1024
SOCKET_TEST_PATCH = Path(__file__).with_name("mihomo-dns-test-sockets.patch")
SOCKET_TEST_SHA256 = "38eeedf8ac00cf387138a50107f87a392f17ef65607710a3cc6e34d320e0c169"
CONDITIONAL_TESTS = review.CONDITIONAL_TESTS
DNS_TESTS = (
    "TestSystemDNSOption", "TestSystemDNSBrokerReadyCannotBeConfigured",
    "TestSystemDNSBrokerFixedVirtualResolver", "TestSystemDNSOwnershipChangeRequiresTunReplacement",
    "TestSystemDNSBrokerRefusesPolicyBeforeDeviceAccess", "TestSystemDNSBrokerRejectsUnsolicitedCompletionAndJoins",
    "TestSystemDNSBrokerAcquireRelease", "TestSystemDNSBrokerRejectsMalformedReplies",
    "TestSystemDNSBrokerRequiresExpectedPeer", "TestSystemDNSBrokerLossInvalidatesReady",
    "TestSystemDNSBrokerRefusesUnexpectedRights",
)
DNS_INTEROP_SKIP = "TestSystemDNSRustChannelInterop"
INTEROP_TEST_PATCH = Path(__file__).with_name("mihomo-dns-interop-sockets.patch")
INTEROP_TEST_SHA256 = "d0132ae4758ddc9baadca7f0826e37fbbed9667436e965c9a5eee65121fa3385"


git_environment = review.git_environment
export = review.export
matrix_receipt = review.matrix_receipt


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


def apply(path, patch, *, reverse=False):
    for arguments in (["--check"], []):
        review.run(["/usr/bin/git", "apply", *(["--reverse"] if reverse else []),
                    *arguments, str(patch)], cwd=path, env=git_environment())


def exercise(mihomo, sing_tun, repository, scratch, *, udp=False,
             fixture=None, fixture_sha=None):
    validate_paths((mihomo, sing_tun, repository), scratch)
    if (fixture is None) != (fixture_sha is None):
        raise RuntimeError("Rust wire opt-in requires path and exact digest")
    fixture_bytes = None if fixture is None else dns_interop.snapshot_fixture(fixture, fixture_sha)
    patches = dns_patches(repository)  # Refuse unknown patch bytes before compilation.
    if hashlib.sha256(SOCKET_TEST_PATCH.read_bytes()).hexdigest() != SOCKET_TEST_SHA256:
        raise RuntimeError("Composition test overlay identity refused")
    corpus = None
    if fixture_bytes is not None:
        if hashlib.sha256(INTEROP_TEST_PATCH.read_bytes()).hexdigest() != INTEROP_TEST_SHA256:
            raise RuntimeError("Rust wire test overlay identity refused")
        corpus = review.run(["/usr/bin/git", "-C", str(repository), "show",
                             DNS_REVISION + ":" + dns_interop.CORPUS_PATH], env=git_environment())
        if (not corpus or len(corpus) > 4096
                or hashlib.sha256(corpus).hexdigest() != dns_interop.CORPUS_SHA256):
            raise RuntimeError("Pinned Rust wire corpus refused")
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
        env = review.compiler_environment(root)
        source = root / "mihomo"
        review.run([review.GO, "mod", "edit", "-replace=github.com/metacubex/sing-tun=../sing-tun"], cwd=source, env=env)
        review.run([review.GO, "mod", "verify"], cwd=source, env=env)
        review.run([review.GO, "mod", "vendor"], cwd=source, env=env)
        # Race instrumentation requires cgo; it is never used in the core build.
        conditional = review.run([review.GO, "test", "-json", "-race", "-mod=vendor", "-tags=with_gvisor",
                                  "./tunnel/statistic", "./hub/route", "-run", "TestConditional", "-count=20"],
                                 cwd=source, env=dict(env, CGO_ENABLED="1"))
        matrix_receipt(conditional, CONDITIONAL_TESTS)
        dns = review.run([review.GO, "test", "-json", "-mod=vendor", "-tags=with_gvisor", "./listener/config",
                          "./config", "./listener/sing_tun", "-run", "TestSystemDNS", "-count=20"], cwd=source, env=env)
        matrix_receipt(dns, DNS_TESTS, (DNS_INTEROP_SKIP,))
        if fixture_bytes is not None:
            apply(source, INTEROP_TEST_PATCH)
            dns_interop.exercise(root, source, env, fixture_bytes, fixture_sha,
                                 corpus, review.run, review.GO)
            apply(source, INTEROP_TEST_PATCH, reverse=True)
        # Restore the exact production composition before compiling its binary.
        # Test source is not linked by Go build, but keeping the two identities
        # visibly separate avoids treating a modified test tree as a package.
        apply(source, SOCKET_TEST_PATCH, reverse=True)
        core = root / "combined-core"
        review.run([review.GO, "build", "-mod=vendor", "-tags=with_gvisor", "-trimpath",
                    "-buildvcs=false", "-ldflags=-s -w", "-o", str(core), "."], cwd=source, env=env)
        metadata = review.run([review.GO, "version", "-m", str(core)], env=env)
        if b"-tags=with_gvisor" not in metadata or b"CGO_ENABLED=0" not in metadata:
            raise RuntimeError("Composition production-tag identity refused")
        loopback.exercise(core, root)
        if udp:
            for _ in range(20):
                udp_loopback.exercise(core, root)
        return hashlib.sha256(core.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for field in ("mihomo-source", "sing-tun-source", "dns-repository", "scratch-parent"):
        parser.add_argument("--" + field, type=Path, required=True)
    parser.add_argument("--udp-loopback", action="store_true", help="Also exercise synthetic SOCKS5 UDP close/reconnect")
    parser.add_argument("--rust-channel-fixture", type=Path,
                        help="Explicit separately reviewed synthetic channel_fixture ELF")
    parser.add_argument("--rust-channel-fixture-sha256",
                        help="Exact SHA-256 of that frozen test artifact, not release attestation")
    args = parser.parse_args()
    sha = exercise(args.mihomo_source, args.sing_tun_source, args.dns_repository,
                   args.scratch_parent, udp=args.udp_loopback,
                   fixture=args.rust_channel_fixture, fixture_sha=args.rust_channel_fixture_sha256)
    print("managed_conditional_composition: passed; offline; synthetic loopback; no installation")
    print("mihomo_sha=" + review.PIN + " sing_tun_sha=" + SING_TUN)
    print("dns_adapter_sha=" + DNS_REVISION)
    print("go_cases=7_conditional_and_11_dns_x20")
    if args.rust_channel_fixture is not None:
        print("rust_dns_interop=4_synthetic_wire_cases_x20_passed; no_real_dns_or_broker")
        print("rust_channel_fixture_sha256=" + args.rust_channel_fixture_sha256)
        print("rust_channel_corpus_sha256=" + dns_interop.CORPUS_SHA256)
        print("interop_socket_test_patch_sha256=" + INTEROP_TEST_SHA256)
    else:
        print("rust_dns_interop=not_run_explicit_optin")
    print("conditional_patch_sha256=" + hashlib.sha256(review.PATCH.read_bytes()).hexdigest())
    print("socket_test_patch_sha256=" + hashlib.sha256(SOCKET_TEST_PATCH.read_bytes()).hexdigest())
    print("go_toolchain=" + review.run([review.GO, "version"], env=review.compiler_environment(args.scratch_parent)).decode("ascii").strip())
    print("core_sha256=" + sha)
    if args.udp_loopback:
        print("udp_loopback=20_passed; wrong_token_preserves_both; exact_close_receipt; same_association_reconnects")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError, tarfile.TarError):
        raise SystemExit("managed_conditional_composition: refused; no installation") from None
