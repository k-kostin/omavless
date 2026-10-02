# T4 private backup and restore proposal

Status: security/product design with an inactive Rust envelope primitive, **not
approved for activation**. There is no backup command, IPC method, picker,
scheduler, file publisher or restore authority. A backup file contains reusable
VPN credentials and subscription bearer URLs; it is not a support report.

## User task and scope

The user goal is to move or recover their own profiles, subscriptions and
routing preferences without copying runtime ownership to another machine.
Backup targets the currently committed private configuration; Restore targets
the whole chosen backup after explicit preview and confirmation. These are
mutations of private data, not Connect, Full Quit, package installation, or
service repair. The management entry may be a TUI/Settings navigation action;
opening it must have no file or VPN effect. The final confirmation must name
the existing destination as being replaced, never silently merge into it.

A first version should contain only an allowlisted, versioned portable payload:
the validated private `profiles.json` and the bounded routing template needed
to reconstruct its configuration. This includes the credentials, subscription
URLs and settings in that store. It must not include `desired.json`, ownership
markers, transaction journals, controller secrets, generated Mihomo config,
core binaries, service/unit enablement, host paths, TUN/routes, caches, logs or
shareable diagnostics. A restored installation starts with desired state Off;
the user separately chooses whether to connect. Future schedule preferences
and other persistent extensions need an explicit schema/version decision rather
than being captured by a wildcard directory archive.

## Threat and authority boundary

- A portable product backup must be authenticated and encrypted before it is
  written to a user-selected destination. There is no silent plaintext export
  or cloud upload. The inactive envelope below fixes a candidate KDF/AEAD and
  byte format for review; do not treat file mode `0600` as encryption.
- The passphrase must not enter argv, environment variables, logs, shell history
  or shareable diagnostics. Wrong passphrase and corrupt/unsupported backups
  get fixed, non-oracular public errors. Losing the passphrase is unrecoverable;
  the UI must say so before backup creation.
- The Rust runtime remains the only owner of its source store and restore
  mutation. QML/TUI may request a fixed semantic operation, not arbitrary
  file read/write, shell execution or privileged host control. A separately
  reviewed design must choose how bounded encrypted bytes and a destination
  cross the same-user boundary without widening the general IPC protocol.
- Refuse unsafe source/destination paths, symlinks, wrong ownership and
  unexpected hard-link targets. Create backup files exclusively with private
  permissions, synchronize bytes and parent directory, and do not overwrite an
  existing backup by default. The envelope uses fixed member names and sizes;
  never extract paths from an archive into the filesystem. A bounded,
  non-compressed v1 is preferable to an unbounded decompressor.
- Do not export private material into Git, issue comments, support snapshots,
  screenshots, telemetry, clipboard or ordinary stdout. Backups are private
  user data even when encrypted. Sanitized tests use synthetic records only.

## Consistency and restore admission

Backup must snapshot one committed generation of the store and template under
the existing ownership/mutation lease. A concurrent profile edit or subscription
refresh cannot produce a mixed pair; the operation either captures a consistent
pair or fails. Backup itself does not quiesce or reconnect an active VPN.

Restore is a separate, high-risk operation. Admit it only when the current
native owner is verified, its VPN is disconnected and owned TUN/core cleanup is
confirmed, background mutations are drained, and no ownership/transaction or
unknown-outcome recovery barrier is present. An active or uncertain state is a
clear refusal, not permission to stop a foreign VPN or change routing. Check
instance/revision again at commit time; a preview is never a reservation.

Authenticate and bound the complete archive before any mutation. Validate its
version, exact member set, size and private-store/routing semantics against the
installed Rust owner. Reject incompatible future schemas rather than dropping
records, silently migrating unknown fields or generating an untestable config.
The preview should show only bounded private local facts (for example counts,
compatibility and that existing data will be replaced), not links or bearer
URLs. It must distinguish “backup readable” from “restore admissible now.”

Prepare a complete candidate before replacing any live files. Multi-file
replacement needs an explicit durable transaction with exact-old-byte rollback
and startup recovery; sequential atomic renames alone are insufficient. On
success, verify both new committed files and leave desired state Off. On a
recoverable failure, verify exact previous bytes and previous disconnected
state. Ambiguous I/O or incomplete compensation becomes manual recovery with
both copies preserved; never claim success, delete the last good data, or
autoconnect. Do not silently overwrite an existing valid store during first-run
setup merely because a backup was supplied.

## Acceptance before activation

1. Pure synthetic fixtures: wrong passphrase, tampering, truncation, duplicate
   or unknown members, oversized payload, incompatible schema, malformed store,
   invalid template, and no plaintext leakage through errors or diagnostics.
2. Filesystem/transaction fixtures: symlink/hard-link and wrong-owner refusal;
   existing destination untouched; concurrent revision change; interruption at
   every prepare/commit/rollback step; exact-byte recovery or explicit manual
   recovery. No tests use a real profile or the owner's backup.
3. Product states: no backup, valid backup, incompatible backup, connected,
   disconnected, pending mutation and uncertain ownership. Confirm the target,
   replacement effect and Off-after-restore semantics in English and Russian.
   Rendered review is separate from handler tests; no current UI is claimed.
4. Exact-head Try Omarchy restore to a fresh installation with synthetic data,
   then verify records and Off state. Arch and future NixOS portability require
   their own package/host checks; host-specific state is never smuggled into the
   portable payload. A physical-PC pass is needed only for a concrete
   hardware-specific behavior, not for the pure archive format.

Open decisions before activation: passphrase UX and independent format review;
private byte-transfer/destination API; broader portable template policy; whether a
later version can offer an explicit non-destructive import/merge; and precise
transaction-journal layout. None is settled by this proposal.

## Inner payload framing candidate

The internal `omavless-domain::backup_payload_candidate` module explores one
bounded inner payload representation. It is compiled for the inactive encrypted
envelope below and has no runtime caller, file handling or IPC surface. Its
plaintext output is **not a backup** and must never be saved as one.

The experimental framing is eight literal bytes `OVTESTP1`, then two big-endian
u32 lengths, then exact store bytes followed by exact template bytes. Member
identity and order are implicit: only those two members exist; no path names,
compression, recursive containers or optional extensions are admitted. Empty
members, unknown magic/version, individual sizes above the existing 5-MiB store
and 2-MiB template limits, truncation and any trailing bytes are rejected before
returning borrowed slices. This is not a stable interoperable format commitment.

Seven synthetic tests cover an independently hand-authored wire fixture, every
truncation, appended/duplicate container data, mutated magic, malicious lengths,
exact limits and fixed private-data-free errors. A deliberate negative-security
test proves that content tampering and malformed member semantics can still
pass framing: structural decode is **not authentication or restore validation**.
No decoded private type implements Debug, Display, Clone or serialization.
Borrowed slices avoid a decoder plaintext copy but provide no zeroization claim.

This narrower gate did not select an AEAD/KDF library or parameters. The inactive
envelope primitive below now fixes one version for review. Authentication must
finish before inner semantic parsing or preview. Public wrong-passphrase and
corruption errors remain fixed; no timing-oracle guarantee is claimed.

Strict current-schema store validation (including duplicate/unknown members),
portable-template policy, consistent owner snapshot, private byte transfer,
exclusive destination publication, disconnected restore admission and durable
multi-file recovery are still required. No filesystem/VM/host/UI acceptance,
production activation or completed backup/restore feature is claimed.

## Inactive strict store admission

The internal `private_store::backup_candidate` validates the store member
after the framing gate. A closed deserialization schema rejects unknown and
duplicate decoded keys at the root, profile, subscription, rule and startup
objects. The existing private-store parser then validates credentials, record
relationships and routing semantics. Admission requires explicit v3 and exact
semantic equality with that parser's normalized document: old versions, omitted
defaults, stale convenience pointers and silently repaired startup references
refuse. It returns the original borrowed bytes and bounded counts, never a
normalized replacement or raw parser diagnostic. No private type implements
formatting, cloning or generic serialization.

