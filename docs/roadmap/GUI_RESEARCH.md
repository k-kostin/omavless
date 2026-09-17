# G1: optional desktop GUI research

Status: research contract refined 2026-09-17; implementation remains deferred
until the Rust runtime and TUI are stable. No GUI toolkit, JavaScript engine or
plugin platform is selected for production by this document.

The [delivery ledger](../../DEVELOPMENT_ROADMAP.md) owns ordering.
[TUI_APP.md](TUI_APP.md) retains Rust + Ratatui as the first full client.
[CONTROL_PLANE.md](CONTROL_PLANE.md) owns IPC and lifecycle;
[PLATFORM.md](PLATFORM.md) and [acceptance policy](ACCEPTANCE_ENVIRONMENTS.md)
own host scope and gates. This document owns G1 evaluation, not another runtime
architecture. Research may inform T2 presentation without adding GPUI to T2.

## Candidates and the question to answer

Evaluate whether a graphical workspace adds enough usability for profile,
routing and diagnostic work to justify its build, packaging and maintenance
cost. Compare the same small screen before implementing a complete GUI.

| Candidate | Potential benefit | Additional obligation |
| --- | --- | --- |
| Rust GPUI + huacnlee's `gpui-omarchy` | Native components, Omarchy themes and interaction primitives in Rust | Review the pinned GPUI dependency graph, release build and target-platform behavior |
| Rust GPUI Shell host + huacnlee's JavaScript `omarchy-ui` | Presentation iteration and hot reload without rebuilding the host | Review script authority, engine/JIT behavior, host bridge and offline script distribution |

These libraries share a GPUI ecosystem but are not interchangeable APIs.
`gpui-omarchy` is a Rust component library over gpui-base through gpui-kit;
`omarchy-ui` is a JavaScript presentation package for gpui-shell. Neither is a
drop-in Quickshell widget or an OmaVLESS domain implementation. A Shell trial
does not reopen the selected Rust language for runtime/domain/CLI/TUI, and does
not authorize a user-installable scripting/plugin system in OmaVLESS.

Direct Rust is the baseline comparison. Select Shell only if measured iteration
or maintenance benefits justify its extra engine and distribution surface.
Keeping G1 deferred is a valid result; an attractive demo is not an adoption gate.

## Runtime and client boundary

Use a separate optional GUI binary/package. Headless, CLI and TUI installations
must not require graphics libraries, a JS engine or GUI assets. The Rust daemon
remains the sole owner of desired state, private stores, mutations, background
work and Mihomo lifecycle.

- Negotiate the existing semantic API and capabilities. Bind cached projections
  to daemon instance/revision; disconnect or instance change invalidates
  freshness. Do not keep displaying a stale verified Connected state.
- GUI entities own focus, filters, disclosure and draft input only. A successful
  local callback is not proof of a completed daemon operation or healthy egress.
- Keep IPC and heavy work off the UI thread, with bounded updates and buffers.
  A slow client must not hold lifecycle locks or delay urgent Disconnect.
- Closing, crashing or reloading the GUI leaves desired tunnel state unchanged.
  Full Quit follows the existing explicit shutdown contract; it is never a
  side effect of closing a window.
- No direct Mihomo controller, private-store writes, arbitrary shell/systemd
  commands or generic privileged IPC from the frontend.
- Extract a shared client helper only when real clients need it. It may handle
  negotiation, bounded transport and freshness, but must not duplicate domain
  logic or depend on GPUI/Ratatui entities.

For Shell, a compiled Rust `HostModule` is a possible narrow async bridge to the
existing daemon. Registration itself grants access; export only named semantic
operations and bounded data. Keep the JS host in the GUI process, outside the
VPN daemon. Script exceptions being recoverable is not process isolation or
proof against native engine crashes. GUI loss must remain tunnel-neutral.

## Reusable presentation ideas

These clarify existing T2/T3 direction, not new backend feature tracks:

| Reference idea | OmaVLESS use and limit |
| --- | --- |
| Focus separate from confirmed selection | Arrow/j/k movement is navigation; explicit activation has a clear target. Selection still does not prove connection. |
| Semantic themes and live reload | Test directory/symlink replacement, whole-palette fallback and light/dark changes; retain a standalone default. TUI adapters stay framework-neutral. |
| Virtual lists, trees and split panes | Candidates for profiles and later T3/C1 views. Sorting, bounded ingestion, privacy, backpressure and stable row identity remain application work. |
| Rem-based GUI scaling | Recalculate row measurements and hit areas after zoom, translation and resize; separately test native HiDPI/Wayland behavior. |
| Persistent errors, transient success | Refresh success may expire; unknown tunnel/cleanup/manual-recovery state must stay visible. |
| Controlled forms and disabled behavior | Preserve visible targets and permitted actions; pending/failed transitions must not look completed. |

