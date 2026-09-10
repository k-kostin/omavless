# Native explicit profile details

`profiles.details` / `profile details ID` restores the original endpoint,
transport/security and SNI display boundary, not credentials or TUN observation.
It holds the existing ownership/private-store lease through projection creation,
uses the stored canonical model's existing preview, and emits only an explicit
seven-field allowlist. Standalone and managed profiles are supported. The
wrapper deliberately has no Debug/serialization implementation; the response
is private UI data, never ordinary status/support output.

No mutation, revision increment, fetch, file export, controller access or host
operation occurs. Exact methods and record-ID validation share the existing
profile-read validator. Malformed input, missing records, unsafe/corrupt/symlink
stores and revoked ownership yield existing stable credential-safe errors.

The established VLESS and non-VLESS canonical corpora plus missing/managed
records compare digest-only outputs against actual Python `details` semantics.
The TUN address field is deliberately absent: a profile read cannot establish
an effective interface address. Transport and security are separate fields;
combining them reproduces Python's existing display, and empty SNI maps to its
existing `--` placeholder only in presentation.

The reference exposed 12 valid Hysteria2/TUIC cases where Python `details`
raises KeyError because its generic code expects network/security keys absent
from those parsed nodes. Those exact 12 cases instead compare against the
already established Python `preview_profile` metadata boundary. The test asserts
this count rather than hiding or copying the legacy bug. No new protocol
fixtures or interoperability/maturity claims follow from this correction.

The QML follow-up reuses the original `DetailPair` grid inside an explicitly
expanded selected profile row. The info action is deliberate: ordinary profile
selection/list/status does not fetch endpoints. Panel close, page/selection
change, mutation admission loss and stale instance/revision discard the private
projection. Fixed-ID launcher, strict bounded parser and disposable read process
prevent private output from reaching argv, logs or shareable reports. An
eight-second watchdog caps frontend waiting. English/Russian privacy text warns
that the view describes saved configuration, not effective connection health.

Installed visual checks remain distinct. No TUN address, copy/export action or
generic IPC metadata dump is added. This does not retire Python or complete R5/R6.

## Candidate checks, 2026-09-10

Dedicated-target backend checks passed: runtime 538 tests, domain 80 tests
(including the 109-case details comparison), strict clippy and formatting.
QML follow-up: six focused Details tests, i18n/contracts, fixed launcher checks,
plugin validation and actual installed-import QML component compilation passed.

Full Python reruns under concurrent VM load took approximately 124 seconds.
The second run passed 342/343 cases (four skips included in the total) but the
existing cross-process status-cache timing assertion failed; an earlier busy
run also hit unrelated core-start/oracle deadlines. No checks were relaxed.
An unloaded serial aggregate run remains required before a full-suite green
claim. No installed profile or live tunnel was used by this branch.
