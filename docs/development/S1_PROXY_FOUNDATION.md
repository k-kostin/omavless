# S1 App proxy transaction foundation

Development checkpoint, 2026-09-29. Rust owns the pure planner, typed codecs and
private journal foundation in `omavless-runtime::app_proxy`. It has no production
caller, IPC method, live host adapter or UI setting. App proxy remains unavailable.

## Scope and semantics

App proxy affects applications that honor desktop proxy settings or their
inherited environment. It is a separate connection scope, not a fourth routing
mode, a full VPN, a DNS leak barrier or a kill switch. Full VPN / Routing /
Direct remain policy choices for the core; their interaction with this scope
needs an explicit product decision before exposure.

The pure lease captures exact original and intended values for two fixed
surfaces: desktop proxy settings and the systemd user-manager environment.
Each canonical adapter snapshot is bounded to 16 KiB and keeps absence distinct
from a present empty value. Snapshots and effects redact their Debug output;
original proxy settings may themselves contain credentials.

Apply records intent before each effect, requires complete readback after each
write, and reports Active only after both observations match. Compensation and
disable run in reverse order and restore the original bytes. A failed write
whose outcome is unknown may have left either the original or the intended
value; both cases can be reconciled. Any third value, or an unexpected change
to a never-attempted surface, refuses restoration. No generic unset/reset path
exists. An exact matching original/target pair requires no write.

Owner instance and generation must match on every step. These are caller-owned
inputs, not proofs created by this library. The production coordinator must
also serialize commands and recheck revision/desired state. The planner's
Active phase means only that settings matched; it must never be used as an
internet-connectivity or core-readiness claim.

## Typed capture and codec boundary

`app_proxy::codec` now supplies version-1 private canonical snapshots for two
prospective adapters. It accepts structured values supplied by a future typed
host API; it never parses `gsettings`/`systemctl` command output or shell text.
No host connection, subprocess, environment mutation or settings write occurs.

