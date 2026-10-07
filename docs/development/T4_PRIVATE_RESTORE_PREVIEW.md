# Opt-in private-pair Restore preview — source candidate

Status: bounded source implementation under review, no installed acceptance. Base is
published Backup UI Draft #705 (`d4f5ac2e`), whose complete non-document tracked
tree equals the installed Backup runtime `662ba087`. Existing API source
`4983f392` and Backup UI62 keep their exact accepted scopes. This proposal does
not activate default methods or add Restore TUI, whole settings, cold recovery,
Connect, service control or historical evidence as authority.

## Two separate semantic requests

Behind the existing `t4-manager-actor-service` feature only:

1. `backup.preview` accepts exactly schema, archive, passphrase, instanceId and
   expectedRevision. It has no operation ID, confirmation or result-cache slot.
   The fixed private-stdin CLI is `backup preview`; no passphrase argv, generic
   method selection, shell, clipboard or caller-chosen runtime path is added.
2. `backup.restore_previewed` accepts the existing seven Restore fields plus
   `expectedCiphertextDigest`, exactly 64 lowercase hexadecimal characters. Its
   confirmation is a distinct literal `replace-previewed-current-private-pair`.
   The fixed CLI is `backup restore-previewed --confirm-private-pair`. A new
   semantic mutation digest includes its distinct action and expected ciphertext
   digest. It uses the existing eight non-evicting result metadata slots and
   existing retained Restore execution/disposition, not a second executor.

Existing `backup.create`, `backup.restore`, their exact schemas, digest domain,
CLI selections and Backup TUI worker remain unchanged. New parsers reject
unknown/duplicate/type-invalid fields and retain existing private frame/input,
path, passphrase, instance and revision bounds. Owned secrets remain zeroizing;
JSON/parser temporaries are not all guaranteed wiped.

Preview success is only bounded counts (`profiles`, `subscriptions`),
`scope: privatePair`, and `ciphertextDigest`, with the original response revision.
It contains no names, credentials, URLs, raw members, host fields, handle,
reservation or receipt. The digest is private local comparison DATA, not proof
of current admissibility or an execution grant. The CLI may emit this exact
bounded JSON only to the requesting local process; it is not a support report.
Lost/unreadable preview reply means unavailable preview, no automatic retry or
follow-on Restore. Preview does not reserve a future request or file.

## SAME current owner, before and after authentication

The existing credential/framing decoder, quit read gate and SAME dispatcher
mutex span a synchronous preview. Only a genuine `ProductionNativeOwner::current`
origin can enter. Instance and revision must match; queued/active mutation,
unfinished external close, background work, unresolved/abandoned pair work,
retained Restore, blocked/uncertain lifecycle and occupied original lease keeper
refuse before archive authentication. Strict Desired and actual state must be
disconnected, pending transaction names absent, and a fresh normal host
observation must contain no owned core/auxiliary/TUN. Repeat these exact current
checks after authentication and require unchanged revision/Desired/generation.

Reuse `restore_readiness_candidate` before and after the KDF, after explicitly
checking original lease keeper vacancy. Each existing short-lived admission
lock is dropped locally before the KDF or return. The dispatcher mutex alone
does not exclude external cooperating store writers; removing these checks
would weaken the existing consistency boundary. No lock/readiness proof is
retained across authentication or returned as authority. Preview never installs
a held execution slot, invalidates Close confirmation, reserves/finishes pair
metadata, advances revision, or reads/writes audit history as authority. Its
observations cannot authorize later effects. One original domain open performs
one fixed Argon2id KDF (64 MiB, three iterations) under that mutex, with bounded
ciphertext/plaintext and no background worker or renewed deadline. Repeated
explicit previews are separately admitted observations, not cached permission.

## One guarded read at actual Restore entry

Extend the existing `backup_destination_candidate` read implementation narrowly:
its existing bounded, no-follow, private/single-link open and descriptor/named
rechecks produce authenticated `OpenedBackup` plus SHA256 of the SAME ciphertext
buffer. Existing `open_existing` remains a wrapper returning only its prior
authenticated value. No extra pathname open/hash precheck or second KDF occurs.

