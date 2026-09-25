# Experimental core DNS ownership adapter — not an installed dependency

This directory retains the minimal **unpublished, review-only** Mihomo patch
needed to test DNS-0 for #270. It is not applied by install, package, release,
runtime, plugin or ordinary CI. No fork distribution or upstream acceptance is
implied. Do not add the proposed option to production configuration yet.

Upstream: MetaCubeX/mihomo, exact source
`ab405bad5beeeac8b003bb01f60f134f6df54471` (1.19.31), with its locked
`github.com/metacubex/sing-tun v0.4.24`. The patch is subject to the upstream
project's GPL-3.0 license; retaining a small review patch does not relicense the
upstream source as OmaVLESS's MIT code.

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
