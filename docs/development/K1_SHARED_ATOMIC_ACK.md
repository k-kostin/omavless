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

CPU gates and a frozen executable identity will be recorded with the exact
source head. No new VM execution or installed/package acceptance is claimed.
Any kernel rerun requires review and the exclusive development-VM lease.

[KILL_SWITCH](../roadmap/KILL_SWITCH.md) still owns the open production
namespace/launch trust, nft continuity, durable orphan adjudication, package/
service/runtime integration and host-acceptance gates. This extraction grants
none of them and does not authorize main/RC merge, activation or release.
