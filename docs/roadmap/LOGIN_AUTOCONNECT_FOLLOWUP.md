# AUTO-1 — enabled login autoconnect acceptance

Status: **OPEN, owner-deferred from R6 on 2026-09-13**. Repository-tracked
follow-up, not a claimed GitHub issue or a successful host test.

The owner explicitly accepted moving optional Last/pinned fresh-login validation
out of Rust runtime retirement. New native configurations retain startup
`enabled: false`. Configuring startup, running the daemon at login and connecting
VPN at login are different actions. This exception does not waive the tested
startup-Off path, Python absence, one-owner lifecycle or cleanup requirements.

## Already implemented / not proven

- Rust owns preferences, validation, once-per-real-user-manager login intent,
  receipts, mutation admission and ordinary reconciliation. No Python fallback.
- Startup Off has real installed Python-unavailable fresh-manager evidence in
  [the Off acceptance record](../testing/R6_INSTALLED_NO_PYTHON_LOGIN_OFF_2026-09-11.md).
- Last/pinned preference saving and deterministic state/replay tests exist.
  They do not prove connected fresh-login success.
- The VM reboot showed On but networking failed; later manual profile changes
  prevent attribution to that login. The selected-profile UI ambiguity is now
  fixed. Do not relabel this history as successful autoconnect.
- Startup was restored to Off through the native API on 2026-09-13, without
  changing the running Routing connection or runtime/core PIDs.

## Completion checklist

- [ ] Use a currently working private fixture and record exact package, running
  executable and frontend identity. Prepare local observation/recovery that
  does not depend on chat connectivity.
- [ ] Last: a genuine new user-manager epoch, no manual Connect after login,
  correct intended profile/mode, one owned core/TUN and private Unix controller.
- [ ] Pinned: repeat a genuine new epoch with explicit pinned selection; prove
  it is not replaced by a different last-used profile.
- [ ] After an explicit Disconnect, same-epoch runtime restart stays
  disconnected even while the future-login preference is enabled.
- [ ] Exercise required/cancelled host authorization without repeated prompts
  or UI claiming success before the observed transition. Keep an unresolved
  outcome explicit; do not bypass policy.
- [ ] Test unavailable profile/server/caches and failed startup: classify
  availability/network failure separately from wrong state or lifecycle bugs.
- [ ] Capture bounded HTTPS evidence and independent network/DNS findings with
  proper attribution. No core/controller-only internet PASS.
- [ ] Restore the owner's requested startup preference and runtime state;
  record actual results and any fixes in the owning bounded change.

Use the [login integration contract](../testing/R5_NATIVE_LOGIN_INTEGRATION.md)
and [human authorization procedure](../testing/HOST_AUTHORIZATION_ACCEPTANCE.md).
Never fake a login by editing receipts, manually invoking preparation, deleting
runtime state or reconnecting after login and calling it automatic success.
Do not repeatedly reboot/reconnect merely to satisfy a checklist.

## Product and publication boundary

### 0.9.9 correction for manual Start after reboot

A stopped application with a fresh, unconsumed user-manager login and explicitly
configured startup Off may retain Connected intent from the previous session.
Manual Start must reach the existing packaged login-preparation service in this
state. That service applies the established Off login policy and consumes the
current receipt. Start then repeats admission and requires disconnected intent
before starting the ordinary runtime. The read-only availability probe performs
neither preparation nor service startup.

A consumed login with Connected intent still refuses this manual Off-start
path. Enabled or unresolved first-login preferences, pending transactions,
unsafe/stale receipts and unavailable ownership keep their existing refusals.
Recovery-only runtime start continues to require an already consumed Off login.

The regression covers the production classifier and ordered executor. Installed
acceptance must additionally start from saved prior-session Connected intent
with autoconnect Off, earn a genuine new user-manager epoch, use the ordinary
application Start entry, and verify Disconnected without an owned core. Check
preserved profile/settings bytes and service enablement, plus a same-login
restart remaining Off. A direct preparation command is fixture administration,
not proof of the corrected user entry. Source tests alone do not close this gate.

Enabled autoconnect is implemented but its connected fresh-login acceptance is
incomplete. Keep this explicit in native installation guidance and future
release notes. Do not advertise it as fully validated or silently enable it
for new users. This deferral is not permission to overwrite an existing user's
chosen preferences during upgrade. Any new UI warning/change remains scoped
work under the UI/UX contract, not a reason to redesign accepted Settings now.

The independent [network diagnosis](../testing/R6_NETWORK_DIAGNOSIS_2026-09-13.md)
also remains open: a failed server TCP path was reproduced without TUN, while
a later different active profile carried HTTPS successfully. No universal DNS,
provider, VM or Rust-networking fix is claimed by R6 closure.
