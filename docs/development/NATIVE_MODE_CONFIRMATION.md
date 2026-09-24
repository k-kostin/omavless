# Native mode confirmation — disposition of Python PR #135

RC-only candidate, 2026-09-24. Relates to #132; does not close DNS authorization
or claim a live cancellation matrix passed.

## Native comparison

Old PR #135 (`bf3af3618488d72536791a88f358d54922e9d472`) stages Python/QML
status fences and controller readiness. Python is retired; merging that branch
would not implement native DNS completion. Preserve its evidence, not its stale
production architecture.

Native lifecycle already serializes mutation/read ownership, checks configured
core/TUN/controller readiness in every mode and verifies rollback before reporting
`TransitionFailedRestored`. `lifecycle.rs` tests cover successful mode replacement,
failed-start rollback, failed-stop manual recovery and disconnected preference.
Service.qml does not optimistically replace snapshots when a button is pressed;
its action boundary fences instance/revision/operation and unknown outcomes.

However, `NativePresentation.project` previously returned desired mode even for
unavailable/recovery states and Panel used that value alone for accent styling.
That made an unconfirmed desired value look like the selected working mode.

## Bounded correction

The normal selector remains in the same position with the same actions and
labels. `modeConfirmed` is false during pending/unknown, stale/unavailable,
failure or manual-recovery states. Accent is shown only for coherent local
connected ownership or a verified clean disconnected saved preference.
Desired mode stays available to explicit action construction; presentation never
rewrites desired state or manufactures rollback.

No new DNS/route/internet claim: even coherent owned TUN/configuration evidence
does not prove resolved authorization. The version-matched sing-tun DNS path
launches asynchronous resolved commands and ignores their failures. The required
DNS work remains owned by #270/#132, including the original attended cancellation
matrix. UI correction alone is not closure of #132 or RC readiness.

## Acceptance

Production-function regressions cover pending, unknown, stale and recovery,
successful/restored mode and disconnected preference. Existing Service admission,
duplicate click, stale reply and rollback tests remain applicable.

Before acceptance: developer/QML suites, exact-head EN/RU isolated rendering of
confirmed/pending/recovery and restored states, installed frontend identity and
preserved real runtime state. Synthetic states are not real authorization tests.
No core restart or VPN transition is needed merely to verify this visual signal.
