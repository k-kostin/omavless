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

## Remaining release gates

Additional installed checks passed: another ordinary UID is denied at the socket;
the enrolled UID can connect but a non-TUN descriptor is rejected before DNS
writes; the installed read-only package guard refuses replacement while the
broker is active. This last check is not an actual ALPM transaction-abort test.

After an exact pidfd-targeted SIGKILL of the runtime-owned core, the broker
removed the DNS link and released its retained descriptor. The crash test as a
whole stopped: the runtime left a zombie child and stale `connected` status until
explicit Disconnect. Fresh observation correctly reported `ownedCoreRunning=false`
and zero TUNs. Explicit attended Disconnect reaped the child and reached a fresh
clean disconnected state. This runtime crash-status/reaping finding must be
resolved separately; it is not a fully passing runtime crash scenario.

Restoration passed: original runtime hash, original template bytes, absent
experimental manager override, original profile in Routing, one healthy core/TUN
and matching DNS readback. The experimental broker is stopped, not boot-enabled;
its package and protected enrollment remain available for further development.
Only identified inactive socket nodes were removed after proving zero retained
descriptors, empty private journal and no TUN; no unknown state was erased.

Actual root-broker crash/quarantine/recovery and installed ALPM abort behavior still need their
declared gates. An unknown write remains a quarantine/recovery condition, never
permission to erase the journal or retained descriptor. The isolated service
runner needs a compatible installed-owner path before being recommended again.

CI exposed a parallel journal test returning ownership refusal instead of the
expected malformed-record recovery classification. The private-bus/forking and
short-deadline suites now run serially in `tests/run-rust.sh`, preserving all
assertions and explicit race cases; no production journal check was relaxed.

This checkpoint does not close #270, make the broker the default, prove a kill
switch, or authorize promotion/publication of RC 0.9.0.
