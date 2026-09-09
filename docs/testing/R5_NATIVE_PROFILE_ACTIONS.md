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
- Exact-head installed package/daemon and English/Russian UI smoke: pending.
  Exercise pin/unpin, standalone rename/cancel/duplicate rejection,
  delete cancel/confirm on a disposable profile, Tab/Shift+Tab, narrow layout,
  panel reopen, failed/unknown result presentation and connection regression.

Do not delete a real profile to obtain test evidence. Use a private disposable
fixture or an isolated synthetic store. Screenshots are private unless their
pixels have been reviewed for fixture identity. Python remains oracle/rollback;
import, editor, subscriptions, routing tools and startup are not completed by
this slice. R5/R6 remain open.
