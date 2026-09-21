# Local R5 full application Quit candidate

## September 21 release follow-up (#263)

The immutable `v0.8.1` prerelease predates the installation-query correction in
`ff4799600419605fd154da2a5316f68e725f5821`. Full Quit's strict four-field
installation parser had been given the shared cutover query's nine-field
response, so it refused a healthy installation before requesting shutdown.
The correction gives Quit its own fixed four-property query, without relaxing
duplicate/unknown-field, unit ownership, drop-in or cleanup validation.

The regression failed against the old query and passed after the correction.
The attended physical x86_64 disconnected Settings → confirmation → Quit test
verified stopped/disabled runtime, disabled plugin, no immediate respawn and
preserved private profiles and foreign VPNs. See the exact-head evidence in
[PR #263](https://github.com/k-kostin/omavless/pull/263).
The earlier connected-Quit evidence below retains its original source identity;
it is not a new connected-Quit test of this correction.

The same follow-up fixes two UI findings: name-only search is labelled honestly
in EN/RU, and opening a subscription transfers keyboard focus after the old
Open delegate disappears. Tests cover delayed focus, navigation away before
the callback, missing subscription, selection versus active identity, and both
catalog strings. The installed PC frontend retains its earlier setup surface;
only these two UI deltas were applied, not a full release-frontend replacement.
The actual EN/RU main/subscription views and Open → immediate Down were checked
with the current tunnel left connected. A shell-only reload was needed for the
catalog cache; the native daemon PID remained unchanged. System locale selection
was restored. Captures stay private and outside Git.

This is scoped fix evidence, not clean guided-install acceptance, a complete
route/DNS test, immutable-release replacement or marketplace publication.

## Historical implementation checkpoint

Owner-directed local work, 2026-09-10. Not merged or published. The current
marketplace version and V0/#30 implementation/evidence are unchanged.

## Ownership and semantics

Settings now has an explicitly confirmed **Shut down OmaVLESS / Quit** action.
It is distinct from closing the panel, closing a terminal, shell reload or a
lost client. Those actions never call the new shutdown path.

The Rust `runtime.quit` request accepts only `instanceId`, `expectedRevision`
and `operationId`. It reuses canonical native disconnect validation/replay and
ownership checks. An admission read/write barrier covers every unary handler,
including remote subscription completion: concurrent work returns busy rather
than racing disconnect with reconnect. The existing auxiliary lease is drained,
fresh disconnected/zero primary/auxiliary/TUN facts are required, background
work is revoked, and new admission is sealed. The runtime exits successfully
after bounded workers drain, so `Restart=on-failure` does not relaunch it.

The fixed `omavless plugin quit INSTANCE REVISION OPERATION` CLI authenticates
the installed daemon on the same socket used for the request: same UID, service
MainPID and installed executable inode. It verifies packaged unit identity,
no override/drop-ins, and agreement with the user manager's private path roots.
After graceful exit it acquires the runtime owner lock, verifies disconnected
desired state plus the strict empty-host predicate, disables the fixed native
user unit, repeats verification, and **only then** disables `kdk.omavless` via
the supported Omarchy command. No arbitrary command/service/path/PID input,
sudo, pkexec, process killing, package deletion, Python fallback, or startup
preference rewrite is added.

Quickshell destroys its direct `Process` child on unload (see the upstream
[Process destructor](https://github.com/quickshell-mirror/quickshell/blob/master/src/io/process.cpp)).
Only this confirmed action uses a waiting shell wrapper around the bounded
Rust child, allowing final verification to complete after plugin disable.
Other launcher commands retain their existing process behavior. There is no
short QML watchdog that kills a human authorization prompt; the native request
has a 120-second outcome-unknown bound. Unknown outcome never means successful
shutdown and never causes an automatic kill/retry.

Any failure before final plugin disable leaves the plugin present. A late
failure may leave a safely stopped runtime with an enabled frontend; this is
reported, not hidden or compensated by reconnecting. If the shell accepts
disable but its acknowledgement cannot be read, the result remains unconfirmed,
though VPN/runtime were already verified stopped.

Profiles, subscriptions, routing preferences and private startup settings are
preserved. Runtime auto-start is disabled at the systemd user-unit level. To
explicitly launch again on the installed Arch/Omarchy host:

```sh
systemctl --user enable --now omavless-runtime.service
omarchy plugin enable kdk.omavless
```

This does not replay the old connected request. Existing per-login receipt and
startup policy still govern a later fresh login; do not delete that receipt to
simulate acceptance. A user-friendly single-action reopening flow is separate
from this shutdown checkpoint.

## Static and deterministic evidence

- Rust runtime suite: 667 passed, six ignored; includes 11 new request,
  admission, connected exit, stale/invalid refusal, cleanup-proof and fixed
  host-ordering tests. The failure matrix covers all nine host boundaries.
  Two further regressions cover the procfs cleanup fix described below.
- Full Python suite: 351 executed, four skipped (347 successful tests).
  Includes fixed Quit launcher arity, no Python fallback and harmless synthetic
  wrapper-destruction survival coverage.
- Six new native Quit JS tests exercise production QML function extraction,
  guarded action, confirmation/cancel wiring and EN/RU catalog lookup.
- Full invoked JS/QML contracts, clippy all targets with warnings denied,
  formatting, Python compile, shell syntax, manifest, plugin validate and
  whitespace checks pass.

These are local static results, not installed or human visual acceptance.
Six synthetic exact-source Quickshell renders (EN/RU Settings, confirmation and
failure) were inspected locally. The failure text exposed a RowLayout height
issue; SettingsActionRow now publishes its content-derived implicit height,
and all six affected/adjacent states were recaptured without overflow. Captures
use synthetic metadata and a no-op backend outside Git; they cannot prove live
shutdown or human keyboard interaction.

### Procfs cleanup follow-up

After the initial successful full suite, a repeat parallel run failed the
existing auxiliary private-config replacement test during `lease.finish()`.
Isolated and full sequential reruns passed, but this did not erase the failure.
Investigation deterministically reproduced Linux returning ESRCH when a task
exits after `/proc/PID/stat` was opened but before that retained fd is read.
The existing group inventory accepted only ENOENT, so unrelated process
turnover could reject otherwise successful owned-core cleanup.

`core_group` now accepts ESRCH only at task `stat` open/read boundaries. Generic
directory enumeration, permissions, malformed records, completeness bounds and
all other I/O errors remain strict. A real harmless owned-child regression
forces the descriptor/reap ordering; a second test proves error-scope limits.
The full parallel suite after this fix passes (667 tests, six ignored).
The original aggregate Cleanup error does not prove ESRCH caused that exact
failure rather than the existing cleanup deadline; no timeout was relaxed.

## Remaining acceptance at implementation checkpoint

### Installed continuation

Rust package source `31d402706e55446bb3cbe6cd2ef742c767cadbd6`
(`omavless 0.0.0.r447.g31d402706e55-1`, ARM64) is installed. Packaged binary
SHA-256 matches its build identity; all 24 frontend/template/launcher files
matched the checkout before the later UI pending-flag follow-up.
Four actual installed-Mihomo opt-ins pass (renderer/controller/native host/
supervisor), using Mihomo 1.19.30 ARM64.

Fixed CLI Full Quit passes from disconnected and connected Routing states.
Connected start observed exactly one owned core and one TUN. Both exits verify
daemon/core/TUN zero, runtime unit disabled, plugin disabled, private store
bytes unchanged, and no immediate respawn. They do not claim provider HTTPS/DNS
reachability or fresh-login acceptance. Explicit re-enable restores the native
unit/plugin, Routing/disconnected/core0/TUN0.

The first smoke wrapper reported restoration uncertainty because it returned
an available IPC envelope before fresh host facts were ready; independent
checks confirmed restoration. Its follow-up waits for `availability=observed`
and non-null facts. Do not describe the initial wrapper runs as clean end-to-end
restoration passes. A QML harness similarly had an early-completion predicate
before asynchronous Process start; that run is **not** live QML exit evidence.
The owning UI now marks Quit pending synchronously before child launch to close
double-admission during that scheduling gap. Frontend source
`a55de809f77c60bc195d4f840be5603d5fee0f6c` is installed; Rust remains the
byte-identical `31d4027` package. The actual installed Service → launcher →
Rust command retest passes; independent inspection confirms inactive/disabled
runtime and disabled plugin before explicit re-enable. This harness invokes
the real Service action but is not a human click on the Settings confirmation.

A final connected repeat also passes cleanly through restoration: one core/TUN
before exit; zero runtime/core/TUN and disabled plugin afterward; then verified
Routing/disconnected with plugin and native unit enabled. Private store bytes
are unchanged throughout. Before exiting, actual panel open/close plus shell
restart retained the same daemon PID, core PID and one TUN. Full Quit does not
change those ordinary UI-close semantics. Final runtime is one native daemon,
zero Mihomo/auxiliary/TUN, no manual recovery and an enabled plugin.

- [x] Exact packaged identity, installed runtime restart and source-matching UI.
- [x] Installed disconnected and connected Full Quit; no immediate respawn;
  explicit reopening and independent state verification.
- [x] Exact EN/RU settings/confirmation/error rendering in isolated synthetic
  Quickshell captures; installed production Service action.
- [x] Ordinary panel close and shell reload retain the requested tunnel.
- [ ] Human pointer/Tab/Shift+Tab/Enter/Escape/cancel review of the new button.
- [ ] Explicit authorization rejection where reproducible; UI must stay visible.

Direct Omarchy disable/remove is now covered by the separate local
[native removal observer checkpoint](R5_NATIVE_PLUGIN_REMOVAL.md), including
installed connected/disconnected/removal gates and a newly recorded intermittent
ordinary-Disconnect recovery follow-up. This button's historical evidence alone
did not prove those entry points. Final
fresh-login, upgrade/rollback, all-surface Python-unavailable R6 acceptance and
the [documented DNS/provider follow-up](R5_NATIVE_SUBSCRIPTION_PROBES.md) remain
open. This checkpoint does not declare R5/R6 or V0 complete.
