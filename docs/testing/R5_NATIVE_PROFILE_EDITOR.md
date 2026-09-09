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

At implementation head `b1d0cfb07204688b8e0ed49821bc3f37d22229c5`, isolated
QML plus the real native CLI/runtime passed rejected input (EN/RU), successful
inactive save, unchanged save, and explicit recovery of an acknowledged
rejected draft. Revision stayed 0 on rejection, advanced to 1 on Save, stayed 1
on unchanged Save, and advanced to 2 after explicit corrected-draft recovery.
Terminal outcomes cleared processes/seed/pending/unknown state; rejection alone
retained the draft. These save cases used a synthetic editor-output wrapper,
not human edits in Zenity, and do not establish active-profile host acceptance.
The earlier actual Zenity open/Cancel gate is separate. No private store was
changed and no test tunnel was started.

## Combined installed checkpoint

The temporary integration candidate `fe8c57f5d0f2cf5a7a2425d622b50a05e216842d`
combines #212, #213 and #214 without making the PRs a permanent stack.
Mechanical conflicts were resolved preserving all three boundaries. Local
gates: 755 Rust passed/four existing ignored, fmt/clippy/parity passed; Python
342 run/341 passed/one root-only skip; 51 native JS cases, 20 launcher tests,
QML contracts and two installed-Mihomo opt-ins passed.

Try Omarchy ARM64 installed package `0.0.0.r379.gfe8c57f5d0f2-1` and restarted
the native service/frontend. Installed binary matches the built candidate:
SHA-256 `0342a03165147f213fc0a2341696370f326fd7c127b52f44acb753a9fe2ac4cd`.
Service/Panel/NativeSnapshot files compare byte-identically. Plugin enabled;
desired and actual disconnected, Full VPN mode preserved, visible Mihomo/TUN
0/0. Package creation initially exhausted the separate /tmp tmpfs; moving only
build artifacts to the main disk resolved it without changing system policy.

Combined human import/QR/editor and connect/disconnect acceptance is pending.
This installed checkpoint is not a claim that the three independent PR heads
have completed all host gates, nor that active-profile editing has passed.
