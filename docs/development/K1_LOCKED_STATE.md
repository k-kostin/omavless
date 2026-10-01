# K1 inactive shared-lock transaction context

This candidate follows [owner/persist lifetime evidence](K1_OWNER_LIFETIME.md)
and the [durable receipt store](K1_RECEIPT_STORE.md). `LockedState` composes the
existing marker writer, receipt writer and transaction planner with a synthetic
[`EffectPort`](K1_EFFECT_PORT.md). It adds no production caller, executor, authenticated socket,
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
authenticate either. Receipt consistency never creates kernel ownership.
The separate effect-only port remains unimplemented in production. It has no
marker/receipt writer responsibility: this context alone owns persistence.
The older coordinator and its receipt-writing port remain separate, with no
blanket adapter. A real namespace/session/provenance adapter remains required.

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

The subsequent process-death gate runs 114 real child-process SIGKILL cases:
38 checkpoints each for create, replace and delete. The child owns the real
private fixture file writers and held directory lock; the parent waits for the
selected checkpoint, proves a competing opener is Busy, kills and reaps that
exact child, then reacquires the lock. No writer destructor, error return or
in-memory poisoning assignment runs after the checkpoint. The checkpoints
include ten transaction boundaries, all eleven receipt-publication boundaries
for both Pending and terminal records, and six marker-publication boundaries.

Restart preserves every publication file, including staging files and guards.
Incomplete transactions refuse status and both arm/disarm retries without any
modeled kernel effect. Stable old state before publication and stable terminal
state after guard removal remain readable. A completed delete retains the
Closed generation fence even when the child dies before returning success.
The test has a ten-second checkpoint deadline and a child kill/reap guard on
failure; it runs automatically with the ordinary crate/workspace suite.

This gate uses a synthetic kernel port, reconstructed from the selected
checkpoint by the parent. It proves destructor-free filesystem/lock restart
behavior, not persistent kernel ownership, packet enforcement or delivery of an
earlier response. It touches only private temporary fixtures; it opens no
installed root state, netlink socket, namespace, service or VPN connection.
Actual exclusive kernel commit/lost acknowledgement paired with file writes,
supported-filesystem power-cut/reboot and installed-host acceptance remain
unrun by this gate. The `owner,persist` orphan
remains untrusted. Canonical namespace and nft-subsystem continuity, orphan
adjudication/provenance, root enrollment/peer authentication, real kernel and
filesystem crash tests, boot ordering, core mark/DNS/firewall integration and
the [K1 host matrix](../roadmap/KILL_SWITCH.md) remain explicit gates.
There is no main merge, package change, live activation or publication implied.

## Opt-in kernel commit / missing acknowledgement fixture

The test-only `locked_state_kernel_crash.rs` adds a narrower real-kernel
composition: PendingCreate is published by the real shared-lock context, an
exclusive empty-table creation can complete through nft, and the effect port
**never returns an ownership identity or verified policy**. Three SIGKILL cuts
cover before creation, after kernel completion/readback but before the effect
returns, and after an explicit uncertain-effect error. The separate namespace
holder survives the writer and inspects the actual table rather than rebuilding
synthetic kernel state from a checkpoint number.

Run only in the coordinated delegated VM:

```sh
OMAVLESS_K1_RECEIPT_VM=1 cargo test --locked -p omavless-netguard --lib \
  locked_state::tests::kernel_crash::kernel_receipt_crash_in_disposable_vm \
  -- --ignored --exact --nocapture
```

The outer test pins its original network namespace, creates a fresh unprivileged
user+network namespace and rechecks the original identity afterwards. The holder
and writer require a different pinned network namespace and only loopback before
every nft subprocess. No interface, address, route, chain, hook, rule, IP probe,
installed state path or service is changed. Internally generated unique `inet`
table names contain only a fixed fixture prefix, holder PID and cut number; no
caller-selected nft syntax is accepted. A separate empty sentinel is preserved.

After each kill, the original directory lock is released. The real PendingCreate
receipt remains; Armed is absent. Status, Arm and Disarm refuse, with zero
effect calls and unchanged file bytes/table metadata. Present tables are always
Foreign/untrusted to the restart port. Fixture cleanup checks exact retained
metadata immediately before deleting its own empty tables in the exclusively
controlled namespace; this is not a production conditional-delete authority or
orphan-recovery path. Namespace teardown is the final isolation boundary.
Subprocesses have bounded deadlines/output, kill/reap guards, and a parked
writer exits on holder death or its own deadline. Raw tool output is not shared.

This does not compose a successful kernel EffectPort: the empty table provides
no packet policy, no live owner socket and no production ownership provenance.
The synthetic epoch identifies only the test namespace, not the canonical host.
The withheld effect response models acknowledgement loss *above* nft; it does
not drop raw netlink ACK packets. The full #416 file-publication SIGKILL matrix
still uses a synthetic kernel. Real replace/delete authority, crashes within
kernel/file publication, power loss, persistent orphan adjudication, root
service/boot integration and the physical-host matrix remain separate gates.
