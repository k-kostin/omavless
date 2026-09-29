# K1 inactive shared-lock transaction context

This candidate follows [owner/persist lifetime evidence](K1_OWNER_LIFETIME.md)
and the [durable receipt store](K1_RECEIPT_STORE.md). `LockedState` composes the
existing marker writer, receipt writer and transaction planner with a synthetic
`KernelPort`. It adds no production caller, executor, authenticated socket,
service, provisioning or activation. K1 remains unavailable.

## One lock and strict admission

The context owns `ReceiptStore`, which owns the original `RootStateStore` and
its pinned-directory flock. `from_root` moves that owner; it does not duplicate,
drop or reacquire the lock. There is no public mutable accessor for either
record. The same lock covers initial observation, receipt and marker writes,
kernel calls, readback and final response validation. Fixed paths, enrollment,
permissions, no-follow checks and publication guard semantics are unchanged.

Only three completely observed stable combinations are admitted:

| Marker | Receipt | Independently observed kernel |
| --- | --- | --- |
| Missing | Missing | Absent |
| Armed | Live in current epoch | Proven-owned Full VPN policy with matching identity |
| Closed | Retired in current epoch | Absent |

Other combinations refuse without repair. That includes pending receipts,
unsafe storage, invalid markers, unknown/emergency policy, old epochs,
missing ownership, foreign tables and absent protection with Armed intent.
This is intentionally narrower than the older marker-only coordinator's
reconciliation model. There is no reconciliation or emergency-recovery entry.

The namespace observation and kernel identity are still **independent input
proof obligations**, supplied only by synthetic tests here. This context cannot
authenticate either. Receipt consistency never creates `TrustedTableIdentity`.
The abstract port remains unimplemented in production; its future durable
identity/executor contract must be reviewed alongside this context before any
real integration. In particular, two production receipt writers must not be
introduced to satisfy the older port's durability contract.

## Transaction ordering

The first operation uses 1; each later mutation uses the retained receipt's
operation plus one with checked arithmetic. Exhaustion refuses before writes.
Status and a completed exact disarm retry neither allocate nor write. The
operation counter is distinct from the existing Closed generation fence.
Receipt retirement retains its counter and never changes that fence.

The context derives every phase from the admitted request and current state:

- Arm from absence: PendingCreate → exclusive synthetic create and full
  readback → durable Armed → Live.
- Arm with independent live ownership: PendingReplace retaining the old
  handle → conditional synthetic replacement and full readback → durable
  Armed → Live with the returned, independently checked handle.
- Disarm: PendingDelete retaining the old handle → durable Closed → conditional
  synthetic delete and verified absence → Retired.

Pending is durably published before any marker/kernel effect and remains until
all transaction effects finish. Only the same invocation may publish its
terminal phase with the same operation number. Every effect reobserves both
records and the kernel; final success rechecks the complete stable combination.
An uncertain effect, write or final observation poisons this instance. Restart
refuses pending state regardless of the observed kernel result; it never
promotes, retires, retries or removes an interrupted receipt automatically.

A crash before Pending publication can leave the old stable state. A crash
after terminal publication can leave the new stable state, which a later
status can observe without proving delivery of the earlier acknowledgement.
Filesystem/kernel commits are still separate. No exactly-once response,
power-loss atomicity or automatic crash recovery is claimed.

## Evidence and remaining gates

Unprivileged tests use private temporary directories and a synthetic kernel.
They check competing lock acquisition during observations, effects and fault
checkpoints; create/replace/delete ordering; Closed and operation fences across
reopen; ten interruption boundaries for arm/disarm; lost kernel replies;
readback drift; missing/foreign ownership; namespace/boot changes; pending and
cross-record mismatches; exhaustion; and marker-write failure before deletion.
The composed fault matrix injects all eleven receipt-publication boundaries at
both Pending and terminal publication, plus six marker-publication boundaries,
for create, replace and delete (84 cases). Existing receipt-store tests retain
unsafe files, rebinding, contention and stage/guard preservation coverage.

These are deterministic composition checks, not VM packet, process-SIGKILL,
filesystem power-cut or installed-host acceptance. The `owner,persist` orphan
remains untrusted. Canonical namespace and nft-subsystem continuity, orphan
adjudication/provenance, root enrollment/peer authentication, real kernel and
filesystem crash tests, boot ordering, core mark/DNS/firewall integration and
the [K1 host matrix](../roadmap/KILL_SWITCH.md) remain explicit gates.
There is no main merge, package change, live activation or publication implied.
