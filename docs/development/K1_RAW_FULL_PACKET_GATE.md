# K1 raw FullVpn isolated packet candidate

This separate opt-in test reuses the unchanged 43 IP-header and ten ordinary
socket vectors from the JSON policy fixture. Unlike that original test, it
creates the policy using exact `full_vpn_wire::encode` output from an ignored
pure child of the same Rust test executable. JSON is only expected readback.
No product caller, privilege grant, service, package or namespace API is added.

The parent pins its network namespace and starts a fresh user/network child.
The retained Rust `PacketGuard` first requires loopback-only isolation, then
creates both veth ends inside that child. Fixed documentation-prefix addresses,
static neighbors and bounded closed routes are identical to the original test;
no endpoint is moved, bridged or connected to an uplink. A separate veth-only
Python guard validates inherited parent/current/pinned namespace identities,
network-namespace type, exact interfaces and indexes, reciprocal peers, no
master/foreign-netns entries, and the same closed route allowlist. The original
loopback-only `capability.Guard` is not changed or bypassed by relaxed predicates.

The worker retains its exclusive creator socket through baseline, complete
FullVpn readback, FullVpn packet checks, and the same-index output-interface
rename to `omavless0`. Cookie, socket address, exact owner/persist table metadata,
generation and ordered full policy are checked around each protected phase.
Each vector has a positive baseline before negative observations; the unchanged
helper emits via the IP stack and captures only the peer. Loopback remains
checked. No target deletion, acquisition or operation after creator loss occurs;
the disposable namespace reclaims the persist-only fixture at child exit.

```sh
OMAVLESS_K1_RAW_PACKET_VM=1 cargo test --locked -p omavless-netguard \
  --test nft_namespace packet::raw::raw_full_packet_enforcement_in_disposable_vm \
  -- --ignored --exact --nocapture
```

Run only in the coordinated disposable VM. Ordinary tests execute pure guard
mutations and direct-invocation refusal, never namespace creation or packets.
The parent has a 45-second bound, worker 30 seconds and individual commands
one second; encoder/readback/netlink exchanges retain their existing bounds.
Only fixed stages/vector numbers are exposed, never packet bytes or private data.

This adds no new Emergency, foreign-firewall or fragment/extension matrix.
Even a green exact-head result would prove only these fixed synthetic vectors,
not actual TUN ownership, comprehensive core marks, DHCP/ND client behavior,
physical NIC/boot/suspend or K1 product readiness. FullVpn ACK validation remains
a test-only collector until its separate bounded Rust transcript candidate.
