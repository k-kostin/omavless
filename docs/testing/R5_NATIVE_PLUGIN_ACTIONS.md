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

Normal marketplace installs remain legacy until explicit activation. No private
fixture data or screenshot is committed.

## Installed acceptance — 2026-09-09

Try Omarchy ARM64 used combined source
`ffcf4d2654b74c0fb746664eb73545eb9028fbc6`, installed binary SHA256
`3ef3ac70b063c910b90fa032d52b4edbe253291e63eee1cfe94cf94abbc0a223` and exact
matching QML/launcher files. Explicit activation committed Rust ownership;
the Python service remained inactive. Full VPN with one existing private VLESS
fixture passed the TUN-bound HTTPS probe, owned core/TUN/Unix-controller checks
and disconnect cleanup. The owner confirmed successful connection and
disconnection through the installed native buttons. Disconnected mode changes,
restoration and daemon restart also passed. See
[the activation report](R5_DISCONNECTED_ACTIVATION.md) for full evidence and
the distinction between legitimate proxy/TUN forwarding sockets and a TCP
controller. Both units remain startup-disabled; login is not accepted here.

The final QML source from `f5cc9bbbeeae7af3b69856f4b282f88809ab3479` was
recaptured after the focus/coherence/color review fixes in an isolated synthetic
panel. English and Russian normal, unknown-outcome and restored-error states
were inspected, plus overlapping long-list captures through all 16 profiles and
the subscription row. No overlap or interpreted markup was observed. Actual
production focus/scroll functions brought the last profile control into view;
the poison Python sentinel remained unused. Synthetic failure displays do not
claim an actual failed runtime transition. Test windows/runtime were stopped;
the real user's locale and private data were untouched.

Rebased candidate `a361e86898e079988ca8c8dcf915ad1819ef6fc6` is based on
main `2d8cd107e94e8ed8747418d99a08edf0a0fb53dc`. Range-diff shows only help
insertion and roadmap context changes. The complete runtime, plugin, launcher,
template and packaging trees match the installed combined source byte-for-byte;
no runtime semantics were changed by this rebase. Combined local validation:
744 Rust PASS / 4 existing ignored; 326 Python PASS / 1 root-only skip;
QML/i18n, formatting, Clippy and parity PASS, plus two installed-Mihomo tests.
This is qualified combined-source evidence, not a claim that an independently
rebuilt binary was installed for every documentation commit. Full frontend
operation parity, lifecycle/login and R6 Python absence remain open.
