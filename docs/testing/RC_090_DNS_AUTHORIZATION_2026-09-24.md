# RC 0.9.0-rc.1: installed pair and DNS authorization

Try Omarchy ARM64, attended September 24–25, 2026. This is real authorization
evidence, not a prompt-free fix or RC release acceptance. Main, release assets,
pins and marketplace remain unchanged. Issues #270 and #132 remain open.

## Exact installed pair

Source `5b5ed848e2464d1c4594788a490299dd8d17ee8c`; Cargo/frontend
`0.9.0-rc.1`, Arch package `0.9.0rc1-1`. The selected artifacts are the **local**
ARM64 package and common frontend from the
[preparation report](RC_090_PACKAGE_PREPARATION_2026-09-24.md), not CI ARM64:

| Identity | SHA-256 |
| --- | --- |
| Package | `e44595335c69de58bbd1c29a0d43d6ea4df6dfd0202eb9f61f4a8cb0aa0b629a` |
| Installed/running ELF | `3c0f96acc6a48687bc2f48e95c3e0678a453be49e2e8539a4ad0b2ece1dcb3c0` |
| Frontend archive | `743d2744bf4af3607c36b6f8a3fc1dd2d5d75634680392b1e7fd19b73e44ddb5` |

The attended upgrade verified the original installed package against its retained
rollback archive, disconnected, stopped the service, installed through normal
sudo/pacman, started the service and installed the exact frontend bundle.
Private store/state fingerprints stayed unchanged across package replacement;
all 41 delivered frontend payload files matched the pinned archive (its two
installer entry points are not installed payload). Running ELF/unit identity
passed. Startup remained Off and service enablement remained disabled.
Original profile and Routing were restored with one core, one TUN, zero auxiliary
cores and no manual-recovery state. This closes the package replacement and
original-state restoration check, not the independent DNS/security gate.

## DNS matrix

Read-only systemd-resolved properties were sampled on the test's only TUN:
root routing domain, DNS-default-route and the fixed fixture DNS server.
The public result contains booleans only. No private IDs, addresses, provider
data, controller paths or raw errors are published.

| Case | Runtime at Connect reply | DNS at reply | After human authorization | Result |
| --- | --- | --- | --- | --- |
| Accept ordinary authorization after upgrade | Connected / Routing, 235 ms | All three absent | All three match | Positive setup/readback observed; premature connected claim also observed |
| Explicitly cancel all connect authorization dialogs | Connected / Full VPN, 123 ms | All three absent | All three still absent | **FAIL: connected with unconfirmed DNS after cancellation** |
| Deliberately delay a live OS prompt, then accept | Not run | Not run | Not run | Still required; delay before terminal `ready` is a different, no-effect wait |

The cancellation result includes the human's explicit `cancelled` receipt after
all dialogs settled. An earlier attempt where the human actually accepted the
requests is ordinary positive evidence, **not** cancellation acceptance. Merely
typing `none` or closing a window cannot override what the human says occurred.
Interrupted acknowledgements stop that invocation and are not test passes.
Do not record every terminal typo as an independent product defect.

In the genuine cancellation case, desired/actual remained connected/global,
core/TUN remained 1/1 and `manualRecoveryRequired` stayed false. No HTTPS result
was used to substitute for DNS readback. This demonstrates the #132 completion
gap independently of provider availability; it does not prove a DNS leak or a
particular external network failure. The explicit cleanup removed core/TUN;
any restoration outcome must be verified separately, not inferred from cleanup.

After the negative test, a restoration attempt encountered PAM authentication
failure/lockout: the user reported the system rejected their password, three
valid failure records were present, and the polkit journal included a lockout.
The keyboard was English (US), Caps Lock off. The installed unoverridden policy
uses three failures/900 seconds and a 600-second unlock delay. No counter reset,
policy edit or passwordless grant was performed; further authorizing tests were
paused. Core/controller restoration with all DNS properties absent was **not**
accepted as network/DNS recovery, even when the semantic action returned success.
This failure does not prove the password itself was incorrect.

### Verified final recovery

After the normal 600-second interval plus a margin, a new separately attended
recovery accepted OS authorization. The original profile/ Routing was restored;
all three resolved properties matched after `settled`, one owned core and one
TUN were present, and no manual recovery was reported. PAM then showed zero
valid failed-attempt records following normal successful authentication; no
agent reset or policy modification occurred. The final recovery check explicitly
requires matching DNS readback, not just core/controller success.

The restored connection passed a bounded generic HTTPS probe bound to its TUN,
with TUN counters increasing. The service owns the single core; the private Unix
controller responded and the generated config has no TCP external-controller.
This session did not repeat privileged TCP-listener PID attribution or claim
IPv4/IPv6/DNS leak coverage from HTTPS. The enabled plugin and original Routing
connection were preserved.

The exact installed TUI capability check passed. The plugin's fixed Open app
launcher opened one TUI window; a repeat focused the same window. Closing it
through the host's supported window-close dispatcher left desired state and the
1-core/1-TUN connection unchanged. No repeated full T2 visual acceptance or
x86_64 installed claim is made.

The negative case exercised Connect from a verified disconnected baseline. It
exposes the shared DNS-completion gap, but is not a pass for #132's separate
connected mode-change cancellation/rollback, panel-reopen or shell-restart matrix.

## Reusable read-only evidence

`python3 tests/native_dns_readback.py` reads only `ip -d -j link show`, resolved
`GetLink` and the three fixed properties. It has a five-second total deadline,
bounded output, strict duplicate-key/UTF-8/type checks, at most one TUN and an
index recheck. No sudo/pkexec, policy/cache changes, reset, DNS resolution or
network probe occurs. Missing/unavailable data never becomes a successful match.
Fifteen deterministic tests exercise privacy, malformed/oversized replies,
partial DNS, cancellation classification, interface changes and command bounds.
The combined developer suite passed 316 tests (two expected skips), with QML/
Node contracts. This adds developer tests only; Python is not shipped or used
by the installed runtime/frontend.

This fixture observer is **not** a production DNS owner, generic network-health
check or secure interface lease. Index equality cannot prevent index reuse;
same-user access or a matching property cannot prove which caller was authorized.
Do not reuse it to grant privileged operations or claim leak prevention.

## Required implementation next

The native transaction currently commits controller/core configuration before
asynchronous resolved authorization completes. #289's local mode confirmation
does not fix that boundary. The
[DNS contract](../roadmap/DNS_AUTHORIZATION.md) still requires an explicit
authorization/apply/readback result joined to the lifecycle transaction, with
cancelled/late outcomes fenced and rollback verified. Increasing the existing
startup timeout, blaming typing speed, or granting broad passwordless access
does not repair it. Keep the distinct ownership/helper prerequisites visible;
no new privileged helper, core fork or polkit rule was installed by this test.
