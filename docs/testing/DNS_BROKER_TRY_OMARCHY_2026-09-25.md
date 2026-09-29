# Experimental DNS broker: installed ARM64 checkpoint

Environment: Try Omarchy ARM64 VM, 2026-09-25. This is opt-in development
acceptance for #270 / Draft PR #295, not released password-free DNS support.
`main` and `rc/0.9.0` were not changed. No private fixture data is included.

## Exact composition

| Component | Source / SHA-256 |
| --- | --- |
| Runtime | source `7ece47d82ca5a6ab750687757b7fa8d316635501`; `6347df464912bec86cefd13fff82f641f6f2b65e1552aedb941ab32731d2fdc6` |
| Corrected broker | source `9634990a352a2eec8b4597602fab85ab9ea72b97`; `7f4c3c6af8d427fe68b59eb3eac025bfafba60bf9d2d0e01cf778325bd5f31ca` |
| Reviewed opt-in Mihomo | retained [adapter patches](../../tests/core_dns_adapter/README.md), with_gvisor build; `db3f80c8f5df8de2931bd53f5b74b72ac55b17cc6be8a6aaf88420fb38a59a86` |

The runtime was installed at its normal package path, using the existing native
activation receipt and user service. Later broker-only changes do not alter that
runtime binary. The separate experimental package installs no default activation
or enrollment and does not replace `/usr/bin/mihomo`. Enrollment and manual
root-service start were separately attended; boot enablement was not performed.

## Host findings and corrections

1. The old isolated service runner cannot exercise the current production owner:
   login admission rejects `OMAVLESS_HOME` and an executable outside the installed
   package identity. This is an obsolete test setup, not grounds for weakening
   activation checks. Installed acceptance used the actual package/service,
   preserved private backups, and the [human barriers](HOST_AUTHORIZATION_ACCEPTANCE.md).
