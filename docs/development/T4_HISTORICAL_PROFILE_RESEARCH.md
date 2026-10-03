# T4 real favorite-mutation historical research

Dev-only successor of #551 at `ff066852d8825f75e035809ffcc04fe4333ee3d5`.
This explores the already-approved same-UID/exact-ownership-generation historical
policy in private synthetic fixtures. Product policy adoption remains unapproved.
No ordinary startup, owner registration, IPC dispatch or general mutation is enabled.

## Actual caller and authority

Ordinary `execute_profile` delegates through the typed Ordinary alternative to
its existing admission, presence guard, preflight lease, transaction and finish.
The shared scheduling function still owns revision, operation-ID collision,
queue and exact Replay behavior. No generic pending predicate is relaxed.

The unit-test-only historical alternative admits only `profiles.favorite`.
It consumes the resynchronized current-Off/current-manager witness, drops that
fixed Off snapshot and independently validates evolving current bundled data.
It retains the same migration lease and original epoch/receipt proof throughout.
The initial resync still requires its caller's fresh Off/empty-owned-host gate;
filesystem/epoch evidence alone is not host authority. This is a direct private
coordinator fixture, not dispatch from #551's observation-only owner wrapper.
An opaque retained Arc binds its first use to exactly one coordinator instance;
a fresh coordinator cannot reuse its revision/replay authority.

The original C1, pending ticket, completion record, template, desired state,
owner marker, login receipt and directory identities remain immutable. Every
admission, including Replay, checks those boundaries and all transient absences.
Preflight and the last pre-write check repeat this under the retained lease.
Only the existing prepared writer and actual `apply_transaction` favorite branch
perform the store edit; there is no alternate store writer or lifecycle effect.

A successful write must have the exact prepared candidate bytes and independently
validated semantics at readback. Only that transition advances the retained
store snapshot. No-op expects original bytes, as the ordinary writer does not
rewrite serialization on a semantic no-op. History is never rewritten to follow
live data. Failed/ambiguous commit or post-write proof loss poisons the context
and independently latches the coordinator into manual recovery. A later matching
file cannot manufacture success, Replay or a silently rebased owner.

Self-review superseded the first checkpoint `27f6f8a`: its post-write comparator
allowed a store-inode change even for a no-op, and pinned output only after the
After callback. The correction requires complete unchanged identity for no-op,
pins verified output immediately after successful writer return, then rechecks
after the callback. Same-byte substitution after a write or no-op is an ambiguous
failure, never claimed restored or converted into a successful result.

Checks are cooperative-lease and point-in-time identity guarantees, not atomic
exclusion of hostile same-UID filesystem writes between checks and syscalls.
Checksums are not authentication. The same-manager proof does not mint a new
receipt after manager restart, and this slice writes no durable recovery record.

## Acceptance scope

Synthetic fixtures use actual private files and actual profile transactions.
Lifecycle methods panic if called. Coverage includes Commit/Abort history,
favorite write, exact Replay, second edit, no-op, unsupported profile actions,
new-owner refusal, late transient/history/store/epoch drift at admission,
preflight and pre-write, drift before Replay, actual writer temporary-slot exhaustion,
injected pre-write refusal and post-write lost-success ambiguity, permanent
context poison and independent owner latching. An injected Before callback that
returns Ok after changing a fence is followed by another full pre-write check.
Fault injection can withdraw a
real writer success, never turn a writer failure into success.

Exact-head local/CI/VM results are recorded on the owning Draft PR. This is not
installed private-profile acceptance, provider interoperability or power-cut
evidence. Ordinary pending and separate DesiredPaths/CutoverPaths regression
fixtures remain mandatory alongside the full frozen runtime suite.

## Product boundary still not approved

The policy decision remains whether intact same-UID/exact-generation historical
disposition replaces future archive re-supply while ordinary data evolves.
This bounded experiment does not adopt that policy for users. Connect and all
other synchronous mutations, background publication, login/new-manager receipt
transition, successor-restore ticket invalidation, ownership rollover, actual
private restore UX/API and installed package/host acceptance remain separately
gated. No additional journal or model is introduced to substitute for those
real caller integrations. C1 and both disposition records remain fences to all
ordinary entrypoints and are never deleted by this slice.
