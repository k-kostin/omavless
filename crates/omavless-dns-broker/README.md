# Experimental fixed-purpose DNS broker

This is a **default-off opt-in candidate**, not released prompt-free support.
It has been installed only as an explicit experimental pair in disposable
Omarchy VMs; the ordinary package does not include or enable it. Compiling or
testing this crate alone changes no installed runtime or machine policy.

`omavless-dns-broker --serve` accepts no path, account, address, shell command or
network operation argument. It can start only as the main process of the fixed
root service, with protected enrollment and pinned system-bus authorities.
See the [review-only unit and enrollment contract](../../tests/dns_broker_host/README.md).

## Composition

The existing transaction model is reused, not replaced by a parallel VPN state
machine. For one authenticated enrolled-UID seqpacket session:

1. Admit the actual single-queue nonpersistent `Meta` TUN descriptor in the
   original namespace. No name-only or numeric-index-only authorization.
2. Require the enrolled `meta-ipv4-v1` exclusive DNS contract and compatible
   empty DNS baseline. Effective readback is not an arbitrary restorable snapshot.
3. Durably record pending intent, retain the actual descriptor in systemd's
   previously empty single-slot store, and verify the transfer/readback chain.
4. Apply only the fixed resolver address, root routing domain and default-route
   policy. Publish Ready only after joined replies, readback, identity checks
   and the durable Active record.
5. For a settled release, recheck ownership, reset the owned link, verify the
   baseline, record cleanup, remove/verify the manager-held descriptor, then
   clear the record. Local references remain held until this finishes.

Unknown D-Bus outcomes, lost identities, untouched-setting drift or uncertain
retention never trigger speculative compensating writes or FD removal. The
manager-held TUN and preserved journal quarantine the old lifetime. Startup
refuses persisted intent or stored descriptors rather than replaying them.
An ordinary channel loss after **joined** application invokes the same verified
cleanup; EOF itself is never cleanup evidence. If the broker is killed mid-write,
the initial recovery contract requires explicit administrator recovery/reboot.
This availability tradeoff is not a kill switch or leak-prevention claim.

## Orderly idle stop (development candidate)

SIGTERM latches a sticky stop request using the already locked signal-hook
dependency. The handler performs no DNS, journal, channel or FD-store cleanup.
Only between complete serialized sessions, or after accepting a channel before
its lease admission begins, can the broker stop admitting work and attempt a
normal zero exit. Idle acceptance is polled with a one-second absolute budget;
the channel's existing ten-second accept API and packet admission stay unchanged.

Zero requires a separate read-only proof under one five-second deadline:
original service/bus identities and socket ACL; the original locked journal
directory's safe metadata and named identity, fresh no-follow absence of both
record and staging names, no cached pending/poisoned state; the original
manager's fixed retention policy, zero count and complete empty dump; then fresh
identity/journal/ACL checks and the final deadline. No fresh Journal, flock
reacquisition, RevertLink, descriptor removal or repair is used. Ordinary
Listener destruction may unlink its own matching socket node after this
positive idle proof; that is IPC teardown, not DNS cleanup evidence.

Once lease admission starts, apply, active wait, release and quarantine behavior
remain unchanged. A stop request does not cancel an active session or turn it
into release. A peer that never releases can remain active until the manager's
existing stop policy kills the service; signal death is not zero and retained
manager state still requires the existing recovery procedure. There is no
universal six-second exit guarantee with slow/unavailable bus replies.

This candidate addresses the missing orderly-stop source path in the exact
`c4e800425243c1b02165f82153e4bf418fe465e6` broker. It does not identify a stopped
T3 fixture's actual wait status or cause. Exact native pins, independent review
and a fresh isolated composition are still required; no stopped scope is retried.

The only file access offered to the enrolled user is a fixed socket ACL. Its
parent and enrollment remain root-owned. Ordinary applications of the enrolled
UID share this narrow authority; executable identity is not claimed. The broker
needs `CAP_NET_ADMIN` for TUN namespace inspection, but exposes no routing or
arbitrary ioctl API. Root administrators remain outside this threat boundary.

## Time and resource bounds

Apply and release use a shared absolute 30-second budget across typed bus calls
and retention notifications; each individual bus call also has a two-second
limit. Idle identity checks share five seconds. Expired budgets prevent new
dispatch; timeout after dispatch is unknown, not cancellation. Resetting a
budget cannot clear poisoned transport or quarantine state.

The reviewed core's 40-second handshake budget accommodates an idle check plus
release. Runtime readiness waits for the actual managed-DNS flags and Ready,
not just the controller or TUN. This is configuration readiness, not internet
health. The candidate runtime stop budget is 55 seconds, leaving 44 seconds
before forced termination, beyond the core's 40-second release budget. Legacy
10-second startup / 5-second stop policy is unchanged. The attended test requires
an explicitly SHA-pinned compatible core: unknown YAML keys accepted by a stock
core are not proof of pre-spawn broker support.

Wire requests are the fixed eight-byte credentialed channel protocol with one
TUN descriptor only on Acquire. D-Bus accepted replies are bounded to 16 KiB;
zbus's larger upstream wire allocation is additionally constrained by the unit's
resource policy. The private journal is at most 512 bytes with atomic writes,
fsync ordering, duplicate-key refusal and an exclusive directory lock.

## Evidence and remaining gates

Unit tests cover admission, ACLs, journal crash boundaries and transaction
ordering. The [actual composition probe](../../tests/DNS_BROKER_COMPOSITION.md)
uses a real kernel TUN and real Rust/ancillary/D-Bus transports in disposable
namespaces, with independent resolved/systemd fixtures. A separate real
user-systemd experiment tests FD-store survival. Later installed experimental
ARM64 and x86_64 evidence is recorded in the
[current status](../../docs/roadmap/CURRENT_STATUS.md): DNS readback/HTTPS,
same/other-user access, normal release, crash retention and several actual
ALPM refusal/removal paths have been exercised. Agent-attended VM diagnostics
do not replace the owner's pre-main acceptance.

Before release: resolve default legacy #132 versus mandatory managed ownership,
complete remaining in-flight negatives, reviewed normal delivery/enrollment
and rollback, firewall prerequisites, exact package/frontend pairing and
owner-attended acceptance. Do not close #270, promote RC or change marketplace
claims on the strength of the experimental checkpoint.
