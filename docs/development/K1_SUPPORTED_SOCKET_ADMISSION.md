# K1 supported socket-dependency admission

This document retains the immutable #625 capture contract and historical gate
ledger. The separate [retained private lifecycle successor](K1_RETAINED_PRIVATE_LIFECYCLE.md)
owns the new #631 tuple and actual cfg(test) coordinator; #625 evidence does not
transfer lifecycle authority to it.

Source-only successor to immutable #621 `c82044ba6f863af04773caf9986cbdae8e97811f`.
Its first capture remains NONPASS. Separately authorized file observation found
RPCs 0–3 validated and RPC 4 refused exactly the missing `Sockets` field. This
does not prove old helper quiescence, release its reference, or authorize cleanup.
No old unit/process query or retry is added.

## Correct supported representation

`Sockets=` is a service configuration directive, not a Service D-Bus property.
The [v261 Service vtable](https://github.com/systemd/systemd/blob/v261/src/core/dbus-service.c)
has no such getter (raw SHA-256
`c1ba24588aef9686906950ea937c880e0791927ae3456eb096518e3e7b859ca1`).
The [configuration parser](https://github.com/systemd/systemd/blob/v261/src/core/load-fragment.c)
adds Wants/After and TriggeredBy dependencies (raw SHA-256
`16d426f1e5ea8ab3241e3ab9cec216f2748037274fbf07ea4a15d5e82d460e88`).
The [socket loader](https://github.com/systemd/systemd/blob/v261/src/core/socket.c)
adds the inverse Triggers dependency for singleton activation (raw SHA-256
`0d2d1f7e14eff13a7b2dc527188db21c450c4e5fdd32ccd17a6465f218683bdb`).
The [Unit getters](https://github.com/systemd/systemd/blob/v261/src/core/dbus-unit.c)
expose typed dependencies and Names including aliases (raw SHA-256
`80e1b2328ab84338dab095a6496adbf1be5f47d0e7093b2089a347dbcded5a42`).

This generation removes ONLY the nonexistent Service getter requirement.
Unit TriggeredBy and Wants must still be present with exact `as` empty arrays;
Names must be exactly `as [new literal unit]`, and Following exactly `s ""`.
Both initial and post-Unref validation require them. Missing, wrong signatures,
duplicates, aliases and nonempty trigger/following facts refuse. No arbitrary
missing field is converted into empty. The strict configured Dump parser still
rejects Alias, TriggeredBy and Accept Socket labels; no absence-based new parser
or permissive unknown-label rule is introduced.

The same connection/unique owner, exact `261.2-1-arch` version, bounded typed
RPC receipts, strict configured Dump before Version-after, original unit/ELF
FDs, all namespace/capability/exec/environment/OpenFile/FD-store restrictions,
never-started state and terminal uncertainty retention remain mandatory.
Upstream source is not an attestation of every distribution patch: runtime
typed observations and exact version still refuse on unfamiliar representation.

## New fixed tuple and retained predecessors

The new unit is `omavless-k1-supported-socket-admission.service`, stage
`/run/omavless-k1-supported-socket-admission`, with create-only delivery from
`/home/kdk_vm/.cache/k1-supported-socket-stage-v1`. Existing response modules
are changed only on this successor branch; #621's complete commit remains
immutable. The existing creator/LocalReadSession/LockedState writer is reused,
not replaced, and the capture contains no Start operation.

The activation catalog observes old #609 plus exactly four original-FD pinned
artifacts and the exact link/fragment for each of #615 and #621. Only literal
generation IDs choose these two fixed tuples. No arbitrary paths, root prefix,
old process/unit/ref query, cleanup or baseline exclusion is allowed. Original
source/link metadata and hashes remain held/rechecked and visible in the full
before/after catalog. Any uncertainty permanently stops the new invocation.

This is current loaded configuration admission only, not proof against dormant
or future socket activation, hostile root, an atomic Start barrier or full K1.
Any later lifecycle gate must repeat effective admission immediately before its
own action and prove actual child descriptors/isolation before the first socket.
No guest invocation is authorized until full source/native gates, fresh frozen
ELF and acyclic pins, parent/peer review and an exclusive VM lease. No old
failure is promoted by this source work.

## Checkpoint gates

Initial native controls: 26 passed, one ignored VM entry; Python controls:
23 passed. The first native run's test-directory label length failure was
corrected without changing admission behavior; that failed log is retained.
The initial source checkpoint `38b720a31de53e193586dd84ca463965e5610d19`
deliberately carried zero executable/source pins: no previous executable was
eligible for this tuple. Its sealed full source gate passed 663 Python tests
(two skipped), frontend and QML checks. Full Rust gates completed with 129
test summaries, 2,141 passed, zero failed and 84 ignored, including formatting,
strict Clippy, TUI checks and parity. The first full Rust attempt failed three
unrelated Unix-socket tests with explicit `SUN_LEN` errors from an overly long
HOME temporary-directory prefix; the unchanged source passed with a shorter
private HOME prefix. The first unsealed Python run observed a mid-edit pin
transition and is retained as NONPASS, not sealed-head evidence.

The fresh original-FD-frozen ELF is 76,304,224 bytes, mode `0500`, one link,
SHA-256 `6e3c6608ce6f3e3501c83c32d97979bc1219b1040820e69b626c7724bb431507`.
Only subsequent source/executable pins and this ledger change after that native
checkpoint; final source revalidation and parent/peer review remain required.
There has been no VM invocation or lifecycle/admission acceptance.