The accepted QML layout remains governed by [UI_UX_CONTRACT.md](UI_UX_CONTRACT.md).
Do not use these references to redesign it incidentally, restore owner-hidden
Test/latency sections, or turn selected-row styling into connection evidence.

T3/C1 still owns full connections/log views; a table primitive does not supply
that backend. S1 owns transactional app-proxy restore, K1 owns fail-closed
protection, and T4 owns scheduling/recovery. Their feature and host gates cannot
be discharged by adding a control or badge. Arbitrary provider YAML and raw
configuration editing remain outside the accepted product boundary.

## Dated source audit and attribution

Credit Jason Lee / [huacnlee](https://github.com/huacnlee) for the Omarchy libraries
and related GPUI work. References are evidence and design inputs, not upstream
endorsement of OmaVLESS. Retain applicable license notices when reusing code.

| Source checked 2026-09-17 | Finding relevant to the decision |
| --- | --- |
| [gpui-omarchy at b186b959](https://github.com/huacnlee/gpui-omarchy/tree/b186b959382494e2ea31fda94ac3677a82f55528) and [gallery](https://huacnlee.github.io/gpui-omarchy/) | MIT Rust library; main shows 47 previews. Early project, not accepted OmaVLESS dependency. |
| [v0.1.2 to inspected main](https://github.com/huacnlee/gpui-omarchy/compare/v0.1.2...b186b959382494e2ea31fda94ac3677a82f55528) | Published tag differs from main, including submenu and WASM asset fixes. Record which one is tested. |
| [Theme loader](https://github.com/huacnlee/gpui-omarchy/blob/b186b959382494e2ea31fda94ac3677a82f55528/src/system_theme.rs) and [watcher](https://github.com/huacnlee/gpui-omarchy/blob/b186b959382494e2ea31fda94ac3677a82f55528/src/system_theme/watch.rs) | Current/legacy path handling, atomic fallback and coalesced filesystem events are useful references. Shell size overrides are not complete; bound reads in any OmaVLESS adapter. |
| [gpui-omarchy manifest](https://github.com/huacnlee/gpui-omarchy/blob/b186b959382494e2ea31fda94ac3677a82f55528/Cargo.toml) | Pins gpui-kit 0.6.1 and contains a release-profile workaround for gpui-pre-macros. Check a downstream consumer release build; dependency profiles are not inherited. |
| [omarchy-ui](https://github.com/huacnlee/omarchy-ui) | MIT JavaScript presentation classes and theme utilities; application owns copy, IDs, callbacks and domain state. Re-pin its revision before a trial. |
| [GPUI Shell introduction](https://gpui-kit.com/shell/) | Documents M0/unstable API and incomplete plugin contribution/authorization tooling. Standalone composition is distinct from a complete plugin platform. |
| [Shell manifest at 27ab8e76](https://github.com/longbridge/gpui-kit/blob/27ab8e76bba409d969accdb9dc1cf9875ccda81f/crates/shell/Cargo.toml) | `publish = false`; already integrates quickjs-jit/runtime with compiler, rather than only stock interpreter dependencies. |
| [Shell engine at the same revision](https://github.com/longbridge/gpui-kit/blob/27ab8e76bba409d969accdb9dc1cf9875ccda81f/crates/shell/src/engine/quickjs/mod.rs) | Uses JitRuntime; suspends JIT for initialization/first render. `shell_jit_config` suppresses tier-up in debug, citing Linux/Windows backend crashes with many short-lived runtimes; release keeps native tiering. |
| [Engine documentation](https://gpui-kit.com/shell/engine/) | Still describes a non-JIT interpreter at the inspection date. Its benchmark/footprint figures cannot establish performance of the newer source. |

The debug-workload comment does not prove every Linux build or release fails.
Equally, compiling the dependency does not prove release JIT correctness,
interrupt enforcement or Wayland suitability. Reconcile docs, resolved engine
revisions and actual build mode at the start of a trial.

The [QuickJS JIT post](https://x.com/huacnlee/status/2097710503191880015) is a
research lead. Reported workload speedups are not measured OmaVLESS UI, startup
or VPN gains. Cached repaints and script-driven updates have different costs;
profile them separately. No native build or performance acceptance was performed
for this documentation refinement. The earlier web gallery observation is not
Linux graphics, accessibility or VPN evidence.

## Dependency, permission and distribution review

Before adding either candidate, record exact revisions/lockfiles, license and
dependency audit, build/runtime dependencies, required capabilities, persistence
and uninstall behavior. Rust/GPUI does not by itself establish a smaller or
faster application. Run an external release consumer, not only library tests.
Cargo reads [profile settings from the workspace root](https://doc.rust-lang.org/cargo/reference/profiles.html),
so explicitly verify any pinned dependency's build workaround there.

For Shell, inspect the actual host policy rather than assuming all execution
paths have identical defaults:

- [Capabilities](https://gpui-kit.com/shell/capabilities/) distinguish an empty
  Rust grant set from standalone/manifest defaults, including storage. Start
  the synthetic trial without credentials, network, process execution or
  private-store access; disable unnecessary persistence explicitly.
- [HostModule](https://gpui-kit.com/shell/host-module/) exports are authority.
  A package runs with the consuming application's grants; package imports are
  not separate security sandboxes. Review every exposed function and payload.
- [Git dependency resolution](https://gpui-kit.com/shell/dependencies/) occurs
  before script capabilities. A denied script network grant does not prevent
  the host fetching dependencies. Moving refs are re-resolved on load and can
  fail offline. Use exact commits and verify package-owned/offline assets or a
  supported pre-materialization path; pinning alone does not populate a fresh
  machine's cache.
- Account for Git on PATH, dependency caches, generated editor links/configs,
  application storage and diagnostic files. User launch must not unexpectedly
  fetch/build code, require Cargo or obtain additional system privileges.
- JIT adds native-code generation and platform-specific executable-memory
  requirements. Test the chosen mode and limits without weakening host security
  policy. Do not equate script capability checks with an audited JIT sandbox.

The adoption decision must include a rollback that removes only the optional
GUI package/trial artifacts. It must preserve daemon installation, private
profiles, service ownership and the user's desired network state. Do not clear
shared caches or rewrite system themes as trial cleanup.

## Evaluation sequence and stopping conditions

These are future scoped checkpoints, not implementation authorization or
completed gates. Use the existing [acceptance policy](ACCEPTANCE_ENVIRONMENTS.md)
to choose environments; ARM64 evidence does not establish an x86_64/JIT result,
and Arch evidence does not establish NixOS packaging.

### G1a: synthetic native comparison

Use an isolated workspace and identical synthetic fixtures for both candidates.
Build a small status + searchable profile/subscription list + details screen.
Include selected A/connected B, filtered connected row, stale/removed selection,
pending/failure/recovery states and long English/Russian names. Begin with a
fixed split layout rather than a dockable workspace.

Record exact source, lockfile, toolchain, engine/mode, OS/architecture, display
backend and fixture size. Check:

- external debug and release builds, startup with network unavailable and
  package/update/remove behavior appropriate to the trial;
- keyboard/pointer activation, Escape/focus recovery, clipboard/IME where used,
  native accessibility, narrow layout and scaling;
- theme replacement, broken/missing theme and selection/scroll continuity;
- list virtualization under a declared large-list/update workload; bounded
  memory and no synchronous full-view rebuild on every traffic sample;
- cold start, idle RSS/CPU, update/scroll latency and binary/package size;
- development iteration cost, hot-reload state loss and crash recovery.

Set measurement budgets before comparing candidates, and report raw values and
conditions rather than a single speedup ratio. A browser demo or upstream CI
does not replace the target native run. Stop and reframe after three substantive
build/platform failures instead of growing a framework fork during the trial.

### G1b: read-only daemon client

Only after G1a supports continuing, attach through hello/capabilities and
existing bounded status/list projections. Check unavailable daemon, stale
responses, reconnect/instance change, slow updates and GUI close/crash/reload.
Do not add mutations or access reusable credentials to make a demo realistic.
Record remaining API gaps rather than inventing direct store/controller reads.

### G1c: adoption decision and separately scoped mutations

Produce a decision record with candidate comparison, costs, permission review,
tested environments, failures/NOT RUN, rollback and an explicit adopt/defer
recommendation. Only a subsequent selected slice adds connect/disconnect and
other semantic mutations with concurrent QML/TUI/GUI, stale-revision and
client-lifetime tests. Publish no cross-platform or VPN-health claims beyond
the exact evidence. Significant results belong in `docs/testing/`, linked here;
scratch measurements stay outside Git under the documentation retention policy.

## Optional synthetic web showcase

The [gpui-omarchy WASM example](https://github.com/huacnlee/gpui-omarchy/blob/b186b959382494e2ea31fda94ac3677a82f55528/examples/gallery-wasm/README.md)
demonstrates sharing gallery source between native and browser targets. A future
OmaVLESS showcase could exercise agreed screens and translated states using a
separate synthetic transport. This is an optional later research result, not a
commitment that either candidate's current native/JIT stack supports that build.

Label simulated connection/health explicitly. Never accept real subscription
URLs or expose the private daemon through HTTP/WebSocket for a public demo.
Native theme watching, browser accessibility and native acceptance remain
distinct. Defer this work until a useful screen boundary exists; do not create
another product frontend merely to reproduce the reference gallery.
