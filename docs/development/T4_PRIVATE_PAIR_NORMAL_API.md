# Opt-in normal private-pair API

Status: bounded agent-attended installed VM acceptance at exact API source
`4983f392d15c123e76e7b71182b45715cf812652`, not default activation or whole T4.
It is based on `44bd54e`, reuses the engine at exact `0f2a1b8a` and the
[private backup contract](../roadmap/PRIVATE_BACKUP_RESTORE.md). Prior installed
Current43/44 and repeated-cycle evidence does not substitute for this new RPC
boundary's separately selected scope46 below.

A separate [preview-bound Restore source candidate](T4_PRIVATE_RESTORE_PREVIEW.md)
adds only explicit developer-feature preview DATA and separately confirmed
ciphertext-bound Restore. It does not rebind the accepted source498/API46 or
Backup UI62 evidence, change the original two method schemas, or activate default
Restore UX. Its new boundary remains subject to its own source/installed gates.

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
A new Restore ID with an occupied original lease keeper returns
`capability_unavailable` before result reservation or engine entry. It leaves
ordinary Backup/mutation eligibility unchanged; exact historical Restore replay
remains first and never runs the engine. This is not a second owning lease or
same-daemon repeated Restore grant. Capability advertisement is descriptive,
not a reservation or bypass of this per-request predicate.

## Source gates and installed acceptance

Deterministic controls cover exact schema/digest, collision/replay/stale revision,
exhaustion, unknown/unwind denial and no extra revision advance. A real local
RuntimeServer credential/frame negative exercises false factory, foreign
instance, busy, duplicate and oversized refusals with unchanged synthetic store.
Real local source-reader/authentication and exclusive-publisher controls also
exercise wrong-passphrase, archive `0400` and existing-destination denials,
then a valid different-ID Backup on the same coordinator. They assert no held
Restore slot, unchanged whole live bytes/metadata/revision, no pending members
and no lifecycle calls. This local backend control is not a genuine-current
factory substitution or installed normal-API acceptance. A separate actual local
retained-engine control performs NEW/Committed completion and original lease
transfer, exact replay, fresh-ID nested refusal without reservation, a different
Backup borrowing the same original lease, and an ordinary onboarding mutation.
The older cached Restore result remains historical data after that mutation.
Default dispatch must still reject both methods. Full changed custody/security
review precedes any VM selection.

Focused SOURCE gates: `normal_pair` 10 PASS, `pair_operation::tests` 3 PASS,
`mutation::tests` 28 PASS (overlapping filters, not a summed total), and the
default-dispatch absence regression 1 PASS. Strict runtime all-target Clippy
passes for no-default plus opt-in, default plus opt-in, and default without
opt-in. These do not claim the whole suite or installed behavior. The first
local handler fixture exceeded Unix socket-path length; its failed original
is retained and the successful control uses a fresh short HOME temp root,
without changing runtime path guards. Initial test-only compile typos are also
retained; neither failure was guest/product acceptance.

Public CI for documentation head `d35a9ca43a7402547f7c413a38170f789cf2f855`
also completed: Test (47m30s), package and package-arm64 all PASS. Test run
`37577244259` and package run `37577244301` are CI evidence for that exact head,
not a replacement for the separately tested runtime `4983f392` or scope46.

### Installed scope46, 2026-10-07

ROOT alone selected all VM installation, normal units, originals and separate
observers. Tested runtime is **only** source `4983f392d15c123e76e7b71182b45715cf812652`:
normal default features plus the explicit opt-in. The installed ELF is 9,040,152
bytes, SHA256 `50561640483c7c520f24eb8be4a3d0c4fc6bdeb336f05c5ebe617fab1a92edb6`;
the normal package is 3,360,180 bytes, SHA256
`86d0d37f8742a5097fa3b146cad0964ea59532266294a69abe4d35fde9784ca6`.
Full twenty-member package inspection and the compatible whole LegacyMeta2
bundle were separate prerequisites. No receipt or constructor was substituted.
Documentation successors are not the tested runtime source.

