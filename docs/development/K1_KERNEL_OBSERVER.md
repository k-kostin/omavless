# K1 read-only fixed-table observer candidate

This slice stacks on #373's effect-only boundary. `kernel_observer` adds a safe
Rust netlink reader with no `EffectPort` implementation, production caller,
service, provisioning, mutation, receipt writer or ownership conversion.

Each inspection's only request sequence is GETGEN, GETTABLE for `inet omavless_netguard`,
GETGEN. `LocalReadSession` retains one nonblocking CLOEXEC socket and the
calling-thread network namespace descriptor across repeated inspections.
The one-shot API uses the same session internally. Sequences increase without
reuse; exhaustion refuses instead of wrapping. Current and pinned namespace
device/inode labels and local socket address are rechecked before and after
each exchange. Requests have fixed types, family and name; callers
cannot select a path, table, command, payload or namespace.

The retained namespace descriptor must be on `nsfs`, while the fixed
`/proc/thread-self/ns` directory must be on procfs. A regular file, procfs
status file or substituted namespace file type refuses. This checks descriptor
types, not canonical host provenance or the network namespace type; it must
not be used as an ownership proof.

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

The ignored absence integration test must run only in the delegated VM:

```sh
OMAVLESS_K1_OBSERVER_VM=1 cargo test -p omavless-netguard --locked \
  --test nft_namespace observer::read_only_observer_in_disposable_vm \
  -- --ignored --exact --nocapture
```

It inherits a pinned parent namespace FD into a fresh unprivileged user/network
namespace, verifies that the child differs and has only loopback, then performs
two actual Rust observations of fixed-table absence on the same retained
session. It then moves only the isolated child thread into a second disposable
network namespace and verifies that the retained session refuses further
inspection. It creates no table, rule,
interface, route or IP connection. The parent rechecks its original namespace.
Only a fixed PASS category escapes the bounded child output; there is no sudo
fallback. Shared test scratch now honors TMPDIR so tmpfs quota exhaustion need
not masquerade as a storage implementation failure. Exact source and binary
identities and VM result belong on the owning Draft PR.

The separate opt-in **present-but-untrusted** gate uses the same disposable-VM
boundary. After verifying a new loopback-only user/network namespace, it opens
one retained session, creates a fixed empty `inet omavless_netguard` table there,
checks that two observations say `PresentUntrusted`, then removes only that
fixture and checks `Absent` on the same session. The parent namespace identity is
rechecked; no service,
host firewall or production policy is changed. Run only in the delegated VM:

```sh
OMAVLESS_K1_OBSERVER_PRESENT_VM=1 cargo test -p omavless-netguard --locked \
  --test nft_namespace observer::present_untrusted_observer_in_disposable_vm \
  -- --ignored --exact --nocapture
```

Direct invocation of either ignored child must refuse before nft/socket I/O.
This gate proves the current kernel's empty-table response can be parsed; it
does not validate complete rules, chains, owner provenance or live K1 protection.

The separate installed nft JSON round-trip also checks this Rust observer at
each stage: absent before create, `PresentUntrusted` for both exact Emergency
and Full policy-shaped fixtures, and absent after each fixture cleanup. Even a
table whose rules match the offline renderer never acquires observer authority.

## Inactive complete chain inventory

A separate retained-session read now brackets one fixed-table GETCHAIN dump
with generation reads. It uses a dedicated multipart parser; the earlier
non-dump GETTABLE/GETGEN exchange is unchanged. The dump must end with a
successful `NLMSG_DONE`, matching sender/port/sequence, unchanged generation
and no interruption, unknown frame, duplicate name, malformed attribute or
truncation. It is limited to 64 KiB, 32 datagrams, 64 messages and the same
one-second observation deadline. It reports only `TableAbsent`, `Empty`,
`ExpectedOutputChainUntrusted` or `OtherUntrusted`. A syntactically matching
`output_guard` means only one base chain has output hook, priority 300 and
drop policy; no rule inventory or ownership has been established.

The kernel's chain dump spans the namespace, not just the requested table.
Every record must first pass the supported schema, generation, framing and
resource bounds. A validated foreign-table record does not contribute to the
fixed-table count or expected-chain shape; duplicate identity is checked per
table and chain. Malformed or unsupported foreign records still refuse rather
than silently disappearing. This is bounded coexistence with supported chain
schemas, **not** a promise that every foreign firewall shape is supported.

The opt-in integration gate runs only in a disposable user/network namespace
inside Omarchy Dev VM. It checks absent and empty states, the expected base
chain, extra fixed-table chain, wrong hook/priority/policy, foreign-table
coexistence with an identically named base chain and cleanup to absent, then rechecks the parent
namespace. No host firewall, route or VPN is changed:

```sh
OMAVLESS_K1_CHAIN_VM=1 cargo test -p omavless-netguard --locked \
  --test nft_namespace observer::chain_inventory_in_disposable_vm \
  -- --ignored --exact --nocapture
```

This is a prerequisite for later complete policy readback, not that readback
itself. Full rule/set inventory, canonical namespace/socket binding,
independently authenticated ownership, conditional effects, root service and
the required bare-metal acceptance remain open.

Wire behavior is based on the
[Linux v6.18 table and generation implementation](https://github.com/torvalds/linux/blob/v6.18/net/netfilter/nf_tables_api.c)
and [netlink framing semantics](https://man7.org/linux/man-pages/man7/netlink.7.html).
The version-pinned source is a review baseline; a live VM result establishes
compatibility only for its recorded kernel and exercised absent-table case.
