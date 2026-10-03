# K1 shared atomic ACK contract

This bounded source integration starts at the sealed
[#573 prefix-observation-loss report](K1_PREFIX_ACK_OBSERVER_LOSS.md),
`c4d4f61f6ce813ee3eea54831b0e939ba5c565df`. It does not inherit that
report's kernel execution as evidence for a new executable.

## Shared mechanism, not authority

`kernel_atomic_batch.rs` is normal-compiled, crate-private and inactive. Its
private `AtomicBatch` representation admits only fixed generation-fenced
FullVpn create, atomic delete-and-exclusive-create replacement, and inet
single-handle deletion. The existing negative drift fixture has a separate
fixed `cfg(test)` builder; arbitrary request vectors are not admitted.

The creator and inventory-delete kernel fixtures use the same `AtomicReplies`
implementation. Every BEGIN, operation and commit-END ACK must be present.
Results are explicitly `UntrustedStatus`: incomplete, all acknowledged,
generation refused, or uncertain. Caller-supplied sender/flags/port are checked
structurally, not authenticated by this pure module. It has no socket, send,
retry, namespace switch, ownership promotion or production EffectPort.

Only exact ERESTART (-85) for BEGIN, before any successful ACK and as the final
frame in that datagram, is structural generation refusal. Capped errors echo
the exact request header; uncapped errors echo the entire exact request.
Success echoes the header. Unknown flags, senders, ports, sequences, errors,
duplicates, lengths and padding poison permanently, including receive after
completion. Total bytes are bounded at 32 KiB; delete has eight datagrams and
create/replacement sixteen. Generation, handle and initial sequence cannot
be zero; delete reserves first+3, create/replacement conservatively first+15
without overflow, preserving the predecessor's sequence limits.

The public legacy `FullVpnTranscript` API remains unchanged. It checks
operation ACKs and a separate GETGEN barrier, not this atomic commit contract.
Its apparent structural completion must not be substituted for the shared
BEGIN/every-operation/END result.

## Coverage and evidence boundary

Pure tests visit every ACK subset and every possible next ACK for all three
fixed batches. This is inductive state-space coverage of all orderings, not
literal enumeration of 15! transcripts. They also cover missing every position,
coalesced delivery, exact error echoes, malformed/truncated/padded frames,
metadata, limits, sequence exhaustion and permanent poison without late repair.
The existing raw BEGIN/operation/END observation-loss and receive-truncation
fixtures retain their exact scenario prefixes and single-send/no-retry paths.
Source guards check shared integration and keep mutation adapters test-only.

## Exact-source gates and measured kernel continuation

Tested code, 2026-10-03:
`d7f6fa3a8abd72ed1f5b5c89f8ed4802cc22b404`. Full source checks passed
506 tests with two existing skips, 93-link documentation navigation and
native/QML checks. The complete Rust script passed 2,014 successful test
invocations with 71 ignored invocations (not unique-test counts), all twelve
terminal checks, strict workspace/feature clippy, feature checks and two-case
parity. Both package architectures and source CI passed on that exact code.
Subsequent documentation-head CI is separate from executable evidence.

The frozen x86_64 test ELF has SHA-256
`a6e8b21b8219448824b8de5582b2ad95007eabd5f335ab4f96c3211724dd9a92`.
It was copied outside Cargo with mode 0500 and no file capabilities. Its
ordinary CPU rerun passed 159 tests with twelve opt-in tests ignored.
No running self-reexecution executable was rebuilt.

Under an exclusive development-VM lease, two independently staged invocations
each passed thirteen existing creator-lifecycle cases, three END observation-loss
cases, six BEGIN/operation observation-loss cases and three actual receive-
truncation cases: **50 actual cells total**. Each invocation used the same frozen
ELF and required four exact aggregate receipts plus four exact successful
ignored-test summaries. These exercised the newly shared collector through
the actual cfg(test) creator and inventory-delete adapters. They are not
synthetic-frame-only results, kernel ACK dropping, installed helper execution,
packet-policy acceptance or a production ownership proof.

The reviewed fixed-selector supervisor has SHA-256
`e1985158299f9bb7071d473480e6545a5a508cbdaada18a0bf75ca98a6910514`;
the preservation guard has SHA-256
`f9b4959e947224508167ba58648d7b8c9e942cd25dbd4ca38f6d8074700b1e9d`.
The supervisor ran as the ordinary VM user without credential setters. It
retained each owned process-group leader with WNOWAIT until nonleaders had
disappeared, bounded live output and runtime, and cancelled only that retained
group on failure. Seven local synthetic-process tests covered orphaned children,
inherited stdout, output bounds, timeout, refusal, success and selector rejection.
Ten pure guard checks covered exact receipt, selector and quiescence validation.
The second VM invocation was admitted only after every first-invocation guard
passed. Neither invocation required a retry.

Both invocations preserved all four canonical private file hashes, service
state/PID, executable, outer network namespace, core/TUN inventories, resolver
state and resolv.conf. All address and IPv4/IPv6 route/rule fields matched,
except explicitly validated nonincreasing preferred/valid address lifetimes.
Owned process-group and scratch-directory survivor checks passed.

The private archive has SHA-256
`e0572e86a9bbd28a19a6265d9cb02840db99b3911f51bbdd860600777fa56fdf`.
Guest and independent local copies were synced and checked as mode 0600 under
0700 parents. Independent archive readback verified both artifact sets, eight
exact test/quiescence receipts, both full-guard PASS stamps and all retained
before/after network fields. Raw private evidence is not committed. The VM lease
was returned; no canonical package, service or network mutation was performed.

[KILL_SWITCH](../roadmap/KILL_SWITCH.md) still owns the open production
namespace/launch trust, nft continuity, durable orphan adjudication, package/
service/runtime integration and host-acceptance gates. This extraction grants
none of them and does not authorize main/RC merge, activation or release.
