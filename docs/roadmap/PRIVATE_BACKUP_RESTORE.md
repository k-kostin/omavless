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

The runtime's test-only `backup_source_candidate` exercises the source-pair
acquisition prerequisite using only synthetic temporary files. It accepts the
existing matching migration lease, checks the exact committed Rust generation
and refuses an existing or unreadable routing-preset pending marker both before
and after acquisition. It never creates state directories or repairs a marker.
The lease remains held across both reads, excluding cooperating store/template
writers. This is not yet the serialized native owner's complete admission:
instance/revision, in-memory recovery barriers and background-operation state
must still be supplied by eventual owner integration.

The candidate pins the private config directory and opens only `profiles.json`
and `route-template.yaml` through that directory descriptor. Both member
descriptors are held before either read. Members must be same-user, regular,
single-link, mode `0600`, nonempty and within the existing individual bounds;
symlinks, hard links, unsafe directories and special files refuse. Reads are
bounded and nonblocking at open. Descriptor and current-path identity, size,
mode, ownership and modification/change timestamps are rechecked after both
reads, together with the directory and owner/pending state. A detected edit or
replacement returns no pair and never overwrites the changed source.

Eight synthetic tests cover fixed-member-only acquisition, matching lease
exclusion, wrong generation/lease, unsafe/missing members and directories,
hardlinks/symlinks, bounds, uncommitted/unsafe ownership and interrupted presets,
same-size in-place/atomic member replacement, directory replacement and late
owner/pending changes. This is filesystem and cooperative-lock evidence, not
protection from a hostile same-user process able to forge the entire state.
No system service, provider, controller, TUN or private installed file is used.

The returned pair is deliberately unvalidated plaintext in memory, not an
encrypted backup or authorization to publish it. Semantic store/template checks
and authentication remain separate; non-formatable owned buffers make no
zeroization guarantee. Source acquisition does not select destination transfer,
cryptography, memory-cleanup policy or restore recovery, and no product caller
or backup command is registered.
