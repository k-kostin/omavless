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
