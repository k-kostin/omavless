# K1 selected normal-owner protected lifecycle

Status: architecture proposal only, based on exact
`65d53d7ab0b6ae97ff666208e1b5f0c291b341fd`, on the separate
`dev/k1-normal-protected-lifecycle` branch. No new authority, daemon selection,
protocol, executable or VM acceptance is implemented by this document. Preserve
[Native57](../testing/K1_NATIVE57_VM_2026-10-07.md),
[cold60/61](../testing/K1_COLD_BOOT_VM_2026-10-07.md) and Draft703's exact tested
source/artifact identities. The [K1 threat model](../roadmap/KILL_SWITCH.md),
[fixed client](K1_FIXED_CLIENT.md), [preparation](K1_PROTECTED_PREPARATION.md)
and [native composition](K1_OWNED_NATIVE_COMPOSITION.md) retain their boundaries.

## Smallest selected behavior

Add a separately compiled, default-off `netguard-normal-lifecycle` feature
depending on the existing runtime candidate, NOT `netguard-native-scenario`.
An explicit closed daemon selection, proposed
`daemon --developer-protected-lifecycle`, registers the SAME genuine current
native owner, singleton, scheduler and mutex. Existing typed Connect/Disconnect
requests and instance/revision/operation-ID fencing are reused. No user request
contains a mark, interface, helper path, PID, namespace, command, coverage token
or arbitrary policy. Normal no-flag builds/invocations remain unchanged.

First scope is one fresh disconnected owner, one supported profile, desired
Full/global intent, the qualified fixed protected core Rule policy, and one
explicit Disconnect. The selected daemon never falls back to ordinary
unprotected Connect. Connected replacement, mode changes, active profile/preset
edits, background writes, auxiliary probes, automatic restart/reconnect and
startup/autoconnect are refused in this first cut. General public/default K1
registration, UI changes and arbitrary profiles/templates are not included.

Retained-phase admissibility is CENTRAL, not a check only in Connect/mode/profile.
Every ordinary mutation and its detached completion must pass it before any
effect: settings/startup, import/replace/rename/favorite/delete, subscription
add/update/refresh/delete/batch, provider refresh, preset/custom rules,
background work, administrative handoff and Quit/shutdown. Armed permits only
the separately admitted protected Disconnect; InFlight/Unknown permits no new
effect or shutdown. Cached replay is DATA, not another execution. Preserve
genuine nonmutating observations and local client appearance/navigation; neither
can clear a phase or implicitly reconcile/start/stop a core. No generic method
or unknown future mutation silently bypasses the selected-mode gate.

Keep only the qualified `RuleTcpVerifiedTlsDohV1` renderer, numeric IPv4 verified
VLESS/TCP/TLS profile subset and exact core/broker/receipt whitelist:

- core `897ada648fe975718ac1b7318702def5b826a9901797a0d13cdd333a012b9fcb`;
- broker `4bbba825bc39209cd821af18f7a776825bf762c46a05e43f4354aca283009c70`;
- receipt `c6e283d9c8b4c2fa59163e6a363b93788a36b210f33d5d9dee2d8b5eec5bd8f2`.

Full user intent does not permit core Global mode: retain Rule mode, UDP reject,
ICMP-forwarding disable, fixed mark/omavless0, numeric tunneled DoH and all
original validation, package/capability/config/TUN/readiness guards.

## Owner and operation custody

Use one private selected connection adapter within the existing registered
owner. It owns persistent protected phase and FixedClient; each operation only
borrows the original executor inside its existing transaction. Never store a
self-reference, extract/replace the executor, construct a second owner, consume
RuntimeServer as Connect, or permanently block a successful ordinary scheduler.
No public/injectable production callback or caller-created admission is added.

Install retaining custody before selected startup/acquisition. The normal
current constructor can reconcile/start connected intent, so selection must
require genuine current ownership/login plus strict disconnected/no-pending/
no-core/TUN/startup-disabled admission BEFORE ordinary startup effects; it
cannot construct normally and check Off afterwards. Existing fixed paths,
original marker/generation and canonical namespace assumptions remain.

Each typed mutation uses the SAME dispatcher mutex and its original checked
MigrationLock for that complete operation, including all local/NetGuard
boundaries and final publication. A positive Connect may return/release that
operation lease only after complete final checks; the original owner/core,
prepared bindings and completed FixedClient remain retained across human time.
Disconnect independently obtains its current operation lease and rechecks those
SAME originals, exact desired/profile/generation and singleton/login/ownership.
This is not old-receipt adoption or reconstruction of a controller/port.
An uncertain operation retains its own original lease with that whole graph.

The existing ordinary NativeHost/OwnedCore Drop stops a child and RuntimeServer
Drop unlinks its socket. Neither may run as cleanup of Armed/InFlight/Unknown
selected state. The selected server/owner custody therefore retains the entire
graph on unwind/error or attempted shutdown while not positively Closed.
Handled uncertainty seals effects and preserves bounded read-only diagnostics;
no resend, replacement, compensation, cleanup or Status-as-repair follows.
Fatal process death remains unavailable, not descriptor survival.

## Strict compatibility-pointer transition

Ordinary Connect commits activeId/lastId AFTER lifecycle success. The protected
Bound currently captures a whole-store digest BEFORE that write; silently
recapturing the changed store would weaken its guard. Ordinary pointer failure
also compensates with Disconnect/reconnect, which is forbidden after Arm.

