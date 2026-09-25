# Scoped native DNS authorization — #270 / #132

Decision candidate, 2026-09-24, for RC development only. **Not a passwordless
implementation or installed policy.** Main, startup Off and OS policy unchanged.
This is a required RC investigation, not permission to hide an unfinished host
gate by closing a triage issue. See [RC scope](../development/RC_090.md).

## Evidence and the actual missing boundary

The [existing attended host finding](../testing/TRY_OMARCHY_ARM64_2026-08-26.md#finding-3--repeated-authentication-prompts)
records the native OS sequence: Mihomo creates the TUN and launches resolvectl;
resolved authorizes domains, DNS-default-route and DNS servers independently.
That older capture is historical evidence, not a fresh capture of this candidate.

Read-only inspection on this ARM64 VM: kernel 7.2.0-2-aarch64-ARCH, Mihomo
1.19.31, systemd 261.2-1 and polkit 127-3. Installed `pkaction --verbose` for
`org.freedesktop.resolve1.set-domains`, `set-default-route`, `set-dns-servers`
and `revert` reports active `auth_admin_keep`, inactive/any `auth_admin`.
No policy was installed, authorization bypassed or cache reset by this audit.

The version-matched [systemd v261 source](https://github.com/systemd/systemd/blob/v261/src/resolve/resolved-link-bus.c)
passes interface metadata to these per-link authorization checks. This makes a
target filter possible; it does not authenticate an application or a particular
incarnation of an interface. An interface name, same UID, process name or user
unit name is not a sufficient privilege boundary.

[Mihomo v1.19.31](https://github.com/MetaCubeX/mihomo/blob/v1.19.31/go.mod)
uses sing-tun v0.4.24. Its [Linux DNS implementation](https://github.com/MetaCubeX/sing-tun/blob/b50ae28a1409c7bce8e96e6c6966cf57d8ace754/tun_linux.go)
runs the domain, default-route and DNS-server commands in a goroutine and
discards their return errors; teardown also discards revert failure. The
library has `EXP_DisableDNSHijack`, but the inspected Mihomo TUN options and
adapter do not expose/wire that switch. An empty `dns-hijack` list is **not**
an evidenced way to disable resolved management.

Consequences for native OmaVLESS:

- successful child/controller/TUN admission is not completed DNS authorization;
- restoring a core/controller does not prove restored DNS;
- a rejected dialog may not produce a failed core-start result;
- a helper added beside the existing DNS path could duplicate ownership and
  leave the same prompts or late writes after a nominally successful transition.

These source facts explain a missing completion signal; they do not prove the
cause of a particular provider/network outage. Current exact-candidate prompt
and cancellation observations remain separate attended gates.

The [attended RC package test](../testing/RC_090_DNS_AUTHORIZATION_2026-09-24.md)
now reproduces the completion defect: after explicitly cancelled OS requests,
the runtime still reports Connected / Full VPN while all three resolved
properties remain absent. Accepted authorization applies them, but only after
the connect reply. This is a confirmed #132 defect, not a provider inference or
a completed #270 fix. Deliberately delayed authorization remains untested.

### Three distinct waits; no user-speed requirement

The acceptance terminal's pre-action `ready` and post-action `settled` waits
have no human deadline. Before `ready`, no host action is dispatched; delaying
there cannot explain a failed connection. `settled` confirms no dialog remains,
not that a transition succeeded. Do not treat the typing speed as test evidence.

The current native core admission has its own ten-second configured-readiness
deadline; the unary test client also has a bounded transaction deadline. Neither
is an authorization completion signal. A future DNS transaction must explicitly
distinguish awaiting authorization, applying, verifying and completed/failed.
Do not hold an optimistic UI-success state while a detached OS request can still
change DNS. Cancellation/late replies must be fenced by transaction/lease identity;
increasing a timeout is not a substitute for joining the actual DNS owner.

The September 24 native XHTTP gate first returned `transition_failed_restored`,
then passed in a separately attended attempt on the unchanged installed runtime.
The human accepted all OS requests and clarified their initial delay was before
`ready`. Therefore that delay is excluded as a cause, but the exact admission
failure remains unexplained. This is not proof of a DNS timeout or a provider
defect. Original Routing/profile restoration passed; no unattended host repeats
or privileged policy changes are justified by this finding.

## Alternatives and decision

Owner clarification, September 25: target one explicit helper enrollment and
no recurring DNS prompts. A single scoped authorization per transition is an
acceptable fallback, not the preferred result. The
[reference comparison and real-core namespace experiment](../development/DNS_AUTHORIZATION_RESEARCH.md)
separate broad permission grants, avoiding needless TUN restarts and grouping
effects behind one authorization. The installed Omarchy DNS Provider command
changes global/physical-link settings and is not our VPN helper API.

| Approach | Decision | Reason |
| --- | --- | --- |
| Four resolved actions allowed for the account | Reject | Any same-user program gains those effects on unrelated links. |
| Rule matching only TUN name / caller name / user unit | Reject as app isolation | Names and user-owned units can be recreated; stale/reused links remain unsafe. |
| Root runtime, extra runtime capabilities or generic command helper | Reject | Excess privilege in a credential/network-processing process. |
| PATH replacement of resolvectl to suppress calls | Reject for initial integration | Fragile child-launch interposition, asynchronous completion and core upgrades are not a reviewed protocol. |
| NetworkManager dispatcher alone | Reject as owner | Event ordering/loss cannot make a transaction or prove cleanup. |
| Fixed-purpose packaged host broker + exclusive DNS adapter | Preferred, gated | Narrow effects and explicit completion; requires the real single-owner integration below. |

Do not silently install an experimental wrapper/core fork merely to complete
this issue. First obtain an explicit, version-tested way for Mihomo to relinquish
resolved management while retaining TUN routing and packet DNS handling. Options
are an upstream-supported configuration capability or a separately reviewed core
adapter/package. Without that prerequisite, the selected exclusive-writer broker
integration is blocked; this is not a claim that all prompt-reduction approaches
require transferring DNS ownership. A broad polkit grant can suppress prompts
but does not meet this contract. A separately owned root core service is another
architecture, not a small exception to the current user-service model.

## Preferred host contract

Coordinate the broker's package/security infrastructure with
[K0](KILL_SWITCH.md), but keep DNS and protection as separate capabilities,
permissions and state machines. DNS does not require delivering the kill switch,
and successful DNS setup is not fail-closed egress protection.

1. Explicit administrator enrollment installs a root-owned broker/service and
   enrolls one desktop UID. No root UI, user-installed executable or arbitrary
   shell/polkit command path. The marketplace plugin only explains/setup-links;
   it never silently installs privileged policy.
2. Same-user callers are not mutually trusted. Authenticate kernel peer UID,
   bound instance/lease and allowed effects. Do not claim that a compromised
   enrolled account cannot invoke allowed requests. Group membership alone is
   insufficient. Reject other UIDs, unknown versions and unexpected file rights.
3. A broker-owned lease binds boot identity, network namespace, interface index
   and kernel object identity, generation and verified core/TUN relationship.
   Index/name alone is insufficient. **Implementation must demonstrate a
   kernel-verifiable binding resistant to deletion/recreation**, not invent a
   token supplied by the unprivileged caller as proof. If existing externally
   created TUNs cannot supply that guarantee, broker-created TUN/FD handoff is a
   separate required host change; do not silently broaden initial authority.
4. Fixed semantic operations: capability/status, acquire DNS lease, apply the
   fixed managed-link DNS policy, verify, release, and administrator recovery.
   No caller-supplied interface name, IP/DNS server, routing domain, shell, path,
   unit, rule, UID or command. DNS values derive only from the validated fixed
   tunnel policy, never a provider profile. Policy updates require package review.
5. Bound strict versioned frames (8 KiB, duplicate/unknown-field rejection),
   same-user admission, per-lease serialization and generation fencing; opaque
   operation identifiers prevent duplicate effects. Public responses contain
   fixed codes/booleans, not DNS values, private interface/endpoint metadata or
   raw D-Bus errors. Root-owned state is bounded, atomic and credential-free.
6. Broker calls resolved's typed D-Bus methods directly; no shell or client
   resolvectl execution. Read each property back. Commit the DNS lease only after
   all required values match for the same link incarnation. The runtime cannot
   publish host-DNS success on controller liveness alone.
7. One DNS writer: disable and verify absence of core-owned resolved calls before
   enabling the broker path. Unknown core capability refuses opt-in, rather than
   silently combining owners or assuming no prompts means successful DNS.

## Transactions, failure and lifecycle

- Connect: create/verify owned tunnel; lease captures only managed-link prior
  settings; apply/verify fixed DNS policy; commit actual readiness. Cancellation
  before commit leaves requested state distinguishable from confirmed state.
- Partial failure: restore only the captured settings on the *same* managed link,
  verify readback, then report restored failure. If ownership changed or cleanup
  cannot be proved, report manual recovery; never overwrite another manager.
- Snapshot capture must establish original configuration semantics, not just
  effective values. Resolved's `DefaultRoute` boolean hides automatic versus
  explicit policy, and whole-link revert resets more than the three DNS fields.
  Refuse an ambiguous baseline before writes; initial broker-owned pristine-link
  scope and exact restoration need their own proof. See the
  [source-backed constraint](../development/DNS_AUTHORIZATION_RESEARCH.md#snapshot-restoration-is-not-just-three-effective-properties).
- Disconnect: release/verify managed-link DNS while identity still exists, then
  destroy the owned tunnel. Link disappearance is a distinct verified outcome;
  never run a delayed revert against a reused name/index.
- Core/runtime crash: broker detects lease death and reconciles only its own
  link. A restart cannot adopt stale root state solely on a matching user token.
- Broker crash/restart: replay the bounded journal only after checking boot and
  link identity. Unknown/malformed state refuses effects and offers explicit
  recovery, not mass reset of resolved/NetworkManager.
- Reboot: ephemeral DNS leases cannot revive by interface name. Normal startup
  stays Off unless separately configured; this does not close AUTO-1.
- Upgrade: negotiate compatible policy before switching ownership. No two DNS
  owners during rolling versions; rollback to legacy requires verified release
  of broker state first. Physical-link DHCP/DNS configuration remains untouched.
- Disable/remove/revoke: disconnect/release and verify; package removal must
  report retained incomplete state and a fixed console recovery path. Never
  remove another package's resolved rules, reset all DNS or unlock all actions.

## Implementation slices and release decision gates

| Slice / owner | Deliverable | Completion gate |
| --- | --- | --- |
| DNS-0, core/host adapter | Supported exclusive-DNS-ownership mechanism and kernel lease binding | Version-pinned positive/negative conformance; no residual core resolved calls; reused-link rejection. **Open design prerequisites above.** |
| DNS-1, host security/package | Fixed protocol, uninstalled fake D-Bus model, root package/service/recovery | Independent security review, permission checks, unknown/malformed/stale/cross-UID cases, partial failure/crash/uninstall tests. |
| DNS-2, Rust lifecycle | Typed DNS readiness/rollback integrated with native owner | Delayed/rejected/partial DNS cannot publish success; no duplicate effects or speculative mode confirmation. |
| DNS-3, installed acceptance | Explicit enrollment then repeated supported operations | Attended no-extra-prompt connect/mode/disconnect, failed/cancelled setup, recovery and exact package/frontend identities. |

These are concrete unimplemented slices, not newly finished roadmap stages.
#270 owns the investigation; #132 owns the original cancellation truthfulness
requirement. Old Python PR #135 is historical evidence, not an implementation
vehicle for the broker. Native UI can improve pending/unknown presentation
independently, but cannot turn absent DNS proof into a successful transition.

Try Omarchy is valid for ordinary broker/parser/package/lifecycle gates. Require
a physical host only for identified suspend, physical-interface or system-policy
differences, with the reason recorded. NixOS needs a declarative service/policy
adapter and its own evidence; mutable Arch provisioning does not establish it.

Before claiming prompt-free support, the negative matrix must include unrelated
interface/caller, forged lease, stale boot, reused interface, malformed/oversized
frame, concurrent mode/stop, rejected enrollment, partial D-Bus failure, broker
crash, expired auth, NetworkManager races and removal while runtime is broken.
Probe IPv4/IPv6 and ordinary/system DNS separately; external HTTPS alone proves
neither resolved restoration nor absence of DNS leakage.

**Current outcome:** reject the broad rule and speculative route workarounds;
select this gated broker direction. #270/RC host closure remains open until the
unresolved ownership mechanism and applicable acceptance are explicitly resolved.
No installed policy, prompt elimination or DNS-cancellation fix is claimed here.

September 25 no-authorization preparation: the separate
[offline Rust transaction/framing foundation](../development/DNS_TRANSACTION_FOUNDATION.md)
executes the failure/cancellation/readback contract without any production
dependency or host writer. An isolated unprivileged user+network namespace
experiment demonstrates same-name/index TUN reuse while the old FD is open;
the old FD detects detachment but does not make a resolved write atomic.
The existing core's FD path also retains teardown DNS calls. These concrete
results narrow DNS-0; they do not close its lease/ownership prerequisites or
turn DNS-1 preparation into installed prompt-free support.
The [review-only core patch](../../tests/core_dns_adapter/README.md) now supplies
compiled DNS-off/default/reload/FD evidence in isolated namespaces and refuses
the unpatched core. It advances the core-mechanism part of DNS-0, not approved
distribution, production routing evidence, secure lease or installed closure.

Further [kernel authority testing](../development/DNS_TUN_AUTHORITY.md) rejects
another shortcut: an inherited FD with no capabilities still permits owner and
persistence changes. Creator-held TUN plus a restricted consumer is the next
candidate; isolated core readiness/close passes with an ioctl filter and empty
capabilities. The experiment also denies FD export; production traffic and
external FD extraction still need their own boundary. Fixed TUN/address/route ownership requires separate review, not a
silent expansion of the DNS-only helper. No installed helper is claimed.

## Next-session boundary

Offline work can validate the fixed protocol/failure model and review the core
adapter options. It cannot prove prompt-free host operations, cancelled/late
authorization, cleanup or a secure lease on a real TUN. Do not install a broker
or a polkit rule while the owner is absent. The accepted native UI correction
in #289 supersedes Python PR #135 as code, while #132 remains open. Retain this
design candidate and its unresolved prerequisites rather than declaring the host
gate complete merely because documentation/static checks pass.