The new Restore path passes its expected digest into the original
`execute_first_restore_mode` open seam. Compare the digest returned from that
SAME original guarded read before `held_restore_execution.install`, migration
lease acquisition, Boundary capture or stage/history/live effects. Mismatch is
a proven pre-entry input denial. It must not become a post-slot no-effect
classification or be inferred from a generic engine error. Everything after
slot installation preserves the existing sticky UNKNOWN/retained graph rules.

At commit, current owner/instance/revision/idle/Off and keeper-vacancy checks are
earned anew; no preview result is consumed as authority. Historical same-ID
replay stays first and returns result DATA only. Changed expected digest under
one ID conflicts. Fresh-ID nested Restore still refuses before reservation.

## Minimal affected files and controls

Changed seams: `private_pair_api.rs` (separate closed request schemas/CLI and
distinct digest), `lib.rs` (feature-only fixed dispatch/trait adapter),
`production_owner.rs` (current-origin preview gate), `native_coordinator.rs`
(existing short-lived readiness composition and shared bound execution route),
`backup_destination_candidate.rs` (same-read authenticated digest result),
`restore_retained_execution.rs` (optional expected digest before slot),
`restore_first_execution.rs` (specific pre-slot mismatch variant), `mutation.rs`
(nonmutating unresolved/idle predicate), reached synthetic tests, and this
owning contract. Existing domain cryptography, default protocol methods,
Backup TUI, package units, history format/capacity and mutation scheduler
capacity do not change. The nonmutating scheduler predicate observes unresolved
pair work without reserving it. Ordinary short-lived readiness locking keeps
its existing operation-lock creation/legacy-validation policy; this is not a
blanket promise that lock bookkeeping or observations perform zero I/O. No
archive, private-pair, Desired or audit mutation is part of preview.

Focused synthetic tests must prove:

- Exact new schemas/default absence, duplicate/unknown/type refusal, lowercase
  digest validation, distinct semantic digest and original schema/digest carry.
- Successful preview counts/digest from the same file, wrong key/corrupt input,
  source inode/in-place/symlink/hardlink/private-mode races, no raw error data.
- False current factory, wrong instance/revision, connected, queued/active,
  external close, unresolved pair, occupied keeper, pending and post-auth drift
  refuse; no revision, close-state, metadata capacity, live bytes/inodes or
  pending/history changes. Preview does not exhaust eight mutation slots.
- Original guarded read replacement yields digest mismatch before held slot
  and lease; even a different authentic envelope carrying identical plaintext
  refuses a stale digest. Matching digest enters the real local retained engine,
  preserves exact completion/revision/lease transfer, replay and fresh denial.
- Unknown/post-slot failures stay sticky; preview cannot rearm them. Lost
  preview response never authorizes Restore or grants a resume/retry token.

Run focused local synthetic default and feature gates, strict affected Clippy,
format/diff/document navigation, then full changed boundary review before any
package or installed selection. Root remains sole VM operator. The first later
installed gate would explicitly preview, separately confirm one digest-bound
Restore and perform normal readbacks/replay/restart; it requires its own exact
source/image and original outcome, not a reclassification of earlier evidence.

Focused writer evidence: preview filters pass eleven tests in both opt-in
configurations (overlapping controls, not a summed total); the existing normal
pair filter passes ten tests, including the real local handler's new-method
false-factory/foreign-instance/busy controls. Default dispatch rejects both new
methods. The distinct-envelope control reaches the real local retained engine
after a stale-digest denial; this is not a genuine-current factory or installed
replacement. Initial test-only non-Debug assertion compilation and a strict
Clippy chunk-iterator style failure remain preserved; their fixes do not add
private formatting or change admission. No package, VM, TUI Restore or default
activation is selected by these source results.

The separate [experimental Restore TUI composition](T4_PRIVATE_RESTORE_TUI.md)
records the later user-facing source slice and its own checks. Its status does
not replace the original preview-only evidence or grant installed acceptance.