- Desktop: exactly 16 keys from the five fixed `org.gnome.system.proxy` schemas.
  Each entry retains effective value, default value, explicit user override
  (absent or present) and writability. Types are string, boolean, bounded signed
  port integer, and ordered string array. Mode is strictly none/manual/auto.
  PAC URL, bypass list order and HTTP authentication values remain private and
  unnormalized. The deprecated HTTP `enabled` key is preserved without treating
  it as proof that proxying is enabled. See the
  [upstream schema](https://github.com/GNOME/gsettings-desktop-schemas/blob/master/schemas/org.gnome.system.proxy.gschema.xml.in).
- User manager: exactly ten case-sensitive variables: lower/upper-case
  `http_proxy`, `https_proxy`, `ftp_proxy`, `all_proxy` and `no_proxy`.
  Explicit absence and a present empty string have distinct tagged encodings.
  Values retain literal quotes, equals signs, newlines and Unicode; they are
  data, never evaluated or converted to shell commands. Unrelated environment
  variables cannot enter this projection. A future adapter must obtain a
  complete manager observation before inferring that a selected variable is
  absent; a failed/incomplete query cannot become an empty snapshot.

Every capture requires the complete allowlist exactly once. Encoding uses a
fixed field/key order, a version and a surface discriminator. Decoding rejects
unknown/duplicate/missing fields, wrong types, wrong surface/version, invalid
UTF-8, NUL and trailing input. Per-value limits (1 KiB, 64 bypass entries) and
the 16-KiB total canonical byte cap apply before a snapshot can enter the lease.
Oversized existing settings refuse; they are never truncated. Debug output for
all value-bearing types is redacted. Explicit private encoding/accessors are
not safe diagnostic outputs or a license to persist outside a private journal.

GSettings' [user-value API](https://docs.gtk.org/gio/method.Settings.get_user_value.html)
and [default-value API](https://docs.gtk.org/gio/method.Settings.get_default_value.html)
make effective-only capture insufficient: a user override may equal a default,
or an administrator lock may hide a stored override. Capture preserves these
distinctions. Write admission refuses any non-writable key or effective value
inconsistent with the observed user/default layer. Reset is a future restoration
operation only for a captured absent override, after current-state comparison;
it is not a global reset-to-default operation. Default changes during a lease
change its complete snapshot and therefore require conflict handling.

Environment admission rejects unknown and separate D-Bus activation environments.
The broker classification is caller-supplied evidence, not host discovery or an
unforgeable capability. The future adapter must verify it again in the matching
session before effects. The systemd
[manager interface](https://github.com/systemd/systemd/blob/main/man/org.freedesktop.systemd1.xml)
distinguishes setting an assignment from removing a variable and affects newly
spawned processes. Per-unit overrides and already-running application
environments are outside this manager snapshot, so matching it cannot establish
that every app uses the proxy. No target proxy values or host write commands are
generated by this codec slice.

## Host adapter boundary to implement next

The [host observation checkpoint](S1_HOST_OBSERVATION.md) specifies the
read-only GIO helper and fake-host gate. Its first source-only candidate is
[documented separately](S1_READ_ONLY_OBSERVER.md). CLI effective-only
snapshots are not accepted. Exact activation/session provenance remains
unresolved; the normal runtime does not build or call the optional GIO helper.

1. Bind the typed codecs above to real host reads and guarded per-field writes.
   Verify actual installed schema IDs, key signatures/ranges and complete reads;
   missing/unsupported schemas refuse, not fallback to guessed defaults. Check
   explicit override, effective/default values and writability before and after
   each operation. These codecs do not establish that Omarchy apps consume the
   GSettings surface. That and broker/user-manager provenance require installed
   readback and new-app acceptance before enabling the feature.
2. Verify one owned Mihomo process with an exclusive loopback listener,
   TUN disabled and no controller TCP exposure. Validate its generated config
   and readback before publishing proxy settings. Keep all endpoint selection
   at the runtime, not in the UI or a provider-controlled config fragment.
3. Integrate the private journal below into owner admission and fixed host
   effects. Establish boot/session identity and a separately reviewed native
   owner takeover on daemon restart; never substitute a new binding or take a
   partly modified host as the new baseline. Missing, unsafe, malformed or
   incompatible journals refuse automatic restoration. Real GSettings writes
   may have partial field outcomes: a future per-field operation plan must
   journal them explicitly before exposure, rather than assuming this two-surface
   foundation makes all desktop writes atomic.
4. Use fixed unprivileged host APIs. Systemd manager environment changes affect
   subsequently started services; a running application's inherited environment
   cannot be retroactively repaired. Verify actual UWSM launch behavior with
   new GUI apps and distinguish application restart guidance from VPN failure.
5. Verify D-Bus activation ownership before enabling the environment surface.
   [UWSM's upstream environment notes](https://github.com/Vladimir-csp/uwsm#concepts-and-features)
   explain that dbus-broker reuses systemd activation environment, whereas the
   reference dbus-daemon has a separate environment that cannot unset variables.
   Substituting an empty value violates exact absent-state restoration. The
   first host adapter must refuse unsupported separate activation environments
   or introduce a separately reviewed exact-restoration solution. Do not modify
   UWSM env files or session-manager configuration as a workaround.
6. Treat host changes as individually observed writes, not a cross-service
   atomic transaction. The runtime lock does not lock external desktop tools.
   Re-read immediately before/after effects, preserve foreign edits, and retain
   the journal on conflict. Matching values do not prove absence of an external
   ABA write; do not advertise exclusive ownership from equality alone.
   The [isolated persistent dconf experiment](../testing/S1_PRIVATE_DCONF_2026-10-01.md)
   proves that an optimistic same-backend read can show an uncommitted value,
   and that `Settings.sync` also returns after a failed commit. Require
   independent persisted readback after settling admitted operations; retain
   unknown-outcome journals when completion cannot be established. The
   [independent readback candidate](../testing/S1_INDEPENDENT_READBACK_2026-10-01.md)
   compares the complete layered snapshot through a fresh fixed helper process.
   It supplies read-side evidence only: no writer-drain barrier, persistent
   outcome transfer or production write admission is established.
7. Add semantic enable/disable/status through the existing owner and localized
   UI. Explicit disable first restores settings, then stops the owned proxy
   listener. A restoration conflict must stay visible; do not stop a listener
   that still has owned references and silently strand applications. The exact
   disconnected/crashed-core policy and user escape route need host validation.

No privileged helper is needed for this scope. NixOS needs its own adapter and
generation/session acceptance; Arch/Omarchy evidence cannot establish it.

### Per-field recovery candidate (stacked, not installed)

`app_proxy::fields` addresses a limitation of the earlier two-surface lease:
one GSettings operation may update only some of its 16 fields. It models the
fixed 16 desktop keys and 10 manager variables individually. Each field intent
is marked before an effect is returned; confirmation requires a fresh complete
readback in which exactly that field changed. On compensation, only fields
whose intent was marked may be either original or intended, and they are
restored in reverse order. Every never-attempted field must remain original.
Unexpected third values, changed defaults/locks, stale owner or an unconfirmed
effect refuse progress. Synthetic tests cover every partial-apply boundary and
both outcomes of an uncertain write, without touching the host.

This is a **pure in-memory planner**, not the production recovery solution.
Its 26 intent bits and pending field still need a durable private journal,
crash/reopen tests, exact host readback and owner takeover proof before any
adapter may execute its effects. The previous two-surface journal is not
silently reinterpreted as a per-field record. App proxy remains unavailable.

The stacked field-journal candidate adds that private intent persistence in a
fixed `app-proxy-fields` child of a trusted private root (pre-created 0700;
symlinks refuse). Its version-2 record uses fixed basenames,
0600 files, exclusive locking, a durable write-before-effect sequence and exact
previous-record comparison from the existing storage layer. It keeps all 26
attempted bits, the pending field and each field's expected original/intended
side. Reopening refuses to continue applying; it allows only compensation
after a complete fresh observation under the same binding. Tests restart at
every partial-apply boundary, exercise both outcomes of a lost confirmation,
and check storage failure, foreign fields, stale binding and malformed state.

This remains an unregistered library, not App proxy availability. The version-1
two-surface journal is **not** auto-migrated or deleted. Production code still
needs a proved native-owner takeover, session/broker provenance, per-key typed
host adapter, complete readback and installed crash/new-app acceptance. In
particular a new daemon cannot simply reuse the old instance ID to open either
journal. Both private journal formats remain isolated development checkpoints.

### Inactive takeover admission candidate

`app_proxy::takeover` is a pure refusal-first model stacked after the field
journal. It returns a compensation-transfer *plan*, never a host effect or a
rewritten binding. It accepts only an exact private record identity and
sequence, distinct monotonic native-owner generations, matching UID/boot/session
and verified manager, bus, settings-profile and shared-activation scope.
It additionally requires an exact graceful handoff receipt saying the old
workers were joined and all asynchronous effects settled. Missing/released
records, unknown predecessor state, changed scope or a stale receipt refuse.
An ordinary crash does **not** produce that receipt, so the model intentionally
does not authorize automatic crash takeover.

All these inputs are supplied by a future trusted coordinator; constructing a
Rust receipt or comparing two snapshots does not prove the operating-system
facts. The current read-only observer verifies identities within one read but
does not carry manager/bus incarnation, settings backend/profile selection or
broker provenance into a transferable proof. A future storage slice would
need to atomically record the new binding and incremented sequence under the
same private journal lock, preserving every original value, attempted bit and
pending effect. It must reject uncertain publication and stale predecessor
handles. A future installed-VM gate must prove actual old-worker quiescence,
session/broker provenance, and exact partial-write restoration. No production
caller, automatic transfer, S1 availability or network change follows from
this model.

### Test-only durable compensation transfer experiment

The field-journal tests now compose the takeover decision with the real private
storage lock, exact-byte comparison and durable replacement sequence. The
experiment stores a canonical bounded envelope containing the unchanged field
record, a nonzero monotonic sequence and synthetic manager/bus/settings scope.
Its identity hashes the complete private bytes. The model changes only the
owner binding and sequence during transfer: original/intended values, expected
sides, attempted bits, pending field and phase are preserved. A successful
transfer or reopen remains compensation-only; it cannot resume application or
confirm the predecessor's unknown write. Every later restoration publication
also advances the sequence, invalidating old receipts.

This is compiled only inside `cfg(test)`, not a new journal version, production
transfer API, migration or host lease. Its scope and joined/settled receipt are
deliberately synthetic inputs; they do not close any identity edge. The normal
v1/v2 journal decoders and all production paths are unchanged. The experimental
format is used only in disposable private fixture directories.

The matrix transfers at all 27 partial-apply boundaries, including both possible
outcomes of the pending write, then reopens under the successor and restores the
exact original state. It rejects predecessor bindings, stale receipts, changed
scope, unknown quiescence, competing lock holders, foreign bytes, missing or
released records, sequence overflow and invalid/duplicate envelope fields.
Injected failures and actual owned-child exit after staging creation, write,
file sync, rename and directory sync prove refusal ordering: pre-rename staging
blocks both owners; a visible post-rename record retires the predecessor and
permits only modeled compensation under the successor. Foreign field edits
still refuse without changing the record. These are process-exit and storage
ordering tests, not power-loss durability or automatic crash-handoff proof.

Before promoting this experiment, the remaining gate is a separately reviewed
coordinator that proves old-worker quiescence and the unchanged installed host
scope, binds a receipt to the exact locked record, and revalidates that authority
before each typed effect. An ordinary daemon crash still supplies no graceful
receipt. The read-only manager diagnostic's unverified success cannot create
one. Installed exact restoration, listener readiness and new-app consumption
remain later gates; this experiment does not access proxy settings or a VM.

### Test-only tracked-worker quiescence experiment

The transfer fixture now has a synthetic worker registry with real Rust threads
and separately retained effect tickets. It closes new worker/effect admission,
joins every registered worker, and refuses a modeled graceful receipt while any
effect is pending or any completion is unknown. Dropping an unsettled ticket or
joining a panicked worker sets an irreversible unknown-outcome flag; an empty
ticket count is not success. Cancellation alone cannot settle an already
admitted write, and joining its initiating worker cannot settle a later callback.

Channel-controlled tests prove these boundaries without timing-based assumptions.
An admitted synthetic field write can complete after cancellation; only after
tracked quiescence does the model transfer and restore the exact saved original.
A child callback deliberately outlives its parent worker and blocks the receipt
until its own tracked completion. Known completion before worker join is also
insufficient. A subsequent durable record revision invalidates an otherwise
settled receipt, so the storage identity check remains mandatory.

This registry is test-only and covers only the workers and effects it explicitly
tracks. Its `settle` call is a synthetic executor acknowledgment, not evidence
that a host API committed a write. It cannot discover detached/unregistered
work, quiesce GIO/D-Bus operations, rule out foreign writes, authenticate a host
scope or provide a production lease. The promotion gate must first define all
real asynchronous effect owners and require bounded known completion or
conservative refusal for each, alongside the still-open provenance gates above.

## Private durable journal foundation

`app_proxy::journal` supplies an unregistered, fixed-schema library over the
two-surface planner. Callers provide the trusted private directory and a binding
to native instance/generation, UID, boot and session identity. These observations
are not discovered by the journal. Zero/foreign identities refuse; opening a
record with a different binding also refuses. A new daemon must not reuse a
stored old instance ID as if that proved ownership: a safe takeover protocol is
still required before production recovery.

The fixed `app-proxy-journal.json` stores version 1, original/intended typed
canonical snapshots, observed-side selectors, attempted surfaces, pending effect
and phase. Size is capped at 192 KiB. Strict decoding rejects duplicate/unknown
fields, malformed/noncanonical snapshots, invalid versions and inconsistent
planner states. Private data is never included in Debug or error messages. The
record is sensitive even if it contains no OmaVLESS profile: prior proxy/PAC or
authentication settings may contain credentials.

The caller-selected directory must already be owned by the current UID with
mode 0700. Each path component is opened without following symlinks; relative
paths and parent traversal refuse. Record, lock and staging files must be regular
same-UID 0600 files with one link. An exclusive nonblocking lock prevents two
cooperating journal writers. File operations use the pinned directory descriptor;
directory/lock identities and exact previous bytes are checked again before
replacement. This detects observed foreign edits or renamed directories; it
does not protect against a malicious process running as the same UID or turn
external desktop APIs into compare-and-swap operations.

Each step writes the fixed exclusive staging file, syncs its data, renames it
over the exact expected record and syncs the directory. Initial publication uses
no-replace rename. Only after durable intent publication may `begin_next` return
an effect. Confirmation and restoration intent have the same durability order.
Any storage failure poisons that in-memory handle and returns no new effect;
the record and any staging artifact remain for explicit recovery. No automatic
unlink, chmod repair or baseline reset occurs.

Reopening never resumes applying or confirms an unknown effect. Fresh complete
observations may begin compensation using the saved original pair. For a surface
whose intent was recorded, the planner accepts either the original value or the
intended value; an unrelated third value refuses. A never-attempted surface must
still match its original value. A surviving staging file refuses reopening,
including when the old record is valid: this conservative checkpoint has no
automatic staging-reconciliation policy. Released records are retained as private
tombstones; deletion/reuse is a future explicit lifecycle operation.

`create` is only the low-level initializer for an independently admitted fresh
lease. A missing journal during recovery is an error, never an instruction to
call `create` with current settings. The future owner must distinguish first use,
intentional completed cleanup and lost state before initialization.

Deterministic tests include real subprocess exit after staging creation, write,
file sync, rename and directory sync; loss of effect confirmation; failed restore
intent; exact original restoration; stale binding; competing writer; symlink,
hard-link, FIFO/public-file/directory refusal; directory rename; malformed/oversized
records; and credential-safe Debug. Process-exit/fault tests establish the library
ordering, not filesystem power-loss guarantees or live proxy recovery. Production
host writes, package installation and VPN/network changes are absent.

## Verification and remaining gates

Deterministic tests cover exact absence/empty restoration, complete apply and
reverse restore, partial application, failed/unknown write outcomes, failed
restoration retry, foreign edits, stale generation/instance, pre-existing target
values, complete readback, size bounds and private Debug output. All fixtures
are synthetic; no host settings or private profiles are read.

Codec tests additionally cover explicit overrides equal to defaults, locked
hidden overrides, full-key completeness/order, lower/upper-case identity,
literal metacharacters, absent versus empty environment values, invalid types
and port bounds, schema/version confusion, nested duplicate/unknown fields,
missing presence fields, aggregate limits, private Debug and unsupported
activation classification. This proves the pure codec contract, not installed
GSettings/systemd/UWSM integration or App proxy availability.

This checkpoint is new Rust behavior, not a Python migration. Existing runtime
paths, CLI capabilities and package behavior are unchanged. Journal tests use
isolated synthetic private directories; no installed host gate is applicable
until an adapter becomes reachable. Future host gates
must cover fresh enable/disable, explicit original proxy/PAC/bypass/auth state,
permission/readback failures, each crash boundary, foreign changes, core exit,
runtime restart, new versus already-running apps, UWSM activation environment,
and exact cleanup. No S1 completion or full VPN protection is claimed.
