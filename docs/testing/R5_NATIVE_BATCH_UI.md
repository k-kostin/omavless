# Native long-operation frontend

This checkpoint connects existing Rust subscription refresh-all and rule-provider
refresh operations to the native QML panel. It does not implement another worker,
change refresh transaction semantics or complete R5/R6.

The existing Subscriptions page gains Update all. Settings gains remote-rule
refresh. A compact shared status area exposes bounded progress, cancellation,
terminal result and explicit reconciliation when the response is uncertain.

## Safety and ownership

- Fixed CLI launchers only: start, get, cancel. No URL/provider/rule payload in
  argv or progress; only instance/operation IDs and revision are sent.
- Rust remains the canonical scheduler and mutation owner. The UI correlates
  exact instance, operation and method, validates the bounded progress schema,
  and does not interpret raw errors as user-visible strings.
- Unknown start does not authorize a new operation ID. Explicit retry retains
  the original request; state lookup and cancellation address that same job.
- Polling is bounded. Closing the panel does not cancel daemon-owned work;
  cancellation is explicit. Disconnect remains reachable during background work.
- Shell restart is not durable UI job recovery: this checkpoint retains the job
  only for the lifetime of the plugin instance. Existing server serialization
  remains authoritative; no claim of a persistent client job journal is made.

## Acceptance

Run launcher, operation parser/state-machine, QML and localization tests.
Installed acceptance must exercise a real start/get terminal cycle, inspect
safe progress only, preserve private subscriptions, and distinguish deterministic
cancellation coverage from an actual live cancellation race. Visual acceptance
covers English/Russian states and panel close/reopen. Main/marketplace remain
unchanged until the complete dependent UI stack is accepted.