The deliberately closed schema rejects extensions such as provider quota,
schedules, embedded host state and future fields; it does not silently discard
them. A later schema decision can add reviewed portable fields. This restriction
is local to the backup candidate; ordinary store compatibility reads
are unchanged. Enabled startup preferences can be valid portable data, but
their presence grants no login/restore authority and does not relax the separate
Off-after-restore requirement.

Synthetic tests cover complete managed/standalone/empty stores, escaped display
text, unknown fields at every object scope, duplicates including escaped keys,
legacy/future versions, missing defaults, stale pointers, invalid credentials,
relationships/rules, bounded input and fixed errors. Framing composition proves
that a valid store can coexist with an invalid template: store acceptance is
neither authentication, portable-template validation nor permission to restore.
The exact input bytes survive successful admission; no file or host is touched.

Current-schema store admission is now compiled for the inactive encrypted
envelope below. Activation still needs a broader portable-template policy,
consistent owner snapshot, private transfer/publication, disconnected
owner/revision admission and durable multi-file recovery. No installed backup or
restore is available or claimed.

## Inactive bundled-template pair admission

The inner framing candidate also offers a deliberately narrow whole-pair
gate. After strict store validation, it recognizes only the exact current
checked-in default, China or Iran template selected by the store's routing
preset, with the existing canonical rule/global/direct mode transformation.
Nine positive combinations and all eighteen cross-preset mismatches are covered.
Both borrowed members remain byte-for-byte unchanged. Portable custom rules and
startup preferences remain store data; this gate does not render, merge or
activate them, nor infer a saved routing mode from an unrelated store field.

Unknown/custom/unconfigured presets and edited templates refuse, including
comments, controller additions, duplicate mode keys and line-ending rewrites.
This is exact source-byte recognition, not a general YAML security parser or a
lossy backup conversion. Invalid UTF-8 and private-looking rejected content
produce only fixed diagnostics. No private result exposes formatting, cloning
or serialization, and no filesystem, IPC or export/restore caller is added.

This bounded subset is an executable candidate, **not** a decision that the
product should permanently reject custom templates. It is version-sensitive to
the checked-in template snapshots and makes no cross-version portability claim.
A broader portable-template policy still needs an explicit contract. The
authenticated envelope's runtime integration, complete native-owner snapshot, private
transfer and exclusive destination publication, disconnected owner/revision
admission, durable multi-file recovery and exact-head installed acceptance all
remain gates before activation. Pair admission does not authenticate bytes or
grant permission to restore them.

## Inactive authenticated envelope primitive

`omavless-domain::private_backup` is compiled in normal Rust builds but has no
runtime caller, CLI, IPC, file picker or restore transaction. It accepts only
the strict v3 store and exact bundled-template pair above. Its `seal` operation
uses OS-generated independent 16-byte salt and 24-byte nonce, Argon2id v19
(64 MiB, three iterations, one lane, 32-byte key), then XChaCha20-Poly1305.
Opening checks total and declared sizes before the fixed-cost KDF,
authenticates the complete header as AEAD associated data, and only then parses
the inner framing and pair semantics. Key and owned plaintext buffers are
zeroized on drop; the caller still owns its passphrase memory. Strict JSON
validation can allocate additional private data that is not guaranteed to be
zeroized. Both APIs require 12–1024 passphrase bytes. The future UI must explain
that losing the passphrase is unrecoverable and avoid argv, environment and
logging exposure.

Version 01 bytes are: `OVBKUP01` (8), salt (16), nonce (24), big-endian u32
ciphertext length (4), ciphertext and 16-byte AEAD tag. The authenticated
plaintext is the fixed `OVTESTP1` inner representation above. No variable KDF
parameters, compression, paths or extra members are accepted. Total input is
bounded by the 5-MiB store and 2-MiB template limits plus 68 bytes of outer
overhead and 16 bytes of inner framing. Wrong passphrase, tampering, unsupported
version and invalid authenticated members all return `backup_unreadable`.
Creation rejects invalid source pairs; no plaintext backup is written.

Synthetic tests and an independent libargon2/libsodium deterministic envelope
vector cover roundtrip/exact bytes, fresh entropy, wrong passphrase,
tampering of salt/nonce/ciphertext/tag, truncation, declared-length and version
refusal, and creation admission. This is a cryptographic format candidate until
full memory/dependency review and product passphrase UX are complete. It does not
close consistent native-owner snapshot,
private destination publication, disconnected restore admission, durable
multi-file recovery or installed acceptance. No backup or restore feature is
available to users.

### Inactive KDF workspace lifetime hardening

The caller now owns Argon2's fixed 64-MiB workspace in
`Zeroizing<Vec<argon2::Block>>` and uses `hash_password_into_with_memory`.
The pinned Argon2 0.5.3 `zeroize` feature supplies block erasure and clears
selected internal intermediates; enabling that feature alone would not erase
the ordinary vector used by its allocating convenience method. Every initialized
workspace block is explicitly erased before either normal success or error
return, with the owning guard additionally covering unwinding. The key remains
separately zeroizing. Fixed parameters, envelope bytes, independent cross-library
vector and public errors are unchanged.

A synthetic test seeds the complete workspace and checks every word after
successful derivation and an invalid-salt KDF failure, while the independent
envelope vector and wrong-passphrase test remain unchanged. This is not a claim
that destructors run after SIGKILL, abort or OOM, nor a guarantee covering swap,
core dumps, registers or every internal cryptographic temporary. The caller's
passphrase and allocated private-store/JSON copies remain separate memory-lifetime
review obligations. Versioned template portability, frozen format compatibility,
bounded concurrent KDF admission and private input UX still gate product
activation; this hardening introduces no product caller or new authority.

## Inactive fixed-member source snapshot

The runtime's internal `backup_source_candidate` compiles in normal Rust builds
but has no product caller. It exercises the source-pair acquisition prerequisite
using only synthetic temporary files in tests. It accepts the
existing matching migration lease, checks the exact committed Rust generation
and refuses an existing or unreadable private-transaction pending marker both before
and after acquisition. It never creates state directories or repairs a marker.
The lease remains held across both reads, excluding cooperating store/template
writers. This is not yet the serialized native owner's complete admission:
instance/revision, in-memory recovery barriers and background-operation state
must still be supplied by eventual owner integration.

The candidate pins the private config directory and opens only `profiles.json`
and `route-template.yaml` through that directory descriptor. Both member
descriptors are held before either read. Members must be same-user, regular,
single-link, mode `0600`, nonempty and within the existing individual bounds;
symlinks, hard links, unsafe directories and special files refuse. The config
directory is reached component by component from `/` without following an
ancestor symlink; root- or same-user-owned ancestors must not be writable by
other users. Reads are bounded and nonblocking at open. Descriptor and
current-path identity, size, mode, ownership and modification/change timestamps
are rechecked after both reads, together with a second no-follow traversal of
the directory and the owner/pending state. A detected edit or replacement
returns no pair and never overwrites the changed source.

Eleven synthetic tests cover fixed-member-only acquisition, matching lease
exclusion, wrong generation/lease, unsafe/missing members and directories,
hardlinks/symlinks, bounds, uncommitted/unsafe ownership and interrupted presets,
same-size in-place/atomic member replacement, directory replacement and late
owner/pending changes, including symlinked/writable ancestors and an ancestor
changed between reads. The seal-composition test uses the captured pair with the
authenticated envelope, verifies private roundtrip and unchanged sources, and
refuses invalid passphrases and templates. This is filesystem and cooperative-lock
evidence, not protection from a hostile same-user process able to forge the entire state.
No system service, provider, controller, TUN or private installed file is used.

The internal pair holds unvalidated plaintext in zeroizing byte buffers until
the envelope's semantic checks and encryption complete. It is not an
authorization to publish the resulting sealed bytes. Additional allocations
made by parsing cannot be promised zeroized. Owner instance/revision and
recovery-state admission, private destination transfer, exclusive publication,
restore recovery and installed acceptance remain separate gates. No product
caller or backup command is registered.

## Inactive exclusive ciphertext destination

