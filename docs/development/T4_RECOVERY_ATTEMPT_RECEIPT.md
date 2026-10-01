# T4 recovery-attempt receipt model

Status: test-only protocol model, stacked on the inactive network-transition
planner. The implementation is compiled only under `cfg(test)` and has no
production caller, production filesystem adapter, event subscription, timer, retry worker,
IPC method, settings change or VPN effect. A separate test-only temporary-file
fixture now composes this protocol with the existing private atomic writer and
migration lock; its scope and crash limitations are recorded below.

## Problem and bounded result

`CandidateOnce` in `network_transition_plan` relies on `Attempt::NoneProven`.
An absent receipt cannot prove this: a machine that never created a receipt and
a machine that lost its receipt after a recovery look identical. Automatically
creating an empty receipt on load failure would turn a crash or deleted file
into another connection attempt.

The model therefore starts only from an **already established, durable Ready
record**, matched to the exact boot, owner instance, ownership generation,
desired revision and network epoch. There is no automatic initializer, reset,
epoch replacement or rearming API. A new owner instance cannot adopt an old
Ready record. Missing, malformed, unsupported, reserved or finished receipts
never produce `NoneProven`.

The modeled transaction is:

1. Poison the owner-local admission handle before any fallible work.
2. Read the existing exact Ready receipt and fresh attributed current facts.
3. Re-evaluate the advisory planner, including desired On, quiet/deadline,
   mutation idleness, proved-empty ownership and recovery-safety evidence.
4. Compare and durably replace Ready with Reserved under the same mutation
   lease. A successful write acknowledgement means the reservation survives
   subsequent modeled crashes. Failure, including a write which took effect
   before reporting failure, stops the transaction.
5. Read back Reserved and repeat the complete owner/desired/network fence and
   current admission checks immediately before the synthetic effect.
6. After one successful synthetic effect, replace Reserved with Finished.
   Failure or uncertain completion leaves Reserved or Finished; both deny any
   further automatic attempt. No receipt is removed on success or failure.

Off or a newer revision discovered after reservation consumes that admission
without an effect. This favors missed automatic recovery over a late Connect.
The global owner lease must serialize actual Disconnect/profile/mode changes
and owner revocation through the effect boundary; two ordinary reads alone
cannot provide that guarantee.

## Crucial lifetime boundary

If reservation persistence reports an error, the durable state may still be
Ready. This does not permit another attempt during the same owner lifetime:
the admission handle stays poisoned. Construct it **once per owner process**,
never once per notification or worker retry. A process restart must mint a new
owner instance and cannot reuse the old handle or instance identity. The old
Ready then fails the exact-instance fence even if the boot and desired state
are unchanged.

The test model intentionally has private construction and no cloning, but it
does not implement the production singleton which must enforce this lifetime.
The durable adapter and owner integration remain required gates. Persisting
only `Reserved`, while casually recreating an admission object on errors, does
not satisfy this contract.

## Storage and provenance limits

The model's `Disk` represents crash-surviving abstract state, not a real
filesystem. Its compare-and-replace contract assumes one pinned, continuously
held mutation lease. The tests inject failure both before and after each
journal boundary, including persisted-but-unacknowledged reservation and
completion. This checks transaction ordering and refusal; it does **not** prove
fsync behavior, path safety, process-crash handling or a working VPN.

The bounded direct-struct decoder rejects duplicate keys, unknown members,
unsupported schemas, zero identity fields and oversized inputs without raw
input in errors. Receipt content is fixed identifiers and enums: no profile,
server, subscription, address, SSID, private credential or host command.

Production activation still requires:

- A trusted event source and non-reused boot/owner/network identities; this
  model does not accept a link notification as proof of a new stable epoch.
- A reviewed explicit provisioning/reconciliation path that establishes Ready
  together with authoritative desired state. It must distinguish first use,
  loss and rollback; absence alone is insufficient. No such path is provided.
- A pinned private storage implementation, strict raw parsing, held lock,
  atomic compare/publication, durable acknowledgement, and crash tests around
  every write/sync/publication boundary. A successful read must not treat an
  unsynced replacement from another failed writer as a durable Ready anchor.
- One owner-local uncertainty latch and fresh instance on restart, with global
  manual-recovery/desired-state fencing. Backup restore/downgrade must not
  resurrect an old Ready anchor; replay protection is not proved here.
- A composition which rechecks owned core/TUN, DNS/routes/protection, foreign
  VPN ambiguity and current intent through the effect boundary using the real
  coordinator. No external reachability result may substitute for ownership.
- Separately justified rearming after a later stable epoch or explicit owner
  reconciliation. An unresolved Reserved attempt blocks later epochs too.

The existing process-start reconciler is unchanged. Any future event worker
must share its attempt/recovery barriers before this can claim cross-trigger
at-most-once behavior. Neither an owner restart nor startup reconciliation is
implemented by this model.

## Evidence scope

### Real-file process-crash fixture

`network_recovery_receipt_files.rs` is reachable only from the `cfg(test)`
receipt module. It creates synthetic Ready state in an exclusive private test
directory, holds the real `MigrationLock`, and uses the existing
`omavless_store::atomic_replace_private` writer for Reserved and Finished.
The effect is only a synced counter byte in that same disposable directory.
Nothing reads the user's store or calls a service/controller/network adapter.

Six rendezvous points cover immediately before/after reservation publication,
before/after the synthetic effect, and before/after completion publication.
The parent test kills a real child with SIGKILL at each point and reaps it.
The restarted fixture obtains the released lock, checks the exact surviving
phase and uses a new owner instance: even the pre-reservation Ready record
cannot authorize it. After any effect the file is Reserved or Finished, never
Ready. A separate six-point error matrix verifies that same-owner retries stay
poisoned even when an error precedes the reservation. Success/duplicate,
lock-contention, missing/malformed/oversized/version-mismatched/stale-instance,
unsafe-mode and symlink fixtures check refusal without initialization or repair.

This closes **process death at acknowledged writer-call boundaries in a trusted
temporary directory**, not the full durable-adapter gate. It does not inject
failure inside the writer's write/fsync/rename sequence, simulate machine power
loss, prove rollback resistance or attribute an unsynced foreign Ready file.
The fixture's path checks are not a pinned-dirfd production storage design and
do not establish safety against concurrent same-user path replacement. Ready
is seeded solely by test setup; provisioning, restart identity provenance,
cross-trigger startup/event serialization and real host admission are still
absent. The ignored child entry point is explicitly exercised by its ordinary
parent test; it is not an unrun installed-host gate.

### Abstract protocol fixtures

Synthetic tests cover missing/lost receipts; exact Ready admission; duplicate
hints; terminal and uncertain attempts; changed boot/owner/generation/desired/
network fences before and after reservation; Off, busy, healthy, uncertain,
unsafe, too-early and expired observations; and failure around each abstract
storage boundary. Effects are counters, not controller, service or network
calls. Current production behavior is unchanged, so no installed-host gate is
claimed. Real suspend/NIC acceptance remains the separate bare-metal gate in
the [acceptance policy](../roadmap/ACCEPTANCE_ENVIRONMENTS.md).
