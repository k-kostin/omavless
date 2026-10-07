# T4 private backup and restore proposal

The owner-approved [execution policy](../development/EXECUTION_POLICY.md)
selects a separate availability-oriented retained-manager actor SERVICE
direction. Its [opt-in Rust implementation](../development/T4_MANAGER_ACTOR_SERVICE.md)
continues Draft #658; the first real scenario is a fixed PID1 observation,
not backup/restore admission. This does not accept or relax the historical
caller-local Bundle contract or change this product's activation gates.

The [opt-in normal private-pair API](../development/T4_PRIVATE_PAIR_NORMAL_API.md)
uses the retained engine and bounded replay metadata. Its exact source `4983f392`
passed a separately selected agent-attended installed Off private-pair sequence:
export, ordinary synthetic data import, Restore, historical replay, fresh-ID
denial with usable unchanged state, Backup and independently earned normal
restart. The owning API contract preserves the failed predecessor and exact
scope46 evidence. This does not activate default methods, a Settings picker,
Restore UX, whole settings or SLEEP/network/OS-transfer guarantees.

Status: ordinary product activation remains **not approved**. The historical
inactive envelope/transaction foundations below are now supplemented by the
explicit developer-feature checkpoint, not default Backup/Restore registration,
picker, scheduler or normal Restore UX. The development-only first-Abort
recovery command is scoped separately below. A backup file contains reusable
VPN credentials and subscription bearer URLs; it is not a support report.

Developer-only installed follow-up: the exact `9b7f33d9` genuine normal-current
export→NEW/Committed completion→ordinary replay/readback→normal daemon restart
passed the [agent-attended VM gate](../testing/T4_INSTALLED_CURRENT_VM_2026-10-06.md).
That report preserves the earlier refusals and harness correction and lists the
remaining current-origin recovery, repeated-cycle and product-activation gates.
It does not activate default/public Backup or Restore or close T4 as a whole;
the older inactive sections below retain their own historical scope.

Development-only follow-up: the [explicit first-Abort CLI slice](../development/T4_FIRST_ABORT_CLI.md)
adds a normal command for the already authenticated, first-cycle rollback path,
not first Restore/Commit or normal-owner admission. It requires a stopped
runtime's existing singleton lock and retains all recovery fences. Its new
caller still needs exact-head source and separately authorized CLI-binary VM
acceptance; earlier private-composition process-loss evidence does not accept it.

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
It also recognizes the normal managed-selection producer's **exact default**
counterpart in those three modes: after the sole `  device: Meta` line, insert
`  disable-system-dns: true` followed by `  omavless-dns-broker: true`, with
their original indentation and LF endings. This adds three trusted byte
alternatives to the nine pristine combinations, not managed China/Iran,
`omavless0`, arbitrary flags or a YAML normalization policy. The transformation
is applied only to trusted checked-in bytes for comparison; input is never
parsed, repaired or rewritten as a template.
Twelve positive combinations and cross-preset mismatches are covered.
Both borrowed members remain byte-for-byte unchanged. Portable custom rules and
startup preferences remain store data; this gate does not render, merge or
activate them, nor infer a saved routing mode from an unrelated store field.

The managed flags are portable **data**, not permission to use a DNS broker,
enrollment, selected package, core/device family or runtime ownership. No such
authority is included in the archive. Normal activation must independently
earn its existing local package/selection/enrollment and owner gates. Default
backup/Restore registration and installed activation are unchanged.

Unknown/custom/unconfigured presets and edited templates refuse, including
comments, controller additions, duplicate mode keys and line-ending rewrites.
Missing, false, duplicated or reordered managed flags, another device, extra
provider/controller/script keys and even altered whitespace refuse. Managed
roundtrip tests authenticate and retain exact source bytes in all three modes;
the strict store schema, including refusal of semantic repairs, is unchanged.
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

