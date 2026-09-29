# K1 offline protocol and transaction foundation

This candidate implements the first bounded part of
[K1 slice 1](../roadmap/KILL_SWITCH.md#10-k1-implementation-slices).
`omavless-netguard` is a library with no executable and no production dependent.
It performs no socket, filesystem, subprocess, service, firewall or package
operation. No installed behavior or advertised capability changes. Python is
neither an implementation nor an oracle for this new feature.

## Fixed protocol

One exchange is one strict UTF-8 JSON document of at most 8,192 bytes, including
whitespace. The versioned envelope is:

```json
{"version":1,"payload":{"operation":"arm","generation":7,"mode":"full"}}
```

The only requests are `status` (no arguments), `arm` (`generation: u64`, exact
`mode: full`) and `disarm` (`generation: u64`). Reconcile and emergency recovery
cannot be represented on the wire. UID, paths, commands, rules, marks, addresses,
interface names and environment are not fields. Root enrollment and SO_PEERCRED
authorization remain mandatory future transport responsibilities.

Responses contain only version/policy version, fixed status/health/error enums
and a generation when validly armed. Corrupt-state emergency protection has no
trusted generation and always reports manual recovery. Uncertain marker/table
combinations produce an error instead of a fabricated armed/disarmed observation.
No rule dump, private endpoint or parser fragment appears in an error.

A strict recursive JSON pass rejects duplicate keys, including escaped aliases,
before typed parsing. Arrays and floating-point numbers have no representation.
Serde's default recursion bound remains enabled; the entire input is capped
before parsing. A typed reserialization equality check rejects fields which
Serde's internally tagged empty variants otherwise silently ignore. This checks
the exact semantic shape, not a canonical byte spelling: whitespace, field order
and equivalent JSON string escaping are intentionally accepted. Numeric floats,
unknown fields, explicit nulls for required values, trailing documents and
unknown versions are rejected. The response decoder applies the same rules.

## Helper transaction model

The pure planner emits a fixed ordered sequence. Each effect needs successful
acknowledgement before the next effect is available or success can be returned.
An out-of-order, failed or uncertain acknowledgement permanently fails that
transaction. Acknowledgement is an adapter contract, not proof of real host I/O.

- Arm: atomically install the fixed Full VPN policy, verify it, durably persist
  the generation, acknowledge armed. An exact retry reinstalls and re-verifies;
  a different generation cannot replace an armed generation.
- Disarm: require the matching armed generation, durably remove its marker,
  atomically delete the owned table, verify absence. A retry with both marker
  and table absent confirms disarmed; a missing marker with a surviving table
  requires internal reconciliation rather than trusting a caller generation.
- Root/internal restart: restore a valid armed marker's policy; remove stale
  owned policy for a missing marker; install emergency restrictive policy and
  retain the recovery requirement for invalid/unreadable/newer state.

The runtime is a separate owner: it must persist desired connected only after
verified arm, and persist desired disconnected plus verify core cleanup before
disarm. The library does not accept caller-supplied runtime/core observations and
does not implement that coordination yet. Generations must be durably unique
across runtime connection attempts; a future transport must serialize requests
and not permit delayed requests to reuse a generation. No production connection
or confidentiality guarantee is inferred from this pure helper model.

## Symbolic policy and remaining security work

The policy representation fixes `inet omavless_netguard` ownership and lists
only loopback, the package TUN, the package core mark, narrow DHCPv4/DHCPv6 and
IPv6 neighbor discovery followed by drop-all output. It contains no generic
established-flow, UID, LAN, physical-interface or DNS exemption. Emergency
policy permits only loopback before drop-all.

This is deliberately **not executable nft syntax**. A complete rule renderer
still needs concrete package-owned mark/TUN constants, hook priorities, exact
DHCP/ND address/port/type/hop-limit predicates, atomic replace semantics and
foreign-firewall interaction review. A symbolic rule name does not prove that
an actual DHCP/ND exception is narrow or that a marked resolver works.

Next: review and implement that renderer plus a fake/injected adapter before
root installation. Then add bounded framed Unix transport with deadlines,
SO_PEERCRED/enrollment and root file safety, locking, durable state parsing,
atomic nft application/readback, fixed console recovery, package/boot ordering
and runtime coordination as separately reviewed slices. Error/uncertain writes
must be reconciled from durable facts, never treated as confirmed success.
The entire [K1 host matrix](../roadmap/KILL_SWITCH.md#9-k1-acceptance-matrix)
remains open, including IPv4/IPv6/DNS leakage, core mark coverage, coexistence,
process deaths, upgrades/removal and mandatory physical NIC/suspend/boot gates.

## Deterministic verification

Credential-free tests cover accepted fixed messages, malformed/duplicate/unknown
fields at every schema level, escaped duplicate keys, integer/size/UTF-8/depth
boundaries, truncation, unsupported versions and safe errors. Transaction tests
inject crashes before/after each arm/disarm effect, reconstruct from durable
marker facts, test stale generations, poisoned acknowledgements, retries and
invalid-state emergency behavior. They simulate effects in memory; they are not
kernel crash, reboot, filesystem durability or installed VM evidence.

Run `cargo test -p omavless-netguard --locked`, `cargo fmt --all -- --check` and
`cargo clippy -p omavless-netguard --all-targets --locked -- -D warnings`.
The crate participates in ordinary workspace CI. No external dependency or
version was added beyond the workspace's existing serde/serde_json packages.
