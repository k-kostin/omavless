# K1 offline nftables candidate

This development slice adds a pure JSON create-template and strict readback
classifier to `omavless-netguard`. It is not an installed kill switch. There is
no executor, privileged process, socket, package, production caller or root I/O.
It stacks on the generation/ownership transaction foundation (#324).

## Fixed candidate

`render_create(Policy)` accepts only Full VPN or Emergency. It emits one
create-if-absent table followed by a fixed chain and rules. No address, endpoint,
interface, mark, LAN prefix or command can be supplied by a request. The table is
`inet omavless_netguard`, chain `output_guard`, output filter priority 300 and
default drop. The explicit last rule drops as well. Emergency admits loopback
only. The normal candidate adds reserved TUN `omavless0`, mark `0x4f4d4101`,
broadcast DHCPv4 68→67, link-local DHCPv6 546→547 to `ff02::1:2`, and narrow
ICMPv6 router/neighbor maintenance with code 0 and hop limit 255.

These are **candidate package constants**, not facts about current Mihomo
configuration. Current generic `Meta` interfaces cannot be accepted as the K1
TUN merely by renaming this constant. Runtime integration must establish exact
owned-interface identity and verify every required core/resolver socket mark.
There is no profile-endpoint exemption: marked core traffic supplies that path.
The separate [packaged-core mark probe](K1_CORE_MARK_PROBE.md) tests one
synthetic direct TCP socket in an isolated VM and records why this is not a
general mark-coverage or activation proof.
There is no generic established-flow, user-ID, DNS, LAN or physical-interface
exception. DHCP/ND are protocol exemptions, not a malicious-user defense.

The candidate intentionally omits unicast DHCP renewal and several unicast/global
IPv6 neighbor-maintenance cases pending connected-link analysis. It can therefore
lose connectivity on valid networks. No production readiness follows from these
tests. Do not widen those exceptions to arbitrary destinations to cure a timeout.
The output priority is reserved for review; final-route changes, hook interaction,
stock firewall coexistence, UDP fragment handling and both IP families require
isolated installed nftables/kernel validation before adding an executor.

## Readback and ownership

Only complete, successful numeric scoped table output may enter the classifier.
The adapter must distinguish command failure, proven absence, namespace identity,
partial output and a table result. An empty JSON array here is unreadable, never
absence. The parser caps input at 32 KiB, rejects duplicate keys recursively,
keeps Serde's depth limit, and accepts only the fixed complete object/rule order
after removing positive handles and optional schema-1 metadata. Unknown flags,
extra rules, a changed hook/priority/policy or a missing terminal rule cannot
produce verified policy. This intentionally narrow vocabulary may reject a
semantically equivalent nft version's output until that version is reviewed.
The installed nft 1.1.7 result removes redundant leading `meta nfproto`
predicates from the six IP-specific maintenance rules. The classifier therefore
also compares against a second fixed full-policy template with those six
predicates absent. It does not strip or normalize any incoming predicate:
remaining IP payloads, addresses, ports, hop limit, ICMP type/code, order and
actions must match exactly. Other omissions, extra accept expressions, wrong
families and unmeasured partial elision remain unrecognized.

Name, comment and policy equality do **not** establish ownership. The caller must
supply a `TrustedTableIdentity` from the future root adapter's independently
verified exclusive-creation receipt, tied to boot ID, network namespace inode
and table handle. No receipt, different epoch/namespace or changed handle yields
Foreign even when every rule matches. A matching receipt with changed policy
yields OwnedUnrecognized. This is a pure trust-boundary representation; its
Rust constructor is not authentication and no receipt persistence exists yet.
An unprivileged request must never supply these trusted facts.

No replace/delete renderer is provided. The root executor must solve ownership
receipt publication after an uncertain create, serialization, revalidation at
commit, reboot recovery and raced table replacement before those operations are
enabled. Handles are not durable identities across boot or namespace recreation.
Unknown creation outcome must require recovery, not adoption based on a name.

## Evidence and remaining gates

Tests use synthetic values only: independent emergency golden, bounded normal
predicates, identity rejection, malformed/duplicate/truncated input, dormant
flags, injected objects and modified/missing policy. No nft command is executed.
These tests prove renderer/parser decisions, not kernel enforcement or installed
JSON round-trip compatibility. Existing protocol and crash/fence tests remain.

Next steps are installed parse/readback fixtures and an isolated network-namespace
packet matrix, then the separately reviewed executor/root receipt service. The
complete K1 host matrix and mandatory physical NIC/suspend/boot gates remain
open. Main/RC 0.9 and the user's installed network are unchanged.

### Opt-in installed nft round-trip harness

`crates/omavless-netguard/tests/nft_namespace.rs` supplies an ignored integration
gate. Ordinary workspace tests exercise only its pure refusal/response checks
and private scratch-file behavior; they do not invoke nft or unshare.

Inside a disposable development VM with `/usr/bin/nft` and `/usr/bin/unshare`:

```sh
OMAVLESS_K1_NFT_VM=1 cargo test -p omavless-netguard --locked \
  --test nft_namespace nft_json_roundtrip_in_disposable_vm -- --ignored --exact
```

The parent pins its network namespace with an open file descriptor, then launches
a child user+network namespace, maps only the current user to namespace root and
clears ambient environment. It passes a duplicated namespace descriptor as stdin
plus its recorded device/inode identity. Before every nft command, the child
fstats that retained descriptor, verifies its kernel `net:[inode]` label and exact
launch identity, proves its own namespace differs, and requires the complete
`/proc/net/dev` inventory to contain exactly loopback. No bridge, veth, physical
interface, route or uplink is created. Refused namespace creation/access is a
failed/unavailable gate; there is no sudo fallback, sysctl change or execution
in the parent namespace. The parent rechecks both its retained descriptor and its
current namespace after the child returns, including failure returns. This
avoids reliance on an immediate PPID or cross-user-namespace proc access; the
first installed attempt refused at that old isolation guard before any nft call.
The VM opt-in is an operator assertion, not automatic
virtual-machine detection; run this only in the delegated development VM.

For Emergency and Full, it checks/applies the fixed JSON, reads actual numeric
scoped table output, verifies foreign-without-receipt and same-created-object
policy, rejects duplicate creation and checks unchanged readback. Only after
successful exclusive create and verified readback does it delete that fixture
table. Test-only identity does not implement production receipt persistence.
Any earlier failure exits the namespace; it does not delete a parent table or
adopt a same-name host table. Both ignored test entry points require explicit
launch data; accidentally running all ignored tests does not run host nft.

Input/output scratch directories are exclusive mode 0700, files mode 0600;
known files are removed on normal return/unwind. SIGKILL can leave a bounded
synthetic scratch directory. Tool runs have a 15-second deadline and bounded
retained output. By default output contains only fixed progress stages and
pass/failure; raw tool stderr and ruleset bytes are not printed. A failed outer
run reports the last allowlisted stage; zero tests executed cannot count as
success. An explicit `OMAVLESS_K1_NFT_DEBUG_SYNTHETIC=1` additionally forwards one
bounded JSON record on classification mismatch: only the fixed synthetic table
just created in the proven child namespace. It never lists a parent ruleset or
forwards arbitrary child output/stderr. This supports reviewing actual kernel
normalization without weakening the classifier speculatively.

On September 29, 2026, the delegated x86_64 development VM passed the installed
round-trip with nft 1.1.7: Emergency and Full check/create/readback, independent
receipt classification, duplicate-create refusal with unchanged rules, cleanup,
and unchanged parent namespace identity. Tested harness binary SHA-256:
`e2b8bc278ee5d2a56676e7ed5987f8a58e396ab233b8408222eb3ce30e1b61dc`.
The first readback attempt correctly refused Full until its redundant-family
elision was captured and reviewed; `tests/fixtures/nft-1.1.7-full.json` retains
only that synthetic table as a regression fixture. The physical PC ran no nft.

This isolated PASS establishes JSON/kernel compatibility and create refusal,
not packet confidentiality, DHCP/IPv6 completeness, Mihomo marks, root-service
durability or the full K1 matrix. Other nft versions remain unverified. The next
packet gate needs separate isolated peer namespaces; none is added by this
harness.

A subsequent [packet-policy harness](K1_PACKET_GATE.md) instead keeps both veth
ends in the same isolated child and observes the receiver at link level. It
never creates a peer in the parent namespace; that bounded gate has its own
explicit opt-in and evidence limits.

Source basis: [upstream nft manual](https://netfilter.org/projects/nftables/manpage.html)
defines create versus add, hooks and marks; [libnftables JSON schema](https://man.archlinux.org/man/libnftables-json.5.en)
defines command/object/handle/expression shape. Both inspected September 29, 2026.
