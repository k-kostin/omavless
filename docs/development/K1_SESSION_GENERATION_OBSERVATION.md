# K1 retained-session generation observation fence

This bounded continuation of #582 changes normal-compiled, private read-only
code. It does not activate K1, introduce an executor or authenticate a namespace,
socket, table owner, package or system-manager launch.

`LocalReadSession` retains one optional last-observed generation. Every completed
GETGEN exchange passes through `finish_read_exchange`, using the existing exact
wire decoder, before any of the five table/chain/policy-shape/raw-rule/complete-
inventory readers receives it. Equal observations, gaps and increases are
permitted. A lower observation, invalid/incomplete response or exchange error
permanently poisons the same retained instance. Its existing pre-send check then
refuses subsequent exchanges. There is no reset, retry, reacquisition or public
generation token. Existing zero-generation rejection is unchanged. Each reader
still requires equal generations at both ends of its own bounded inventory;
allowing an increase across completed inventories does not relax that check.

The test-only `FixtureCreator` no longer owns a separate generation tracker.
Its original error handling still clears its created handle. Its observations,
conditional-delete preparation and post-delete absent readback now all use the
same normal session history. All effects remain test-only; public untrusted
reader result types, fixed requests and ACK contracts are unchanged.

## Concrete prior-path counterexamples

At base #582 `8b8ca76061717192e9113db3f37cf1121bc5cabe`, the creator tracked
generation only in its own `inspect`. `delete_owned` instead called
`prepare_inventory_delete` and `inspect_policy_inventory_once` directly. Thus a
creator observation at 100 followed by an otherwise valid inventory bracket
99/99 satisfied the reader's equality predicate without consulting the creator's
100. Likewise, creator observation 10, delete preparation 20, absent readback 21,
then creator observation 15 passed the creator's stale `>=10` comparison.

Immutable counterexample checkpoint
`51bd4e354d4e9d3a48c75dce7685e0ddd3fb5f94` extracted the old exchange completion
as an unchanged pass-through and added two regressions. Both failed at the
expected lower-generation acceptance. Their history comes from complete ACK and
GETGEN byte frames parsed by the existing `Exchange`, never preassigned history
or poison. The corrected shared finalizer must reject those same prefixes and
remain refused after later equal/higher replies. This is executable wire/parser
counterexample evidence, **not an observed kernel reset, wrap or old delete
effect**. No socket or namespace is opened by these regressions.

Additional pure cases cover both ACK/body orders, stable observations, gaps,
increases, MAX-to-low/zero refusal, incomplete exchanges, malformed/duplicate/
wrong-peer/wrong-sequence/truncated replies, transport errors, non-GETGEN
noninterference, irreversible refusal and the separate bracket-equality check.
Source guards pin central routing in all five reader families and both delete
readbacks, and prevent return of the creator-only tracker.

## Limits and gates

This is only a local observation fence. Monotonic observations cannot detect
reinitialization at the same/larger value, unseen wrap/ABA or namespace
switch-and-return. Generation values and local socket ports are not canonical
namespace identity or exclusive-create history. No safe namespace API dependency
is adopted. Older #324's symbolic durable Closed(N)/Arm replay fence is a
different protocol invariant and is not replaced here.

Focused Rust, strict clippy, full Rust/source gates and exact frozen-artifact
identity will be recorded after execution. No new VM gate has run for this
change; #582's isolated evidence remains attached only to its tested code.
Installed launch/ownership, nft continuity, recovery, package/runtime exchange
and physical-host gates remain open under [KILL_SWITCH](../roadmap/KILL_SWITCH.md).
No main/RC merge, release or product closure is implied.
