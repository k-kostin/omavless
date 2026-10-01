# T4 private backup and restore proposal

Status: security/product design candidate, **not implemented or approved for
activation**. This document creates no backup command, IPC method, picker,
archive format, scheduler, or restore authority. A backup file contains reusable
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
  or cloud upload. Specify the interoperable envelope, KDF/AEAD parameters,
  passphrase policy and recovery behavior in a later cryptographic review; do
  not invent a home-grown cipher or treat file mode `0600` as encryption.
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

Open decisions before implementation: cryptographic format and passphrase UX;
private byte-transfer/destination API; portable template policy; whether a
later version can offer an explicit non-destructive import/merge; and precise
transaction-journal layout. None is settled by this proposal.

## Test-only inner payload framing candidate

The `omavless-domain` test-only `backup_payload_candidate` module explores one
bounded inner payload representation. It is excluded from non-test builds and
has no runtime caller, file handling, credentials, cryptography or IPC surface.
Its plaintext output is **not a backup** and must never be saved as one.

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

This narrower gate does not select an AEAD/KDF library or parameters. Before
adding encryption, review the interoperable outer format, authenticated header
coverage, unique nonce/salt generation and RNG failure, bounded KDF resource
policy, passphrase encoding/UX, memory cleanup, dependency advisories and
independent known-answer/interoperability evidence. Authentication must finish
before inner semantic parsing or preview. Public wrong-passphrase/corruption
errors must remain fixed; this framing test makes no timing-oracle guarantee.

Strict current-schema store validation (including duplicate/unknown members),
portable-template policy, consistent owner snapshot, private byte transfer,
exclusive destination publication, disconnected restore admission and durable
multi-file recovery are still required. No filesystem/VM/host/UI acceptance,
production activation or completed backup/restore feature is claimed.

## Inactive strict store admission

The test-only `private_store::backup_candidate` now validates the store member
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
is local to the test-only backup candidate; ordinary store compatibility reads
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

Current-schema store admission now has this executable candidate, but activation
still needs the authenticated envelope, portable-template policy and whole-pair
validation, consistent owner snapshot, private transfer/publication, disconnected
owner/revision admission and durable multi-file recovery. No installed backup or
restore is available or claimed.
