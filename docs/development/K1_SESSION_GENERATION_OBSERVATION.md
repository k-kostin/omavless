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

## Exact-source evidence — 2026-10-03

Tested code is `8777f7eab3d955063d29cafd234ee7ead237c0f5`. Seven focused
wire-prefix tests, three source-routing guards and strict all-target netguard
clippy passed. The full source suite passed 509 tests with two existing skips,
93-link documentation navigation, native frontend and QML checks. It ran in an
exact detached clone with private temporary fixtures outside Git ancestry;
initial archive/cwd and default-temp setup refusals were retained, not hidden
by weakening the package-identity or private-file guards.

The full Rust script passed: 2,021 successful test invocations and 71 ignored
entries across its repeated suites (not unique-test counts), 12 terminal tests,
strict workspace/feature clippy and checks, and two-case parity. Frozen binaries
copied outside Cargo before VM execution, mode `0500` and xattr-empty:

- library test ELF SHA-256
  `60681e14c2dd55133260a2f18c790ac39891f4fd2d8a65a3c6a940494047cd18`;
  ordinary CPU suite 166 passed / 12 ignored;
- integration test ELF SHA-256
  `bd681ae6967d3e030bdd520ff414d092450a4af48eddd8aa5d706754d96fe8ad`;
  ordinary CPU suite 18 passed / 21 ignored.

At report preparation, both code-head package CI jobs passed and the CI test
job was still pending. This report does not label pending CI green; final
remote status belongs to [Draft #584](https://github.com/k-kostin/omavless/pull/584).

An explicitly leased x86_64 Omarchy development VM passed two independently
staged invocations of these unchanged binaries. Each invocation ran 13 creator
lifecycle, six conditional-delete, six complete-inventory, one chain-inventory,
three END-observer-loss, six BEGIN/operation-observer-loss and three actual
recvmsg-truncation scenarios: **38 per invocation, 76 total**. The separate
integration ELF supplies the chain reader; a chain dump inside the composite
inventory was not miscounted as execution of `inspect_chains`. These tests cover
all five real reader families with stable/increasing observations. Lower/wrapped
values remain the pure wire tests above, not an actual kernel-reset claim.

The fixed two-ELF/seven-selector external supervisor has SHA-256
`e58eb604963b267dea9fb1e6e92b10b84fc51c7384339290dff5b49641395c1a`;
the strict preservation guard has SHA-256
`875d3d7af92740fe6f78710558ac368eb3a2c74a03790855c75384a97c0f9b41`.
Twelve local supervisor tests and three receipt/guard test families passed.
The supervisor pins executable descriptors, bounds output and deadlines,
retains a WNOWAIT process-group anchor until EOF and descendant disappearance,
and quarantines further invocations on uncertain cleanup/anchor loss. No sudo,
credential setter, generic caller command, root helper or canonical activation
was used. Failure would stop later cases; neither invocation required a retry.

Both runs passed all seven exact Rust-result/owned-group-quiescence receipt
checks and preserved all eight canonical categories: private files, user service
and PID, executable, namespace, core inventory, TUN inventory, resolver and
resolv.conf. Raw address and all IPv4/IPv6 route/rule snapshots matched except
verified nonincreasing numeric address valid/preferred lifetime counters. Stage
scratch was empty, no owned group survived, and the separate `/tmp` nft-helper
scratch inventory was preserved. Canonical service/package/network state and
the primary PC were not changed.

The private evidence archive has SHA-256
`36d9dd3b79ec9de11aa1989c38818a5bf33b6aae765a9b351f5f04c4be7e7675`.
Guest and independent local copies are `0600` under `0700` parents. Local
readback and fsync verified 64 safe members, both exact artifact sets, 14 result/
quiescence receipt pairs, two PASS stamps and raw countdown-only network
equality. The VM lease was returned; stages remain retained pending independent
review and separate cleanup authorization. Raw private snapshots stay outside
Git. Documentation-only report commits do not change the tested code identity.

Installed launch/ownership, nft continuity, recovery, package/runtime exchange
and physical-host gates remain open under [KILL_SWITCH](../roadmap/KILL_SWITCH.md).
No main/RC merge, release or product closure is implied.