The later [private first-restore owner composition](../development/T4_FIRST_RESTORE_OWNER_EXECUTION.md)
connects authenticated native-owner staging to this executor under one lease,
retaining the staging writer's original descriptors and exact owner gates.
It returns only Commit-still-fenced and blocks the owner after the operation;
explicit Abort recovery and product registration remain separate. This is an
inactive implementation bridge, not installed restore or historical adoption.

The [explicit Abort prerequisite](../development/T4_FIRST_RESTORE_ABORT_OWNER.md)
adds a private Intent-only lower rollback entry retaining the exclusive writer's
original Abort descriptor through post-publication checks. Its inactive private
recovery-only caller authenticates NEW against a fresh archive, retains original
stage/live/slot identities and resynchronizes sources under a fixed observational
host boundary. It returns only AbortedStillFenced, never an ordinary owner; OLD
has checksum transaction provenance, not archive authentication. No normal
registration or installed acceptance is implied; ordinary blocked owners and
startup fences are unchanged.

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

## Inactive one-shot C1-only recovery session

A private consuming session holds one uninterrupted existing migration lease,
the pinned owner/desired/login/runtime boundary, fresh authenticated archive and
opaque exact final-source evidence across admission and resynchronization. Its
only operation calls the bounded C1-only resync and returns `StillFenced`; it
cannot unlink, rewrite, start a normal owner or export a reusable phase-based
capability. Missing locks must be handled separately, not recreated implicitly.

The evidence capture is strictly read-only, stores zeroizing Off bytes and a
borrow of the authenticated template, and exposes no raw members or Copy/Debug.
It is identity evidence, not authority by itself. Every resync gate rechecks the
original C1/live/source phase, owner/desired/login/lock identities and a fresh
empty-owned-host observation before and after that observation. A different
valid record or same-byte replacement cannot silently replace initial admission.
Only the C1-only phase is accepted; any handoff, receipt, next, stage or journal
entry refuses. Errors after any sync, including the final return gate, preserve
the canonical startup fence and require fresh admission.

Synthetic tests cover the admission-to-first-sync gap, all five file/directory
post-sync boundaries and the final wrapper gate, hostile substitution, host
drift, wrong archive, and fourteen actual SIGKILL/re-entry cases across Commit
and Abort. They preserve exact source bytes/inodes and do not establish installed
clean restore, user-manager epoch freshness or physical power-loss acceptance.
Product passphrase transport, destination UX, independent encrypted-format
review, installed synthetic restore, and EN/RU rendered recovery states remain
separate gates. This is not RC readiness or normal-owner startup admission.

## Pure closure disposition policy model

The inactive in-memory model distinguishes no closure evidence, unresolved C1,
transient fences, missing authenticated recovery, unresolved policy, manual
recovery and a hypothetical historical candidate that remains fenced. Its
opaque binding covers exact closure identity, terminal transaction/outcome and
owner generation. A matching disposition assertion is only an input to this
model: there is no disk format, encoder, publisher, filesystem observer or
normal-owner admission capability. Neither checksums nor caller-supplied facts
prove authenticated recovery or durability.

All unresolved H/R/next/stage/journal/slot/routing evidence overrides hypothetical
history. Invalid or unknown evidence refuses. Commit and Abort remain distinct
even for identical output bytes. A wrong archive refuses; absent fresh archive
does not become acceptable because a hypothetical disposition exists. After H
retirement the claim is terminal output/completion only, not predecessor lineage.

Normal operation will legitimately change profiles/templates after a restore.
Consequently a permanent C1-to-current-live equality check would prevent later
startup, while ignoring divergence without a reviewed disposition policy would
lose the recovery boundary. Generation rollover, later edits and unknown live
state therefore return policy-unresolved, not permission. This model does not
choose whether a future durable disposition may replace archive re-supply.

