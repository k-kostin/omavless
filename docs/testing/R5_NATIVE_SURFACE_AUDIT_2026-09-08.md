# R5 remaining native surface audit — 2026-09-08

Read-only code audit; no runtime, private store, desktop or Cargo changes.
This is a planning artifact, not exact-head acceptance or a merge verdict.
References below are repository-relative; line numbers refer to the inspected
candidate and may shift after consolidation.

## Inspected snapshots

- Main includes diagnostic PR #177 at `ccaab476a422dd5144198888323f140aa5ec1a01`.
- #174 custom rules: `e4c91f0`, custom-rule worktree.
- #175 desktop helpers: accepted `4d74856`, already merged.
- #176 startup policy: root worktree `877d301` at inventory time.
- #179 routing presets: `c1e8480`, preset worktree at inventory time.
- #180 route fast paths: `44a6a31e1050f27e66be95b57b0589ae40c59df2`.
- Existing batch scheduler branch `934bc8d` inspected only to avoid mistaking
  its implemented dispatch for wholly absent work; acceptance/merge not inferred.

Installed `plugin/Service.qml` still invokes Python via `backend.sh`; registration
behind a committed native ownership marker is not installed plugin activation.

## Already implemented: do not duplicate

| Existing UI action | Native foundation / registration | Actual remaining boundary |
| --- | --- | --- |
| Connect/disconnect/set mode | `lib.rs` native mutation list; `connection_transaction.rs`, `native_dispatch.rs` | Host cutover and frontend composition, not a new state machine |
| Custom-rule list/add/delete | #174 `routing.custom_rules.list/add/delete`; private transaction and fixed CLI | Combined exact-head gate, bridge and installed acceptance |
| Apply routing preset / keep mode | #179 `routing.set_preset`; `routing_preset.rs` transaction | Combined gate, bridge; do not conflate preset application with refreshing live providers |
| Advanced loaded rules/providers | #177 `diagnostic_read.rs:18-49`, `diagnostics.summary/rules/providers` | Bridge existing advanced-diagnostics UI; not support snapshot parity |
| Global/direct/custom/disconnected route check | #180 `route_check.rs`, `route_check_protocol.rs` | Live unmatched Routing observation deliberately absent |
| Save login preferences | #176 `startup_protocol.rs`, `native_coordinator/startup.rs`, `startup_validation.rs` | Offline only: production dispatch, login integration and host gate still required |
| Import/edit/export/clipboard/QR | Existing semantic profile APIs plus #175 `desktop_helpers.rs` | QML composition, lifecycle cleanup and native chooser fallback decision |

## Real remaining code operations, grouped by owning task

### 1. Live rule-provider refresh

Current UI: `Service.qml:1318-1323`; Python owner
`backend.py:4279-4315` (`refresh_rule_providers`).

No corresponding native mutation is registered. #177 is read-only GET and
#179 changes preset/template/store, neither updates live providers. Required:
fixed-purpose discovery of refreshable provider identities internally, bounded
private-controller updates, partial-failure classification, then fenced
`rulesUpdatedAt` store commit. Preserve bounded concurrency (Python uses four),
ownership/revision checks and disconnect responsiveness. Do not accept arbitrary
controller paths, provider URLs or raw HTTP methods from a client. Determine
the long-operation boundary before exceeding the unary deadline.

### 2. Connected Routing destination observation

Current UI: `Service.qml:1327-1340`; Python
`backend.py:4422-4495` (`live_route_match`). #180 explicitly returns
`capability_unavailable` when connected Routing has no custom-rule match.

Missing: bounded probe through the owned active mixed port, private controller
connection/rule observations, safe result projection and cleanup. The Python
oracle prefers a new matching connection and falls back to changed hit counters;
concurrent-traffic ambiguity needs explicit treatment, not fabricated certainty.
No network work is needed for #180's already implemented deterministic paths.

### 3. Subscription latency test operation

Current UI: `Service.qml:1404-1509`, `subscription-probe-stream`; Python
`backend.py:1083-1570` includes resolver selection, pinned addresses, isolated
Mihomo probe configuration/process, fixed public probes and progress output.

`omavless-mihomo/src/lib.rs:364-431` supplies response accumulation and a fixed
probe schedule only. It is not a native execution/stream/cancel API. Missing:
bounded resolution, private temporary config/core/socket ownership, supervised
job, cancellation/progress and guaranteed cleanup without changing live tunnel
intent. Reuse long-operation infrastructure; do not equate subscription fetch
or refresh-all with latency probing.