The internal runtime destination candidate accepts only the opaque result of
the source-and-seal primitive, not arbitrary plaintext bytes. It has no CLI,
IPC, picker or product caller. Its deliberately conservative first policy
requires an absolute path under a same-user `0700` parent, reached through
root- or same-user-owned ancestors with no symlinks or group/other write
permission. Other user-selected directories refuse rather than weakening this
boundary. The eventual user-facing destination policy and transfer API remain
undecided.

On supporting Linux filesystems it writes ciphertext to an unnamed `O_TMPFILE`
inode with exact `0600` mode, syncs that inode, then links the requested final
name exclusively with `AT_EMPTY_PATH`. Existing files, including symlinks, are
never overwritten. There is no named temporary artifact or fallback if the
filesystem lacks `O_TMPFILE`. The pinned parent is rechecked after publication
and synchronized. Once the exclusive link succeeds, a parent-path replacement
or sync failure is reported as an ambiguous result, not a safe retry or a claim
that no file was created. The file is encrypted, but remains private user data.

Synthetic tests cover one successful private publication, existing destination
and symlink refusal, unsafe parents, wrong owner and a replaced parent after
link. The latter preserves the encrypted file and reports ambiguity. This does
not prove safety against a hostile same-user process able to move the entire
directory concurrently, nor complete native-owner admission, passphrase UX,
disconnected restore, durable multi-file recovery or installed acceptance.

## Inactive bounded source open and preview

The same internal runtime candidate can read an existing backup under its
conservative private-parent policy. It opens the member relative to the pinned
directory without following links, requires a same-user regular file with one
link and exact `0600` mode, and bounds the read before authentication. It
rechecks the opened member, current directory entry and directory path after
reading; a detected change refuses before decrypting. No caller, file picker,
IPC method or restore mutation is registered.

Only the authenticated envelope can produce an opened pair. A separate preview
returns profile and subscription counts, never names, credentials or URLs.
Wrong passphrases and invalid ciphertext get a fixed unreadable result. The
preview is informational, not a reservation: an eventual restore must reopen,
reauthenticate and recheck its owner/revision and disconnected-state gates.
Synthetic tests cover roundtrip counts, wrong passphrase, tampering, symlink,
hard link, public mode and file/directory replacement during the read. A
hostile same-user process is outside this primitive's guarantee; broader
owner admission and durable restore are still required before activation.

## Inactive disconnected-owner restore preflight

The native coordinator now has an internal read-only restore-readiness
candidate. Under the existing migration lease it requires the exact committed
Rust ownership generation, no pending preset or recovery barrier, no active or
queued mutation, no active background operation, and a safe auxiliary-core
slot. It reads desired state twice around a fresh local observation and admits
only an Off/Disconnected owner with no owned core, auxiliary process or managed
TUN. A foreign VPN is neither stopped nor adopted; unrelated visible processes
and interfaces do not by themselves grant or withdraw restore authority.

The result records only revision and generation for comparison. It is not a
reservation or authorization to write: a future restore commit must repeat all
checks, hold background work quiescent across the transaction, authenticate the
backup again and prove durable replacement/recovery of both fixed files. This
candidate has no product caller, IPC, UI, file mutation or VPN effect.

## Inactive restore preview and pair preparation

The follow-up coordinator candidate combines the authenticated file counts
with the *separate* disconnected-owner result. A valid backup can therefore
preview as readable while restore is currently unavailable. No caller-visible
label may imply that either fact reserves an eventual commit. Wrong passphrase
and invalid ciphertext retain the same fixed unreadable class; no profile
content, URL or endpoint enters this preview.

A second inactive method authenticates the backup outside the owner lease,
then holds one matching migration lease across disconnected-owner admission,
exact old store/template acquisition and a repeated final admission. Both
old and new pairs remain only in non-formatable, zeroizing memory. Synthetic
tests prove byte-exact preservation of a custom old template, refusal while
connected, refusal when desired state changes during preparation, no host
action, no file replacement, and no credential-bearing output. This preparation
is intentionally **not** a restore plan that can be
committed later: its revision/generation are observations, not a lock or token.
The eventual transaction must reopen and authenticate the selected backup,
repeat owner/host checks at commit, stage both complete replacement files,
durably preserve the old pair and provide restart-safe recovery before any
product method or UI is added.

## Inactive durable restore staging candidate

The next internal Rust slice reopens/authenticates the backup and, under the
same owner lease as the final readiness and old-pair capture, creates exactly
one private `restore-pair.pending` directory in the state root. It stores four
fixed, bounded data members: old store, old template, authenticated new store
and authenticated new template, followed by a fixed-size `ready.bin` marker.
The directory and members use exact `0700`/`0600`
permissions, no-follow/exclusive descriptor-relative creation and file plus
directory synchronization. Existing or inaccessible pending state refuses; a
failure after the directory name is created is ambiguous and never triggers
automatic retry or deletion. The live pair is not replaced. The primitive
itself does not authenticate or validate arbitrary input: only the coordinator
candidate supplies previously admitted bytes.

An existing staging directory joins the common private-transaction ambiguity
fence used by connection, stop, batch and cutover admission, source capture and
later restore preview. Synthetic tests check private fixed members, partial
staging, existing/symlinked targets, unchanged live bytes, and the fence. The
test interruption hook models step failures; it does **not** prove crash-safe
commit. There is still no commit or rollback protocol, startup verifier,
automatic recovery, public method, IPC or user-facing restore feature. The
pending directory is deliberately not produced by normal installed operation.

A follow-up read-only inspector requires the exact five-member set, safe
single-link private files and a final `ready.bin` containing versioned lengths
and SHA-256 checksums for all four staged byte strings. It reopens entries and
the parent after bounded reads to refuse detected replacement. Synthetic
fixtures cover missing, partial, malformed, extra, public, symlinked and
checksum-mismatched members. The checksum detects accidental corruption and
torn staging; it does **not** authenticate the directory against a malicious
same-user writer or prove that a commit ever occurred. Even a complete stage
remains a pending recovery fence until a separately designed, verified
commit/rollback procedure exists. The inspector never clears it.

Under a matching owner lease, a further read-only recovery classifier compares
the two currently live fixed files with the verified staged old/new checksums.
It reports only `old`, `new`, `identical`, `mixed` or `diverged`; unsafe live
files, an invalid stage and changed ownership refuse separately. Synthetic
fixtures exercise every changed outcome, a wrong generation, a symlinked live
member and a torn marker. This is a point-in-time diagnosis, not proof that a
particular write committed or permission to roll forward/back. No class
automatically clears the pending fence, and future recovery must retain the
same lease and reprove all facts before each effect.

The existing login transaction and its startup receipt checks now refuse a
surviving private-transaction pending marker, including this inactive restore
stage. This prevents an imported `startup.enabled` preference from producing a
new connected desired state while staged restoration is ambiguous. Synthetic
tests cover both marker kinds, an already-consumed login receipt and a marker
appearing during host validation. This is an admission fence, not a completed
restore decision or a policy for how imported startup preferences behave after
the stage is eventually resolved.

## Inactive recovery-decision model

An internal pure Rust model now binds a candidate decision to a verified
point-in-time v1 stage digest, exact Off desired-state bytes (or explicit
absence), owner generation and a nonzero transaction identifier. Its fixed
138-byte `OVRDEC01` representation contains a phase (`Intent`, `Committed` or
`Aborted`) and a domain-separated SHA-256 tear checksum. It contains no raw
profile, template, desired-state or passphrase bytes. The checksum is **not**
authentication against a same-user writer. This record is neither written to
disk nor read at startup by current product code. The inactive executor below
can write it only from synthetic tests; there is no product caller, cleanup or
restore command.

The pure review table treats a valid nonterminal intent with live old/new/mixed
bytes as a *candidate* for exact-old rollback, never as a completed restore.
Only a matching terminal `Committed` plus new pair can be reviewed as commit;
only `Aborted` plus old pair as abort. Divergence and contradictions require
manual recovery. A missing/invalid record or existing v1 stage alone grants no
recovery authority. Even a table match still requires an independently
verified, durable on-disk intent, fresh owner/disconnected/host checks,
descriptor-relative writes and readback under one lease. The model does not
settle imported startup preferences after recovery.