Before any writer or startup change, review durable publication and restart
ordering under one lease, current user-manager/login epoch semantics, missing
volatile runtime state, first-run versus lost evidence, private bounded secret
transport, and authorized ordinary mutation after admission. Preserve C1 rather
than deleting the last provenance record; transient artifacts must always retain
priority. Existing normal startup and mutation existence fences are unchanged.

## Inactive pre-authentication closure status

A separate read-only diagnostic can request archive re-supply without already
having one. It uses only an existing verified lease and accepts only a private,
bounded C1 with exact owner/Off desired binding, private live members, and no
H/R/next/stage/journal/slots/routing pending evidence. Two exact source/boundary
snapshots bracket fresh empty-owned-host observations. Unsafe, missing, unknown
or changing evidence refuses; no lock or directory is created or repaired.

Its sole status is `NeedsAuthenticatedArchiveStillFenced`. Decoding a checksum-
valid C1 is not authenticated Commit/Abort, live-pair equality, compatibility,
completion, durability or startup authority. Even pre-existing changed live
bytes can only request authentication; the later authenticated adapter must
independently reject inappropriate output. No raw bytes, outcome or phase is
returned. Login absence/consumed checks still do not prove current boot epoch.
There is no public IPC, secret input, recovery effect or normal-owner caller.

## Proposed two-phase disposition policy — owner decision required

The separate pure model explores, but does not approve or implement, a transition
from unresolved C1 to historical terminal-output evidence. Its schema/domain
labels are simulation inputs, not an on-disk format or reserved path. It has no
writer, persistence observer, normal-owner admission or reusable capability;
all candidate results explicitly remain fenced.

Dev-VM research of a completed disposition replacing fresh archive re-supply on
later startups is approved **only within the exact same Rust ownership generation
and UID**. Product admission remains unapproved until its reader, effect gates
and acceptance are reviewed. Ordinary native validation must handle subsequent
live edits. This must not become a `generation >= old_generation` rule. Rollover,
missing/torn/orphan evidence or ticket/C1 mismatch refuses.

### Before disposition

- Require one continuous verified lease and pinned C1/source boundaries, exact
  current owner/desired binding, Off, fresh empty-owned-host observations and
  C1-only state with every H/R/next/stage/journal/slot/routing fence absent.
- Require a freshly authenticated archive whose payload matches staged NEW,
  including Abort where current output is OLD. If OLD and NEW payloads are
  byte-identical, any correctly authenticated envelope carrying those exact
  payload bytes is semantically equivalent: C1 does not identify the originally
  selected ciphertext file. Commit and Abort remain distinct terminal outcomes.
- A future publication protocol must be create-only, bounded and private, sync
  its bytes and parent, reopen and recheck exact evidence before the first
  ordinary effect. A visible checksum-valid ticket after interruption is not
  durability; it requires a separately reviewed resync/restart path.
- Define the durable commit/linearization point and ambiguity handling before
  implementing publication. Successful process return is not that definition.
  Observable source drift before commit must refuse; a checksum or model fact
  does not establish authentication, provenance, durability or absence of drift.

### Proposed completed disposition

Under the proposed policy, immutable exact C1 plus UID/generation/schema/domain
binding would describe historical terminal output, **not current live lineage**.
Legitimate later edits and Connect must use normal native schema/revision/lease
checks rather than permanent equality to archived profile/template bytes. The
model therefore requires a separate ordinary-live-validation fact and does not
require re-supplying an archive in this hypothetical completed phase. The model
requires the archive input to be explicitly `NotSupplied` in that phase: a
supplied valid or invalid archive fact refuses rather than being silently
ignored. The other before-publication output/Off facts are inapplicable after
completion; current live validity is independently required. This is
the Dev-VM-only policy choice, not existing product behavior.

Any transient fence still overrides the historical candidate. A subsequent
restore must first establish a durable transient fence, then invalidate the old
ticket through a separately reviewed operation. An old ticket never authorizes
rotated successor C1. Missing ticket after legitimate later edits means manual
recovery, not silently replaying an archive or deleting C1. Canonical C1 remains
retained; these records are not a complete predecessor audit history after H is
retired and do not protect against arbitrary hostile same-UID rewrites.

