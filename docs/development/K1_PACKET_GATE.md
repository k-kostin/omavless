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
inventory, pinned veth indexes, reciprocal peer indexes, absence of masters or
foreign namespace IDs, and bounded allowlisted local routes without gateways.
The original loopback-only round-trip guard is retained for its original gate.

Fixed documentation-prefix addresses, link-local/multicast maintenance targets,
one RFC1918 target and one ULA target are synthetic fixtures, not real endpoints.
Static neighbors avoid depending on neighbor-discovery availability for the
policy tests. The receiving veth endpoint has no IP address. A Python standard
library helper captures only that peer's link frames; send sockets use the IP
stack (`AF_INET`/`AF_INET6`), **not** packet-socket injection that could bypass
the output hook. Python is developer-only test tooling, not a production
runtime fallback or installed dependency.

The socket helper independently verifies the inherited pinned descriptor,
exact child identity/interface inventory/indexes before socket creation and
sending. There is no selectable command, policy, address or caller-provided
packet. Captured bytes are never printed or retained. Fixed stage and case
numbers are the only failure diagnostics. Exclusive scratch directories/files
use modes 0700/0600 and bounded output/deadlines; normal exit removes known files.
Child namespace exit reclaims the two fixture links and any surviving policy.

## Matrix and control observations

Every one of 43 fixed IP-header vectors must first be observed at the peer
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
tests the interface predicate: all 43 vectors must then reach the peer. This is
not evidence that production owns an actual TUN. Emergency must block all 43
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

## Evidence

Ordinary workspace tests run only pure topology/route refusal checks, bounded
failure parsing, synthetic packet/checksum/frame matching and namespace-fact
checks. They do not create a socket, namespace or network interface, or invoke
`ip`/`nft`. Both installed packet entry points are ignored and require explicit
launch data. Installed VM execution is pending until its exact binary/hash and
outcome are recorded on the owning Draft PR. No physical-PC packet/firewall
test is authorized or claimed by this document.

Primary references inspected September 29, 2026:
[Linux packet sockets](https://man7.org/linux/man-pages/man7/packet.7.html),
[raw IP sockets](https://man7.org/linux/man-pages/man7/raw.7.html),
[veth pairs](https://man7.org/linux/man-pages/man4/veth.4.html),
[Arch ip-link manual](https://man.archlinux.org/man/ip-link.8.en).
