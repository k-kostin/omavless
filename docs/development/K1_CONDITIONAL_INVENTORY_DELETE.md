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
to [Draft PR #547](https://github.com/k-kostin/omavless/pull/547).

On the Omarchy x86_64 development VM the six-scenario gate passed, followed by
ten consecutive complete repetitions (60 scenario executions). The exact
tested implementation is `803982824637d506f5937fb58aab3fe9cd5f51c9` and test
binary SHA-256 is
`b0b516abbdc5a4fcbc8ddf9132a51d08b6b937a245470f8f8a40741c1d786d00`.
The ordinary netguard suite passed 191 tests, with 27 opt-in tests ignored;
strict fmt/clippy passed. These are disposable namespace mechanism checks,
not a normal installed disarm, namespace-identity authority or physical-PC
firewall acceptance. No host/guest parent firewall or VPN was changed.

Remaining K1 gates include adopted reviewed namespace APIs, canonical launch,
creator/durable ownership, generation-wrap/namespace continuity, replacement
and recovery transactions, installed package/service integration and physical
host acceptance. A matching untrusted shape or local owner port is still not
production deletion authority. This slice must not be used to advertise an
enabled or complete kill switch.
