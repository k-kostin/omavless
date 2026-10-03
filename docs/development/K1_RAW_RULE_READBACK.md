# K1 retained-socket rule readback

Successor: [complete retained-socket table inventory](K1_COMPLETE_INVENTORY.md)
adds the other object kinds without promoting readback into effect authority.
The exact evidence below remains scoped to the original rule-only head.

This candidate reads fixed `inet omavless_netguard` rules with GETRULE on the
same retained NETLINK_NETFILTER descriptor as the generation/table checks.
`LocalReadSession::inspect_rules` launches no child and accepts no table,
command, namespace, descriptor or expression input. An error poisons the session.

The one-second exchange requires a complete bounded multipart dump, correct
kernel sender/port/sequence, unchanged GETGEN before/after, unique nonzero rule
handles and exact predecessor positions within each chain. Dump interruption,
truncation, unexpected fields, partial completion, duplicate fields and resource
exhaustion refuse. Limits are 128 KiB, 32 datagrams and 64 messages; expression
containers have explicit depth/field bounds. No raw bytes enter diagnostics.

The complete ordered expression list is compared against the audited fixed raw
FullVpn and Emergency creation templates. Attribute order within a typed
expression is immaterial; expression/rule order is not. Only expected nested
containers may omit the creation-side nested flag. The kernel's explicit
bitwise MASK_XOR default is required. Unknown expression meaning cannot produce
an exact match. An independent manually encoded Emergency oracle covers the
positive parser path; malformed and changed-policy cases are separate negatives.

`ExactRulesUntrusted` means exactly that. GETRULE does not inventory empty
chains, sets, objects or flowtables, and does not prove canonical namespace or
creator ownership. The older complete JSON policy observer remains in place:
replacing it now would weaken its extra-object rejection. Both are read-only
prerequisites, not a working root service or product kill switch.

The opt-in unit fixture `raw_rules_in_disposable_vm` creates each fixed policy
in a new user/network namespace with only loopback. Its test-only creator uses
the same retained Rust socket for atomic creation, strict ACK/barrier parsing,
two raw rule reads. It also verifies that the older JSON classifier continues
to return OtherUntrusted for owner,persist table flags, which its original
creation template did not support. The parent
namespace descriptor stays pinned and is compared after child exit. Direct
child invocation without the fixture environment refuses before socket work;
the outer fixture requires `OMAVLESS_K1_RAW_RULE_VM=1`. No production executor
or mutation API is exposed. Namespace destruction is the fixture cleanup.

Run only in the explicitly delegated isolated VM:

```sh
OMAVLESS_K1_RAW_RULE_VM=1 cargo test --locked -p omavless-netguard --lib \
  kernel_observer::rule_wire::tests::raw_rules_in_disposable_vm -- --ignored --exact --nocapture
```

Exact code `a0d925e344233609914629972bc0e5b3889586b4` passed the isolated
x86_64 Omarchy Dev KVM gate on kernel 7.2.5-3-omarchy and systemd 261.2-1-arch.
Transferred test-binary SHA256:
`9abcafddbddc8929a44b18998abb0b700b7ee8833c4b0e20524b21b19ca2cda6`.
Both FullVpn and Emergency passed strict atomic-create replies, repeated raw
readback, live appended-accept detection and permanent refusal after a thread
namespace change. No sudo, guest VPN/service action or parent firewall change
occurred. The transferred binary and empty private staging directory were
removed; child namespaces were destroyed on process exit. This is isolated
mechanism evidence, not physical/canonical-host protection acceptance.

The first fixture failed because it expected the older JSON classifier to
accept owner,persist flags; that classifier correctly returns OtherUntrusted.
A second fixture correction enclosed the test-only extra-rule mutation in a
required atomic nft batch. Both failed attempts stayed inside disposable
namespaces; the final head above reran both complete policy cases successfully.

The full NetGuard crate passes 181 tests (26 ignored) including doctests, and
strict all-target crate clippy passes on the same code. Cloud CI belongs to its
own exact PR head, not automatically to later documentation commits.
Namespace safe APIs, trusted system-manager launch, structural prohibition of
namespace transitions, complete same-socket object inventory, conditional
effects, orphan recovery, root package/runtime wiring and physical acceptance
remain open. No existing evidence is promoted to canonical-host authority.

The parser's kernel ABI review uses Linux v6.18
[rule dump](https://github.com/torvalds/linux/blob/v6.18/net/netfilter/nf_tables_api.c)
and [bitwise dump](https://github.com/torvalds/linux/blob/v6.18/net/netfilter/nft_bitwise.c):
ordinary rules carry MULTI|APPEND, predecessor handles and bare nested
containers; bitwise dumps emit the operation explicitly. This source review
does not substitute for installed-kernel validation.
