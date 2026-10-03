# K1 isolated owner/persist socket-lifetime gate

This opt-in test follows the [crash disposition proposal](K1_CRASH_DISPOSITION.md).
It checks a Linux mechanism, not application/table provenance. No production
adapter, receipt authority, root service, package behavior or K1 activation is
added. The existing no-adopt recovery boundary is unchanged.

## Scope and invocation

Run only in the delegated development VM after exclusive-use coordination:

```sh
OMAVLESS_K1_OWNER_VM=1 cargo test -p omavless-netguard --locked \
  --test nft_namespace owner::owner_lifetime_in_disposable_vm \
  -- --ignored --exact --nocapture
```

The Rust parent pins its network namespace FD and passes it as stdin to a new
`unshare --user --map-root-user --net` child. The child reuses the
[capability fixture's guard and bounded codec](K1_KERNEL_CAPABILITIES.md): network
namespace type, pinned identities, differing current/parent namespace and an
exact loopback-only inventory are checked before opening sockets and before
every netlink exchange. The parent rechecks its own namespace after completion.

The developer-only Python helper is embedded in the Rust test, uses isolated
Python mode and loads only its fixed embedded sibling. It exposes no arbitrary
command, table, interface, address, payload or path input. Ordinary workspace
tests check only pure builders and direct-invocation refusal, without sockets.

Only two fixed empty `inet` tables exist: a target and a foreign sentinel.
There are no chains, hooks, rules, routes, links, IP connections or sudo fallback.
Three private netlink sockets represent observer, creator and challenger; later
a fresh creator socket is used after the first closes. Requests are bounded to
the inherited 15-second process deadline, one-second netlink exchange deadline
and 32-KiB input/output limits. Only fixed PASS/failure-stage categories escape.

## Assertions

1. A creator exclusively creates an owner-only target. Observed owner matches
   that socket's port ID. A different socket's handle deletion receives EPERM;
   the complete target metadata, generation and sentinel are preserved.
2. Closing the creator socket removes the owner-only target. The sentinel stays.
3. A fresh socket exclusively creates an `owner,persist` target. Challenger
   deletion and attempted acquisition while the owner lives receive EPERM.
4. Closing its owner socket preserves the target's handle and data, clears its
   owner attribute and leaves `persist`. The sentinel is unchanged.
5. The challenger can then acquire the orphan by requesting `owner,persist`.
   The table handle is unchanged; its owner is now the challenger socket.
   Observer deletion is refused while the challenger owns it.
6. Closing the challenger releases ownership again. Cleanup deletes only the
   observed fixture handle, verifies target absence and sentinel identity, then
   removes the known sentinel handle and verifies both fixed names absent.

Mutations use nonzero generation preconditions; deletion uses exact handles.
Every target was exclusively created by this fixture in its newly isolated
namespace. The acquisition test must never be repurposed as a production orphan
adopter. On failure no host cleanup or broad flush is attempted; child exit
destroys its disposable namespace. Temporary scripts are removed by the parent.

## Interpretation and limits

The [nftables table-flag manual](https://www.netfilter.org/projects/nftables/manpage.html)
and version-pinned [`nft_rcv_nl_event` / `nf_tables_updtable` implementation](https://github.com/torvalds/linux/blob/v6.18/net/netfilter/nf_tables_api.c)
describe the behavior being tested. Netlink socket ownership can exclude other
sockets while the owner is alive, but it is not persistent OmaVLESS identity.
Successful orphan acquisition by a different socket is evidence of the gap,
not evidence that the socket has authenticated the original creator.

Owner-only lifetime is unsuitable for the accepted K0 fail-closed promise:
helper/socket death would remove active policy. Persistent lifetime avoids
that deletion but still needs independent application provenance/recovery.
Empty-table tests do not prove packet protection, root service identity,
process-SIGKILL behavior, module lifetime, boot ordering or filesystem/kernel
atomicity. No protected connection, orphan cleanup policy, main merge, release
or marketplace readiness is inferred. Exact-head VM evidence belongs on the
owning Draft PR and must identify the tested kernel and binary hash.
