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

Source basis: [upstream nft manual](https://netfilter.org/projects/nftables/manpage.html)
defines create versus add, hooks and marks; [libnftables JSON schema](https://man.archlinux.org/man/libnftables-json.5.en)
defines command/object/handle/expression shape. Both inspected September 29, 2026.