### Caller matrix before any startup exception

Never change the existence-only `pending_private_transaction::pending_at` into
“ticket exists, therefore clear.” A future ticket path must itself be registered
as a conservative existence fence before any writer. Only separately reviewed
typed, lease-bound admission could distinguish historical evidence at each
caller; all unknown/transient evidence remains refusal.

| Current caller | Required future integration/re-entry tests |
| --- | --- |
| `login_transaction` receipt review, startup consume and start gates | Current user-manager/login epoch remains independent; stale epoch and C1/ticket swap between checks refuse. |
| `connection_transaction` blocked-state gate | Connection/mutation effects retain the same lease and recheck historical identity plus all transient fences. |
| `native_coordinator` admission snapshot, mutation and replay paths | Admission replay cannot reuse stale generation/ticket evidence; late H/next/journal appearance blocks each effect. |
| `native_coordinator::batch` background admission | Async refresh/fetch cannot regain authority from a cached status; publication revalidates exact owner and fences. |
| `native_coordinator::restore_candidate` restore admission | Historical disposition never grants successor authority; a fresh durable transient fence must precede separately reviewed old-ticket invalidation. |
| `backup_source_candidate` source capture | Historical permission cannot mix store/template generations or override pending restore evidence. |
| `production_cutover` ownership transition | Exact-generation policy refuses rollover until an explicit new transition design is approved. |
| `production_owner` restart/recovery reviews | Diagnostic phases remain non-authoritative; normal startup requires a new typed integration, not a Copy status. |

The pure matrix tests cover before/visible/completed hypothetical states,
identity/UID/generation/schema/domain mismatch, missing archive, later edits,
successor mismatch and transient precedence. A static source-retention test
checks that these eight callers still use the existing presence predicate and
do not import the model. **This is not behavioral integration coverage of future
admission.** Each row still needs its real admission/effect/crash tests, plus
installed restore, private UI/IPC and normal-owner acceptance after owner policy
approval. No current normal-startup or mutation fence is weakened.

## Inactive create-only disposition-ticket candidate

`restore-disposition.pending` is a new conservative existence fence, not an
approved historical disposition or startup permission. Empty, partial, complete,
inaccessible, symlink, directory and other unexpected forms all block normal
startup. The successor publisher, public final-closure review and pre-auth
status also refuse it. No ticket-presence exception is added to `pending_at`.
Independent review identified direct inactive-effect paths that do not enter
normal startup. Their shared gates now also refuse the fixed ticket: first-cycle
staging/executor/receipt/slot/cleanup/finalization, successor coexistence and
preparation, successor execution/receipt evidence, rotation and the repeated
next-publication check. The exception remains solely the ticket publisher's
private exact-owned-destination path, not a generic "ticket exists" permission.

The fixed candidate record contains versioned magic, UID, exact owner generation,
zero reserved bytes, the entire canonical C1 record and a domain-separated SHA256
checksum. Nested terminal outcome, transaction, desired/output and stage digest
bindings remain exact. Unsupported size/version/reserved fields, torn checksum,
noncanonical nested C1 and crossed generation refuse; current UID/generation/C1
are independently checked. A checksum is corruption detection, not authentication.
No passphrase, profile/template bytes, selected archive path or ciphertext-file
identity is persisted. This candidate format is not a product compatibility or
historical-admission commitment.