A further inactive read-only journal inspector recognizes two fixed private
state-root files, `restore-decision.intent` and an optional
`restore-decision.terminal`. It requires exact owner/permissions/single-link
regular files, bounded records, one matching transaction, an unchanged Rust
owner generation, the current complete stage and the exact current Off desired
state. A terminal without an intent, torn file, symlink, changed desired state
or mismatched transaction refuses. The inspector reopens the members and
rechecks the marker/stage/state directory during one lease-bound pass. This is
not a product journal **writer**: synthetic tests and the inactive executor
below create its records, but normal installed operation never does. It never
clears a stage or changes live profiles. On startup, orphan decision records
also fence mutation even if the pending-stage directory is absent; they cannot
be mistaken for a clean state.

## Inactive two-file restore executor

The internal `restore_executor_candidate` composes the verified four-member
stage and decision journal into a lease-bound synthetic transaction. There is
no runtime registration, CLI/IPC operation or GUI button for it. Its caller
must supply a fresh disconnected/idle host gate; the tests supply only a
synthetic gate. This is **not** usable backup or restore in an installed build.

Before each effect it rechecks the Rust owner generation, exact Off desired
bytes, complete stage identity, fixed configuration directory and same journal
transaction. It publishes a fixed, exclusive intent before live writes. For
each of the two files it synchronizes a private replacement inode, links it
under a fixed name in the pinned directory, rechecks identity/content and
renames it over the live member, then synchronizes the directory. Before a
terminal decision it synchronizes and reopens both exact live members and
independently classifies the pair. Intent without terminal recovers toward the
exact old pair; committed/aborted terminals are verified, never reversed.
Every uncertain result retains stage and journal as a startup/mutation fence.
No path can automatically overwrite a divergent pair. These are bounded
synthetic guarantees, not proof against a hostile same-user writer or arbitrary
power-loss behavior. Tests include actual subprocess termination between
effects, reopen/retry of forward and rollback paths, owner/desired/host drift,
and symlink/hardlink/replacement-slot refusal.

The executor deliberately leaves even a successful stage and journal in
place. Remaining gates before activation include a reviewed finalization and
cleanup protocol, real host admission and imported-startup policy, explicit
destination/passphrase UX, product caller/API authority, installed VM and host
acceptance, and a separate decision whether the portable format extends beyond
the fixed v1 pair. Do not interpret synthetic PASS as T4 completion.

## Inactive restore-finalization receipt

The internal `restore_retirement_candidate` can publish a single exclusive,
fixed-size `restore-finalization.pending` receipt only after a committed or
aborted terminal pair has been reopened and verified. The receipt binds the
terminal transaction, Rust owner generation, desired state and the exact two
live members by length and digest; it contains no profile or template bytes.
Publication synchronizes the file and its directory, checks that the same
inode remains in place, then independently reopens the receipt and live pair.
It refuses an undecided intent instead of invoking rollback. A surviving
receipt is an existence-based startup/mutation fence even if the original
stage and journal later disappear. Synthetic tests verify that independent
inspection works after that disappearance and refuses torn, duplicate or
drifted evidence.

This is a **publication and inspection candidate only**. It has no product
caller, does not delete a single staged or journal member, and never removes
its own fence. A separately reviewed cleanup/retirement protocol must define
the exact unlink order, crash recovery and authorization before activation;
these synthetic tests are not installed backup/restore acceptance.

## Inactive replacement-slot retirement candidate

The internal `restore_slot_retirement_candidate` handles the four fixed
credential-bearing replacement names that an interrupted executor may leave
in the live configuration directory. It requires a durable matching terminal
receipt, complete staged pair and journal, unchanged Rust owner/Off desired
state/live pair, matching lease and a supplied fresh host gate. It checks
**every** present slot before removing any: `.new` files must equal their
corresponding staged new bytes, `.old` files the staged old bytes, all through
private no-follow, single-link descriptor reads. Arbitrary subsets are valid;
foreign, unsafe or changed slots refuse. Each fixed-name unlink is followed
by directory synchronization and full independent reinspection. No live file,
stage, journal or receipt is removed.

Synthetic tests reproduce an aborted transaction after a new slot was linked,
exercise matching subsets and identical old/new pairs, refuse unsafe or
foreign slots, and reopen after process termination at every unlink/sync
boundary. The candidate has no product caller or automatic invocation. It
must precede staged-artifact retirement whenever a slot survives.

## Inactive fixed-artifact retirement candidate

The separate `restore_cleanup_candidate` tests that protocol without a
production caller. Under the same owner lease and a supplied fresh Off/idle
gate, it first synchronizes and reopens the terminal receipt, then checks the
owner, exact desired state, live pair and complete artifact inventory. Only a
prefix of this fixed deletion order is resumable: the four staged data files,
`ready.bin`, the now-empty stage directory, terminal journal, then intent
journal. Each unlink is descriptor-relative and followed by parent-directory
synchronization and independent reinspection. There is no recursive removal.
The receipt remains unchanged as a startup/mutation fence even when those
eight artifacts are gone.

While any staged data member survives, `ready.bin` must remain and its digest
must match the transaction in the receipt. Every surviving member is checked
against that ready marker's length and digest. The cleanup refuses gaps,
unknown entries, missing/changed receipt, mismatched journals, unsafe member
types, changed owner/desired/live bindings and host-gate drift. Tests exercise
every post-unlink and post-sync interruption, including abrupt subprocess
termination, followed by independent reopen and idempotent continuation.
This is checksum binding and crash-prefix testing, not protection against a
hostile same-user writer or a guarantee about arbitrary power loss.

This candidate refuses to retire the stage while any fixed replacement slot
survives in the live configuration directory: losing the complete stage could
otherwise lose proof of a slot's provenance, especially after abort. The
separate inactive slot candidate above can be tested first, but neither has
product authority. This candidate **does not clear the receipt fence**, register a
command, expose UI, or complete T4 acceptance. Product authority, admission,
UX and installed-environment checks remain separate gates.

## Inactive native-owner retirement composition

An internal `OfflineNativeCoordinator` candidate now joins the terminal
receipt, replacement-slot retirement and fixed-artifact cleanup under one
migration lease. It requires exact Rust ownership, an Off desired state, a
fresh absence of owned core/TUN, idle mutation and auxiliary work, and no
independent lifecycle/store block or unrelated routing-preset transaction.
Every effect repeats that host check and binds the same durable receipt inode
and terminal transaction. A complete stage retires provenance-matched slots
first; a valid already-partial cleanup prefix continues directly. A partial
stage with a surviving slot refuses instead of discarding the evidence needed
to prove its provenance.

Synthetic composition tests start from an authenticated backup, exercise both
committed and aborted terminals, an interrupted new-slot creation, a partial
cleanup restart, foreign visible VPN preservation and unrelated owner/host/
queue drift. Live and desired bytes are not changed by retirement; the
receipt and startup/mutation fence survive even an idempotent retry. This is
**not** a product recovery command or an accepted installed restore flow.
Import/startup policy, explicit owner authority, receipt-fence release design,
passphrase UX and VM/host acceptance remain open. Other manually-blocked
lifecycle states are not silently reclassified as terminal restore cleanup.

## Inactive native-owner backup composition

The native coordinator now has one internal-only composition of the earlier
source, authenticated envelope and exclusive ciphertext publisher. Under its
mutation lease it checks committed Rust ownership and rejects recovery and
active/queued mutations, captures one fixed private store/template pair,
encrypts it, rechecks owner/revision and publishes only ciphertext to a new
file. A failed destination publication preserves the source; an ambiguous
post-link outcome is never reported as safe to retry. The method does not
observe, stop or reconnect the VPN and has no product caller, CLI, IPC or UI.
Synthetic evidence includes wrong passphrase, exact-source preservation,
authenticated count-only reopen, exclusive destination refusal and the
private-transaction recovery fence. This is **not** backup availability for
users: explicit destination authority, passphrase-entry UX, operational
cancellation/latency, installed-host review and a separately approved client
protocol remain open before activation.

