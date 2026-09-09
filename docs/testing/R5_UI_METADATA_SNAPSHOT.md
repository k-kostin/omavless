# R5 private UI metadata snapshot

`ui.snapshot` is a fixed read-only native method with empty params, exposed by
`omavless plugin snapshot`. It is a bridge building block, **not** a replacement
for the current Python `status` response and not a frontend cutover.

## Consistency and state meaning

The committed Rust owner holds its existing migration/store lease while reading
one canonical store and desired intent. The serialized owner supplies the
last-known lifecycle state; the result's `instanceId` and ordinary envelope
`revision` identify the runtime response. Candidate, stale and revoked ownership refuse. No host
command or controller probe runs as part of this metadata read.

Consistency assumes cooperating writers honoring the existing lease. Revision
is not a content hash, and this does not promise atomicity against direct file
edits or stronger filesystem hardening than the canonical reader provides.

The payload distinguishes desired connection state from `lastKnownActual`.
`healthFresh` is false and `liveHealth` is unavailable. A cached connected state
or selected profile is not evidence of a currently healthy core/TUN. Manual
recovery must remain visible, not be converted into an ordinary disconnected
boolean. A future client must not use this response alone for live VPN health.

Metadata includes existing safe profile/subscription list projections, last
selection, stored startup preferences, onboarding completion and stored routing
metadata. Stored startup enablement is not proof of an enabled login unit;
stored routing is not proof of active kernel routes. Unsupported host facts
are not filled with invented false/zero values.

## Privacy and bounds

This is **private same-user UI data**, not a shareable support report. Internal
IDs and user-authored display names are intentionally present so the client can
render and select existing records. Dedicated URI, credential, subscription URL,
endpoint, raw config, controller secret and raw core error fields are omitted.
User-authored names can themselves contain private text; this is not a general
secret scrubber. The
existing private socket and ordinary response bounds apply. No export/edit-input
call is used to reconstruct information withheld from normal list projections.

## Compatibility and remaining bridge work

Python remains the installed plugin owner and rollback reference. Existing list
projections and domain semantics are reused, not reimplemented. The new typed
response intentionally does not imitate legacy `Service.qml.applyStatus`:
that interface still requires host readiness, effective startup, route metadata,
uptime/conflicts and explicit presentation of transitions and stale health.

`backend.sh` and QML remain unchanged. Later activation must honor the existing
generation-fenced frontend selector and explicit command mappings. Once Rust
owns the tunnel, failure must not silently fall back to a second Python owner.
R5/R6 and Python removal are not established by this snapshot.

Tests cover protocol/CLI bounds, ownership refusal, metadata semantics,
malformed private state, no lifecycle effects and credential exclusion using
synthetic fixtures. Real Unix-socket tests establish the read registration;
they do not claim installed UI or live VPN acceptance. No plugin reinstall or
private fixture is required for this read-only checkpoint.
