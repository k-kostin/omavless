# K1 fixed FullVpn atomic wire candidate

This inactive pure Rust encoder follows the fixed Emergency encoder and its
separate untrusted reply parser. It adds no socket, executor, syscall binding,
authority token, package dependency, installed caller or service. K1 remains
unavailable. General kernel compatibility and packet enforcement are **not
proven**; the separate isolated result below covers only its exact mechanism.
Previous JSON-renderer packet results do not transfer to this encoder.

`full_vpn_wire::encode(generation, first_sequence)` accepts only two nonzero
u32 fences and rejects sequence overflow. One batch contains generation-fenced
begin, exclusive `owner,persist` table, exclusive output/drop chain at priority
300, ten fixed ordered rules, and end. The rules match the existing renderer:
loopback, reserved TUN name, fixed core mark, DHCPv4, DHCPv6, two router
solicitations, neighbor solicitation, neighbor advertisement and final drop.
All twelve table/chain/rule operations request ACKs. The separate GETGEN uses
sequence `first_sequence + 14`; it cannot substitute for the operation ACKs.
The Emergency-only reply parser is unchanged and cannot validate this transcript.

The shared internal helpers encode only framing, nested attributes and verdicts.
FullVpn predicates are fixed private code, not inputs. Scalar netlink attributes
are big endian, headers are native endian, and packet comparison bytes preserve
their representation: ports/addresses are network order while `skb->mark` is
native order. IPv4/IPv6 family and UDP/ICMPv6 protocol checks precede maintenance
payload loads. Transport-header-relative offsets avoid assuming a fixed IP
header length. Prefix predicates use explicit 128-bit mask/XOR expressions.
No general established-flow, DNS, LAN or UID exemption is added.

Independent tests decode every message, attribute and expression without the
production encoding helpers, and compare with a separately written fixed plan.
They check flags, sequence extremes, table/chain constants, family/protocol
dependencies, offsets, field widths, prefix masks, verdicts and byte order.
A small test-only evaluator checks six maintenance cases and near misses for
family, protocol, ports/type/code, destination, hop limit and prefix boundary,
including absent transport bytes. This evaluator is not the kernel, does not
simulate fragment/extension parsing, and is not packet enforcement evidence.
Existing renderer goldens and the original 53-vector packet fixture are unchanged.

Source basis: Linux v6.18 [nf_tables UAPI](https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/netfilter/nf_tables.h),
[meta evaluation](https://github.com/torvalds/linux/blob/v6.18/net/netfilter/nft_meta.c)
and [payload evaluation](https://github.com/torvalds/linux/blob/v6.18/net/netfilter/nft_payload.c).
These are source-level ABI references, not installed-kernel acceptance.

Before a transport/executor can use this candidate: independent wire review,
exact-head isolated kernel readback/rollback and packet gates (including
transport dependency, fragments/extensions), a FullVpn-specific reply validator,
safe namespace APIs, trusted canonical launch, retained exclusive creator,
conditional effects and durable receipt ordering remain required. Actual TUN
ownership and core/resolver mark coverage are independent of matching a name
or number. No root service or desktop-network effect is part of this slice.

## Separate opt-in isolated kernel fixture

`atomic_full::atomic_full_in_disposable_vm` is ignored by ordinary tests and
requires `OMAVLESS_K1_FULL_VM=1`. Run it only in the dedicated disposable VM:

```sh
OMAVLESS_K1_FULL_VM=1 cargo test --locked -p omavless-netguard \
  --test nft_namespace atomic_full::atomic_full_in_disposable_vm \
  -- --ignored --exact --nocapture
```

The Rust parent pins and rechecks its own namespace, then starts a fresh
unprivileged user/network namespace. Existing guards require a distinct pinned
child, loopback-only inventory and matching socket namespace cookie. No veth,
uplink, default route or host-service operation is performed. Only child
loopback is brought up. Existing bounded raw-netlink fixture code retains the
exclusive creator socket. An ignored pure child of the **same test executable**
calls `full_vpn_wire::encode`; its exact batch/barrier bytes are sent. JSON is
only the independent expected readback, never the creation mechanism.

The late-collision fault inserts one sentinel collision after all twelve
operations but before end: EEXIST must roll back the target with generation
and sentinel unchanged. Stale generation must similarly refuse with ERESTART.
Successful exact bytes require all operation ACKs and the GETGEN barrier,
complete ordered renderer readback (only the previously reviewed family-predicate
elision is an alternative), owner/persist metadata, stable generation/handle
and unchanged sentinel. This still uses the **test-only Python** reply collector,
not a production FullVpn reply validator. Closing the creator leaves the exact
persist-only orphan; another creator must refuse it. Namespace teardown removes
fixtures without orphan adoption/deletion.

IPv4 and IPv6 UDP loopback packets pass before and after creation. This is basic
loopback functionality only: **no non-loopback drop, physical egress, DHCP/ND,
mark/TUN bypass or 53-vector packet gate is proven for these bytes**. An outer
15-second bound and inner read/exchange/encoder bounds limit the test. Ordinary
tests cover reply/shape refusal and direct unisolated invocation without sockets.

Initial exact code `aa30bd6e908814cf0b43b5f49728bedbc315fb93` passed in dedicated
x86_64 Omarchy Dev KVM on 2026-10-02, kernel `7.2.5-3-omarchy`, nftables
`1:1.1.7-3`, Omarchy `4.0.4-1`. The transferred executable SHA256 was
`e2f171a20392c8a9f2c791f74c1247e9c6aa90a5f7f08171d6152f164ad47dc7`, checked
before and after execution. The exact staged executable and its empty directory
were removed. This establishes only the bounded kernel/readback/rollback and
loopback cases above, not K1 protection or product readiness.
