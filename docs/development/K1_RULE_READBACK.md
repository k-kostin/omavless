# K1 fixed-table full-rule observation candidate

The later [raw retained-socket reader](K1_RAW_RULE_READBACK.md) checks ordered
rule expressions on the retained descriptor. It does not yet replace this
whole-table JSON check, whose extra-object rejection remains necessary.

This inactive development slice follows the retained read-only metadata and
chain observers. `LocalReadSession::inspect_policy_shape` reads the entire
`inet omavless_netguard` table with the fixed `/usr/bin/nft` numeric JSON
command. No caller supplies a path, command, namespace, table, mark, rule or
policy. It does not create, change, delete or adopt a table, and no installed
OmaVLESS service calls it.

The reader uses the retained namespace/socket checks and GETGEN before and
after the command. It clears the child environment, uses no shell, ignores
stderr, has a one-second total deadline and reads at most 32 KiB plus one byte
from stdout. Failure, absence, truncation, timeout, generation change,
namespace/socket drift and malformed JSON refuse; a failed `nft` command is
never treated as an empty policy or table absence. A failed read poisons that
session rather than retrying an uncertain stream.

`classify_untrusted_shape` shares the strict complete readback parser with the
existing ownership-gated classifier. Its `Exact(FullVpn|Emergency)` result
requires the fixed table/chain, exact ordered rule list and no extra objects,
chains or rules. It also accepts only the separately reviewed nft 1.1.7
family-predicate elision. `OtherUntrusted` is used for any recognized but
different content; `Unreadable` and `ForeignTable` remain distinct. An exact
shape is **never ownership evidence**. Name, matching rules, handle, owner-port
value and a copied receipt cannot mint an ownership token or authorize K1
effects. The future root adapter needs independent live-session provenance
from its own exclusive create before any `EffectPort` operation.

The ordinary pure tests include a captured synthetic nft 1.1.7 readback,
extra set/chain/rule injection, partial output and wrong-table refusal. In the
delegated x86_64 Omarchy Dev VM, the opt-in nft round-trip creates only a
fixture in a new unprivileged, loopback-only user/network namespace. Both
fixed policies produced `Exact` through the new retained reader. Adding a late
extra `accept` rule to the isolated fixture produced `OtherUntrusted` before
fixture cleanup. The parent network namespace and host firewall were not
changed. This is full policy **readback**, not full K1 acceptance.

Still open: authenticating the canonical host and the live exclusive creator,
proving nft subsystem lifetime across the session, binding conditional kernel
effects to that proof, durable orphan disposition after helper loss, installed
root-service integration and the mandatory physical NIC/suspend/boot egress
matrix. Neither this result nor the VM gate can be labelled a working kill
switch or protection PASS.

A subsequent [isolated live Emergency creator experiment](K1_LIVE_EMERGENCY_OWNER.md)
combines complete Emergency-policy readback with the socket that exclusively
created it, and refuses after its loss. It does not promote this observer's
shape result to ownership or implement a production effect adapter.