The low-level inactive publisher borrows one continuously held existing lease
and an authenticated `OpenedBackup`. Freshness and retained-source lifetime remain
the caller's boundary obligation; the ticket itself authenticates nothing.
Admission requires exact C1-only terminal
output. Its caller must provide fresh Off/idle-host and login/owner-boundary
gates; there is no product adapter or listener. Internally it pins C1/live bytes
and metadata, state/config/runtime directories, ownership/desired/login member
identities and the lease. It synchronizes C1/live files and state/config parents
before exclusively creating the fixed 0600, single-link ticket. Each source and
destination checkpoint rechecks the same original evidence. File sync, parent
sync, descriptor-relative reopen and exact bytes/inode re-verification precede
`PublishedStillFenced`. The just-created zero-length prefix has a separate strict
metadata check; it is never accepted as a valid source record.

After successful exclusive creation, any write/sync/reopen/gate error is ambiguous
and leaves the prefix intact. There is **no unlink, rollback, existing-ticket
retry, repair, resync-to-history or ordinary-owner grant**, even for apparently
complete bytes after process death. Public final evidence refuses all tickets;
only the publisher's private source observation can coexist with its exact held
destination, without weakening public reader semantics. C1 remains unchanged.

Tests cover Commit/Abort publication and replay refusal, bounded canonical format,
rechecksummed invalid metadata, unsafe/partial existing destinations, immutable source
and destination substitution, late marker/pending/directory/gate changes and
actual process SIGKILL at five publication checkpoints for both outcomes. These
include initial/late ticket injection against direct successor execution, next
publication, exchange, displaced/H/R retirement and direct staging; a static
source matrix only protects retention of the other shared gates, not behavioral
coverage of every leaf operation. These
are synthetic filesystem/process tests, not power-cut or installed product
acceptance. Host observation integration, a reviewed interrupted-ticket protocol,
owner-approved historical policy and typed normal-admission integration remain
separate gates; subsequent restore must not ignore or silently delete this ticket.

### Existing-complete-ticket resync candidate

A separate inactive entry can resynchronize an already complete ticket after
process interruption, returning only `ResynchronizedStillFenced`. This does not
prove that its original publication finished, establish historical admission,
or promote the pure model's hypothetical durable-disposition input into authority.
The create-only publisher still refuses replay. Empty, partial, missing, unsafe,
foreign, crossed UID/generation/C1 or incompatible-output tickets refuse without
repair, creation, overwrite or deletion; C1 and the ticket remain fences.

Recovery requires an authenticated payload matching staged NEW, including Abort
where the live pair is OLD. An unavailable/mismatching payload cannot be inferred
from OLD or the ticket. The caller retains the same existing lease and fresh
Off/idle/login/owner boundaries. The private recovery path opens the existing
ticket read-only/no-follow, pins exact canonical bytes and pathname/descriptor
identity, captures owner/desired/login and state/config/runtime boundaries before
host callbacks, and requires matching source observations before any sync. Public
final readers continue to require ticket absence.

It synchronizes pinned C1 and live members, state/config/runtime directories, the
existing ticket and its parent, reopening/rechecking immutable sources, ticket
bytes/inode and lease after every effect and final callback. A resync error leaves
the fences intact; no rollback, truncation, normal owner or IPC activation exists.
The same-user point-in-time and installed/power-cut limitations above still apply.

Synthetic tests cover Commit/Abort repeatability, wrong archive payload,
unsafe/partial/foreign records, stale lease/generation, source and
ticket replacement at every checkpoint, admission-boundary drift, final gate
failure, actual writer SIGKILL before/after ticket sync and actual resync SIGKILL
followed by fresh re-entry. Shared direct-effect fences retain their initial/late
ticket tests and static caller matrix. The older first-cycle receipt publisher's
last callback now rechecks ticket absence before reporting its still-fenced
outcome, closing the previous diagnostic-only gap without adding authority.
An authenticated `OpenedBackup` reference is mandatory in the API; missing-input
UX and passphrase transport are not exercised by these tests and have no fallback.

### Private one-shot ticket recovery boundary candidate

