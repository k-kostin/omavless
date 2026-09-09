# R5 offline login-intent transaction

This checkpoint applies the [login planner](R5_LOGIN_INTENT_PLANNER.md) to real
private files under real nonblocking locks. It is still **unreachable from
production**: no CLI/IPC method, systemd login unit or daemon startup caller is
registered. Python remains the installed startup owner and rollback reference.

## Transaction boundary

The future host supplies a trusted user-manager-lifetime trigger identity and
expected committed Rust ownership generation. Neither an arbitrary epoch string
nor a different hash establishes that a new login occurred. Acquisition follows
daemon order: exclusive runtime owner lock, then shared migration lock.

Before any desired-state effect, the transaction checks committed Rust ownership,
private paths and receipt, snapshots the private store/template/desired/ownership
inputs, asks a fixed read-only host validator to prove an empty runtime and
validate a requested connected candidate, then rechecks the snapshots. An
enabled no-op still needs validation before consuming its first-login trigger.
An absent routing template does not block a disabled/no-op decision; if present,
it is still a private bounded input. A connected candidate needs its template
and explicit validation before publication.

The write sequence is:

1. Publish a private pending receipt before desired state can change.
2. Publish changed desired state, or leave an identical desired state untouched.
3. Verify the resulting state and relevant fences.
4. Publish the consumed receipt for the same epoch and ownership generation.

The pending receipt is a barrier, not an automatic rollback journal containing
credentials. It retains no raw profile/store/configuration/trigger payload.
Same-epoch consumed retry preserves current desired intent, including a later
manual Disconnect. Pending, malformed, unsafe or mismatched receipts refuse
automatic application. A caller cannot reset the barrier just by supplying a
different epoch. No receipt-removal/recovery command is added here.

## Failure and recovery contract

Failures before publication leave desired state unchanged. Once publication may
have happened, uncertainty returns manual recovery and never deletes existing
receipt evidence. Failure before the first pending rename can leave no receipt
and no desired effect; retry then starts from a fully revalidated snapshot.
No blind restore of an old desired state, deletion of a pending receipt, or
automatic reapplication is attempted. Tests distinguish failures before and
after file publication; a write error does not prove that rename never occurred.

In particular, terminal receipt replacement may report an error after rename
(for example directory-sync failure). The initial call reports uncertainty;
a later read may find a valid consumed receipt and safely recognize completion
without writing desired state again. It may instead find pending and refuse.
The contract promises retained evidence, not that every failed terminal write
necessarily leaves the old pending bytes visible. No cross-boot durability
conclusion is drawn from a failed directory sync.

This is ordered private-file publication, **not** an atomic multi-file commit.
The safe response to a crash between files is refusal, not invented exactly-once
execution. A completed receipt can be recognized on a retry, but a pending
receipt is never silently treated as success.

The runtime-base receipt survives deletion of the daemon's own RuntimeDirectory,
but not necessarily user-manager teardown or reboot. This is not cross-boot
recovery evidence. Trusted epoch creation/reset, startup ordering, orphan-receipt
recovery and legacy unit-enable conversion remain future host work.

The subsequent [startup barrier](R5_LOGIN_STARTUP_BARRIER.md) makes native owner
construction inspect this receipt before reconciliation. **Activation remains
blocked:** the host must still establish trusted once-per-user-manager ordering
before any production entry point calls this transaction. Do not infer complete
production login crash safety from these offline tests alone.

## Reference and acceptance

Selection and normalization reuse #197 and its canonical Python-reference
contract. Python has no corresponding native desired/receipt pair; the
transaction's failure semantics are an intentional Rust ownership contract,
not a claim of literal Python file-output parity.

Acceptance uses synthetic temporary private stores, actual filesystem writes
and locks, a fixed-purpose deterministic host validator and fault injection.
No real VPN profile, network route, TUN, installed unit or polkit setting is
needed or changed. Host validator results in these tests are not evidence of
real core readiness. R5/R6, #178 and production login activation remain open.