Before effects, prepare the existing exact pointer plan under the original
operation lease. The selected binding predeclares BOTH complete old and complete
candidate store digests from that immutable original plan, and admits only its
fixed pointer change with unchanged selected canonical profile. Preserve the
old binding until the original plan's known completed write plus complete
postchecks; only that explicit phase transition selects the predeclared new
binding. No hash supplied by IPC, live-byte recapture, broad ignored-field mask
or exception for another mutation is allowed. The historical native roundtrip
keeps its original unchanged store guard.

Pointer publication after Arm that is failed, ambiguous or fails final checks
retains Armed/manual recovery; NEVER invoke ordinary rollback/disconnect or
reconnect. Disconnect's pointer plan similarly remains under its same original
operation lease. Positive closed policy/core emptiness is distinct from a later
metadata-publication failure; do not silently reconnect or erase that failure.
Keep a distinct `ClosedMetadataFailure` disposition: the positively verified
Closed generation/core-empty fact survives, but failed/uncertain metadata
completion keeps dependent mutations/shutdown sealed and the graph retained.
It is neither Armed uncertainty nor ordinary positive Closed completion. Normal
status/result must not advertise healthy connected/armed, automatic repair or
a successful whole transaction from that partial terminal fact.

## Completion matrix and affected seams

| Boundary | Required selected behavior |
| --- | --- |
| Default/no flag | Existing factories, methods, features, templates and lifecycle unchanged |
| Fresh selected startup | Genuine current, stable Off/empty, no pending/startup; no unprotected reconciliation or copied authority |
| Connect | Existing scheduler/replay and original lease; validated bound policy; checked max(desired, closed floor)+1 reservation while Off; exact Arm; connected intent; original core/TUN/readiness; exact pointer commit; one revision advance |
| Armed between RPCs | Same retained owner/port/bindings; no mutable background/ordinary replacement bypass; no traffic child or synthetic probe in product code |
| Every mutation/quit | Central retained-phase admission before entry AND detached completion; only protected Disconnect in Armed; none in InFlight/Unknown/ClosedMetadataFailure |
| Disconnect | Exact current originals and generation; disconnected intent then owned stop/empty, exact Disarm and positive final Status; pointer completion and one scheduler outcome |
| Closed plus metadata failure | Preserve known Closed/core-empty versus failed metadata as distinct facts; retain/seal, no automatic reconnect or successful-completion claim |
| Refusal/uncertainty | Pre-entry invalid/stale/busy/replay remains distinct; effect uncertainty seals/retains, never reset or compensating fallback |
| Shutdown | Ordinary client exit is neutral; selected runtime releases only a known positively Closed graph, otherwise retains; fatal loss outside live custody claim |

Affected source is expected to be selected main/server construction and lifetime,
the registered connection adapter, normal coordinator scheduling/transaction
pointer completion, and private protected lifecycle/preparation. NetGuard v2
protocol, root service/receipt/creator/kernel code, vendor trees and cold bootstrap
do NOT need a new authority producer. Separate interval-only trait/code from
the production preparation path: no `development-native-tests` selection,
test-traffic ELF, observer receipt or fixture scratch is a product dependency.

## Source gates before installed selection

1. Default feature/factory/wire absence and unchanged ordinary regression cases.
2. Selected genuine-Off/startup and unsupported mode/profile/pair/capability
   refusals before Arm; no silent ordinary fallback or startup activation.
3. Real scheduler/owner tests for stale instance/revision, shared IDs/replay,
   busy/collision/exhaustion, no double revision and EVERY listed mutation/quit/
   detached-background denial; nonmutating observations remain available.
4. Protected ordering and all pre/post-Arm failure cuts; original graph Drop/
   unwind counters, no compensation/retry, distinct operation-lease admission,
   same peer/package/desired/profile/generation and final-Status guards.
5. Exact predeclared pointer old/new transition; any unrelated byte/profile
   change, failed/ambiguous commit or drift refuses; historical guard unchanged.
   Distinct known Closed plus metadata-failure cuts never reconnect or rearm.
6. Protected config/readiness and normal observational consumers; no fixture
   child/traffic selection compiled/reachable in this feature. Focused tests,
   strict opt-in/default Clippy/fmt, then meaningful combined source gates.

Full primary and independent architecture/affected-source review precede
authority implementation and ROOT's separately selected VM effects.

## One executable installed gate

On a fresh whole-qualified disposable image with genuinely empty/disarmed root
state, ROOT installs the exact new app/package and matching complete K1 DNS,
NetGuard/unit/enrollment bundle; earns normal login/group/current ownership;
and admits the unchanged controlled numeric VLESS/TLS plus DoH/HTTP fixture.
No old cold image/records are cleared or used as fresh state.

Start the explicit selected NORMAL daemon, not an ignored test ELF. Observe
ordinary owned Off. ONE existing typed Connect for the stored synthetic profile
in global/Full intent must complete originally, with verified Arm generation,
desired On and attributable core/controller/omavless0. Separately selected
ordinary unmarked DNS/TCP requests through the controlled fixture establish the
bounded positive traffic result; passive packet/rule observation is DATA, never
input to admission. No bypass mark is granted to a traffic requester.

ONE ordinary typed Disconnect must complete originally after owned core/TUN
emptiness, matching closed generation and final Status. Independent normal
Status and finite read-only kernel/record evidence verify owned Off/no TUN and
preserved high-water. Positive normal Quit/stop and whole-image preservation are
separate original operations, not cleanup of uncertainty. No physical, broad
protocol, fault/restart, default distribution or whole K1 closure is claimed.
