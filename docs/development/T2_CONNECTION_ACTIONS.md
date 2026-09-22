# T2b: explicit terminal connection actions

Development-only successor to [T2a](T2_READONLY_CLIENT.md), stacked on that
checkpoint because it reuses its client/model/terminal loop. This is not the
complete T2 MVP, default package activation, or permission to update main.
Installed 0.8.2 runtime, QML, package pins and marketplace submission are unchanged.

## Interaction decision

The user can connect a selected profile, disconnect the **current runtime
session**, or change its mode without creating a second tunnel owner. Browsing
and search remain navigation, not connection actions. The connected header stays
independent of selection. The same fixed footer remains in place in each view.

- `c`: review selected available profile, source and intended mode.
- `d`: review current session, never the highlighted alternative profile.
- `1` / `2` / `3`: Full VPN / Routing / Direct confirmation. A disconnected
  mode change saves configuration only; it does not connect.
- Enter submits **only** from an explicit confirmation; Esc cancels locally.
  Repeated/held Enter events cannot confirm. Search consumes these characters.
- `q`, Ctrl+C, terminal close: close the client, **not** cancel a command or
  disconnect. A runtime may finish a submitted command after the TUI exits.
- Minimum action layout is 70 columns by 24 rows. A smaller viewport refuses
  actions, including a confirmation opened before resizing. Exit remains usable.

English and Russian fixed keys share existing mode/status vocabulary with QML.
Provider names are untranslated, bounded plain text. Targets are shown beside
fixed labels rather than inserting them into translated sentence fragments.
No plural/number formatter or global OS locale change is introduced. Broader
shared-catalog consolidation remains a separate T2 task.

## Admission and execution

The feature-enabled `omavless tui` adapter adds only `plugin.action` to the
four existing read methods. It uses `call_plugin_action`, not a short unary
deadline or shell subprocess. The runtime keeps its existing 120-second bounded
response wait, socket ownership/peer checks, canonical parser, serialization,
revision checks, compensated replacement and recovery semantics.

Capabilities must advertise the facade. New commands require a fresh coherent
snapshot, an unchanged instance/revision/target since confirmation, and an
available selected profile for Connect. Connect/mode require a verified settled
local state; recovery still permits explicit Disconnect with current metadata.
There is no optimistic connection/mode change, direct store access, controller
access, arbitrary method API, runtime start or privileged helper.

One capacity-one action worker and one retained request permit only one
outstanding mutation. Random 128-bit operation identifiers are generated from
the OS, independent of private profile identity. Every request carries the
original `instanceId`, `expectedRevision` and `operationId`. Read/input/signal
handling remain responsive while a command waits. A pre-completion observation
cannot certify a post-command result.

Applied replies must match the exact five-field facade schema, instance,
operation, action and non-regressing revision. They invalidate observation and
request a fresh read; success never proves Internet/DNS access. Remote messages
and unknown codes are not echoed. Known errors use bounded local EN/RU messages.

## Unknown outcome

Timeout, transport loss and unverifiable replies retain the original request
and block new mutations. No retry is automatic. `u` opens confirmation for an
**identical** retry (including old instance and revision). A rejected retry does
not establish what happened to the earlier request and leaves it unresolved.

Alternatively `a` opens an explicit acknowledgement of freshly observed,
coherent, settled current state. Enter releases only the local pending lock;
it does not label the old operation successful/failed. Changed state while
reviewing invalidates this acknowledgement. This mirrors the existing QML
facade's semantics; durable receipts across client restarts are still separate
runtime/client work. Do not blindly resubmit after reopening a client.

## Validation boundary

- Deterministic TUI tests: targets, stale/restarted/deleted/renamed confirmation,
  capability gates, recovery Disconnect, search/cancel/repeat, narrow viewport,
  exact retry, unknown acknowledgement, response privacy and EN/RU rendering.
- Feature-enabled runtime test feeds TUI requests to the actual canonical
  facade parser; no duplicate mutation validator is added to the client.
- Synthetic PTYs prove q/SIGTERM during a deliberately slow action restore the
  terminal promptly. The example emits only a fixed test receipt to a new
  private temporary file, not real request data or an installed runtime call.
- `action_preview` is an isolated, explicitly synthetic developer example.
  It never opens the private socket/store; screenshots are not VPN evidence.
- Real host transitions require the attended
  [authorization gate](../testing/HOST_AUTHORIZATION_ACCEPTANCE.md). The bounded
  ARM64 disconnect/reconnect/Full-VPN/restore sequence passed; see the
  [exact-candidate report](../testing/T2_CONNECTION_ACTIONS_2026-09-22.md) for
  evidence, test-tool findings and unrun gates.

Run `bash tests/run-rust.sh` and `bash tests/run.sh`. The default runtime still
excludes Ratatui; build the candidate with `cargo build --locked -p
omavless-runtime --features tui`. No release package/frontend is replaced by
that build. Full T2 still includes grouped subscriptions, management, probes,
traffic/details, theme support, packaging and launch/focus integration.
