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

Installed native UI composition/visual checks remain distinct. This backend
checkpoint does not change QML, retire Python or complete R5/R6.