The fresh disposable thin-copy boot was `4ce16f29-8d21-4fc6-b950-daf8c3dcbc66`.
The genuine current owner was Off with configured default routing, onboarding
complete and startup disabled. All 25 fixed phases had their expected original
exit: original0 except the predeclared fresh-ID Restore denial's original2.
Preparation emits its own data-only record; the remaining 24 originals were
followed by separate observer0. The private pair, credentials,
captures and exact instance IDs remain outside Git.

| Selected boundary | Original / separate observer | Scoped result |
| --- | --- | --- |
| Fresh preparation, hello, status, profiles, snapshot | `b691f8`; `f31de7/8ec170`, `3fc0c9/1cb4fe`, `e92ea6/550480`, `ceeb91/e9e6b3` | Genuine same-instance owned Off baseline and strict supported snapshot |
| Export | `343fec/255ebf` | Normal typed Backup creates authenticated private archive |
| Ordinary public synthetic import and readbacks | `314c74/4a261e`, `e79f7c/219606`, `c16093/2b6759` | One loopback profile added as data only, no activation; revision advances once |
| Restore and readbacks | `92b6c3/62fac6`, `256dcd/0381bc`, `9076fd/e09721` | NEW/Committed completion; one further revision advance; whole original pair bytes and profile projection restored; incumbent audit preserved with exactly one new member |
| Exact historical replay and status | `9f4c55/dc7bd4`, `f8688d/761731` | Same original input/ID, unchanged current revision; replay is result data, not a new grant |
| Fresh-ID Restore denial and readbacks | `794ff2` original2 / `2b8d99`; `463b90/23f9a5`, `1ce6bd/bc1b64` | Exact grouped known-refusal marker; pair, profiles, history and usable owned Off state unchanged |
| Different-ID Backup and final status | `beb12d/a1edda`, `e3c8cb/447283` | Backup remains supported after denial, no revision advance |
| Normal stop, stopped, start | `43edf1/d99a87`, `645ee1/42f431`, `d3b0a3/cc3c56` | Canonical unit inactive/dead with zero PIDs and success; new ordinary start |
| Independent readiness, hello, status, profiles | readiness `7322a1`; `90b600/66a086`, `da6744/73a3b9`, `ad8300/1c3c38` | New normal instance, owned Off; whole pair/profile/audit retained across independently earned restart |

The normal canonical runtime changed from PID2419 to PID3571; readiness proved
the new listener before the one new hello. PID/string equality is not authority.
The denial CLI marker groups several known errors and does not attest an exact
wire code. The reviewed source's occupied-keeper predicate and real local
control establish the intended pre-entry `capability_unavailable` path; the VM
result separately proves the grouped denial and unchanged/usable postconditions.
Likewise, a positive CLI replay marker alone is not a wire replay attestation.

The rejected source predecessor's audit-tuple rebasing and preparation deadline
gaps remain preserved. Its fixed revision2 passed sixteen inert controls but
the first scope45 status observer read its own not-yet-created observation:
prepare `7197b0`, hello `52a3be/56a1dd`, status `f876d8` original0 followed by
observer `f26c27` exit2/STOP. ROOT's read-only reproduction `afb321` confirmed
the real Off predicate. No export/import/Restore began in that failed scope;
its STOP was neither erased nor continued. Fresh revision3/scope46 uses the
preceding hello revision, with seventeen source controls and independent review.
ROOT's separate bound-versus-ZERO control failure `af9e96` and corrected-scope
result `02b4ae` are retained as source-check history, not runtime results.

This accepts one agent-attended, opt-in **private-pair** API sequence, not human
or physical-host acceptance, whole settings, default/public activation, Settings
picker/preview/passphrase UX, connected Restore, arbitrary custom templates,
cold/unknown recovery, fatal descriptor survival or whole T4. SLEEP, network and
OS resume/transfer keep their separate owning contracts and evidence. No such
guarantee follows from this Off sequence. ROOT remains sole VM operator; main
merge/release and default activation remain outside this cut.