## Inactive restore startup and imported-login gate

The restore candidate now derives a separate store copy with
`startup.enabled=false` **after authentication but before staging or journal
digest binding**. The authenticated archive remains unchanged. Last/pinned
startup choices and other records survive as data, but cannot automatically
reconnect on the restored installation. Synthetic tests cover both choices and
the exact staged copy; there is no user-facing restore operation yet.

A separate read-only production-boundary review can inspect a surviving stage,
intent/terminal journal or final receipt while holding the migration lease.
It requires an existing safe operation lock without creating or repairing it,
and checks committed Rust ownership, the existing login receipt, Off desired
state and two fresh empty-owned-host observations; it never constructs a normal
owner, repairs a pointer, mutates a file or starts/stops a core. An undecided
transaction remains a recovery candidate, not permission to roll it back.
Unsafe, unrelated or incomplete evidence remains manual recovery. A same-process
ambiguous final receipt removal also latches the coordinator into manual
recovery even if the existence fence may have disappeared.

These checks do **not** activate automatic startup recovery. The later
completion-record candidate addresses the post-restart last-unlink gap by
retaining a second fence; absence of both records still cannot prove closure.
The complete crash-checkpoint matrix, packaged recovery-only owner path,
installed synthetic restore, private transfer/confirmation UX and independent
format review remain open before T4 can be exposed to users.

## Inactive last-fence completion model

The final receipt unlink cannot be treated as proof of success after a
post-unlink I/O failure and restart. An internal pure decision model now
requires a separately durable completion record for that interval: publish
and synchronize the record while the pending receipt still exists, then
consider unlinking the pending receipt. The completed-only state is a
**verification candidate**, not normal-owner admission; both records present
remain fenced. Missing both records proves nothing about an interrupted
restore. Corruption, mismatched identities, unretired fixed artifacts or
owner/Off/live-pair drift demand manual recovery.

The model tests the crash-observable evidence combinations. A separate
fixed-size candidate record wraps the complete validated terminal receipt,
with domain-separated checksum and strict decode, so a restart reader can
recheck its owner/desired/live-pair binding even after the pending receipt
disappears. This is tear detection, not authentication against a hostile
same-user writer. The following inactive slice implements durable publication
and read-only inspection. Independent format review, retention/replacement
across a second restore and product startup interpretation remain open before
activation.

## Inactive durable completion publication and receipt retirement

An inactive candidate now adds `restore-closure.complete` to the ordinary
startup/mutation existence fence **before** any writer creates it. Only after
terminal receipt, exact Rust owner/Off/empty-owned-host gate and full
stage/journal/slot retirement are independently checked does the candidate
exclusively create the fixed private completion record, synchronize member
and state directory, reopen the same inode and repeat all bindings. An
existing or ambiguous member is never overwritten. Synthetic subprocess
crashes after create, write, file sync, directory sync and reopen leave the
older pending receipt as a fence.

The older receipt's unlink candidate now requires the matching completion
record to be synchronized and rebound first. After unlink it verifies the
surviving completion record and live pair again; `Closed` has become
`ReceiptRetiredStillFenced`. The separate read-only startup review refuses
invalid or mismatched dual evidence and classifies completion-only as a
verification candidate, never a normal owner. Synthetic abrupt-termination
checks cover both sides of the pending-receipt unlink. There is no product
caller, UI, passphrase transfer or automatic recovery, and this slice never
deletes the completion record.

Remaining gates include independent fixed-format review, exact installed
synthetic crash/boot tests, a recovery-only owner that can safely admit a
post-reboot missing `/run` lock, retention/replacement of a prior completion
record before a second restore, and a separately reviewed transition from
verified completion to ordinary owner startup. Neither the existence of a
completion file nor a locally green test removes these requirements.

## Inactive successor handoff model

A second restore cannot delete or overwrite the previous
`restore-closure.complete` merely to clear admission. A separate pure model
now binds the complete predecessor completion record to a distinct successor
intent, exact Off desired snapshot, owner generation and the planned staged
pair. The planned old pair must match the predecessor's completed live pair.
The fixed-size record contains no raw profile/template bytes and rejects
tears, wrong phase, reused transaction ID and owner/desired/stage/old-pair
drift. The fixed `restore-successor.pending` name is already included in the
ordinary existence fence, before any writer creates it.

This is **not** a second-restore implementation. The following inactive slice
only publishes the handoff; there is no predecessor-unlink operation, restart
continuation or product admission. A writer must durably publish the handoff while the previous
completion record remains, then stage and commit the matching intent before
it can even consider retiring the predecessor record. During coexistence it
must use a separate verifier; the existing completion-only inspector correctly
rejects new stage/journal artifacts and must not be weakened globally.
Structural decode alone never authorizes a successor: a future reader must
also repeat `matches_verified_plan()` against authenticated staged members and
current owner/desired evidence. Before activation, the v1 formats need frozen
test vectors and an installed downgrade policy; an older runtime that does not
recognize the new fence name must not start over it.

## Inactive successor publication only

The internal successor publisher accepts an authenticated `OpenedBackup`, not
raw new member bytes. It derives the startup-disabled store before binding its
planned stage and distinct transaction. Under the existing migration lease it
requires the exact Rust owner/Off desired snapshot, a caller-supplied fresh
idle/empty-owned-host gate, unchanged private live pair and predecessor inode,
and completion-only state. The older finalization receipt, all stage/journal
and replacement-slot names, any successor entry and unrelated routing-preset
transaction must be absent. The existing completion inspector alone is not
sufficient: it intentionally allows the old receipt to coexist.

It synchronizes the retained predecessor, exclusively creates the fixed
`restore-successor.pending` private file, writes the existing v1 record,
synchronizes file and state directory, then reopens and rechecks the same inode
and all bindings after the last gate callback. It returns only
`PublishedStillFenced`. Any uncertainty after creation leaves the name in place;
existing, partial and unsafe entries are never overwritten, repaired or deleted.
The prior closure is never removed. Synthetic fixtures exercise authentication,
startup-Off derivation, completion-only refusal, unsafe file types, owner/desired/
live/directory drift, partial writes, inode replacement and subprocess termination
after create, write, file sync, directory sync and reopen.

This slice does not stage the successor pair, publish a decision intent, change
live or desired bytes, offer retry/restart continuation, or register any product
caller. All crash states remain startup/mutation fences; a valid handoff is not
recovery authority. The inactive read-only startup review also refuses any
successor entry, including one appearing during its final host observation;
it cannot classify the older closure as completed over this new transaction.
Later staging/intent publication requires its own coexistence
verifier and full authenticated planned-member comparison. Predecessor retirement,
recovery-owner authority, installed synthetic crash/boot testing, frozen format
vectors, downgrade handling and normal-owner admission remain separate gates.
Checksums and inode checks detect tears/replacement, not hostile same-user access
or arbitrary power-loss outcomes. No host VPN, private profile or service is used
by these synthetic tests.

## Inactive authenticated successor coexistence review

A separate read-only candidate now reviews exactly the coexistence state with
the prior closure, complete successor handoff, complete four-member stage and
matching nonterminal intent. It requires the authenticated backup again,
derives the startup-disabled pair, compares all actual staged bytes, binds the
old pair to both the predecessor and current live files, and checks current
Rust ownership and exact Off desired bytes under the migration lease. It refuses
terminal/finalization/routing markers and replacement slots rather than treating
them as an execution or cleanup phase. Two complete observations surrounding a
fresh host/idle gate must agree on all record/stage/live inode identities and
private directory bindings. The older completion-only inspector stays unchanged.

The only result is `MatchingIntentStillFenced`, a point-in-time observation,
not a durability claim, durable token, predecessor-retirement permission or
ordinary startup admission. Missing/torn stages and intents, mismatched archives,
changed bindings and same-byte inode substitutions refuse. Synthetic subprocess
termination after handoff, partial stage, complete stage, partial intent and
complete intent exercises the restart-observable states without invoking an
executor. No production writer, automatic recovery or fence deletion is added.