The separate inactive production-boundary adapter retains one existing migration
lease from admission through complete-ticket resync. Its private consuming session
pins the Rust ownership marker, desired Off state and login receipt boundary;
the inner resync pins the original authenticated C1/live/config and exact ticket
before the first host callback. No phase/result can be carried out and reused as
recovery authority. Every later callback is bracketed by original evidence and
lease checks, plus fresh complete empty-owned-host observation and owner/Off/login
checks. Only `ResynchronizedStillFenced` is returned, without constructing an owner,
creating a missing lock, listener registration or any historical/startup override.

Caller obligations remain explicit: paths must come from trusted product path
derivation, not merely pass shape validation; the borrowed `OpenedBackup` must be
freshly authenticated by the eventual private request flow. Borrowing alone proves
neither passphrase freshness nor original ciphertext-file identity. Consumed login
generation does not establish the current boot/user-manager epoch. No integration
currently discharges these product obligations, and ordinary startup remains fenced.

Synthetic checks cover Commit/Abort, lease contention and missing/stale leases,
wrong archive/UID/generation/Off/login/host state, source substitutions in the first
and later host callbacks, late transient evidence and final-observation refusal.
Wrapper process SIGKILL/re-entry complements the lower-level per-fsync matrix;
neither proves physical power-loss, installed clean restore or product acceptance.

### Inactive ordered-completion candidate

The next Dev-VM-only slice adds a second fixed private record,
`restore-disposition.complete`. It is **not** a startup exception: both the
pending ticket and the completion record are conservative existence fences in
normal runtime and direct inactive successor effects. The public final review
also requires both to be absent. The new record embeds the complete canonical
ticket (UID, exact generation, C1, transaction and terminal outcome) with a
separate versioned, domain-separated checksum. It contains no profile payload,
passphrase, subscription URL or archive path; the checksum is not authentication.

Its inactive create-only entry retains one existing lease and the same pinned
sources across authenticated complete-ticket resync and completion publication.
It requires the caller's fresh owner/Off/login/empty-host observation throughout.
After source resync and re-verification, it exclusively creates a private 0600,
single-link record, writes and syncs its bytes and parent, then reopens and
checks exact bytes, inode, ticket and original sources. Any error after creation
leaves the visible prefix and both fences in place; no retry, repair, unlink,
ordinary-owner admission or VPN effect follows. A complete record without the
pending ticket is an orphan that still blocks startup.

Synthetic Commit/Abort, wrong-prefix and late-gate tests plus actual process
SIGKILL at each completion checkpoint cover this ordering. They do not prove
power-cut durability, authenticated archive UX, current boot/user-manager
epoch, or installed product behavior. A future typed historical reader must
bind both records to a fresh owner and live-store review before any startup
exception. The generic presence predicate must remain conservative; every
normal-runtime caller and mutation effect needs its own reviewed integration.

### Inactive archive-free historical reader

A separate read-only candidate can compare complete C1, pending ticket and
completion record without receiving an archive. It requires one existing lease,
the exact current UID and generation, safe private state/config/runtime paths,
canonical mutually bound records, and observed absence of every other restore,
routing and slot transient. It pins file and directory descriptors, compares
bounded live pair and owner/desired/login member identities across callbacks,
and returns only `ConsistentStillFenced`. The live pair must still match the
terminal pair digest embedded in C1. Missing, torn, crossed, replaced or
orphan records refuse. Existing normal-startup and direct-effect fences remain
unchanged. Synthetic process re-entry without an archive returns the same
still-fenced result; it is not a power-loss test. This conservative initial-output
reader also requires current desired bytes and live pair to match historical
evidence, so legitimate later edits refuse. Future typed admission must use
independently validated ordinary live state instead of treating either
historical match as a permanent requirement.

This reader verifies internal consistency, **not** completion durability,
archive authentication, current boot/login epoch, live semantic validity or
permission to construct an owner. A narrow typed startup candidate needs
separate review; raw presence checks must not be relaxed.

### Inactive archive-free historical resync

