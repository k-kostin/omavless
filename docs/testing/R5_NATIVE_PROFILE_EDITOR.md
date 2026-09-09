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