The same authenticated reader now also verifies two explicitly selected earlier
phases: handoff-only (stage and intent both absent), and a complete matching stage
without an intent. Wrong-phase, intent-without-stage, partial-stage and torn-intent
states refuse rather than being guessed or repaired. Every phase binds the
planned old pair to the unchanged completed live pair and the new pair to the
authenticated startup-Off archive, repeats all owner/desired/fence checks around
the fresh gate, and remains a read-only fact. Crash-reopen tests cover all five
preparation checkpoints against all three phase requests. This supplies admission
checks for a future create-only preparer; it is not that preparer and does not
grant durability, retry, predecessor-retirement or normal-startup authority.

## Inactive create-only successor preparation

An internal preparer can now advance an authenticated handoff-only state to a
complete stage and the exact intent embedded in that handoff. It first rechecks
the lease, owner/Off/idle gates and pinned predecessor/handoff/live identities,
then synchronizes both retained fences. It exclusively creates the four-member
stage with a binding check after each publication boundary; any partial stage
remains ambiguous. Only a complete independently reopened matching stage admits
exclusive intent creation, file synchronization, directory synchronization and
same-inode readback. Final authenticated coexistence review must preserve both
the original live/fence identities and the stage identities observed before
intent publication. Same-byte stage or intent replacement refuses success.

The result is only `PreparedStillFenced`. No live member, desired state or prior
fence is changed or removed; there is no product caller or executor invocation.
Existing stage or intent entries, including apparently complete ones, refuse this
create-only API. Twelve interruption boundaries, actual SIGKILL/reopen checks,
owner drift during a partial stage and final artifact substitution are exercised
with synthetic data. Readable stage/intent bytes after a crash remain evidence,
not proof of prior synchronization or automatic restart permission. A separate
recovery admission/durability step is still needed before any continuation.

## Inactive interrupted successor preparation recovery

An explicit internal recovery candidate requires a freshly authenticated matching
archive and an exact caller-selected phase. Handoff-only uses the create-only
preparer. Complete stage-only evidence is reopened, every pinned file is synced,
then stage/config/state directories are synced and the whole authenticated phase
is rechecked before exclusive intent publication. Complete intent evidence is
similarly resynchronized and verified without replacing its inode. A ready marker
visible before the stage-directory sync is not treated as prior durability proof.
Partial stage, torn/empty intent, terminal evidence, changed identities, ownership
or host gates remain fenced; there is no repair, deletion or plaintext-only
recovery authority. Sync interruption is ambiguous and requires fresh admission.

The result remains `PreparedStillFenced`, not permission to execute or start a
normal owner. Current `execute_staged_pair` exclusively creates its own intent
and therefore cannot consume this prepublished intent. A separately reviewed
exact-intent execution handoff, predecessor-fence lifecycle, recovery-owner
admission and installed startup/UI acceptance remain required. Synthetic composed
publication/preparation/recovery tests do not claim a second live restore or
product recovery acceptance; passphrases are not persisted.

## Inactive exact-intent successor execution

A separate internal executor consumes the authenticated prepublished intent,
without recreating it. Its immutable-evidence guard binds predecessor closure to
staged OLD and the supplied archive's Off copy to staged NEW; it pins all eight
closure/handoff/intent/stage records and their private directories. Matching
temporary slots may appear and live bytes may progress old/mixed/new, but the
existing transaction classifier and phase-aware journal checks remain responsible
for those mutable states. Terminal evidence is pinned once first observed.
Every link and rename is surrounded by current owner/desired/host and immutable
evidence checks. Wrong archive, foreign slots, unsafe records, changed identities
and terminal/live mismatches refuse without repair or predecessor deletion.

Restart re-synchronizes immutable source files and directories before rollback.
Current live bytes need not be considered durable before rollback: each observed
old/mixed/new state without terminal is restored from the resynchronized OLD
source, and both resulting live inodes plus their directory are synchronized
before Abort is published. Another crash before that terminal remains fenced and
reclassifiable; it cannot become successful merely because both new files were
visible. Committed terminals only verify NEW, aborted terminals only verify OLD.

Synthetic coverage composes a real first stage/execution/retirement/closure with
successor publication/preparation/execution, including forward and rollback
rename-before-directory-sync process loss. This is not product acceptance or
second-restore retirement: both predecessor and successor fences remain. Normal
owner admission, successor closure lifecycle, authenticated recovery UI/API and
installed boot acceptance are still separately gated. Foreign same-user live
inode swaps beyond the existing byte/classifier trust boundary are not a new
guarantee; immutable-evidence same-byte inode substitution is explicitly refused.

## Inactive successor terminal receipt publication

Successor receipt admission is separate from single-cycle cleanup. A freshly
authenticated archive, exact C0/handoff/stage/intent/terminal binding and current
terminal live pair are required. Intent-only state cannot authorize rollback or
publication. The terminal-only verifier resynchronizes the decided live pair and
journal; all eight immutable source members and their directories are resynced
before exclusive receipt creation. Each publication boundary rechecks immutable
evidence, host gates and the terminal live pair. Final readback compares both
exact receipt bytes and the writer's created inode after the last callback.

The publisher shares only the fixed receipt constructor and create-only durable
writer, not the old single-cycle publisher's admission. Existing, partial, unsafe
or replaced receipts refuse retry. C0, handoff, all stage bytes and both journals
remain present even on success. Receipt presence therefore intentionally blocks
the next step after SIGKILL as well: a partial publication is a fence, never
automatic authority to retry, execute, retire or rotate. Receipt presence blocks
the earlier executor entry points. Receipt-driven restart/retirement and closure
rotation remain separate work: no canonical closure is deleted or replaced here.

## Read-only successor rotation phases

The reserved fixed `restore-closure.next` path is an existence fence at mutation
and startup admission before any writer exists. A new pure helper reconstructs
the successor stage identity from C0's retained OLD lengths/digests and freshly
authenticated NEW bytes, so retiring raw OLD does not discard archive binding.
This is identity proof only, never inode, durability or mutation authority.

The read-only reader pins C0/H/R1/C1, desired bytes, current live pair and every
surviving fixed artifact across two observations. Before next publication, only
complete stage and exact journals are admitted. With canonical C0 and next C1,
only the validated cleanup prefix is admitted; slots must all be absent once
stage cleanup starts. Canonical C1 with displaced C0 in next, or with next absent,
requires cleanup Done and no slots. Missing canonical, crossed/equal closures,
torn/unknown artifacts, partial stage without next, and late inode substitution
refuse. Commit receipts bind authenticated NEW; abort receipts bind C0's pair.

The prefix parser performs no synchronization. Existing cleanup keeps its prior
state/stage sync behavior through a separate callback, with a test hook forbidding
those synchronization paths during read-only phase checks. Every phase remains
fenced by H and R1. No next-closure publication, cleanup, exchange, displaced
record removal, third live restore cycle or product owner admission is provided.

## Inactive create-only next-closure publication

A separate internal publisher admits only two matching authenticated BeforeNext
observations with complete stage and journals. It pins all source bytes, optional
slot absence/presence and private directory identities; every guard reparses the
strict cleanup prefix to detect unexpected stage entries. All present sources
and stage/config/state directories are synchronized and rechecked before the
fixed next slot is exclusively created. The predecessor closure, handoff,
successor receipt, stage and journals are never removed or changed.

Each publication boundary checks those same sources and the newly created next
inode. Only empty bytes at creation and exact C1 bytes after writing are allowed;
same-byte inode replacement, late gate changes and source-directory swaps refuse.
After file and parent sync, a fresh two-pass NextPublished review and final
source/destination checks precede `PublishedStillFenced`. Neither this result nor
the read-only phase grants normal startup or permission to retire anything.

