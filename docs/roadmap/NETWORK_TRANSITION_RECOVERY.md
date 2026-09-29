# T4 suspend and network-transition recovery proposal

Status: inactive design candidate. No event subscriber, timer, IPC method,
retry worker, route/DNS write or host-network behavior is implemented here.
This is distinct from the existing bounded **process-start** reconciliation in
`lifecycle.rs` / `connection_transaction.rs`. It does not make resume or network
changes a second unconditional startup path.

## Existing boundary and user task

The current Rust owner separates durable desired intent from observed actual
state. At process start, it adopts an exactly verified owned connection,
recovers a proved-empty one once when connected intent remains, or requires
manual recovery for an inconsistent owned observation. A failed recovery keeps
connected intent and does not loop. A local core/controller observation is
not proof of working DNS, routes, remote reachability or leak protection.

The user's goal is that a previously requested connection survive ordinary
suspend/resume or a network-interface change where this can be proved safe.
An OS event is only a hint to re-observe, not evidence that the VPN failed,
that its route is safe, or that a new connection is authorized. The primary
product action remains the user's Connect/Disconnect; background handling must
not change Off to On. During inspection the UI may show a neutral, bounded
“Checking connection” state, never a premature Connected or fatal recovery
claim. Actual UI wording and rendering require a later review.

## Candidate event fence

- Only the committed Rust owner may process events. An event record is scoped
  to that owner instance and the exact desired generation/revision observed
  when admitted. It contains no SSID, BSSID, public IP, profile URI, provider
  URL or raw network-manager payload in shareable status/logs.
- Suspend begins an observation pause, not a disconnect, service stop or
  desired-state write. On resume or a network-change hint, coalesce a burst
  into one pending check after bounded stabilization using monotonic time.
  Wall-clock jumps cannot create extra attempts. An expired/stale event is
  discarded without lifecycle effects.
- Re-read owner marker, desired state and fresh owned-host facts under the
  existing mutation lease immediately before any action. If an explicit
  Disconnect, Full Quit, profile/mode change, runtime replacement or later
  network epoch won, discard the old event. A queued event never reserves a
  future Connect and never interrupts an in-flight user mutation.
- Desired Off means no reconnect, regardless of a visible old core or an event.
  Desired On with exactly healthy owned state means adopt/observe; do not
  restart merely because external internet or one endpoint probe failed.
  Desired On with **proved empty** owned state may be eligible for at most one
  bounded recovery for that stable epoch and exact generation, through the
  existing coordinator. This is a candidate policy, not current behavior.
- Missing, contradictory or un-attributable controller/core/TUN facts are
  uncertainty, not “empty”. Unknown ownership, pending recovery barrier,
  failed cleanup, incomplete DNS/route/protection proof or foreign-VPN
  ambiguity cannot trigger an automatic retry. Preserve connected intent,
  expose safe remediation and require the established explicit recovery path.
  Do not kill/reconfigure foreign cores or relax K1 protection to get online.
- A failed bounded attempt is terminal for that event/generation. Repeated
  resume/link notifications, daemon restarts or a backward clock cannot turn
  it into a retry loop. A later attempt needs a separately justified new
  stable epoch and fresh admission; if the outcome is uncertain, no later
  automatic attempt is allowed until explicit reconciliation. The receipt and
  interruption semantics for this guarantee need their own durable design.

## Before implementation

Choose the supported event source per host family and prove that it reports a
new stable epoch rather than merely rebroadcasting a stale link fact. No
desktop/NetworkManager assumption belongs in shared Rust domain logic. Define
whether the event can be handled without a route/DNS repair step; if repair is
needed, it requires a separately reviewed transactional host adapter and
corresponding K1/protection ordering. Do not infer that the existing start
reconciler is safe to call on every resume event.

Deterministic pure fixtures should cover: Off/resume; On/healthy; On/proved
empty; observation unavailable; mixed owned/foreign inventory; event burst;
out-of-order event; generation change by Disconnect, Quit, profile or mode;
suspend during active mutation; recovery failure; interrupted/unknown result;
owner restart and monotonic/wall-clock discontinuity. Assert both the decision
and **zero host calls** in every refused/stale case. All fixtures are synthetic
and credential-free. Exact-head Try Omarchy can validate ordinary event
integration, but a real physical suspend/NIC transition is a separate
bare-metal gate under `ACCEPTANCE_ENVIRONMENTS.md`; VM success cannot claim it.
