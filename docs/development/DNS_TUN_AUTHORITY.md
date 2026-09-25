# DNS-0: TUN authority experiments and selected DNS-only boundary

September 25, 2026, review-only #270/#132 continuation. No installed helper,
host policy, route, DNS setting or core changed. Main and RC unchanged.

## A TUN FD is not just packet access

The earlier kernel experiment showed that a held FD cannot prevent a
CAP_NET_ADMIN actor deleting/recreating the interface. This experiment instead
tests a client with **all** effective/permitted/inheritable/bounding/ambient
capabilities empty and NoNewPrivs set.

On Try Omarchy ARM64 / Linux 7.2.0-2, it cannot create/rename/delete/change link
state through netlink, but **can set TUN owner and persistence through its FD**.
These operations have no additional CAP_NET_ADMIN check on an attached FD;
see the [kernel ioctl implementation](https://github.com/torvalds/linux/blob/v7.2/drivers/net/tun.c).
The persistent link can outlive last-FD closure. Cap removal alone is not enough.

## Restriction tested, not installed

`tests/dns_tun_authority_probe.py` checks fresh user/network/PID namespaces and
loopback-only inventory before creating its nonpersistent TUN. The owner keeps
the original FD. A worker receives it through inheritance and loses capabilities
using setpriv. Before mutation attempts it checks **its own** proc capability
sets and NoNewPrivs. No PAM, system bus, private store, routes or DNS are used.

| Operation | Empty capabilities only | Plus ioctl filter |
| --- | --- | --- |
| Read held TUN identity | allowed | allowed |
| Create another TUN | EPERM | EPERM |
| Set owner / persistence | succeeds | EPERM |
| Export FD with SCM_RIGHTS | succeeds | EPERM |
| Delete / rename / bring link up | EPERM | EPERM |
| Client exit while creator holds FD | link survives | link survives |
| Creator clears persistence and closes FD | link removed | link removed |

The owner clears persistence through **its held FD**, including after the
negative control; never by caller-supplied name/index. Namespace teardown is
outer containment on failure.

The experimental filter in `tests/dns_tun_fd_policy.py` checks native syscall
architecture, rejects foreign/compat/x32 ABI and permits only TUNGETIFF,
read-only TCGETS and descriptor-only FIOCLEX among ioctl requests. Mutating or
unknown requests fail EPERM independently of FD number, preventing a dup-FD
bypass. All three io_uring entry points are denied to avoid an alternate command
submission path. Both sendmsg/sendmmsg are also denied: an ioctl-only filter
would let a consumer send SCM_RIGHTS to an unfiltered peer. A real local ancillary
transfer is the positive control; the filtered sender gets EPERM. This is not
cross-UID admission evidence. Blocking those syscalls also affects ordinary UDP
ancillary traffic and netlink tools: it is an experiment, not a production-ready
policy. Other syscalls are unchanged: **not a complete core sandbox**.
TCGETS/FIOCLEX support libc/Python, not TUN administration. Only the disposable
worker/core child receives the filter, never the host shell or installed service.

Independent interpreter tests cover AArch64 and x86-64 filter bytecode.
**Actual-kernel execution evidence is ARM64 only.**

## Actual core behind this boundary

The [review-only DNS-off core](../../tests/core_dns_adapter/README.md), SHA-256
`62d2c18f0ccaed78b67360641ebc5da86d04bb04400d0a04eaba6c21d0f60f3d`, starts and
stops cleanly with an externally configured/up TUN, empty capability sets and
NoNewPrivs verified before exec, the inherited filter and no recorded DNS calls.
The ownership probe now passes **13** facts, including restricted-core readiness
and clean close. Authority positive/negative controls and cleanup pass.

This is synthetic DIRECT-only system-stack startup/close, **not** VPN traffic,
SO_MARK, production gVisor, IPv6, real routing/DNS or a secure socket/lease API.
Kernel-enforced restrictions and cooperative exact-core conformance are distinct.

Opt-in reproduction; ordinary tests only use mocks:

```sh
python3 tests/dns_tun_authority_probe.py
python3 tests/dns_core_ownership_probe.py /absolute/test/core EXACT_SHA256
```

## Investigated broader alternative — not the selected prerequisite

The initial experiment suggested **root-created TUN + restricted consumer**.
That remains a broader alternative, **not a required production migration**:

1. Root-owned broker creates/holds the fixed managed TUN and owns its bounded
   address/route setup. Mihomo FD mode skips that setup: this is a separate host
   ownership change, not silently adding route privilege to a DNS-only helper.
2. Runtime/core receive packet-use authority. Production syscall/capability,
   FD-transfer and peer policy need testing. Same mapped UID in this fixture
   is **not** evidence of cross-UID authentication. Prevent external FD extraction
   (for example ptrace/pidfd_getfd) with a reviewed process/UID and dumpability
   boundary; filtering the consumer cannot restrict another process's syscalls.
   Verify that required UDP ancillary sends work or design a different boundary;
   a startup-only pass does not establish transport compatibility.
3. Derive identity from the held kernel object/fixed policy. Check identity
   before/after effects and observe link/resolved restarts. Resolved still takes
   an index, not a TUN FD; no atomic lease API has been invented here.
4. Root/CAP_NET_ADMIN actors can still interfere. Explicitly state this threat
   boundary; it does not defeat administrators or already-privileged managers.
   Existing externally executable capability-bearing binaries are separate
   privilege paths to audit, not removed by sandboxing one child.
5. Exclusive creation supports a known baseline but does not prove resolved's
   pristine state/restoration. Refuse foreign ownership/configuration rather
   than reverting unrelated settings.
6. Implement typed D-Bus transactions, crash/replay, enrollment/removal/recovery
   and lifecycle binding only with those facts established; then attended
   exact-package DNS/routing/HTTPS acceptance.

Do not install this fixture as a broker, accept caller-selected filters/routes
or close #270. It rejects an unsafe shortcut and proves a constrained consumer
path without changing the running VPN.

## Narrower preferred boundary after exact-core review

A capability-bearing Mihomo executable does not give its caller arbitrary
CAP_NET_ADMIN code execution. The inspected stock sing-tun implementation does
not expose a generic caller-selected TUNSETOWNER/TUNSETPERSIST/LinkDel API.
The destructive kernel control above was a separately privileged experimental
actor. Do not convert that fact into an invented stock-core attack or require
root route ownership solely because file capabilities exist.

The preferred DNS-only candidate keeps TUN creation/address/routes in Mihomo.
A reviewed adapter duplicates its actual, newly attached, fixed-name TUN FD and
sends it to the broker. The broker checks the real character device, TUN flags,
single queue/nonpersistence, kernel network namespace (TUNGETDEVNETNS), fixed
name and index, retaining the original object across DNS effects. A second
single-queue attach fails EBUSY; ordinary creator closure does not destroy the
held device. Administrators/already-privileged link managers remain outside
this boundary. Same-user admission is limited resource authority, not proof of
application identity or protection against a compromised enrolled account.

`tests/dns_tun_lease_probe.py` exercises 15 kernel facts in fresh unprivileged
namespaces; the independent compiled `omavless-dns-tun` leaf exercises 11 facts
using its actual Rust admission code. Both passed on ARM64. Wrong device/type,
name, persistence, multiqueue, detached object and foreign namespace are refused.
These checks establish object admission, **not** pristine resolved settings,
atomic D-Bus, crash-safe lifetime or recovery. In particular, losing both broker
and core FDs while a D-Bus operation is queued still needs retained lifetime
through process death. Per-call rechecks do not solve that race.

The kernel leaf has a visible, local unsafe exception for exactly two typed
ioctls; the workspace forbid is unchanged. Its [review rationale](../../crates/omavless-dns-tun/README.md)
and negative tests are part of this boundary, not hidden implementation detail.
The [descriptor channel](../../crates/omavless-dns-channel/README.md) authenticates
kernel credentials per packet and retains received rights on transaction errors.
Neither crate is installed or imported by the production runtime.

The separate packet probe also passed ten facts with the earlier review core:
actual TCP and UDP echo pass through the TUN in both ordinary and restricted
consumer cases, with marked client/unmarked core route separation and measured
TUN counters. All peers/routes/sysctls are synthetic and namespace-local. This
does not establish production gVisor, external providers, IPv6 or host DNS.

```sh
python3 tests/dns_tun_lease_probe.py
python3 tests/dns_core_packet_probe.py /absolute/test/core EXACT_SHA256
```

Deterministic validation at this checkpoint includes 17 authority/filter tests
and 17 core-ownership guard tests; the existing TUN/reload probes add 23. These
57 checks use mocks or local descriptor operations, never host networking. The
ordinary full suite and exact-head CI are recorded in PR #295.