2. The initial live DNS attempt failed before DNS mutation: the adapter expected
   `GetLink(42)` to return `.../link/_42`, while resolved actually uses
   `sd_bus_path_encode` and returns `.../link/_342`. The mock repeated the wrong
   spelling. Read-only installed `GetLink(1)` and independent libsystemd encoding
   checks identified this mismatch. Fixed exact-path validation and golden cases
   preserve rejection of foreign targets. See the
   [systemd implementation](https://github.com/systemd/systemd/blob/v261/src/resolve/resolved-link-bus.c).
3. Fixed-enum refusal diagnostics distinguish TUN, resolver, baseline and channel
   admission without logging descriptor values, profile data or raw bus errors.
   No security check was relaxed to make the fixture connect.

## Installed positive cycle

| Gate | Result |
| --- | --- |
| Root service admission, fixed capabilities and private socket ACL | PASS |
| One native-runtime-owned Mihomo and one TUN in Full VPN | PASS |
| Actual-child private Unix controller, managed-DNS Ready | PASS |
| Generated TCP external-controller configuration absent | PASS |
| systemd retains one original TUN descriptor while connected | PASS |
| Real resolved fixed DNS / root domain / default-route readback | PASS |
| Bounded external HTTPS bound to TUN, both traffic counters advance | PASS |
| Normal Disconnect: no core/TUN, zero retained descriptors | PASS |
| Fresh disconnected observation: no manual recovery required | PASS |
| Separate DNS/route authorization dialogs on Connect/Disconnect | NONE — owner confirmed |

Administrator package installation is deliberately still attended and authorized.
The no-dialog result applies to this installed experimental pair's positive
Connect/Disconnect cycle, not all platforms or all failure paths. No external
service/provider identity or reusable private configuration is published.

## Local regression evidence

- Broker: 47 passed, one opt-in namespace test excluded from ordinary unit run.
- Resolved transport: 35 passed, including independent escaped-path goldens.
- Separate actual-kernel/private-bus composition: all 10 scenarios passed.
- Strict affected-crate clippy and diff checks passed.
- Loaded parallel runs encountered private-bus startup/deadline failures; only
  successful sequential runs are counted, not those attempts.

## Additional installed security and crash gates

Additional installed checks passed: another ordinary UID is denied at the socket;
the enrolled UID can connect but a non-TUN descriptor is rejected before DNS
writes; the installed read-only package guard refuses replacement while the
broker is active. The actual retained-state ALPM gate below is separate evidence.

After an exact pidfd-targeted SIGKILL of the runtime-owned core, the broker
removed the DNS link and released its retained descriptor. The initial crash
runner stopped because its raw process-name inventory still counted the zombie
leader before explicit Disconnect. Fresh observation correctly reported
`ownedCoreRunning=false` and zero TUNs. Explicit attended Disconnect reaped the
child and reached a fresh clean disconnected state.

Subsequent source/contract review corrected the initial **runtime-bug inference**:
[owned-helper cleanup](R5_OWNED_HELPER_CLEANUP.md#native-ownership-contract)
deliberately retains a waitable leader to pin PID/process-group identity until
explicit cleanup. Observation must not reap it early. The native presentation
uses fresh owned-core facts, not last-known lifecycle alone, for connected state.
Do not weaken that ownership mechanism to satisfy a process-name-only assertion.
The stopped test is still not a full PASS; a corrected host gate must distinguish
dead pinned leader from live residual processes, then prove explicit cleanup.

The earlier positive/core-crash cycle's restoration passed: original runtime hash, original template bytes, absent
experimental manager override, original profile in Routing, one healthy core/TUN
and matching DNS readback. The experimental broker is stopped, not boot-enabled;
its package and protected enrollment remain available for further development.
Only identified inactive socket nodes were removed after proving zero retained
descriptors, empty private journal and no TUN; no unknown state was erased.

### Root-helper SIGKILL and retained-state refusal

A separately attended run used the same pinned runtime/broker/core composition.
Before the crash it again verified Full VPN, runtime-owned core, private Unix
controller with managed-DNS Ready, actual resolved policy, one retained TUN and
successful bounded TUN-bound HTTPS. The user runtime was masked without stopping
it to prevent unattended reconnection after the recovery reboot; the root helper
was never boot-enabled.

| Actual installed gate | Result |
| --- | --- |
| SIGKILL of exact checked root-helper PID via pidfd | PASS |
| Original TUN and one systemd-stored descriptor survive helper death | PASS |
| Same active journal survives, without deletion or reinterpretation as clean | PASS |
| Explicit helper restart refuses retained state; journal/retention unchanged | PASS |
| Actual same-package `pacman -U` refused by ALPM PreTransaction hook | PASS |
| Broker binary/journal/TUN/retention unchanged by refused transaction | PASS |
| Coordinated owner reboot establishes a different boot epoch | PASS |
| After reboot: helper inactive, FD store empty, no old journal/socket/TUN | PASS |

This is crash quarantine, **not a kill switch or seamless availability**. No
forced FD-store cleanup, journal removal, DNS reset, enrollment replacement or
unit removal was used to escape unknown state. Package removal refusal remains
a separate unexecuted installed transaction gate; upgrade refusal does not prove it.

### Post-reboot restoration and separate stock-path observation

The preserved ordinary runtime package, exact original route-template, absent
experimental core override and original disabled user-unit enablement were
restored; the temporary mask was removed. Read-only inspection confirmed the
running original executable, stock Mihomo selection and valid generated config.
The root experimental helper stayed stopped with zero stored descriptors.

The final ordinary Routing Connect failed, as did one separately attended retry
(`transition_failed_restored`). The owner confirmed that the latter OS prompts
appeared, accepted the password and closed. Fresh observation shows disconnected,
no core/TUN and no manual-recovery flag. Classified core logs show one other
warning, but no DNS/TLS/timeout/connection or setup-permission classification;
these hints do not establish the cause. Do not blame typing speed, claim restored
VPN connectivity, or call the whole recovery workflow PASS from empty resources.

A separately supervised stock-core diagnostic then verified its private
controller, TUN and fixed resolved readback; it was stopped cleanly. The next
separately attended native Connect on the **unchanged** ordinary binary succeeded.
Bounded exact-child read-only observation confirmed mode, legacy DNS ownership,
rules/providers and expected selector; four endpoint reads took 9–13 ms in that
successful run. Original profile/Routing, actual DNS and TUN-bound HTTPS were
restored in that run. This closes restoration, not the cause of the two earlier failures:
neither user typing speed nor a readiness-timeout hypothesis was established.

### End-of-session state: subsequent gate stopped before runtime start

A later mode/core/removal attempt installed the experimental candidate again and
applied its trial configuration. Its human authorization barrier stopped with
`human_authorization_unsettled` before runtime start. No subsequent automatic
Connect, cleanup or rollback was attempted. The mode sequence, corrected core
crash and package-removal gates remain NOT RUN, not failed protocol evidence.

Final read-only inspection: user runtime inactive/disabled with MainPID zero;
no TUN; root broker active with zero stored descriptors. The experimental runtime
and core override remain installed/configured. The earlier successful Routing
restoration is evidence for that earlier cycle, **not the current VM state**.
Private backups and original rollback package remain locally preserved. The
[PC continuation](../development/RC_090_PC_CONTINUATION_2026-09-25.md) starts from
its own independently inspected VM, not assumptions about this ARM host.

## Remaining release gates

An unknown write remains a quarantine/recovery condition, never permission to
erase the journal or retained descriptor. Correct/repeat the core-crash gate
without treating its deliberately pinned zombie as a live core; finish installed removal and
applicable negative/mode-transition acceptance, and reviewed distribution before
closing #270. The isolated service runner needs a compatible installed-owner path
before being recommended again. Actual mode transitions and broader host failure
coverage are not inferred from a successful single Full VPN cycle.

CI exposed a parallel journal test returning ownership refusal instead of the
expected malformed-record recovery classification. The private-bus/forking and
short-deadline suites now run serially in `tests/run-rust.sh`, preserving all
assertions and explicit race cases; no production journal check was relaxed.
Exact checkpoint `59a6610a30824d4183deec936e0cdbbc04d22662` passed all three
GitHub jobs (test, package, package-arm64). Its local full non-Rust runner reported
451 tests, 449 passed and two expected skips, with QML/JavaScript contracts green.

This checkpoint does not close #270, make the broker the default, prove a kill
switch, or authorize promotion/publication of RC 0.9.0.
