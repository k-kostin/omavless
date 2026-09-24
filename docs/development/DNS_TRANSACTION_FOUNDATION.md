# DNS-1 offline transaction / framing foundation

Development checkpoint for #270 / #132, 2026-09-25. Based on RC
`a49ec92598a3f7c195177cb3f3ca2c1b44319749`; **not installed, not prompt-free
DNS, not a privileged helper, and not RC closure**. Stable main is unchanged.
The installed 0.9.0-rc.1 runtime retains its recorded cancellation defect.

## Executable boundary

`crates/omavless-dns-transaction` is a standalone, effect-free library. No
production crate depends on it. It has no socket listener, command invocation,
system bus, private-store reader, systemd unit, polkit rule, package install
hook or root executable. The only dependencies are already-locked serde and
serde_json. Workspace tests include it; the normal application does not.

The model makes the ordering in [the DNS contract](../roadmap/DNS_AUTHORIZATION.md)
testable before granting any new privilege:

1. Require enrolled peer, exclusive DNS writer, verified kernel lease and
   compatible fixed policy. These are **trusted adapter inputs**, not checks
   implemented by this library and never assertions accepted from IPC clients.
2. Capture original managed-link settings before the first write.
3. Set fixed DNS servers, root routing domain and DNS default-route, joining each
   completion before the next effect. Servers come first to avoid deliberately
   selecting the link before configuring its resolver; this is not an atomic
   three-property systemd transaction or a leak-protection claim.
4. Read back all three properties. Only the complete exact match produces
   `AppliedVerified`, meaning transaction-time DNS proof, **not internet health**.
5. On settled failure or cancellation, restore the captured settings and verify
   the exact snapshot before reporting `FailedRestored`. Normal explicit release
   follows the same ordering but ends in `Released`.

Cancellation during a pending write waits for its actual completion. A late
successful reply after cancellation cannot publish success; it proceeds to
restoration. An unknown outcome is different from a settled error: a timed-out
D-Bus call may still write later, so compensation must not race it. The model
blocks with `ManualRecoveryRequired` instead of assuming timeout cancelled it.
There is no artificial deadline on human authorization in this model. A real
driver still needs bounded resources and an explicit unknown-outcome policy.

Every dispatch and completion checks the lease classification. Lease loss,
unavailable proof or owner restart after any possible write blocks further
effects. Tickets reject duplicate/out-of-order/other-transaction/old-epoch
completions. They **do not cancel outstanding OS work or prove kernel identity**.
The future driver must allocate non-reused epoch/transaction identities, retain
in-flight calls and provide secure lease verification. No replay cache, crash
journal, caller authentication or mutation dispatcher is implemented here.

The in-memory fake resolved adapter independently changes three properties,
captures original settings, injects pre/post-effect failures and detects a
falsely acknowledged restoration. These tests prove ordering, not real D-Bus
behavior. Lease tests inject classifications; they are not security acceptance
of a future host adapter.

## Draft broker request vocabulary

This is an **unpublished protocol candidate**, separate from `omavless.control`
v1. Nothing is registered on the existing control socket. The future privileged
broker's installation and socket/peer policy still require their own review.

- API `omavless.dns`, version `1` (exact; no fallback).
- One UTF-8 NDJSON frame, maximum **8 KiB including LF**; exactly one final LF,
  no CRLF or multiline payload. Unary reader/socket deadlines remain unimplemented.
- Required top-level keys: `api`, `version`, `id`, `method`, `params`.
- `params` must be an object, not an array or null. Duplicate and unknown keys
  are rejected at both object levels, including escape-equivalent names.
- Structure has only two object levels and primitive leaves; no arbitrary JSON
  values, recursive containers or profile/config data are accepted.
- Request and operation IDs: 1–64 ASCII alphanumeric / hyphen / underscore.
- Broker epoch and lease references: exactly 32 lowercase hexadecimal characters.
  These references are **not bearer authorization**. A future broker must bind
  them to the authenticated enrolled peer, current epoch and actual held lease.

| Method | Exact params keys | Intended boundary, not implemented effect |
| --- | --- | --- |
| `hello` | none | Discover supported protocol/capabilities |
| `status` | none | Bounded broker status, no host/private metadata |
| `acquire` | `epoch`, `operationId` | Resolve trusted managed-TUN ownership; no client-selected target |
| `apply` | `epoch`, `operationId`, `leaseId` | Apply package-fixed policy |
| `verify` | `epoch`, `leaseId` | Read back this same held lease |
| `release` | `epoch`, `operationId`, `leaseId` | Restore captured managed-link settings |

No interface/index, DNS IP, routing domain, path, UID, PID, unit, command or
caller-selected policy is accepted. Enrollment and emergency recovery are
administrator actions, deliberately absent from this unprivileged vocabulary.
Success/capability response schemas, transport, replay admission and binding
wire references to model tickets remain future work. Current parser failures
encode only fixed bounded codes with `id: null`; raw input, IDs, tokens and JSON
parser fragments are never copied into errors. Debug formatting redacts tokens.

## Hypotheses checked without host authorization

### Existing core does not expose the required DNS ownership switch

