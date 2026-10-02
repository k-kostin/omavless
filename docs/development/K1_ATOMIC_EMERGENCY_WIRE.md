# K1 fixed atomic Emergency wire candidate

This inactive Rust prerequisite follows the
[namespace API/launch boundary](K1_NAMESPACE_API_PREREQUISITE.md) and the
[live creator experiment](K1_LIVE_EMERGENCY_OWNER.md). It introduces no socket,
executor, authority token, production caller, root service or activation. K1
remains unavailable. All kernel effects described below belong only to the
explicit isolated-VM developer fixture.

## Fixed pure encoding

`emergency_wire::encode(generation, first_sequence)` accepts two nonzero u32
values and refuses sequence overflow. It emits one complete nf_tables batch:

1. generation-fenced batch begin;
2. exclusive inet `omavless_netguard` table with `owner,persist`;
3. exclusive `output_guard` base chain, output hook, priority 300, policy drop;
4. loopback interface-name match and accept;
5. unconditional drop;
6. batch end.

All four operations request acknowledgements. A separate fixed GETGEN request
provides an ordered response barrier, not effect success. The encoder accepts
no paths, table names, arbitrary operations or policy expressions. It allocates
only fixed bounded wire material, uses native netlink headers and big-endian
nf_tables numeric attributes, and contains no unsafe Rust or I/O. Returning
bytes establishes neither fresh generation nor namespace/table authority.
The future session owns sequence non-reuse; this stateless function does not.

The fixed ABI basis is Linux v6.18's
[nf_tables UAPI](https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/netfilter/nf_tables.h)
and [nfnetlink UAPI](https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/netfilter/nfnetlink.h).
Unsupported kernel flags/expressions must refuse; no compatible-looking
fallback or partial policy is permitted.

## Independent and isolated validation

Pure tests inspect message order, flags, byte order, padding, sequence bounds
and generation zero refusal. A separate test-only Python codec independently
builds the complete expected message bytes at both fence extremes. The
Rust-rendered expected policy still has its independent golden tests.
Synthetic reply-result tests require all operation ACKs plus the valid GETGEN
reply/ACK, refuse missing/extra acknowledgements and propagate errors. These
are collector-result tests, **not real transport ACK-loss injection**; no
production reply decoder or executor is implemented here. `checked_reply` is
test-only and must never be treated as production ACK authority.

The VM fixture calls the exact current Rust test executable's pure encoder
child, then submits those exact bytes through its retained socket in a newly
isolated loopback-only namespace. Only a fixture fault case inserts an extra
colliding sentinel create after all four policy operations, before batch end.
The kernel's EEXIST must roll back the entire staged policy: target absent,
sentinel unchanged and generation unchanged. A stale generation must similarly
return ERESTART without a target or sentinel/generation change.

Successful creation must pass full ordered rule JSON readback plus exact raw
metadata/generation checks. The retained-socket fixture then exercises the
same cases as the prior live experiment: identical foreign collision, late
same-handle extra-rule drift, foreign-socket deletion refusal, closed-socket
proof refusal and persistent exact-policy orphan refusal. The orphan is left
for namespace teardown, never adopted or deleted by a new creator.

The harness retains the prior pinned-parent namespace, child loopback-only
inventory, cookie/namespace-ID checks, 15-second outer bound and 32 KiB output
limit. Pure encoder subprocesses additionally have a two-second bound. Python
remains test-only; it is not a dependency or shortcut for production Rust.

```sh
OMAVLESS_K1_ATOMIC_VM=1 cargo test --locked -p omavless-netguard \
  --test nft_namespace atomic_emergency::atomic_emergency_in_disposable_vm \
  -- --ignored --exact --nocapture
```

## Remaining boundaries and crash windows

The one-batch mechanism removes the older fixture's committed empty-table
interval: no complete batch commit means no acknowledged policy, and successful
commit includes the complete Emergency rules. It does not make kernel effects
and filesystem receipts atomic. A crash after kernel commit but before receipt
publication leaves uncertainty; socket loss leaves a persist-only orphan, not
fresh ownership. Lost/incomplete ACKs or readback cannot authorize retry,
adoption, deletion or a success response. Pending/invalid durable state keeps
the existing explicit recovery requirement. This slice does not implement or
accept a manual recovery procedure.

Safe typed namespace APIs, trusted canonical host launch, a non-revivable Rust
live-creator authenticator, bounded production reply validation, exact full
readback including owner metadata, conditional replace/delete and receipt
composition remain required before any executor. Full VPN atomic encoding,
real process-kill/ACK-loss/durable-publication gates, subsystem reset behavior,
root boot integration and physical NIC/suspend/boot acceptance remain open.
This mechanism gate is not packet enforcement, host protection or bare-metal
acceptance. Exact tested head and environment belong in the Draft PR evidence.
