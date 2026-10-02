# K1 fixed FullVpn atomic wire candidate

This inactive pure Rust encoder follows the fixed Emergency encoder and its
separate untrusted reply parser. It adds no socket, executor, syscall binding,
authority token, package dependency, installed caller or service. K1 remains
unavailable. Kernel compatibility and enforcement of these bytes are **not
proven**; previous JSON-renderer packet results do not transfer to this encoder.

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
or number. No VM, root operation or desktop-network effect is part of this slice.
