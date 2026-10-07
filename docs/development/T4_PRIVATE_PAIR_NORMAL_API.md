# Opt-in normal private-pair API

Status: SOURCE candidate atop `44bd54e`, not installed acceptance or default
activation. It reuses the engine at exact `0f2a1b8a` and the
[private backup contract](../roadmap/PRIVATE_BACKUP_RESTORE.md). Prior installed
Current43/44 and repeated-cycle evidence does not accept this new RPC boundary.

## Closed vertical slice

Only explicit `t4-manager-actor-service` developer-feature builds compile
`backup.create` and `backup.restore`. They use the existing credential-checked
unary RuntimeServer, SAME owner mutex and quit gate, not another daemon or
generic privileged dispatcher. Default method registration/capabilities remain
unchanged. Feature capabilities advertise the two methods only for an available
owner issued by genuine `ProductionNativeOwner::current()`; ordinary initialize,
research and completed-origin constructors are not current issuers.

The payload is the **whole private pair**, not all application settings:
validated `profiles.json` and the finite supported `route-template.yaml` bytes.
Desired, login/ownership receipts, history, package/enrollment authority,
generated core configuration, services, logs and caches are excluded. Strict
store and twelve-template admission remain unchanged; unsupported custom or
unconfigured templates refuse. This cut has no picker, preview or Settings UI.
The [UI contract](../roadmap/UI_UX_CONTRACT.md) still owns that work.

The opt-in normal CLI selects only `backup create --confirm-private-export` or
`backup restore --confirm-private-pair`. Exact correlated params below are read
from bounded private stdin, never secret arguments. It verifies normal hello's
SAME instance, then submits exactly that original revision/ID; no silent refresh,
new ID or resend. Interactive stdin is refused. Default CLI stays unchanged.

Exact params: `schema` (1), `archive`, `passphrase`, `confirmation`, `instanceId`,
`operationId`, `expectedRevision`. Create confirmation is
`export-current-private-pair`; Restore is `replace-current-private-pair`.
Unknown/duplicate/type-invalid fields, invalid IDs, relative/unnormalized paths,
invalid UTF-8 and bounds refuse. Original decoder checks duplicates recursively;
these secret-method frames have the existing 32-KiB bound. Parsed paths are data,
not authority: original source/destination acquisition guards remain mandatory.

Passphrases never enter argv/environment/logs/ordinary stdout. Owned typed
strings, digest input and serialized frame are zeroizing; JSON/library
temporaries are not all guaranteed wiped. Result metadata stores only operation
ID, fixed digest and closed outcome, never private input bytes or a Debug grant.

## Scheduler and uncertainty

Retained Restore requires an idle scheduler and already advances revision once
through its positive typed disposition. The API must not enqueue a second
mutation or increment again. Eight non-evicting pair-result slots belong to the
existing scheduler; one metadata reservation precedes backend work under the
SAME owner mutex. It is not execution/receipt/lease authority and cannot be
cloned or serialized. No descriptor role or engine/history capacity changes.

IDs share collision checks with ordinary queued/active/cached operations,
external-close work and the long-operation registry. Different input under one
ID conflicts. New admission requires current revision, idle queue/active state,
no unfinished external close/pair work and drained batches. Restore additionally
requires strict Desired and actual disconnected before entering its engine.
Backup preserves its existing consistent snapshot/lease checks; it never stops
or reconnects a VPN.

Exact SAME-instance replay returns only historical result data before the new
revision test. It neither runs the backend nor verifies present files equal
that result or restores lost authority. Digest binds action, path, passphrase,
instance and original revision. Callers preserve the original request after
loss; no new ID/revision or automatic resend is allowed. Success exposes only
`completed`, `replayed`, `scope: privatePair` and original result revision.
Create does not advance revision; Restore reports its engine's one advance.

Parser/instance/current-origin/revision/busy/collision/capacity denials precede
backend entry. The original retained Restore `Prepare` error occurs only during
archive opening/authentication, before slot installation or lease acquisition;
it settles as `invalid_argument`. Backup's exact invalid-backup-input and invalid
destination variants settle as `invalid_argument`; exclusive publication
`Exists` settles as `conflict` without replacing the destination. These cached
denials leave a distinct operation eligible only through unchanged fresh owner
and scheduler checks. They never grant entry or waive authority loss. Other
backend errors conservatively remain `manual_recovery_required` (UNKNOWN),
including post-slot Restore admission, source drift and unavailable/ambiguous
publication. UNKNOWN stays cached and blocks new
ordinary, remote, external-close and pair work. Client transport loss is UNKNOWN
even if the server retained a positive result; historical replay does not prove
the original reply was received. Reservation Drop after unwind
permanently marks abandonment. Restoring matching bytes cannot rearm it. Restore
keeps its existing held-engine lifetime; Backup retains its original publisher's
ambiguous-persistence contract. Result metadata does not invent FDs or change
either backend's custody guarantee. No post-process-exit custody is claimed.

Eight pair requests per daemon instance is a separate bounded SOURCE capacity,
not eight VM executions or the durable history limit. Exhaustion refuses before
backend work, without eviction or audit purge. A fresh daemon independently
earns startup/reconciliation, never copies cached authority. Owned-only original
lease admission and nested Original-borrowed Restore refusal remain unchanged.

## Gates and next acceptance

Deterministic controls cover exact schema/digest, collision/replay/stale revision,
exhaustion, unknown/unwind denial and no extra revision advance. A real local
RuntimeServer credential/frame negative exercises false factory, foreign
instance, busy, duplicate and oversized refusals with unchanged synthetic store.
Real local source-reader/authentication and exclusive-publisher controls also
exercise wrong-passphrase, archive `0400` and existing-destination denials,
then a valid different-ID Backup on the same coordinator. They assert no held
Restore slot, unchanged whole live bytes/metadata/revision, no pending members
and no lifecycle calls. This local backend control is not a genuine-current
factory substitution or installed normal-API acceptance.
Default dispatch must still reject both methods. Full changed custody/security
review precedes any VM selection.

Focused SOURCE gates: `normal_pair` 9 PASS, `pair_operation::tests` 3 PASS,
`mutation::tests` 28 PASS (overlapping filters, not a summed total), and the
default-dispatch absence regression 1 PASS. Strict runtime all-target Clippy
passes for no-default plus opt-in, default plus opt-in, and default without
opt-in. These do not claim the whole suite or installed behavior. The first
local handler fixture exceeded Unix socket-path length; its failed original
is retained and the successful control uses a fresh short HOME temp root,
without changing runtime path guards. Initial test-only compile typos are also
retained; neither failure was guest/product acceptance.

Installed normal API acceptance is pending: actual current export, a different
synthetic ordinary-data mutation, Restore, exact replay, full pair/profile
readback and independently admitted restart. No frontend, current/login/package,
default activation, cold bootstrap or whole-T4 result follows from SOURCE tests.
Root is sole VM operator; main merge/release are outside this cut.
