# Native profile editor acceptance

## Scope and reference

Restore explicit editing of a standalone profile through the existing Rust
canonical transaction, not the legacy automatic edit/import queue. Python
remains the UI behavior reference and rollback implementation. This slice does
not complete R5/R6 and does not remove Python.

The fixed semantic bridge is `plugin profile-replace INSTANCE REVISION OP`.
Its private stdin is `ID\nNAME\nINPUT`, bounded to 33,126 bytes; only the first
ID line is split before reusing the canonical replacement parser. Existing
profile identity, managed-profile restrictions, duplicate validation, operation
replay and compensated active replacement remain canonical runtime policy.
There is no generic method/command passthrough or new privileged operation.

The frontend must capture instance and revision before reading the private
editor seed. Saving retains those exact fences, never silently adopts a new
revision. A lost reply retains the same operation and payload. A known rejected
save retains returned edited text separately from the pending request, with
explicit reopen/discard, rather than auto-reopening or auto-saving.

Desktop editing already uses private stdin, private scratch storage, bounded
output and unconditional cleanup. Returned text above the protocol's 32-KiB
limit must not be truncated. The desktop helper itself rejects invalid text or
output over 64 KiB before returning it; this UI cannot claim recovery of data
the helper did not return. Cancel and unchanged text perform no mutation.
Subscription-managed profiles are not editable through this path.

## Required gates

- Canonical framing, malformed/oversized private input, no argv/error leakage.
- Exact instance/revision admission, duplicate operation replay and collision.
- Inactive replacement preserves identity/favorite/count and desired state,
  with zero lifecycle effects.
- Existing canonical active replacement/compensation regression tests remain
  green; the frontend must not add disconnect/reconnect logic.
- Editor draft, cancel/unchanged, stale read/save, oversized returned text,
  acknowledged rejection, exact unknown retry and explicit discard tests.
- Exact candidate Try Omarchy editor smoke in EN/RU; synthetic error/cancel
  states and installed ordinary connection regression. Active replacement needs
  its own applicable host evidence before claiming that UI path accepted.

Implementation and acceptance are in progress. No merge readiness is claimed.

## Try Omarchy checkpoint

At `fa0527929a7fdcc2c55971d7ae47b588cf14b538`, the isolated synthetic-store
QML frontend opened the actual Zenity editor. The owner confirmed Cancel;
subsequent observation showed no editor process, no retained draft, no pending
operation, no unknown outcome, cleared seed and unchanged revision 0.
The installed private store and VPN were not involved.

This gate found a real QML integration defect: dynamically created Process
properties copy the context object, so JavaScript reference identity rejected
a valid read response. The fix compares a per-request token plus captured
instance/revision/profile identity and then resolves the original draft.
Thirteen frontend tests pass, including copied-context and stale-token cases.
The Rust checkpoint passed 750 tests with four existing ignored, fmt/clippy and
parity; Rust code did not change in this QML correction.

The first keyboard automation attempt lacked a verified target and could send
Escape to the agent terminal when the editor failed to open. That step was
removed; it is not acceptance evidence. Further keyboard automation must verify
the target window or use human confirmation. Save/rejection/recovery visual
gates and final installed integration remain pending; Cancel alone does not
establish complete editor acceptance.

The rejected-save integration test also exposed a local admission distinction:
invalid profile text can fail canonical CLI validation before any socket call.
Replacement-only exit 74 now marks this known not-submitted outcome with a
fixed public message. QML retains the draft for correction without claiming an
unknown mutation. Exit 73 remains reserved for transport failures after entering
the socket client. Deterministic tests assert that rejected input never connects;
other plugin command exit contracts are unchanged.