A separate inactive continuation pins C1, pending ticket, completion record,
the live pair, owner/desired/login members and their private directories under
one pre-existing lease. It requires the same mutually bound initial-output
snapshot and a fresh caller-supplied Off/idle-host boundary. Before any sync,
and after each one, it reopens the original names, rechecks exact bytes/member
identities and the lease, and brackets the external gate with source checks.
It syncs the pinned records, live pair, present boundary members and the three
private directories. Missing, torn, crossed, replaced or late transient
evidence refuses; an error after the first sync is ambiguous and leaves all
fences intact. No archive is required for this narrow check because the
complete record embeds the canonical ticket and C1. The function never writes
record content, deletes a fence, constructs an owner or enables a listener.

Commit/Abort, replay, wrong evidence, injected file/directory-sync errors,
late owner/desired/login changes, each effect checkpoint substitution and
actual synthetic SIGKILL/re-entry tests return only
`ResynchronizedStillFenced`. This makes no claim of power-cut durability,
semantic validity of the current live store, trusted product path provenance,
current boot/login epoch or startup admission. Its strict historical live/desired
equality is specific to the initial-output experiment; later legitimate edits
require independent ordinary-state validation. Existing normal startup remains
blocked until a separately reviewed typed policy can discharge every obligation.

### Inactive independent current-live review

The `dev/t4-product-recovery-admission` candidate separates exact historical
closure/ticket/completion identity from present ordinary data. Same UID and
exact Rust ownership generation remain mandatory; a later desired generation
does not redefine the historical terminal or authorize another owner. Current
v3 private-store semantics, exact known bundled template (Rule/Global/Direct)
and current desired schema/profile reference are validated independently.
This avoids imposing original restored-byte equality forever after legitimate
ordinary edits. The initial-output reader and resync retain their older strict
contract unchanged.

The new reader pins historical and current file/directory identities across
two fresh caller gates and final readback. Missing or crossed evidence, new
ownership generation, malformed/current unsupported store, custom/partial
template, missing desired target, duplicate desired fields, late transient,
valid concurrent edit or same-byte inode replacement refuses. It never rewrites
history or removes a fence. Both Commit and Abort remain repeatable read-only
observations, and the normal pending classifier still reports pending.

This result is explicitly `ValidHistoricalAndCurrentLiveStillFenced`, not a
durability receipt or startup permit. Connected desired state may be inspected,
but cannot authorize disconnected recovery. Matching template mode to an
actual connected owner, fixed rendered-config validation, startup Off, trusted
current user-manager epoch, empty-host proof, provenance and a one-shot typed
normal-admission boundary remain separate obligations. No production caller,
IPC, CLI backup/restore or installed state change is enabled by this checkpoint.

### Inactive current-Off durability resync after ordinary edits

`dev/t4-current-live-resync` retains the exact historical disposition, but
independently validates and synchronizes the current complete bundled pair.
Current desired state must be explicitly present, valid and disconnected;
startup must be disabled, and the exact bundled template mode must match
current desired. A changed ordinary desired generation is not a new ownership
generation and never rewrites the historical ticket or terminal digest.

One existing lease and the original file/directory descriptors remain retained
through every file, present boundary and directory sync. Each effect is followed
by a fresh exact source, inode, lease and caller host/login recheck. Missing Off
intent, connected intent, enabled startup, mismatched template mode or incomplete
ordinary semantics refuses before any sync. Late drift, replacement, failed
sync/hook or process interruption keeps every recovery fence in place.

Synthetic Commit/Abort, repeated resync, zero-effect refusal and every-checkpoint
replacement and SIGKILL/re-entry cover this continuation without an archive.
The original initial-output resync remains unchanged. The result remains
`ResynchronizedStillFenced`: it is not a power-cut test, current trusted
user-manager epoch, fixed installed-core validation, one-shot owner permit or
ordinary startup exception. No normal pending predicate or product caller is
relaxed by this candidate.

