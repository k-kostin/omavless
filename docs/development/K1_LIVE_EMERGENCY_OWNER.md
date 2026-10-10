# K1 isolated live Emergency-policy creator experiment

This test-only continuation of [full-rule readback](K1_RULE_READBACK.md)
composes actual exclusive creation, retained creator-socket ownership and the
complete fixed **Emergency** policy in a fresh loopback-only user/network
namespace. It does not implement a production authenticator, `EffectPort`,
root service, recovery or protected connection. Full VPN policy composition
with a live creator remains a separate gate.

The developer-only Python fixture uses the existing bounded raw netlink codec;
the Rust harness embeds its source and the fixed Emergency renderer output.
Neither Python nor this harness is a production dependency. The two rules are
independently encoded as netlink expressions: loopback accept and final drop,
in the fixed output base chain at priority 300 with drop policy. A first
transaction exclusively creates `owner,persist`; a second adds the chain and
two rules. **There is an intentionally unprotected empty-table interval in this
isolated experiment.** A production arm must install complete policy atomically
before returning success; this harness is not that executor.

Before sockets or effects, the child proves a pinned parent network namespace,
a different current network namespace and loopback-only inventory. The creator
requires its `SO_NETNS_COOKIE` to equal the pinned namespace's nonzero
`NS_GET_ID`; unsupported capabilities refuse. Each observation revalidates
namespace, cookie and retained socket address. A nonzero-generation exclusive
create must succeed with all acknowledgements before the fixture retains any
creation evidence. That evidence is process-local causality, not a receipt or
matching name/handle/owner-port inference.

Verification brackets a fixed bounded `nft` numeric JSON read with generation
and exact raw table-metadata checks on the retained creator. The test-only
strict JSON verifier requires exactly the expected table, chain and ordered
rules, including the observed owner/persist flags, positive unique object
handles, and no extra properties or objects. The expected rule objects come
from the Rust renderer, whose independent golden test remains unchanged. The
production readback parser is not relaxed to accept owner metadata in this
slice. No production authority token escapes the fixture.

The integration test checks:

- another live creator's identical policy rejects exclusive creation;
- a same-handle extra rule invalidates the live proof and poisons retry;
- a foreign socket cannot delete the retained creator's table, and its failed
  transaction preserves generation and the independent sentinel;
- closing the creator invalidates proof before any further I/O;
- complete Emergency policy survives owner loss with `persist`, unchanged
  handle and absent owner, but a new creator still refuses that orphan;
- no orphan acquisition/deletion occurs; namespace teardown removes the final
  fixture and sentinel. The intermediate drift fixture is deleted by its own
  still-live creator only, as isolated test cleanup.

The parent command has a 15-second bound, exchanges and readback have one-second
bounds and output is limited to 32 KiB. Child output exposes only fixed stage
and PASS/failure categories. Ordinary tests exercise builders, strict shape
refusals and direct-invocation refusal without sockets or network operations.

Run only in the coordinated isolated VM:

```sh
OMAVLESS_K1_LIVE_OWNER_VM=1 cargo test --locked -p omavless-netguard \
  --test nft_namespace live_owner::live_emergency_owner_in_disposable_vm \
  -- --ignored --exact --nocapture
```

This is a bounded live-session experiment, not independent persistent
application identity. Canonical host launch, production safe Rust namespace
binding, nft subsystem continuity/reinitialization, actual process-kill and
kernel/filesystem publication windows, Full VPN atomic creation and conditional
effects, orphan adjudication, root service/boot integration and physical
NIC/suspend/boot acceptance remain open. No installed firewall, route, TUN,
Mihomo, credentials, package or release is changed.

The [Rust namespace API prerequisite](K1_NAMESPACE_API_PREREQUISITE.md) identifies
the missing reviewed safe syscall wrappers and the independent trusted-launch
boundary. The test's cookie equality must not bypass either requirement.
