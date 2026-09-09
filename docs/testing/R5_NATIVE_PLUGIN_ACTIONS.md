# R5 native frontend connection actions

## Implemented boundary

The native panel adds local profile selection and explicit Connect, Disconnect,
Routing, Full VPN and Direct actions. Selection is not persisted until the
confirmed action is executed. The legacy panel is not re-enabled and native
errors never fall back to Python. This is not complete frontend parity or R6:
unported tools remain explicitly unavailable in the native pane.

Fixed CLI commands are:

```text
omavless plugin connect INSTANCE REVISION OPERATION PROFILE MODE
omavless plugin disconnect INSTANCE REVISION OPERATION
omavless plugin mode INSTANCE REVISION OPERATION MODE
```

`plugin.action` accepts only the corresponding bounded fields, requires all
three freshness/idempotency fields, rejects a stale daemon instance before
effects, and maps to the existing canonical connection/mode mutation executor.
Revision checks, operation replay, rollback and manual recovery are not
reimplemented in the UI. Ordinary existing semantic CLI commands are unchanged.

Success has the normal envelope and exactly
`{schemaVersion:1, instanceId, operationId, action, applied:true}`. It confirms
the mutation outcome, not internet access. Canonical negative responses preserve
stable error codes; QML discards messages and renders its own EN/RU catalog.

## Long operations and ambiguous outcomes

The dedicated action client uses a bounded 120-second total response deadline;
the ordinary five-second unary read deadline is unsuitable for native
validation/start/compensation budgets. Transport loss, invalid response or
timeout exits 73 with a fixed outcome-unknown message. No automatic retry is
performed and no timeout is called a rollback.

The UI retains the exact original instance/revision/operation/payload, blocks
new actions and offers an explicit retry of that same request. A restarted
daemon refuses the old instance. The user may instead refresh, review coherent
current state and acknowledge it before a new action. This acknowledgement does
not claim the old action succeeded or failed. Closing a panel is not Disconnect.
Persistent cross-UI-restart receipts remain separate lifecycle integration work.

Metadata and local observations are combined only with matching daemon instance,
revision, desired generation/state/mode and last-known actual state. Unavailable
facts are not empty inventories. Manual recovery blocks action admission. The UI
does not infer route, DNS, TUN ownership or internet protection from local counts.
Confirmed results invalidate observations and trigger new reads; buttons do not
optimistically update the selected routing mode or connection state.

## Acceptance matrix

- Rust parser/socket/CLI: exact shapes, stale instance/revision, duplicate replay,
  changed-payload conflict, dropped acknowledgement and server restart.
- Production QML JS functions: pending and duplicate-click guards, matching
  results, unknown outcome retention, exact retry and acknowledgement guards.
- Launcher execution: fixed native mappings, legacy refusal, exit73/stdout
  preservation, no shell evaluation and no native-to-Python fallback.
- EN/RU visual: normal disconnected panel, long profile labels and scrollbar
  gutter, unknown-outcome/error states and keyboard focus scrolling. Synthetic
  display states are not represented as actual transport failures.
- Try Omarchy isolated actual QML/CLI/daemon: disconnected mode transition and
  refreshed observation, with poison Python sentinel remaining unused.
- Required before production ownership switch: refreshed package policy, exact
  installed runtime/unit identity, real private-fixture connection/mode/cleanup
  evidence and the transactional activation gate.

The installed production owner remains Python until explicit activation. No
private fixture data or screenshot is committed. Exact candidate, counts and
performed versus pending host gates are recorded in the PR.