### Inactive retained current-manager epoch and current-Off resync

`dev/t4-current-epoch-resync` factors strict consumed-receipt identity from the
ordinary login admission check without changing any normal pending guard. Its
non-cloneable epoch proof uses the existing fixed package and system-manager
checks, pins the original private receipt descriptor before reading, and
brackets rechecks with fresh manager identity and package observations. Missing,
unconsumed, old-manager or wrong-generation receipts refuse; this continuation
does not mint a receipt for a new manager or login.

The opaque current-Off witness retains the original complete snapshot before
external epoch acquisition. It consumes that proof once under the same lease,
rechecks receipt, manager, source and caller host gate around every durability
checkpoint, and returns only `ResynchronizedStillFenced`. Receipt replacement,
source drift, manager drift, gate loss and synthetic process death never remove
the historical fence or construct a normal owner. Existing normal startup and
current-receipt checks still reject the pending disposition.

This closes a source-level same-manager proof prerequisite, not product
admission. Installed positive package/manager acceptance, fixed rendered-core
validation, complete host provenance and typed integration through every
transaction/coordinator/effect guard remain outstanding. Cross-manager recovery
also needs a separately authorized crash-safe fresh-receipt transition; an old
receipt cannot attest the new manager. See the bounded
[evidence and remaining gates](../development/T4_CURRENT_EPOCH_RESYNC.md).

### Dev-only real Off-startup caller research

`dev/t4-owner-off-research` consumes the retained current-manager/current-Off
witness through the real production-owner initializer, native coordinator,
connection transaction, lifecycle reconciliation and compatibility-pointer plan.
The historical alternative and its constructors compile only in unit tests;
normal builds retain only ordinary startup admission and conservative pending
guards. The fixed-path research counterpart performs no orphan cleanup.

Research permits only the existing `SettledDisconnected` lifecycle branch and
an already-no-change pointer plan. Owned cleanup, reconnect, pointer repair and
auto-start are not authorized. The same original evidence and lease are checked
before/after observation and before/after the no-change commit. The result holds
the actual constructed owner privately, without dispatch, registration,
coordinator access or extraction; it is not normal-owner admission. C1, ticket,
completion and other files remain retained and unchanged.

This is explicitly **not adoption** of the proposed historical product policy.
The owner decision and the rest of the caller matrix above remain outstanding,
as do installed positive package/epoch/host acceptance and new-manager receipt
recovery. Repeated archive authentication is not newly required by this slice;
it tests the already-proposed same-UID/exact-generation historical policy only.
See [the bounded research evidence](../development/T4_OWNER_OFF_RESEARCH.md).

The subsequent [inactive System bridge](../development/T4_SYSTEM_HISTORICAL_OFF_ADAPTER.md)
reuses that startup body in normal compilation, but remains private and absent
from normal dispatch. It internally resolves current paths and captures a genuine
System proof after retaining the original snapshot; actual native Off/empty
observations, the witness and one existing lease span the complete operation.
The owner is destroyed before only `ReviewedOffStillFenced` returns. Boolean
research constructors and historical mutation conversions remain test-only.
This closes the bounded source bridge, not installed System-positive acceptance
or the owner decision to adopt historical evidence in ordinary startup.

### Dev-only synchronous favorite caller research

The successor research routes only `profiles.favorite` through the actual native
coordinator admission/Replay, preflight and existing atomic profile transaction.
Its test-only typed context consumes resynchronized Off evidence, retains exact
history/current-manager/lease identity, and advances only the current store after
successful exact prepared-candidate readback. Failed/ambiguous commits poison the
context and latch the owner; later matching bytes cannot fabricate success.
Ordinary entrypoints and generic pending guards remain conservative, with no
Connect, other mutation, registration or background/login admission. See
[scope and evidence](../development/T4_HISTORICAL_PROFILE_RESEARCH.md).
This is Dev-only historical-policy research, not its product adoption.
