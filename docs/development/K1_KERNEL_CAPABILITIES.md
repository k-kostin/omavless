# K1 isolated kernel capability gate

This test-only candidate follows [receipt assessment](K1_RECEIPT_ADMISSION.md).
It probes actual namespace and nftables primitives without adding a runtime
caller, installed helper, root service, firewall policy or receipt authority.
It does not make K1 available or permit adoption of an existing table.

## Run only in the delegated development VM

```sh
OMAVLESS_K1_CAPABILITY_VM=1 cargo test -p omavless-netguard --locked \
  --test nft_namespace capability::kernel_capabilities_in_disposable_vm \
  -- --ignored --exact --nocapture
```

The ignored Rust entry point pins its original namespace FD, creates a private
test script and starts `/usr/bin/unshare --user --map-root-user --net` with the
FD inherited as stdin. Its Python standard-library helper is developer-only,
following the existing packet-test convention. It is neither packaged nor
imported by production. There is no sudo fallback: unavailable unprivileged
user namespaces or scoped capabilities refuse the gate.

Before creating sockets and before every netlink exchange, the helper checks
`NS_GET_NSTYPE`, both pinned identities, a fresh current-namespace FD, and an
exact loopback-only interface inventory. The child must differ from the parent.
The outer process rechecks its pinned and current namespace after the child
exits. The child creates only two fixed empty `inet` tables. No chains, rules,
hooks, network interfaces, routes or IP connections are created. One table acts
as a foreign sentinel; each operation may target only a handle established by
this fixture's exclusive creation. Child exit also releases its private netns.

Input is fixed synthetic data. The helper exposes no arbitrary name, operation,
command, path, expression or raw-payload option. Output is bounded and contains
only fixed stage/capability/pass categories. No kernel replies, namespace IDs,
raw errors or traceback are emitted. The test has a 15-second outer deadline,
one-second per-exchange deadline and 32-KiB input/output bounds. Normal completion
removes its two tables and temporary script; the transferred test binary is
removed after VM acceptance. Ordinary workspace tests exercise only wire-codec
refusal and direct-invocation refusal, without sockets or namespace creation.

## What it checks

- Network namespace type and differing pinned parent/child identities.
- `NS_GET_ID`, when supported: distinct IDs for parent and child, unchanged at
  completion. Unsupported ioctl is reported explicitly, never fabricated.
- `SO_NETNS_COOKIE`: a nonzero child socket cookie, equal to `NS_GET_ID` when
  both APIs are available. The UDP socket is never bound or connected.
- Exclusive table create rejects an existing target with `EEXIST` and leaves
  its complete observed table metadata and ruleset generation unchanged.
- Atomic delete-by-handle plus exclusive recreate obtains a different handle.
- A stale nonzero batch-generation precondition returns `ERESTART`; the current
  target and generation remain unchanged.
- Deleting the former handle returns `ENOENT` and preserves its replacement.
- A batch that deletes/recreates the target and then collides with the sentinel
  rolls back completely: both table observations and generation are unchanged.
- Cleanup deletes only the fixture handles; the sentinel survives target
  cleanup, then is removed itself, and both fixed names are verified absent.

The helper encodes the fixed Linux netlink messages directly. Every mutation
includes a nonzero generation precondition and an explicit `inet` family.
Deletion uses a handle, never a name fallback or ruleset flush. A separate
GETGEN request is the completion barrier; sequence/type/kernel-sender checks,
ACK/error validation, truncation refusal and bounded parsing are mandatory.
Failures never trigger automatic retry, retargeting or policy weakening.

## Evidence and limits

On September 29, 2026, the isolated x86_64 Omarchy guest with kernel
`7.2.5-3-omarchy` passed the complete gate in 0.05 seconds. Both `NS_GET_ID` and
`SO_NETNS_COOKIE` were available, with the expected equality/difference checks.
The tested binary SHA-256 was
`8fbdcc295ec9b6a5c6eee4720582625301b398e6beef15e34894568a174c5cfa`.
The guest service remained inactive and the parent interface count remained
two. The transferred binary was removed. Exact source-head checks are recorded
on the owning Draft PR; later implementation changes require affected retests.

This is kernel-capability evidence, not a production namespace authenticator,
table-ownership receipt, complete nft executor, packet enforcement, protected
connection, crash/reboot recovery or physical-host acceptance. In particular:

- Namespace ID/cookie identity does not establish that a caller is in the
  canonical host namespace; trusted service launch and namespace provenance
  remain necessary.
- Boot + namespace + table handle do not by themselves prove durable table
  provenance. Nftables subsystem reinitialization and finite counters require
  separate review; this test neither unloads modules nor forces counter wrap.
- A coherent observation followed by conditional mutation still cannot make
  filesystem receipt publication atomic with kernel commit. Pending/orphan
  outcomes retain the existing refusal contract.
- The ioctl encoding is for the supported x86_64/ARM64 developer targets;
  capability availability must be checked on each installed target. This VM
  pass is not ARM64 acceptance.

Primary sources: [namespace lifetime](https://man7.org/linux/man-pages/man7/namespaces.7.html),
[Linux namespace UAPI](https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/nsfs.h),
[nfnetlink batch UAPI](https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/netfilter/nfnetlink.h),
[nftables UAPI](https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/netfilter/nf_tables.h),
and [kernel table-handle/generation implementation](https://github.com/torvalds/linux/blob/v6.18/net/netfilter/nf_tables_api.c).
