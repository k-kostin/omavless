# Resolved wire conformance boundary (uninstalled)

This crate exercises actual typed D-Bus messages against a mock resolved service
on a disposable `dbus-daemon`. It is not a root helper, lease implementation,
installed runtime integration or permission policy. Development broker
composition may use its trusted-library bridge, but there is no binary or
system/session-bus discovery in this crate. Tests discover neither the host bus
nor the user's private store. No host resolver call is made by this suite.

## Fixed boundary

The typed semantic methods use only resolved's `SetLinkDNS(i, a(iay))`,
`SetLinkDomains(i, a(sb))`, `SetLinkDefaultRoute(i, b)` and `RevertLink(i)` at
the fixed Manager path/interface. Reads use separate uncached `Properties.Get`
calls for Link DNS, DNSEx, Domains, DefaultRoute, LLMNR, MulticastDNS,
DNSOverTLS, DNSSEC and DNSSECNegativeTrustAnchors. Tests verify
these signatures and exact values through independently implemented mock
methods, not a boolean transport stub. The test admission discovers the well-known
service's unique owner and pins that owner; a replacement cannot be silently
adopted during the same instance.

The synthetic fixed policy is IPv4 `198.18.0.2`, route-only root domain and
default route true. This is only a credential-free namespace policy, **not an
accepted production TUN/address/routing policy**. There are no public arbitrary
DNS values, domains, service paths, commands or method arguments. The internal
interface index will eventually have to come from a trusted creator-held lease,
not client IPC or a name-based lookup.

Calls request no interactive authorization. A future enrolled privileged
adapter must already have narrow authority; a missing permission is an error,
not a request to launch an authentication dialog in the background.

## Bounds and uncertainty

- Every full asynchronous method send/reply future is wrapped in a finite
  deadline, presented as a synchronous semantic method. Tests use two seconds
  normally and forty milliseconds for a deliberately late response.
- zbus's own blocking `method_timeout` alone is insufficient: its timer begins
  after sending. The outer `async-io` timer covers that send as well.
- Accepted reply size is 16 KiB **before body decoding**, with no received FDs;
  DNS/DNSEx/domain/negative-trust-anchor collections have at most eight entries,
  IP addresses exactly four or sixteen bytes and ASCII domains at most 253 bytes
  with labels capped at 63 bytes. Empty DNSEx server names are allowed; nonempty
  names obey the same domain bounds. Extended and legacy address projections
  must agree. DNSEx port/name cannot disappear behind a legacy DNS-only match:
  fixed-policy readback requires the exact zero/default port and empty name
  produced by SetLinkDNS. Multicast/TLS/DNSSEC strings accept only known enums.
  All public errors are
  fixed codes/messages and retain no remote error string or target.
- This acceptance cap is **not a 16-KiB wire-allocation guarantee**: zbus first
  receives the message under its upstream 128-MiB ceiling. Private tests trust
  their own bus/service. Production use still requires reviewed authenticated
  root bus/service admission and a deliberate resource-limit decision; do not
  expose this crate to arbitrary socket peers based on its decoding cap.
- A write timeout, disconnected bus, malformed success or arbitrary service
  error leaves the effect unknown and permanently blocks further writes on
  that instance, including automatic compensation. The timeout test proves
  the remote mutation can arrive later. Known authorization refusal is separate;
  a refused second operation does not undo an earlier successful write.

`Observation` contains effective private values, with redacted Debug and no
serialization or value accessor. It is **not a reversible snapshot**. Reads are
separate requests, not an atomic snapshot; future transaction code must also
budget the entire observation, not just each bounded method call.

## Sealed owned-link baseline

The raw `capture_baseline` requires `ExclusiveOwnership`, whose fields are private
and which deliberately has **no public constructor**. The managed bridge mints
it internally only under its trusted caller's explicit ownership contract; this
is not an independent attestation of that contract. Composition must prove
the actual retained TUN, fresh/exclusive DNS ownership and reset entitlement;
an empty property set, same UID or matching interface name cannot mint it.

The returned opaque `Baseline` is bound to the pinned unique owner, link path
and internal index. It records comparison values, not setters or arbitrary
restoration arguments. Capture rejects nonempty DNS/DNSEx/domains/negative trust
anchors and an effective DefaultRoute of true. The first fixed plaintext/fake-IP
policy accepts only DNSOverTLS=no and DNSSEC=no. TLS=yes is incompatible with
the plaintext virtual resolver; TLS=opportunistic and DNSSEC=yes/allow-downgrade
remain separate compatibility-review cases. **No additional privileged setter
turns these settings off to force acceptance.** These conservative refusals do
not mean the corresponding OS features are insecure or generally unsupported.

`verify_policy_preserves_baseline` checks both the exact applied DNS policy and
all unmodified effective settings. `verify_reset` checks empty DNS and unchanged
LLMNR, mDNS, TLS, DNSSEC and trust-anchor observations. Both refuse another
service/link binding and any already unknown write; a fresh successful read
cannot clear quarantine. They dispatch no recovery mutation. The underlying
Revert primitive still needs the independent ownership/completion proof.

