# K1 bounded FullVpn reply transcript

Status: inactive pure Rust candidate, child of the raw FullVpn packet fixture.
It opens no socket, changes no firewall and supplies no ownership or canonical
namespace authority. Emergency code and tests are unchanged; deliberate small
decoder duplication avoids widening this slice into a shared-parser refactor.

## Fixed contract

`full_vpn_wire::reply::FullVpnTranscript::new(generation, first_sequence,
local_port)` only accepts the fixed FullVpn encoder: nonzero generation,
sequence and port, with space for sequence +14. It derives the exact request
headers from that encoder, not caller-supplied operation descriptions.

The complete transcript requires twelve distinct zero-error NLMSG_ERROR ACKs
for table, chain and ten rules (+1 through +12), the GETGEN ACK (+14) and exactly
one NEWGEN reply (+14). Batch begin/end ACKs (+0/+13), negative errors, duplicate
or foreign replies, DONE and multipart traffic refuse. ACKs accept only flags
zero or CAPPED, exactly twenty body bytes, and an exact sixteen-byte echoed
request header. Any reply order or coalescing is supported.

Every datagram must declare kernel sender port/group zero, zero receive flags,
and the expected local port in every message header. A maximum of sixteen
nonempty datagrams and 32 KiB total is accepted. Message/attribute lengths,
alignment and zero padding are checked. NEWGEN requires family/version zero,
nonzero network-order generation attribute 1 and matching low sixteen-bit
res_id. Optional attributes 2 (four bytes) and 3 (single NUL-terminated name,
at most sixteen bytes) are structural only; duplicates, unknown attributes and
attribute flags refuse. No task name or PID is authenticated.

Any malformed input permanently poisons the collector. `finish` requires every
reply, closes the collector and returns only `CompleteUntrustedTranscript` with
the observed generation. A missing reply cannot later repair a failed finish.
Caller-supplied metadata is not authenticated here: forged inputs can satisfy
this structural parser. No successful return is an effect, ownership, stable
generation/readback or durable receipt.

## Gates and limits

Normal tests cover all fourteen required replies, missing/duplicate/foreign
replies, all request-header echo byte mutations, every truncated message,
malformed generation attributes, bounds and poison semantics. They open no
socket. The existing opt-in `atomic_full::atomic_full_in_disposable_vm` captures
actual recvmsg bytes and metadata for its successful exact Rust batch, passes a
bounded binary frame to a pure child decoder, and requires exactly one parser
PASS. The frame is not stored and carries no private profile data. Failed and
fault-injected batches remain refusal/rollback checks, not success transcripts.

Run that ignored test only in the dedicated disposable VM with
`OMAVLESS_K1_FULL_VM=1`; it retains the existing loopback-only guard, retained
creator, exact readback, rollback and orphan-refusal gates. The raw 53-vector
packet fixture is unchanged semantically and does not opt into this bridge.
VM results must identify the exact tested commit/artifact; no kernel acceptance
is inferred merely from synthetic parser tests.

K1 remains unavailable. Safe namespace API, trusted launch, retained transport
authentication, production effect authorization, installed acceptance and owner
recovery are separate prerequisites. No dependency, installed unit/package,
IPC operation, unsafe binding or product entry point is added.
