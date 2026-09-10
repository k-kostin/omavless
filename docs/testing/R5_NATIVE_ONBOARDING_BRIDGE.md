# R5 fenced onboarding completion bridge

This bounded checkpoint exposes the already registered `onboarding.complete`
transaction through fixed `plugin onboarding-complete INSTANCE REVISION
OPERATION` CLI syntax. It has no private stdin, flags, arbitrary method or
caller-provided setup values. The existing direct `onboarding complete` command
is unchanged. No QML, installed package, service or private fixture is changed.

## Ownership and parity

Python's `onboarding-complete` dispatch remains the legacy/oracle path. Rust
already owns the native store-only completion transaction; this change adds the
same plugin-instance/revision/replay adapter used by the other native actions.
It does not implement a second state mutation. The native coordinator retains
whole-store validation, same-user atomic private writes and compensation,
shared operation IDs, migration ownership and manual-recovery barriers.

An accepted completion only means the persisted onboarding flag was saved.
It does not establish Mihomo installation, TUN capability, successful import,
working connectivity or effective login activation. The frontend must not use
this command to mark any of those independent checks successful.

The fixed CLI uses exit 74 only for pre-dispatch refusal and exit 73 for an
unknown socket outcome. Exact replay retains the old operation ID and revision;
a changed runtime instance is refused before effects. The plugin response has
the existing bounded applied envelope and no new credential-bearing field.

## Deterministic acceptance

New tests cover exact fixed CLI mapping, every required field, extra setup/path
fields, invalid metadata, unexpected stdin, private-error non-echo, real private
Unix CLI request framing, pre-admission refusal without a socket connection and
dropped-response outcome-unknown. The actual native owner socket test covers
wrong instance, successful store-only save, replay, stale revision, shared
operation collision, pending manual-recovery barrier and ownership revocation.
Startup configuration, current profile/routing selection and host-call count
remain unchanged. Existing coordinator completion/compensation tests remain
the oracle for transaction mechanics. All fixtures are synthetic private files.

Installed QML completion acceptance is a separate future bridge gate. No live
VPN or login behavior is claimed from these deterministic tests. Python cannot
be retired by this checkpoint; R5/R6 remain incomplete.

## Startup preferences remain intentionally unregistered

The existing `startup.configure` offline transaction is not exposed here. Its
own [contract](R5_STARTUP_POLICY_FOUNDATION.md) requires registration alongside
a trusted once-per-user-manager-login trigger. Saving enabled preferences must
not masquerade as effective autoconnect.

The next real login integration prerequisite is a resource-aware isolated
validator for the exact snapshotted candidate, or explicit refusal when safe
validation is impossible. Current Mihomo `-t` with persistent resource paths
can fetch or write; the [snapshot report](R5_STARTUP_VALIDATION_SNAPSHOT.md)
explicitly excludes it from the read-only login host contract. Then integrate
trusted user-manager epoch/one-shot ordering, strict empty-host observation,
ordered pending/consumed receipt transaction and disabled/legacy-conversion
gates. Ordinary daemon restart must reconcile current desired state and must
never reapply login preferences after an explicit disconnect. No workaround to
these requirements is introduced for UI parity.
