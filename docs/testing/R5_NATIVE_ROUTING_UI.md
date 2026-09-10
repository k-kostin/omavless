# Native routing-tools frontend

Scope: reuse the existing `RoutingToolsPrompt` and `RoutingPresetPrompt` through
the native owner. The legacy Python frontend remains the migration reference;
this does not remove Python, finish R5/R6, activate login autoconnect or change
marketplace 0.7.0.

## Boundary

- Settings exposes preset selection and routing tools using existing localized
  controls. Native preset selection preserves the current connection mode.
- Custom-rule list/add/delete and route checking use fixed Rust CLI mappings.
  Destinations travel on bounded stdin, never argv. Rule lists are private UI
  data, not shareable diagnostics.
- Mutations reuse `plugin.action` instance/revision/replay admission and the
  existing compensated routing transactions. No new lifecycle owner, generic
  method dispatcher or privileged command is introduced.
- Read replies are bounded and validated before entering the existing dialog.
  Closing the dialog and stale identity invalidate private samples.
- Provider refresh remains unavailable here until its long-operation progress
  and cancellation client is integrated. Startup remains a separate runtime gate.

## Acceptance

Deterministic coverage includes canonical routing parser reuse, exact CLI stdin,
private socket mutation/replay/stale admission, frontend parsing/privacy and
reused-dialog navigation. Run `tests/run.sh` and the runtime Rust suite.

Required installed evidence: exact package + plugin identities, native custom
rule add/list/check/delete with restoration, preset confirmation, English/Russian
dialog layout and close/reopen, healthy normal connection/disconnection and no
duplicate core/TUN or TCP controller. Record only classifications/counts and
booleans; no private destinations, profiles or subscription metadata.

When installing this branch's UI stack, preserve the independent #218 owned
controller socket permission fix in the package integration candidate. Do not
replace that accepted local fix with an older binary merely to update QML.