Existing, empty, partial or valid next records refuse this create-only entry.
Actual process loss at publication checkpoints leaves all prior fences intact.
A complete next visible after process loss is not proof of durability: a later
separately reviewed recovery/cleanup candidate must re-establish it. Cleanup
prefix advancement, atomic canonical exchange, displaced closure retirement,
handoff/receipt retirement and third-cycle/product acceptance remain open.

## Inactive successor cleanup under both closures

A separate cleanup-only candidate freshly authenticates NextPublished evidence,
pins every surviving source and private directory, and synchronizes all surviving
files plus their directories before its first deletion. Restart from every valid
prefix repeats this durability step; visible C1 or a previous reader result is
never cached mutation authority. Wrong archives, missing/torn/crossed closures,
unknown stage contents, unsafe members and journal holes remain fenced.

An initial executor result may contain any of the sixteen matching replacement
slot subsets while the complete stage and journals survive. Missing slots are
not attributed to this cleanup writer or used as proof of previous progress.
Each present slot is independently verified against the complete stage, and
fresh authenticated closure/receipt/live proofs supply the separate authority.
The writer removes present slots in fixed order before beginning stage cleanup.
During an invocation, any non-selected disappearance or same-byte substitution
poisons the exact expected transition.

After all slots are absent, the four stage members, ready marker, empty stage
directory, terminal and intent are retired in their fixed order. Each unlink is
followed by fresh two-pass verification permitting only that one absence and the
exact next prefix, parent-directory synchronization, and another complete check.
The maximum is twelve removals; there is no repair loop or recursive deletion.
C0, C1, handoff and receipt keep their original bytes and inodes throughout.
The live decided pair and desired Off binding are unchanged.

Hook interruptions and actual SIGKILL/reopen cover every unlink and directory
sync, including repeated process loss during restart resynchronization. Even Done
remains `CleanedStillFenced`: canonical exchange, displaced closure retirement,
last handoff/receipt closure, third-cycle and normal-owner/UI/installed acceptance
are separate gates. No product caller or startup permission is introduced.

## Inactive atomic canonical-closure exchange

Only authenticated NextPublished with cleanup Done, no slots and the exact six
retained closure/handoff/receipt/live files admits exchange. Every file and both
private directories are resynchronized and rechecked first. A safe typed Linux
GNU `renameat2(RENAME_EXCHANGE)` exchanges only the two fixed closure names.
Unsupported platforms, syscalls or filesystems refuse; no sequential rename
fallback exists. Both names remain, with C1 canonical and C0 displaced in next.

The writer pins both file descriptors before the syscall. A successful exchange
may change their ctime, so only this owned transition uses a local comparator
that preserves device/inode, ownership, mode, link count, length and mtime.
Post-syscall metadata is captured before any callback and installed into the
expected swapped snapshot. All subsequent two-pass checks use ordinary strict
member comparison, including ctime, and exact swapped bytes. The global member
comparison is unchanged. State-directory synchronization and final readback are
required before `ExchangedStillFenced`.

Any failure after the syscall is ambiguous and never triggers a rollback. A
separate authenticated Exchanged resynchronizer verifies that exact phase and
only syncs/rechecks surviving evidence; it never calls exchange. Re-entering the
Exchange operation on Exchanged refuses, preventing accidental swap-back after
process loss. Commit and abort fixtures cover real post-syscall/directory-sync
crashes and repeated loss during resynchronization. Neither closure, handoff nor
receipt is removed. Displaced-record retirement and explicit post-handoff/last
receipt phases remain separate gates; the old completion reader alone does not
prove handoff/next absence or normal-owner readiness.

## Inactive displaced-closure retirement

Only a freshly authenticated Exchanged/Done phase admits removal of the fixed
next name holding C0. All six surviving files and both private directories are
resynchronized first, with exact two-pass checks around each synchronization.
The next inode is pinned before unlink. The only accepted transition is next
absence plus unchanged canonical C1, handoff, receipt and live-pair bytes and
inodes. State-directory synchronization and fresh readback complete the operation.
Failures after unlink remain ambiguous; no recreation or repair is attempted.

A separate DisplacedRetired resynchronizer requires next absence and preserves
the other five files. It never unlinks anything. Retire on DisplacedRetired
refuses, while restart after visible unlink but before directory synchronization
must resynchronize and recheck the retained evidence. Commit/abort synthetic
fixtures cover all effect checkpoints and actual repeated SIGKILL/reopen.
This is process-crash evidence, not physical power-cut acceptance.

The result remains `DisplacedRetiredStillFenced`. Handoff and receipt retirement
need explicit additional post-handoff/post-receipt phases and are not performed
here. Canonical C1 remains a permanent startup fence. Third-cycle admission,
normal-owner recovery, product UI/CLI and installed acceptance remain separate.

## Inactive final-closure output review

A separate read-only reader recognizes exactly three fenced phases: before
handoff retirement (H and R1 survive), H absent with R1 present, and H/R1 both
absent. H present with R1 absent refuses. Next, stage, journals, replacement
slots and routing-preset pending must be explicitly absent; only ENOENT counts
as absence. Torn, unsafe or inaccessible optional receipt entries refuse.

While H survives, the existing DisplacedRetired classifier supplies the complete
predecessor relationship. Without H, canonical C1 must wrap exactly R1 whenever
R1 survives, bind the current owner/desired Off state, and match the live pair.
Committed output must equal the freshly authenticated archive's Off pair. For
Abort, the terminal stage identity is reconstructed from live OLD and fresh
authenticated NEW bytes. The intent-only successor helper is not weakened.
Identical OLD/NEW bytes do not erase the recorded Commit/Abort distinction.

After H is gone, these checks prove only bounded terminal output/completion
evidence. They cannot reconstruct predecessor lineage, prove prior fsync, grant
live replacement authority or admit a normal owner. Two observations compare
phase, exact presence, bytes, inode metadata, directory identities and desired
state; the reader performs no sync, unlink or repair. Canonical C1 remains the
startup fence. A phase-specific H-then-R1-last writer, restart resynchronization,
third-cycle admission and installed product acceptance are still separate gates.
In particular, the older completion inspector does not itself exclude the
next-closure slot. The separately described publisher hardening below supplies
that exact absence/pinned-source gate; this reader does not authorize that path.

## Inactive handoff-only retirement

The next mutation slice admits only BeforeHandoffRetirement with complete
authenticated predecessor evidence, fixed absences and the exact C1/H/R1/live
snapshot. It synchronizes all five surviving files and both directories, pins
H, rechecks the snapshot, and unlinks only H. The accepted post-effect snapshot
is exactly HandoffAbsentReceiptPresent with unchanged C1/R1/live bytes and
inodes. Directory synchronization and fresh readback are required afterward.
Any failure after unlink is ambiguous, with no rollback or recreation.

Restart in HandoffAbsentReceiptPresent has a separate resynchronizer. It
reauthenticates terminal output (not predecessor lineage), synchronizes the
four remaining files and both directories, and never unlinks anything. Retire
on that phase refuses; missing R1 also refuses this slice even when C1 survives.
Tests cover same-byte H replacement after pin, all synchronization/effect
interruptions, retained-source substitutions, Commit/Abort and actual process
death before unlink, after unlink, after directory sync and during restart.

The result remains HandoffRetiredStillFenced. R1-last retirement, third-cycle
publisher hardening and normal-owner/installed admission remain separate gates.

## Inactive last successor receipt retirement

Only HandoffAbsentReceiptPresent admits R1-last removal. Canonical C1, R1 and
both live members are pinned and resynchronized together with both directories.
The final reader independently requires H/next/stage/journals/slots/routing
absence before and after effects. The writer pins R1, checks again, and unlinks
only R1. It accepts exactly HandoffAbsentReceiptAbsent with the same C1/live
bytes and inode metadata, then syncs the state directory and reopens evidence.
Failure after unlink is ambiguous; no receipt recreation or fallback occurs.

A separate completion resynchronizer requires C1-only and freshly authenticated
output binding. Visible C1 after a process crash is not treated as durable until
C1, the live pair and both directories are synchronized and rechecked. This
resynchronizer never unlinks anything; repeated Retire on C1-only refuses.
C1 remains the permanent startup fence, not normal-owner permission.

