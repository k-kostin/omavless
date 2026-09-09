# Native profile controls

This bounded successor to PR #206 restores profile rename, pin/unpin and
confirmed deletion in the native QML surface. It reuses the existing rename
window and delete confirmation. Managed subscription profiles may be pinned,
but rename/delete stay unavailable until subscription management is restored.

Before this change, the committed Rust owner already implements canonical
profile mutations, but the installed native frontend refuses them. After this
change, fixed `plugin.action` mappings reuse that same owner, private atomic
store writer, lifecycle compensation, revision and replay namespace. There is
no second state machine, Python fallback or new privileged path.

The three fixed CLI actions take instance/revision/operation metadata in argv;
record ID, display name and pin state travel only through bounded stdin.
QML retains the exact private input while an unknown outcome can be retried,
then drops it on a conclusive reply or explicit state acknowledgement. It
does not optimistically rename/delete/pin the displayed profile. Only a fresh
snapshot updates displayed state. Public errors discard raw backend messages.

## Acceptance

- Reference: existing Python rename/favorite/delete UI, and accepted native
  canonical profile mutation/differential corpus from PRs #120/#121/#129.
- Fixed parser, CLI and private socket tests must cover metadata fencing,
  bounds, malformed input, replay and private error handling.
- Executed QML JavaScript tests cover private stdin, exact retries, stale
  admission, managed-profile restrictions and no optimistic state change.
- Launcher tests cover fixed dispatch, no shell interpretation, unchanged
  legacy ownership behavior and no native-to-Python fallback.
- Exact-head installed package/daemon and English/Russian UI smoke: passed below.
  Exercise pin/unpin, standalone rename/cancel/duplicate rejection,
  delete cancel/confirm on a disposable profile, Tab/Shift+Tab, narrow layout,
  panel reopen, failed/unknown result presentation and connection regression.

Do not delete a real profile to obtain test evidence. Use a private disposable
fixture or an isolated synthetic store. Screenshots are private unless their
pixels have been reviewed for fixture identity. Python remains oracle/rollback;
import, editor, subscriptions, routing tools and startup are not completed by
this slice. R5/R6 remain open.

## Try Omarchy ARM64 checkpoint — 2026-09-09

Installed source: `4f22099bc46707117b8e90e8f0a19996d89293ae`, directly on
main `8ec90a73b422dac530bdb2175bb6224fca9fe01e` after #206 merged.
The rebase preserves both feature/test patches (`=`/`=`) and the complete
tree is identical to tested `ee05b2a26902b2a9969b57aa6a0de31175905596`.

- Rust: 748 passed, four existing ignored; strict clippy, format and parity pass.
- Python: 337 run, 336 passed, one root-only skip with installed Mihomo enabled.
- QML actions: 15 executed JavaScript tests; snapshot: six; launcher: 15.
  QML, localization, shell syntax and diff checks pass.
- Exact-head CI run `34374013077` passes.
- Local package `omavless 0.0.0.r363.g4f22099bc467-1` installed normally.
  `/usr/bin/omavless` and the running daemon executable both have SHA256
  `cc7bd98525464d2c58c0b40cae7bf68864a86d31f26cba14483ea32b6bedb711`.
- Exact QML/launcher installed; native metadata/fresh-fact reads pass. Native
  service active, legacy inactive, both startup units disabled, plugin enabled,
  disconnected with zero visible Mihomo/TUN.

The same executable and QML were also exercised against a separate synthetic
store/socket, never the user's real profile records. Actual QML rename
confirmation updated the snapshot, pin/unpin completed, and English/Russian
rename/delete-cancel surfaces rendered without overlap. Markup-like synthetic
names remained plain text. The native Tab-target cycle reached 25 controls;
long-list bottom rows remained scrollable. Private screenshots were inspected
locally, not committed or published. One closed-panel capture was discarded
and repeated with the dialog actually visible.

The owner confirmed installed pin/unpin, standalone rename followed by Escape,
and connect/disconnect on this candidate. Post-check native facts were current,
disconnected, with no pending/unknown action and zero visible Mihomo/TUN.
The former #206 connection evidence is not relabeled as a new instrumented
network probe: this candidate's connection regression is human-confirmed.

Isolated QML delete confirmation reduced the synthetic profile count from 16
to 15; refresh finished without pending/unknown state. No real profile was
deleted. Duplicate rename was rejected with confirmation disabled in both
English and Russian. Repeated captures showed the actual duplicate hint and
unknown-outcome recovery controls without overlap. Unknown-outcome screenshots
use injected test-only presentation state, not a claimed live transport fault;
lost-reply/replay semantics are covered by deterministic tests. Captures taken
after a dismissed dialog were rejected and repeated in one uninterrupted run.
These checks complete this bounded profile-controls acceptance, not full UI
parity or R5/R6.