Inspection of [Mihomo 1.19.31 config](https://github.com/MetaCubeX/mihomo/blob/ab405bad5beeeac8b003bb01f60f134f6df54471/config/config.go),
[listener config](https://github.com/MetaCubeX/mihomo/blob/ab405bad5beeeac8b003bb01f60f134f6df54471/listener/config/tun.go)
and [TUN adapter](https://github.com/MetaCubeX/mihomo/blob/ab405bad5beeeac8b003bb01f60f134f6df54471/listener/sing_tun/server.go)
confirms the raw → listener → sing-tun option chain does not carry
`EXP_DisableDNSHijack`. Empty packet `dns-hijack` is not that switch.
The inspected `Meta` ref resolves to this same commit; no newer source
capability is established by looking at that ref again.

The matching [sing-tun Linux implementation](https://github.com/MetaCubeX/sing-tun/blob/b50ae28a1409c7bce8e96e6c6966cf57d8ace754/tun_linux.go)
also rules out two tempting shortcuts:

- `auto-route: false` is not a DNS-off control: DNS setup occurs independently
  after route/rule configuration. Disabling routes would also change behavior.
- `file-descriptor` bypasses `configure` on creation, including address/route
  setup. `Close` still invokes DNS revert unless the separate disable flag is
  set. An FD alone is therefore neither exclusive DNS ownership nor a drop-in
  lifecycle-preserving solution.

These are source findings, not a compiled/custom-core or live network gate.
No core fork/wrapper, PATH interposition or package replacement was installed.

### Holding a TUN FD does not prevent name/index reuse

Opt-in reproducible probe:

```sh
python3 tests/dns_tun_namespace_probe.py
```

The ordinary-user parent runs `unshare --user --map-root-user --net`. The child
checks both namespaces differ from its parent's and netlink inventory contains
only loopback **before opening `/dev/net/tun`**. It creates one nonpersistent
test TUN, keeps its FD open, deletes that link, then creates another TUN with
the same name and explicitly requested old index. It has no host bus/runtime,
physical interfaces, routes, DNS or credentials. All objects die with the new
namespace. No sudo or authorization prompts are used.

Try Omarchy ARM64 / kernel `7.2.0-2-aarch64-ARCH` result:

| Fact | Observed |
| --- | --- |
| Original FD attached before deletion | yes |
| Link deletion succeeds with original FD open | yes |
| New TUN reuses both name and index | yes |
| Original FD remains attached after deletion/replacement | no |
| Replacement FD attached | yes |

This matches the kernel's [TUN detach/ioctl implementation](https://github.com/torvalds/linux/blob/v7.2/drivers/net/tun.c).
Retaining the real FD and checking `TUNGETIFF` detects this sequential reuse;
it does **not** make a subsequent index-addressed resolved call atomic with the
check. The probe intentionally has CAP_NET_ADMIN **only in its new namespace**.
It does not show that an ordinary unprivileged host process can delete the
real tunnel, nor prove a production exploit or secure broker implementation.

The future privileged design must state the root/CAP_NET_ADMIN threat boundary
and resolve the remaining check/use and ownership issues explicitly. Do not
silently turn this experiment into a production FD-passing protocol.

## Gates and continuation

Automatic checks: 35 transaction/fault tests, 18 request-framing/privacy tests,
10 namespace-probe guard tests with injected adapters (no namespace effects in
the default suite). Full workspace/developer gates also apply. The standalone
namespace experiment is separate actual-kernel evidence, not installed VPN smoke.

Local Try Omarchy ARM64 validation on this source checkpoint:

- `tests/run-rust.sh`: **1,155 workspace tests passed, 11 ignored**, plus the
  script's one repeated targeted TUI protocol test; format, strict clippy,
  feature checks, terminal fixture tests and parity smoke passed.
- `tests/run.sh`: **326 developer tests passed, 2 expected skips**;
  Node/QML contracts and plugin validation passed.
- Python compile, shell syntax, documentation navigation and diff whitespace
  checks passed. Dependency inversion lists only the new crate itself: no
  production consumer was introduced.
- The separate namespace probe produced the table above. Read-only installed
  observations still showed connected Routing, matching DNS properties and the
  unchanged installed binary. No host transitions or authorization attempts.

These tests found and corrected a draft parser bug: serde-derived structs
accept positional arrays, including `[]` for all-optional params. Both object
levels now explicitly require maps, with a regression for positional envelopes.
This was caught before any production registration or privileged implementation.

The next implementation prerequisites are still **not merely sudo**:

1. Review a supported core DNS-off adapter. A minimal upstream change would
   carry a default-off disable-system-DNS option through raw config, listener
   config/equality/reload, and sing-tun options, preserving packet DNS handling,
   TUN routing and teardown semantics. It needs compile/fixture evidence and
   reviewed distribution; this checkpoint neither patches nor selects a fork.
2. Establish the real managed-link lease and fixed privileged service boundary,
   with unrelated-link/reused-link/owner-loss refusal. The typed model and
   synthetic lease booleans are not that implementation.
3. Implement typed D-Bus host effects, original-value bounds, response schemas,
   serialization/replay, crash reconciliation, package enrollment/removal and
   fixed recovery. Bind native lifecycle only after these boundaries pass.
4. Then build/install an exact candidate during an attended session. Verify one
   enrollment followed by repeated connect/mode/disconnect without extra DNS
   prompts, actual readback, failed/partial setup and recovery. Negative tests
   follow [host authorization policy](../testing/HOST_AUTHORIZATION_ACCEPTANCE.md),
   not another chain of blind cancellations that can lock PAM.

Do not ask the owner to retry the unchanged installed cancellation defect just
to obtain another log. A deliberately delayed legacy prompt remains untested,
but is not needed to rediscover the already confirmed completion gap. Owner
availability tomorrow enables future installed gates, not a claim that this
offline foundation already supplies an installable passwordless helper.
