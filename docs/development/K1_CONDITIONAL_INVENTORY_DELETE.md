# K1 complete-inventory conditional delete mechanism

This is a **test-only** continuation of #538, not an installed helper, disarm
capability or authority to mutate the host firewall. Production builds contain
no deletion executor. No `EffectPort`, receipt, root unit, normal runtime or
workspace dependency is changed.

The retained complete inventory now internally keeps its matched generation
and table metadata alongside the existing untrusted classification. Its public
read-only classification remains unchanged. A test-only, non-cloneable witness
borrows that same mutable socket/session and captures the table handle from that
same inventory. It cannot be reconstructed from a serialized generation or
caller-selected table. Cancel has no effect; the witness expires within the
inventory's original one-second budget.

The sole fixed effect is an `inet` table-handle deletion inside a batch with a
nonzero `NFNL_BATCH_GENID`. No table name supplied by the caller, UNSPEC family,
flush-all or fresh-generation retry is encoded. The kernel serializes the
generation comparison with commit processing; a changed generation rejects
the batch before its operations. See the pinned Linux v6.18
[batch handler](https://github.com/torvalds/linux/blob/v6.18/net/netfilter/nfnetlink.c)
and [generation/table-handle processing](https://github.com/torvalds/linux/blob/v6.18/net/netfilter/nf_tables_api.c).
This source review is not installed-kernel acceptance.

Only an exact kernel-peer, sequence, original-request and `ERESTART` receipt for
batch begin counts as GenerationChanged. Success needs exact begin/delete/end
ACKs, followed by a fresh complete absent-table readback/barrier. End's success
is essential: begin/operation success may already be queued when commit fails.
Unknown,
lost, malformed, duplicate, truncated or other error replies poison the
session. Once the send syscall is attempted there is no resend, reacquisition
of a newer generation or compensating mutation. Later absence alone is not a
successful receipt.

The opt-in `conditional_delete_in_disposable_vm` gate uses six fresh user/network
namespaces. It pins the unchanged parent namespace descriptor and verifies
loopback-only isolation before socket creation. Full VPN and Emergency positive
cases delete only the selected table and preserve an independent foreign table.
Foreign changes after inventory must produce strict generation refusal and
leave both table byte projections unchanged. Cancellation and expiry preserve
the selected table. Exact VM source/binary identity and repeat results belong
to the owning PR; until executed, this gate is pending.

Remaining K1 gates include adopted reviewed namespace APIs, canonical launch,
creator/durable ownership, generation-wrap/namespace continuity, replacement
and recovery transactions, installed package/service integration and physical
host acceptance. A matching untrusted shape or local owner port is still not
production deletion authority. This slice must not be used to advertise an
enabled or complete kill switch.