Public DefaultRoute hides automatic versus explicit configuration; effective
LLMNR/mDNS/DNSSEC/TLS also do not expose every inherited-versus-explicit choice.
Revert resets these properties as well as DNS servers/domains/anchors. Therefore
the additional reads improve refusal and detect unintended reset changes, but
**do not prove a freshly created link or authorize arbitrary snapshot restore**.
If reset changes an unmodified field, verification fails even when Revert was
acknowledged. Production concurrent-writer exclusion, default inheritance,
kernel identity/reuse and daemon restart remain composition prerequisites.

The deterministic tests use real private-bus messages, including extended
port/name mismatches, inconsistent DNS projections, all enum/collection/domain
bounds, maximum legal structures, nonempty-baseline refusal, conservative policy
refusal, per-field reset mismatches and unknown-outcome quarantine. No real
system bus, host DNS or installer is used.

## Trusted managed-link composition

`ManagedResolved::from_admitted_parts(Connection, pinned_owner, HeldTun)` is a
library API for the development broker, **not a privileged IPC constructor**.
It performs no bus discovery and accepts no interface name/index, DNS value or
caller-selected object path. The caller's root context must already verify its
fixed system-bus connection, permitted service identity and explicit enrollment
policy `meta-ipv4-v1`. Merely calling a library does not grant an unprivileged
process root permissions; calls still prohibit interactive authorization.

The bridge owns the actual admitted HeldTun, derives its index internally,
checks the current `resolve1` unique owner and resolves only the fixed GetLink
path for that index. Each semantic operation checks kernel identity and owner
before and after dispatch. Failed checks permanently poison the wrapper;
unknown writes remain blocked. The raw transport is not exposed by the wrapper.
`capture_reserved_baseline` names the trusted caller's explicit DNS reservation,
not a claim that passing an arbitrary matching descriptor proves fresh ownership.

These checks are **not atomic with D-Bus**, not a late-call fence, and not
descriptor retention across broker death. Root composition must preserve the
independent manager-held descriptor and durable Pending journal before writes,
serialize mutations and handle quarantine. The caller must also budget the
entire transaction: the two-second deadline is per wire operation, not an upper
bound on all multi-property reads together. Dropping this wrapper only closes
its retained descriptor; it is not cleanup proof.

Constructor/guard tests use the same real GetLink/owner-check wire code on a
private bus and a private kernel-check fault seam. The public constructor accepts
only actual HeldTun, not that seam. Real kernel admission is independently
covered by the TUN leaf and namespace composition tests; the private-bus tests
do not masquerade as real-kernel or host-DNS acceptance.

## Dependencies and checks

Untouched-field drift (LLMNR, mDNS, DNS-over-TLS, DNSSEC or negative trust
anchors) is distinct from a wrong fixed-policy readback. It invalidates reserved
ownership and permanently blocks subsequent managed writes. Whole-link reset
requires the opaque baseline and rechecks those untouched fields before dispatch;
the broker must quarantine, not erase a foreign change. That comparison is not
atomic against administrators and never turns unknown write completion into a
safe compensation boundary. Real Host composition covers drift both during
apply and after Ready; see `tests/DNS_BROKER_COMPOSITION.md`.

Direct transport dependencies are pinned to zbus 5.19.0 (MIT), async-io 2.6.0
and futures-lite 2.6.1 (MIT/Apache-2.0); Cargo.lock pins their full graph. zbus
uses its default async-io executor plus blocking connection setup. No native
libdbus, shell adapter, busctl or resolvectl dependency is added. The existing
workspace `unsafe_code = forbid` applies to our code, not a claim that all
transitive libraries contain no unsafe internals. Tests additionally use
tempfile for private disposable directories and require `dbus-daemon` as a
developer-only executable. Missing dbus-daemon fails rather than reporting PASS.

```sh
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
  cargo test --locked -p omavless-dns-resolved
```

Sources checked for this boundary:

- [systemd v261 resolved API](https://github.com/systemd/systemd/blob/v261/man/org.freedesktop.resolve1.xml)
- [systemd v261 DNSEx projection](https://github.com/systemd/systemd/blob/v261/src/resolve/resolved-bus.c#L1271)
- [systemd v261 whole-link reset and effective properties](https://github.com/systemd/systemd/blob/v261/src/resolve/resolved-link.c#L69)
- [zbus 5.19.0 connection implementation](https://docs.rs/zbus/5.19.0/src/zbus/connection/mod.rs.html)
- [zbus blocking builder](https://docs.rs/zbus/5.19.0/zbus/blocking/connection/struct.Builder.html)

Before host integration: prove enrolled peer/service authentication, actual
kernel lease, exclusive core DNS-off, pristine-link ownership and fixed route
policy; integrate serialized transaction/recovery with real state; review
resource caps, package/enrollment/removal and run attended host acceptance.
This crate does not close #270 or authorize any privileged installation.
