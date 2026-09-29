# S1 App proxy transaction foundation

Development checkpoint, 2026-09-29. Rust owns the pure planner and typed codecs
in `omavless-runtime::app_proxy`. It has no production caller, IPC method, host
adapter, persisted journal or UI setting. App proxy remains unavailable.

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
3. Add a private durable journal bound to native owner, host/session identity,
   schema version and original/intended values. Sync intent before any effect;
   sync confirmed progress after readback. Crash/restart must inspect this
   journal, not take the partly modified host as its new baseline. Missing,
   unsafe, corrupt or incompatible journals refuse automatic restoration.
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
7. Add semantic enable/disable/status through the existing owner and localized
   UI. Explicit disable first restores settings, then stops the owned proxy
   listener. A restoration conflict must stay visible; do not stop a listener
   that still has owned references and silently strand applications. The exact
   disconnected/crashed-core policy and user escape route need host validation.

No privileged helper is needed for this scope. NixOS needs its own adapter and
generation/session acceptance; Arch/Omarchy evidence cannot establish it.

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

This checkpoint is new pure Rust behavior, not a Python migration. Existing
runtime paths, CLI capabilities and package behavior are unchanged. No installed
host gate is applicable until an adapter becomes reachable. Future host gates
must cover fresh enable/disable, explicit original proxy/PAC/bypass/auth state,
permission/readback failures, each crash boundary, foreign changes, core exit,
runtime restart, new versus already-running apps, UWSM activation environment,
and exact cleanup. No S1 completion or full VPN protection is claimed.
