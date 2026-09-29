# K1 read-only fixed-table observer candidate

This slice stacks on #373's effect-only boundary. `kernel_observer` adds a safe
Rust netlink reader with no `EffectPort` implementation, production caller,
service, provisioning, mutation, receipt writer or ownership conversion.

The only request sequence is GETGEN, GETTABLE for `inet omavless_netguard`,
GETGEN. A fresh nonblocking CLOEXEC socket and a retained calling-thread network
namespace descriptor span the entire sequence. Current and pinned namespace
device/inode labels and local socket address are rechecked before and after
each exchange. Requests have fixed types, family, name and sequences; callers
cannot select a path, table, command, payload or namespace.

Each exchange requires exact kernel sender address, destination port ID,
sequence, response type and echoed ACK header. Successful reads require both
the data response and ACK. Only an exact GETTABLE ENOENT response establishes
absence. Negative errors, interrupted/multipart/unknown flags, truncated data
or control messages, duplicate/unknown attributes, invalid sizes/padding,
missing mandatory fields and unexpected messages refuse. Bounds are 32 KiB and
16 datagrams per exchange and one second for the whole observation. There is
no retry or fallback. The before/after full generation must match, and each
response's low 16-bit generation must agree. Raw names, userdata and kernel
errors are neither returned nor logged.

## Deliberately narrower than kernel authority

The result is only `LocalTablePresence::{Absent, PresentUntrusted}`. Present
metadata includes the table header, flags, use count, handle and optional
userdata/owner fields, but no chain/rule/set/object inventory. It is **not full
policy readback**. An owner-port match, identical handle, copied userdata or
receipt cannot turn this result into ownership. There is no create, replace,
delete, adoption, protection status or disarmed-intent inference.

The workspace forbids unsafe Rust. Its current safe nix/rustix APIs do not
expose NS_GET_ID / SO_NETNS_COOKIE, so this candidate does not authenticate a
socket cookie against the pinned namespace or prove canonical host namespace
provenance. Pinned descriptors and repeated procfs labels only constrain this
local read. A namespace switch-and-return outside this function, subsystem
reinitialization and finite generation wrap are not disproved. These are
explicit blockers to implementing the effect adapter, even after a VM PASS.
Do not replace those gates with Python, weaken the unsafe lint, or treat this
API as an ownership constructor.

The [kernel capability](K1_KERNEL_CAPABILITIES.md) and
[owner lifetime](K1_OWNER_LIFETIME.md) experiments remain separate evidence.
Their exclusive create/conditional mutation and orphan-acquisition mechanics
do not authorize this observer to write. Complete policy inventory, independently
authenticated live ownership, conditional effects, persistent orphan disposition,
and the K1 service/host acceptance matrix remain open.

## Validation

Normal tests use synthetic frames only: fixed request allowlist, exact ACK/data
completion in both orders, ENOENT versus permission failures, every truncated
frame, sender/port/sequence mismatch, duplicate frames/attributes, unknown and
nested attributes, owner flag inconsistencies and generation mismatches.
All recognized present flag combinations remain untrusted. A separate normal
test invokes the ignored child without its isolation context and verifies
refusal before any observer socket is opened.

The ignored integration test must run only in the delegated VM:

```sh
OMAVLESS_K1_OBSERVER_VM=1 cargo test -p omavless-netguard --locked \
  --test nft_namespace observer::read_only_observer_in_disposable_vm \
  -- --ignored --exact --nocapture
```

It inherits a pinned parent namespace FD into a fresh unprivileged user/network
namespace, verifies that the child differs and has only loopback, then performs
two actual Rust observations of fixed-table absence. It creates no table, rule,
interface, route or IP connection. The parent rechecks its original namespace.
Only a fixed PASS category escapes the bounded child output; there is no sudo
fallback. Shared test scratch now honors TMPDIR so tmpfs quota exhaustion need
not masquerade as a storage implementation failure. Exact source and binary
identities and VM result belong on the owning Draft PR.

Wire behavior is based on the
[Linux v6.18 table and generation implementation](https://github.com/torvalds/linux/blob/v6.18/net/netfilter/nf_tables_api.c)
and [netlink framing semantics](https://man7.org/linux/man-pages/man7/netlink.7.html).
The version-pinned source is a review baseline; a live VM result establishes
compatibility only for its recorded kernel and exercised absent-table case.
