# K1 isolated packet-policy gate

This development-only harness stacks on the installed JSON/readback gate
(#329). It does not install NetGuard, enable a feature, change the production
runtime, grant capabilities to an application, or claim completed K1 acceptance.
The fixed candidate policy is unchanged.

## Isolation and execution

Run **only inside the delegated disposable development VM**, never on the
physical desktop. Required tools are `/usr/bin/unshare`, `ip`, `nft`, `python3`;
the child user+network namespace must support scoped `CAP_NET_ADMIN` and
`CAP_NET_RAW`. Missing capabilities/tools are refusal, not a reason to use sudo,
change sysctls or run in the parent namespace.

```sh
OMAVLESS_K1_PACKET_VM=1 cargo test -p omavless-netguard --locked \
  --test nft_namespace packet::nft_packet_enforcement_in_disposable_vm \
  -- --ignored --exact --nocapture
```

The outer process pins the parent network namespace descriptor and checks its
identity again after completion/failure. Its child initially requires exactly
loopback. Only then does it create a veth pair, keeping **both ends inside that
same child namespace**. It never moves an interface to another namespace,
creates a bridge/uplink, or installs a default route. Every subsequent network
command verifies the child differs from the pinned parent, the exact interface
inventory, pinned veth indexes, reciprocal peer names/indexes, absence of masters or
foreign namespace IDs, and bounded allowlisted local routes without gateways.
The original loopback-only round-trip guard is retained for its original gate.

Fixed documentation-prefix addresses, link-local/multicast maintenance targets,
one RFC1918 target and one ULA target are synthetic fixtures, not real endpoints.
Static neighbors avoid depending on neighbor-discovery availability for the
policy tests. The receiving veth endpoint has no IP address. A Python standard
library helper captures only that peer's link frames; send sockets use the IP
stack (`AF_INET`/`AF_INET6`), **not** packet-socket injection that could bypass
the output hook. Ten additional probes use ordinary `SOCK_DGRAM` and
`SOCK_STREAM` sockets, including IPv4/IPv6 UDP/TCP port 53 and port 443, plus
two explicitly marked DNS-shaped datagrams. Python is developer-only test tooling, not a production
runtime fallback or installed dependency.

The socket helper independently verifies the inherited pinned descriptor,
exact child identity/interface inventory/indexes before socket creation and
sending. There is no selectable command, policy, address or caller-provided
packet. Captured bytes are never printed or retained. Fixed stage and case
numbers are the only failure diagnostics. Exclusive scratch directories/files
use modes 0700/0600 and bounded output/deadlines; normal exit removes known files.
Child namespace exit reclaims the two fixture links and any surviving policy.

## Matrix and control observations

Every one of 43 fixed IP-header vectors and 10 ordinary socket probes must first be observed at the peer
without a policy. A negative result therefore cannot pass merely because a
route, socket, neighbor or capture is broken. Under Full:

- unmarked IPv4/IPv6 UDP/TCP, UDP/TCP port 53, TCP 853, RFC1918/ULA/link-local
  application traffic and wrong-mark traffic must not reach the peer;
- the exact core-mark IPv4/IPv6 vectors must reach it;
- the exact candidate DHCP broadcast/link-local and RS/NS/NA header exceptions
  must reach it, but wrong source/destination/ports, ICMP type/code or hop limit
  near-misses must not;
- IPv4/IPv6 loopback UDP remains usable in every phase.

Renaming the same pinned veth output endpoint to the fixed reserved TUN name
tests the interface predicate: all 53 vectors must then reach the peer. This is
not evidence that production owns an actual TUN. Emergency must block all 53
non-loopback vectors again, including the mark and interface exceptions.
Only after strict readback of an exclusively created test policy may the
harness delete that test table. No production ownership receipt is issued.

The 80 ms negative observation window is bounded and supported by positive
controls on the identical fixture, not a proof about arbitrarily delayed or
fragmented packets. Packets contain valid IP/transport checksums and synthetic
bodies, but DHCP/ND **header tests are not client-protocol or link-acquisition
tests**. They do not validate full neighbor discovery, DHCP renewals, real DNS
resolution, HTTPS, encrypted DNS, connection tracking, fragments/extensions,
Mihomo socket marks or stock-firewall coexistence. Mark generation/socket
coverage, interface ownership, root receipt durability and mandatory physical
NIC/suspend/boot cases remain separate acceptance gates.

The later stacked coexistence gate adds a second, **test-only** `inet` table
inside the same disposable namespace. Its fixed output chains at priorities 0
and 400 both accept packets, before and after the candidate's priority-300
guard. The complete 53-vector positive-control/Full/TUN/Emergency sequence
must still pass. The foreign table's full numeric readback must remain identical
through candidate creation, the fixture's Emergency switch and removal; only
then is the foreign fixture itself deleted. This checks that an earlier
`accept` does not bypass a later K1 `drop`, and a later `accept` cannot revive
a dropped packet, without a host firewall mutation. It does **not** exercise
Omarchy's actual UFW/nftables rules, firewall reloads, conflicting drop
policies or a production NetGuard.

## Evidence

Ordinary workspace tests run only pure topology/route refusal checks, bounded
failure parsing, synthetic packet/checksum/frame matching and namespace-fact
checks. They do not create a socket, namespace or network interface, or invoke
`ip`/`nft`. Both installed packet entry points are ignored and require explicit
launch data. The first installed x86_64 VM pass on September 29, 2026 covered
the 43 header vectors and loopback (binary SHA-256
`47051041bcd6e593a96ece38e377b4f73aaa06e320a142208f1070fa88d8cd0e`).
Initial topology refusal exposed iproute2's same-namespace reciprocal `link`
names rather than `link_index`; both complete representations are now checked,
while mixed, foreign, self-referential or changed identities are rejected.
The expanded 53-vector gate requires its own exact-binary VM retest recorded
on the owning Draft PR. The later stacked run below repeats it with the
additional coexistence fixture. No physical-PC packet/firewall test is
authorized or claimed by this document.

On September 30, 2026, the stacked foreign-accept coexistence gate passed in
the x86_64 Omarchy development VM on kernel `7.2.5-3-omarchy`. The final tested
binary SHA-256 was
`b60111462525e4836efb12b76ca751fbb0c07983d5c7581c13faff125cd0329d`.
All baseline/Full/interface/Emergency vector phases passed with fixed earlier
and later accepting chains; the foreign table's numeric readback was unchanged
after each candidate transition and after candidate removal. The test created
no table or link in the parent namespace; its interface count remained two and
no NetGuard service was active. This is synthetic base-chain coexistence in a
child namespace, **not** stock-firewall integration or host K1 acceptance.

Primary references inspected September 29, 2026:
[Linux packet sockets](https://man7.org/linux/man-pages/man7/packet.7.html),
[raw IP sockets](https://man7.org/linux/man-pages/man7/raw.7.html),
[veth pairs](https://man7.org/linux/man-pages/man4/veth.4.html),
[Arch ip-link manual](https://man.archlinux.org/man/ip-link.8.en).
