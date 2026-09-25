# Experimental core DNS ownership adapter — not an installed dependency

This directory retains the minimal **unpublished, review-only** Mihomo patch
needed to test DNS-0 for #270. It is not applied by install, package, release,
runtime, plugin or ordinary CI. No fork distribution or upstream acceptance is
implied. Do not add the proposed option to production configuration yet.

Upstream: MetaCubeX/mihomo, exact source
`ab405bad5beeeac8b003bb01f60f134f6df54471` (1.19.31), with its locked
`github.com/metacubex/sing-tun v0.4.24`. The patch is subject to the upstream
project's GPL-3.0 license; retaining a review patch does not relicense the
upstream source as OmaVLESS's MIT code.

The original minimal DNS-off patch is retained unchanged. The newer
`mihomo-dns-broker.patch` is an **alternative full patch** against the same base;
it includes DNS-off, so do not apply both. It additionally requires
`sing-tun-descriptor.patch` on a separate copy of locked sing-tun v0.4.24
(upstream GPL-3.0-or-later). No patched dependency is downloaded, installed or
selected by the OmaVLESS build. The disposable review build uses a local Go
module replace pointing to that patched copy; no machine-specific path is
retained in either patch or the product's manifests.

## Boundary

The proposed `tun.disable-system-dns` defaults to false. It maps raw config to
listener config and sing-tun's existing `EXP_DisableDNSHijack`, which guards
Linux resolved setup **and** teardown. TUN equality includes this flag so a
reload cannot keep a listener with the previous ownership policy. The flag is
visible in the TUN controller projection; an older core without the boolean is
refused by the experiment. A boolean alone is not production capability proof:
future enrollment still needs approved package/source identity and conformance.

This does not disable packet DNS interception, create a broker, repair cancelled
authorization, configure DNS itself or make an unconfigured tunnel ready. The
unit test preserves packet interception, routes and FD fields; the live probe
uses DIRECT-only `stack: system`, empty packet DNS interception and no routes.
No external connectivity / IPv4+IPv6 routing claim follows from that probe.

## Reproduce outside the application checkout

Use a disposable checkout of the exact upstream commit above and a verified Go
toolchain. No sudo, installation or file capabilities are needed. Before applying,
check the upstream HEAD and clean worktree; fail rather than adapting to a new
revision silently. `PATCH` below means the absolute path to this directory's
`mihomo-disable-system-dns.patch`.

```sh
git apply --check "$PATCH"
git apply "$PATCH"
go test -mod=readonly ./listener/config ./config -run TestSystemDNS -count=1
CGO_ENABLED=0 go build -mod=readonly -trimpath -ldflags='-s -w' -o ./dns-test-core .
sha256sum ./dns-test-core
```

Return to the OmaVLESS checkout and explicitly run:

```sh
python3 tests/dns_core_ownership_probe.py /absolute/path/to/dns-test-core EXACT_SHA256
```

The parent validates an absolute regular file, root/current-user ownership,
non-group/other-writable mode, size and digest. The isolated child checks fresh
user/network/PID namespaces and loopback-only inventory **before** device access,
copy or launch. It copies bytes without xattrs/capabilities, rechecks the digest,
uses a scratch-only recording `resolvectl`, no live D-Bus address, synthetic
config and Unix-only controller. It never reads a private store or calls the
installed runtime. Owned children are joined; the PID namespace is also killed
on parent failure. Ordinary tests only exercise mocks and do not run this probe.

Positive cases: omitted and false retain setup/close calls; true suppresses both;
changed TUN stays external-owned; true→false and false→true reloads change the
writer; FD handoff false still reverts on close whereas true does not. Negative
control: the stock core fails with `unsupported_dns_ownership_capability`, not
PASS merely because YAML accepted the unknown option. Absence is measured over
finite one-second windows and owned process exit, not indefinite asynchronous
system DNS behavior.

## Evidence on Try Omarchy ARM64, September 25

Go 1.26.8 (`linux/arm64`), CGO disabled, default tags (not the installed package's
`with_gvisor` build). Toolchain archive SHA-256:
`211ffced9dcb9633a55eac6364816ec0ddd951389a740e88fa8b3337971bdda0`.
Candidate binary SHA-256:
`62d2c18f0ccaed78b67360641ebc5da86d04bb04400d0a04eaba6c21d0f60f3d`.
The source package's two test functions pass (three config subcases plus equality),
and the complete core builds. The isolated probe's final result and remaining
gates are recorded in the [research report](../../docs/development/DNS_AUTHORIZATION_RESEARCH.md).

