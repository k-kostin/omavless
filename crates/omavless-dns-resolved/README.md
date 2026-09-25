# Resolved wire conformance boundary (uninstalled)

This crate exercises actual typed D-Bus messages against a mock resolved service
on a disposable `dbus-daemon`. It is not a root helper, lease implementation,
runtime integration or permission policy. No production crate depends on it;
there is no binary, system/session-bus constructor or public constructor at all.
Only its in-crate tests create a transport. They discover neither the host bus
nor the user's private store. No host resolver call is made.

## Fixed boundary

The typed semantic methods use only resolved's `SetLinkDNS(i, a(iay))`,
`SetLinkDomains(i, a(sb))`, `SetLinkDefaultRoute(i, b)` and `RevertLink(i)` at
the fixed Manager path/interface. Reads use separate uncached `Properties.Get`
calls for the fixed Link DNS, Domains and DefaultRoute properties. Tests verify
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
  DNS/domain collections have at most eight entries, IP addresses exactly four
  or sixteen bytes and ASCII domains at most 253 bytes. All public errors are
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
serialization or value accessor. It is **not a reversible snapshot**. Public
DefaultRoute hides automatic versus explicit configuration; a whole-link Revert
resets more than these three properties. Only the disposable mock's known
pristine state authorizes Revert in these tests. Production restoration,
concurrent writers, link identity/reuse and daemon restart are not solved here.

## Dependencies and checks

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
- [zbus 5.19.0 connection implementation](https://docs.rs/zbus/5.19.0/src/zbus/connection/mod.rs.html)
- [zbus blocking builder](https://docs.rs/zbus/5.19.0/zbus/blocking/connection/struct.Builder.html)

Before host integration: prove enrolled peer/service authentication, actual
kernel lease, exclusive core DNS-off, pristine-link ownership and fixed route
policy; integrate serialized transaction/recovery with real state; review
resource caps, package/enrollment/removal and run attended host acceptance.
This crate does not close #270 or authorize any privileged installation.