### 4. Support diagnostics and private connection facts

- `backend.py:5595-5677`: support diagnostics inventory/environment/readiness/
  consistency snapshot, safe text and atomic export. #177 `diagnostics.summary`
  means combined rules/providers only and does not implement this snapshot.
- `Service.qml:1733-1743`: diagnostics destination chooser/export composition
  still shell/Python; #175 provides atomic client export, not this semantic
  snapshot or its save-chooser integration.
- `backend.py:5704-5714`, `Service.qml:647-676`: private connection details
  (interface addresses, server/transport/SNI). No equivalent registered native
  operation. Keep private details separate from shareable diagnostics and from
  ordinary credential-free status; infer interface from owned runtime rather
  than accepting arbitrary host paths/interfaces through IPC.

### 5. Telemetry and explicit connection test

`Service.qml:641-719,2026-2060` still directly runs shell helpers for interface
byte counters, tunnel-bound ping and two fixed public exit-IP endpoints.
No runtime telemetry/test dispatch is registered in the inspected candidates.

These shell helpers are not Python dependencies by themselves, but R4 explicitly
owns traffic/status sampling and probe orchestration. Required native boundary
must preserve tunnel binding, timeout versus local-unavailable distinction,
generation-scoped stale-result refusal and bounded scheduling. An observed
exit IP is not proof all traffic uses the tunnel. Do not put telemetry history
into the small durable state file.

### 6. Startup activation and legacy migration

#176 `docs/testing/R5_STARTUP_POLICY_FOUNDATION.md` states the exact gap:
`startup.configure` remains unregistered and no login unit/CLI is installed.
Implement a once-per-user-manager-login trigger, distinct from runtime restart
reconciliation. Preserve explicit disconnect across daemon restarts. Convert
or refuse legacy `startupConfigured=false` unit-derived enablement; do not
silently interpret it as disabled. Verify selected-profile disappearance,
validation failure, unit rollback and duplicate-core refusal.

Host blocker #178 is separate: packaged `NoNewPrivileges=yes` conflicts with
Mihomo gaining file capabilities. Solve the accepted host architecture before
claiming native Full VPN/login readiness. Do not weaken OS security to unblock
this foundation. This does not block pure semantic/protocol work above.

### 7. Small UI state and lifecycle helper semantics

- `Service.qml:1276-1282`, `backend.py:6014`: onboarding completion writes the
  private store; no native mutation found. Existing store projections preserve
  the flag but do not implement setting it.
- `Service.qml:1900-1914`, `backend.py:5091-5143,5795-5826`: explicit intent,
  observed-active rearming and deduplicated unexpected-drop notifications.
  Native bridge must replace these with canonical owner transitions/event
  deduplication or a fixed client notification boundary. Do not copy legacy
  marker files into a second state machine blindly.
- #175 intentionally lacks the existing Python GTK4 chooser fallback. Decide
  supported native chooser integration before switching all file-import users;
  preserve file-content versus path semantics and cancellation exit behavior.

## Composition and acceptance, not new semantic implementations

`Service.qml:929-1010` expects a monolithic legacy status payload with routing,
startup, readiness, desktop capabilities, inventory, conflicts and uptime.
Native `lib.rs:456-477` status returns desired/actual/profile/mode/transition/
ownership; separate profile/subscription lists already exist. The bridge needs
an explicit bounded status/settings composition contract and missing projections,
not an assumption that replacing the executable name makes the schemas match.

Refresh-all also remains a merge/activation dependency: the separate inspected
batch scheduler has actual `subscriptions.refresh_all/operations.get/cancel`
dispatch. Audit/gate that branch before opening a duplicate implementation.

Final activation gates remain: exact packaged host readiness (#178), controller
liveness versus selector/config readiness, one canonical owner, reversible
cutover/rollback and legacy startup state, desktop/helper composition, QML
error/status localization, install identity and real connect/disconnect plus
shell restart/remove/disable acceptance. Existing primitives or synthetic tests
do not satisfy these production gates. R5/R6 remain incomplete.

## Suggested independent next ownership lanes

1. Support snapshot + safe client export + settings/status read projections.
2. Live provider-refresh operation, then destination observation as its own
   bounded controller/probe task.
3. Subscription probe supervisor/job using shared operation infrastructure.
4. Onboarding mutation and notification transition semantics; fixed desktop
   composition can proceed separately without enabling tunnel cutover.
5. Host #178 + once-per-login integration, then final bridge and exact installed
   acceptance. This lane is a real activation dependency, not a blocker for all
   other Rust migration work.