Before product integration: independently review upstream/distribution choice;
validate required production build tags and packet/routing behavior; establish
the secure managed-TUN lease and pristine baseline; implement the typed broker
and lifecycle/rollback integration. No password prompt is necessary for this
research checkpoint, and it is not a reason to repeat the known broken legacy
cancellation test on the host.

## Core-facing descriptor adapter checkpoint

The newer patch adds opt-in `tun.omavless-dns-broker`, requiring Linux, fixed
`Meta`, DNS-off and a newly core-created TUN (not caller-supplied FD mode).
The private sing-tun accessor duplicates the actual attached descriptor under
`SyscallConn.Control`; it never looks up another process's FD or reopens by name.
The fixed root-owned socket is `/run/omavless-dns/control.sock`. Path ancestry,
root SO_PEERCRED and per-packet SCM_CREDENTIALS are checked; received unexpected
rights are closed. Fixed eight-byte frames match the
[Rust corpus](../../crates/omavless-dns-channel/cases.json).

The listener waits for Applying → Ready. `omavless-dns-ready` is computed from
the live lease, cannot be supplied in YAML and is not a configuration-equality
key. Close sends Release and requires Releasing → Released; it always closes the
channel and joins its observer, including failed writes. Loss/recovery/EOF is
not clean release; unexpected loss invalidates readiness and sends SIGTERM to
the core. The future broker must retain/quarantine the original object while a
DNS outcome is unknown. The installed OmaVLESS runtime does **not** consume this
new readiness signal yet.

Real wire tests use disposable sockets and ordinary synthetic-file FDs. They
exercise refusal, credentials, malformed replies, unsolicited completion,
unexpected rights, observer joining and lease loss. Four actual Rust↔Go cases
passed 20 repetitions: acquire/release, initial rejection, recovery-required
release and loss after Ready. Opt-in interop reproduction:

```sh
# First build the test-only fixture in the OmaVLESS checkout.
cargo build --locked -p omavless-dns-channel --example channel_fixture
# Then run in the separately patched core checkout; paths below are local inputs.
OMAVLESS_DNS_INTEROP_SERVER=/absolute/target/debug/examples/channel_fixture \
OMAVLESS_DNS_INTEROP_CORPUS=/absolute/omavless/crates/omavless-dns-channel/cases.json \
  go test -mod=readonly ./listener/sing_tun -run TestSystemDNSRustChannelInterop -count=1
```

An additional whole-core probe uses a real `Meta` TUN and the actual compiled
Rust admission leaf, with fresh user/network/PID/**mount** namespaces. `/run` is
a private tmpfs; `/proc` belongs to the new PID namespace. It never mounts over
host state, reads profiles, changes host routes, or calls a system bus. It proves
pending/refused is not ready, acknowledgement projects ready, listener close
releases through the channel, loss stops the core, and the held TUN survives
core exit until explicit last-FD release. Normal listener close is exercised
through a synchronous TUN-disable reload: upstream's controller starts before
main registers signal handling, so early SIGTERM alone is not a deterministic
normal-close test.

**Fixture Ready/Released are synthetic acknowledgements, not DNS evidence.**
The probe has no resolved writer; it does not certify cleanup of real settings.
Eleven facts passed 20 repeats on Try Omarchy ARM64 with core SHA-256
`3ac82f111167e72caecd320368716d89a5d6c961d09b9d868219a3658f082c8e`
(same Go 1.26.8, CGO off/default tags). The original DNS-off ownership and packet
results above still belong to their original core digest.

```sh
cargo build --locked -p omavless-dns-channel --example kernel_channel
python3 tests/dns_core_broker_probe.py /absolute/patched/core CORE_SHA256 \
  /absolute/target/debug/examples/kernel_channel FIXTURE_SHA256
```

Root service/package enrollment, real typed resolved integration, pristine
baseline policy, systemd-held crash quarantine, recovery/removal, production
build tags, runtime admission and attended host operations remain required.
See [descriptor-store proof](../../docs/development/DNS_FDSTORE.md). This is not
a silently shipped Mihomo fork or a completed no-password implementation.
