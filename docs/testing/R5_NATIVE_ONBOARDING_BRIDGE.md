# R5 fenced onboarding completion bridge

This bounded checkpoint exposes the already registered `onboarding.complete`
transaction through fixed `plugin onboarding-complete INSTANCE REVISION
OPERATION` CLI syntax. It has no private stdin, flags, arbitrary method or
caller-provided setup values. The existing direct `onboarding complete` command
is unchanged. The follow-up QML bridge below reuses the existing wizard. No
installed package, service or private fixture is changed by development tests.

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

Installed QML completion acceptance is a separate integration gate. No live
VPN or login behavior is claimed from these deterministic tests. Python cannot
be retired by this checkpoint; R5/R6 remain incomplete.

## Existing three-step wizard restored for native ownership

The same OnboardingWizard component retains its legacy default and gains an
explicit native context. It displays the independent core-installation facts
from the core-readiness prerequisite, always labels service permission/TUN
creation unverified, and hides the legacy install/setcap/path commands. Browsing
setup is not gated on fabricated `tunReady` facts. Native steps reuse the
existing routing-preset action and clipboard/file private import flow; preset
selection advances only after the matching stored preset is observed, not on
the initial click. Imported-profile counts do not claim tested connectivity.

Unacknowledged first use opens the guide once per panel opening. Dismissal does
not write anything; completed setups remain accessible through Settings Open.
Finishing submits only the fenced completion acknowledgement and closes to the
normal pending/error/reconcile surface. Closing a panel does not cancel or
relabel an admitted mutation. Unknown outcomes retain the original operation
for the existing exact replay/state-review controls. No startup preferences or
login units are configured by the guide.

UI tests cover first-use/dismiss/completed states, stale metadata, concurrent
import dialogs, preset-confirmation ordering, explicit completion admission,
unchanged legacy defaults, English/Russian semantic text and the actual QML
component compile graph. Exact installed English/Russian three-step visual and
import/completion acceptance remains required before merging this UI slice.

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
