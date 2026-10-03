# K1 retained-socket complete table inventory

`LocalReadSession::inspect_policy_inventory` composes fixed GETTABLE, GETCHAIN,
GETRULE, GETSET, GETOBJ and GETFLOWTABLE queries inside one GETGEN-bracketed,
one-second window on the retained NETLINK_NETFILTER socket. Table identity and
metadata are reread after all dumps. There is no caller-supplied name, command,
namespace or expression and no production write operation.

An exact **untrusted** result requires owner,persist flags, this socket's owner
port, one table use, no table userdata, the exact output base chain with no
extra chain counters/ID/userdata, ordered FullVpn or Emergency rules, and
complete empty set/map, stateful-object and flowtable dumps. Other namespace
tables are not claimed or modified. GETSET/GETOBJ/GETFLOWTABLE requests are
fixed-table filtered; a foreign-table response refuses instead of being skipped.
GETCHAIN's namespace-wide response retains its existing strict foreign-record
handling. An extra empty chain is not hidden by an otherwise exact rule dump.

Every dump checks sender, sequence, recipient port, generation, message flags,
alignment, uniqueness, bounded framing and successful multipart completion.
Object dumps are limited to 64 KiB, 32 datagrams and 64 messages each. Any
set/object/flowtable prevents Exact; their nested payloads cannot authorize
anything and are not interpreted as policy. Interrupted/partial/unsupported
responses or table/generation drift poison the session. Every kind is drained
even after another kind establishes nonempty inventory.

This closes the missing object-kind inventory prerequisite, **not ownership**.
The later [retained-session observation fence](K1_SESSION_GENERATION_OBSERVATION.md)
also rejects lower GETGEN observations across reader and conditional-delete
paths. It preserves this reader's equal-generation bracket and untrusted result;
monotonic observations do not establish nft-subsystem continuity.
Socket-port agreement alone is not exclusive-create history; generation and
handle checks are not canonical namespace identity. The result cannot construct
an effect identity, verified-owned table or executor receipt. Existing JSON and
rule-only readers remain available with their original untrusted semantics.

## Exact evidence

Code `ef020227c338448598c2697f389309e57158f9c3` passed the isolated x86_64
Omarchy Dev KVM gate on kernel 7.2.5-3-omarchy. Test-binary SHA256:
`88f5daacb8f2d78d3fef8372bc5cb4d780573d844db6afcb8a970f5304dc0aa5`.
The existing opt-in `raw_rules_in_disposable_vm` fixture now covers six fresh
user/network namespaces: FullVpn and Emergency with appended accept rules,
and FullVpn with an added set, counter object, flowtable or empty chain.
All cases first verify absent table, repeated exact composite readback and
second-socket owner mismatch; all then reject the injected addition. The last
four retain exact rules, proving that rule-only matching is insufficient.
Thread namespace change permanently refuses further reads.

Run only in an explicitly delegated disposable VM:

```sh
OMAVLESS_K1_RAW_RULE_VM=1 cargo test --locked -p omavless-netguard --lib \
  kernel_observer::rule_wire::tests::raw_rules_in_disposable_vm -- --ignored --exact --nocapture
```

The successful completion marker is `K1_COMPLETE_INVENTORY_VM_PASS`. All writes
are fixed test-only bytes through the retained creator socket, inside atomic
batches in loopback-only child namespaces. Parent namespace descriptors stay
pinned and unchanged. No sudo, normal guest service/network action or host
firewall change occurred. Child namespaces were destroyed; transferred binary
and empty staging directory removed. An initial set fixture lacked its required
transaction ID and refused; correcting that fixture preceded the complete pass.

Local crate tests: 185 passed, 26 ignored including doctests; strict all-target
clippy and formatting passed. Kernel ABI reference:
[Linux v6.18 nftables API](https://github.com/torvalds/linux/blob/v6.18/net/netfilter/nf_tables_api.c).
This is isolated mechanism evidence, not canonical-host protection acceptance.

Next admission still requires the reviewed [safe namespace API prerequisite](K1_NAMESPACE_PATCH_REVIEW.md),
trusted system-manager launch/provenance, structural namespace-transition
prohibition, retained exclusive-create/conditional effects, orphan recovery and
root service/runtime/package integration. The review-only upstream patch is not
adopted, and this slice does not bypass these gates.
