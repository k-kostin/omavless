# K1 retained borrowed-lease regression — inactive preparation

This is a fresh developer fixture stacked on the reviewed
[borrowed original-session lease](K1_BORROWED_INVENTORY_LEASE.md).
It does not reuse #631's stage, unit, frozen ELF or acceptance evidence.
No native fixture, manager operation or VM action has been executed for this
checkpoint. Normal runtime ownership, package dependencies and K1 availability
remain unchanged. Every actual effect remains under `cfg(test)`.

The first source checkpoint supplies the restricted native writer, not a
complete runnable delivery. A new fixed retained-manager adapter, unit,
root guard, source pins, independently reviewed build/freeze provenance and
explicit executor authorization are still required. Missing delivery pieces
must not be filled with #631's identity or an arbitrary unit/source path.

## Fixed identity and sequence

- Stage: `/run/omavless-k1-retained-lease-regression`.
- Unit: `omavless-k1-retained-lease-regression.service`.
- Exact ignored test:
  `kernel_observer::creator_lifecycle::retained_lease::manager_retained_lease`.
- Sole writer opt-in: `OMAVLESS_K1_RETAINED_LEASE_WRITER=1`.

The existing restricted isolation checks are shared: retained inherited host
FD is a negative anchor only; a distinct actual namespace, loopback-only
interfaces, root with only CAP_NET_ADMIN, effective filter and no-new-privileges
are mandatory before sockets/effects. A receipt naming the old unit cannot
admit this writer, and the new receipt cannot admit the old writer. Neither
receipt authenticates canonical-host authority.

The primary retained LockedState and real FixtureCreator perform create,
matching-generation atomic replacement, then delete. Replacement must return
a different live creator handle. Three acknowledged effects, full same-session
inventory, absence and status are required. The resulting primary Closed/Retired
record is not reused by the negative case.

A separate retained state and creator then attempt one create. While its actual
inventory lease is held, a second retained socket sends one fixed conditional
atomic create for the empty `k1_retained_lease_cut` table in that private namespace.
This deliberately advances the kernel generation. The original conditioned
create must receive the existing strict BEGIN-only ERESTART classification,
remain poisoned and retain PendingCreate. A separately opened, already-held
reader checks untrusted absence of the target table. The send counter counts
attempts, not successful kernel effects; it cannot be described as a successful
create. No poisoned-session retry, compensating delete or receipt adoption runs.

All effect-bearing objects are installed in a leaked owner before use. Any
unexpected error or panic parks with original namespace/socket/lock owners
retained. The fixture never spawns nft, timeout or other children, calls a
namespace transition using a namespace FD, signals, reaps or removes files.
The expected refusal is not a recovery ticket; its pending state and deliberate
foreign table remain for the separately reviewed whole-lifetime retirement.

## Evidence boundaries

Ordinary controls validate strict pending JSON, disjoint receipt identities and
the literal generation-cut wire. They perform no nftables effects. Compilation
and these controls cannot establish actual kernel refusal, manager ordering,
network preservation or successful lifecycle acceptance. The synthetic
HostEpoch remains fixture vocabulary, not an implementation of CanonicalCreator.
No immutable upstream safe namespace API or trusted production launch gate is
adopted by this work. The earlier #631 and #641 results retain their own heads.
