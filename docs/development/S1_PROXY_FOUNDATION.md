# S1 App proxy transaction foundation

Development checkpoint, 2026-09-29. Rust owns the pure planner in
`omavless-runtime::app_proxy`. It has no production caller, IPC method, host
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

## Host adapter boundary to implement next

1. Define versioned typed codecs for an explicit allowlist of desktop proxy
   fields and environment variable names. Capture effective values **and**
   original explicit overrides, so restoring a default does not accidentally
   pin it as a new override. Never snapshot/write an entire environment or
   desktop configuration tree, evaluate shell syntax, or permit client-selected
   keys/paths. Reject an unsupported desktop schema or incomplete capture.
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

This checkpoint is new pure Rust behavior, not a Python migration. Existing
runtime paths, CLI capabilities and package behavior are unchanged. No installed
host gate is applicable until an adapter becomes reachable. Future host gates
must cover fresh enable/disable, explicit original proxy/PAC/bypass/auth state,
permission/readback failures, each crash boundary, foreign changes, core exit,
runtime restart, new versus already-running apps, UWSM activation environment,
and exact cleanup. No S1 completion or full VPN protection is claimed.