Abort recovery still needs the authenticated NEW archive to reconstruct its
stage digest from live OLD plus NEW. Missing archive, wrong passphrase or an
archive containing only the OLD output cannot substitute for that input. This
is a manual-recovery boundary if the user-selected archive is unavailable after
restart; no NEW data is inferred from C1/OLD and no passphrase is persisted.
Future product acceptance needs an explicit archive-re-supply/recovery UX and
must not claim unattended recovery here.

Synthetic Commit/Abort checks cover all ten operation checkpoints, five final
resync checkpoints, pinned R1/source swaps, C1 same-byte substitution, late
receipt/H/next/intent reappearance, and repeated real SIGKILL/reopen around the
last unlink. They do not certify physical power loss. Third-cycle publisher
next-slot hardening, reusable-name composition and installed normal-owner
acceptance remain separate gates.

## Inactive reusable-name three-cycle synthetic acceptance

The handoff publisher now excludes every next-slot entry at initial admission
and every source check, including after each create/write/file-sync/directory-
sync checkpoint. A late next entry stops publication immediately and retains
all fences; no old or foreign slot is deleted. Source pinning is rechecked at
those boundaries as well. The prior operation's archive-specific final reader
is deliberately not called with the newly selected successor archive.

The composed synthetic fixture performs a real first stage/execution/receipt/
closure, then a full second Commit or Abort through last-receipt retirement.
Actual SIGKILL after the second last unlink and again during completion resync
requires a new lease and fresh authenticated resynchronization. All four
second/third Commit/Abort combinations then publish a fresh third handoff,
prepare, execute/recover, publish receipt/next, clean, exchange and retire the
displaced record, handoff and last receipt using the same fixed names. Final
checks require the third transaction ID and recorded terminal choice, exact
decided live bytes, all reserved transient-name absences and the third canonical
completion still present. Reusing the predecessor transaction ID refuses before
publication.

This is inactive synthetic reusable-name/protocol acceptance, including repeated
process-crash recovery. It is not installed normal-owner, reboot/power-cut,
private provider, CLI/QML recovery UX or marketplace/release acceptance. The
permanent canonical closure still fences normal startup. Abort archive re-supply
and a separately reviewed product recovery owner remain explicit product gates.

## Inactive final-closure startup review adapter

The production-boundary adapter acquires only an existing migration lease and
requires committed Rust ownership, exact fixed desired/owner paths, desired Off,
and two fresh observations with no owned runtime objects. Foreign visibility is
not ownership. The existing login check permits absence or a matching consumed
receipt; this diagnostic does not prove current user-manager epoch freshness.
It never constructs a coordinator, reconciles, repairs permissions, syncs files,
retires evidence or admits the ordinary owner.

The authenticated final reader's exact two-pass evidence spans the second host
observation. Separate snapshots pin owner, desired and optional login bytes and
member identity, plus state/runtime directory identity, before and after both
host observations and before return. All three final phases remain fenced;
normal startup's existing refusal is unchanged. Post-handoff output/completion
evidence is not reconstructed predecessor lineage or durability proof. Abort
still requires re-supply of the authenticated new archive. This adapter has no
product caller and grants no mutation, login or IPC capability.

## Shared migration lease identity hardening

Both shared-lock constructors pin the private runtime directory and exact held
file inode. Existing unsafe locks refuse. Normal
creation uses exclusive create on a safely absent fixed name; an intervening
creator causes refusal, not an unchecked reopen. Permission adjustment applies
only to the newly created descriptor or the exact legacy 0644 inode after
exclusive flock and full identity recheck, never a pathname. Frozen Python used
plain append-open, so preserving Busy and tightening that bounded legacy case
is necessary; other non-0600 modes refuse. Existing-only recovery
review still cannot create a lock. The legacy-compatible lock name and nonblocking
busy behavior are unchanged.

Every `authorizes` call rechecks the pinned runtime directory and the held/current
lock inode, owner, private mode and single link. An old process holding an
unlinked lock cannot authorize merely because a replacement uses the same name.
Ownership-marker writes use this same check before reading/preparing state and
again immediately before replacement; they cannot bypass it with uid/path alone.
This is a point-in-time stale-lease check, not atomic exclusion of arbitrary
same-UID filesystem attacks between a check and an effect. Existing per-effect
gates remain necessary; cooperating processes must not delete or replace an
active lease. Missing `/run` recovery construction, reboot/epoch provenance and
normal-owner startup remain separate gates. Package units specify private runtime
directories and `UMask=0077`; the accepted runtime parent is exactly mode 0700.

## Inactive lost-lease final-closure diagnostic

The separate missing-name adapter requires fixed paths, preliminary Rust/Off
state and a valid canonical closure, then exclusively creates an absent fixed
lock within an already-existing private runtime directory. Any existing name,
including a safe unlocked 0600 file, refuses: this is not an unchecked retry or
legacy permission migration. It passes the same newly held lease directly into
the authenticated final-review boundary with two empty-owned-host observations
and exact source/owner/desired/login/lock snapshots; it never drops and reacquires
between creation and review.

Only the inert lock file may be created. Failure after creation retains that
file and every restore fence; re-entry must deliberately use existing-only
review. There is no runtime-directory creation, source repair, effect-capability
return, C1 removal, normal-owner construction or product caller. A returned phase
is informational, never a grant for later mutation. Archive re-supply and the
post-H output-only lineage limitation are unchanged.

Synthetic checks cover every final phase and Commit/Abort, existing/unsafe/raced
names, missing runtime parent, old held-inode invalidation, unchanged source bytes
and inodes, late closure substitution under the same flock, and actual SIGKILL
after lock creation followed by existing-only re-entry. A fixture recreates its
own volatile directory to model missing runtime state; that is not an actual
reboot, user-manager epoch proof, old-process-death proof or power-loss acceptance.

## Inactive private recovery request candidate

This internal reader consumes one already-connected Unix stream; it does not
create a socket, listen, register an IPC method or execute recovery. Kernel
`SO_PEERCRED` must identify the current UID (with real/effective UID equal and
a positive peer PID) before even the fixed header is read. No caller-supplied
path, UID, operation override or timeout is accepted. Same-UID socket-pair tests
exercise the kernel check; cross-UID kernel acceptance remains an installed gate.

The candidate header is exactly 20 bytes: `OVRREQ01`, one fixed resync-completed-
still-fenced opcode, three zero reserved bytes, a little-endian u32 ciphertext
length, a little-endian u16 passphrase length, and two zero reserved bytes. Only
nonempty ciphertext bounded by the envelope maximum and 12–1024 passphrase bytes
are accepted. Checked total length is validated before proportional allocation.
Ciphertext then passphrase occupy one owning zeroizing buffer; accessors borrow
without String/JSON copies. The request has no Debug, Clone or production encoder.
Malformed header and trailing-byte buffers are also zeroizing. This covers normal
drop/unwind, not SIGKILL, allocator/kernel copies or physical memory forensics.

One three-second absolute deadline spans header, body and exact EOF; a sender
must half-close after its frame. Truncation, trailing data, unsupported opcode,
version/reserved fields, oversize, peer mismatch and I/O failure yield one fixed
non-private refusal. No archive open/KDF is called. Accepted framing is not
archive authentication, semantic validation, lease ownership, C1 proof or
permission to resynchronize. The future operation boundary must separately hold
one continuous verified lease and exact C1/owner/Off/host evidence while
authenticating an archive with payload matching the intended NEW pair.

This slice depends on the reviewed caller-owned zeroizing Argon2 workspace, but
does not connect that primitive to an operation. No general IPC protocol, CLI,
QML property, argv, environment variable, log, normal startup or mutation is
changed. Listener lifecycle, endpoint permissions, connection/resource limits,
private UI secret transport, admission integration and installed synthetic clean
restore remain separate review/acceptance gates.
In particular a future listener must bound concurrent readers/KDF work and
aggregate allocated memory before per-connection allocation; one maximum-size
frame per connection is not a global resource limit.
