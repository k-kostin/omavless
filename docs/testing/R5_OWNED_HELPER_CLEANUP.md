# R5 owned core/helper cleanup

This checkpoint repairs a native lifecycle parity gap exposed by the
[Arch service gate](R5_ARCH_SERVICE_GATE_2026-09-08.md). Python's installed
service remains the production owner and rollback reference. No plugin bridge,
cutover entry point, privileged service or OS authorization policy is changed.

## Reference and root cause

Python stops its whole user service, including core-created helpers. The old
Rust supervisor stopped/reaped only its direct Mihomo child. The live service
gate attributed the remaining TUN to a same-service resolvectl process holding
one inherited /dev/net/tun descriptor.

Pinned upstream source explains why a single group TERM was insufficient:

- [Mihomo v1.19.30 dependencies](https://github.com/MetaCubeX/mihomo/blob/v1.19.30/go.mod)
  select sing-tun v0.4.22.
- [sing-tun Linux implementation](https://github.com/MetaCubeX/sing-tun/blob/v0.4.22/tun_linux.go)
  opens the TUN without O_CLOEXEC, starts sequential DNS setup helpers, and
  invokes a synchronous revert helper during Close before closing the TUN file.
- [The helper launcher](https://github.com/MetaCubeX/sing/blob/v0.5.7/common/shell/shell.go)
  uses exec.Command without a new process group/session. A helper may therefore
  be created after the first TERM while the parent is handling shutdown.

Known resolved/polkit prompts are normal reference behavior. Explicit disconnect
must terminate owned pending work rather than require every outstanding DNS
authorization to finish. No unrelated helper may be killed by name.

## Native ownership contract

Each owned core starts in its own process group. Readiness/status use
waitid(WEXITED | WNOHANG | WNOWAIT): observing exit does not reap the leader.
The unreaped child reserves its PID/group identity until cleanup completes.
[waitid documentation](https://man7.org/linux/man-pages/man2/waitid.2.html)
defines the retained waitable state.

Stop verifies the direct child is still waitable/alive and remains its group
leader before sending any group signal. It sends TERM to that group, reserves
the final fifth of the caller's existing timeout for repeated KILL and drain
verification, and reaps only after the leader exited and two consecutive scans
find no live group members. A helper which ignores TERM or appears during
shutdown is therefore not silently abandoned when the parent exits.

No successful reap precedes subsequent group signalling. ECHILD or an escaped
leader refuses signalling; a failed stop preserves the handle for retry/manual
recovery. Existing final controller/core/TUN checks remain authoritative. The
normal daemon itself does not stop during disconnect.

Group scans are bounded: 32,768 entries, 4,096 bytes per stat, 64 live matching
members. Only disappeared processes are ignored; malformed/unreadable records
or exceeded bounds fail closed. Parsing uses the final ')' so arbitrary comm
bytes cannot shift numeric fields. Kernel group zero is valid but unrelated;
zombies/dead tasks do not hold live resources.

## Limits, not hidden guarantees

Process groups are lifecycle grouping for the supported core/helper behavior,
not a security sandbox against the same account or arbitrary hostile descendants.
Escaped helpers are not killed by guessed PID/name. Residual TUN or uncertain
cleanup remains manual recovery. Proc enumeration is not atomic; repeated empty
observations reduce ordinary exit/fork races, not an adversarial containment
proof. Stronger containment would need a separately designed cgroup boundary.

Persistent Drop failure cannot promise cleanup: the bounded retry retains no
unlimited wait and process/service teardown remains the recovery backstop.
The existing five-second unary deadline is not extended; host gates must record
actual disconnect completion, not merely eventual systemd cleanup.

## Tests and acceptance

`tools/owned_core_helper_fixture.py` is developer-only synthetic scaffolding,
not a runtime dependency. An inherited flock models a retained kernel resource.
Cases cover helper startup, TERM-triggered helper creation, ignored TERM and
parent exit before stop/observation. Tests check the lock is actually released
before removing its file and that an unrelated process survives. A missing
waitable-child identity test proves refusal before group signalling. Fixture
cleanup has an independent marker and watchdog, without real TUN/network.

The read-only group scanner has separate malformed/bounds/disappearance tests.
Full Python parity/QML tests preserve the existing production reference.
Real service acceptance must repeat the semantic Rust CLI lifecycle and prove
one owned core/TUN, private socket/controller, no new TCP listener and empty
runtime after disconnect. Isolated synthetic tests are not private-provider,
package installation, login-autostart or production cutover evidence.

Full native package/activation gates and #178 remain open unless separately
completed. #183's configured-readiness implementation is already closed; its
remaining packaged-host evidence is not a request to reopen that implementation.
R5/R6 and full Quit are not completed by this cleanup checkpoint.

The opt-in `tests/native_service_acceptance.py` runs the actual semantic CLI
against a unique temporary user service and synthetic store. It never installs
the unit, changes the production owner, or sends provider traffic. It verifies
the daemon is unprivileged, core capabilities and process-group ownership,
private controller peer identity, requested mode, TUN and complete service
membership after disconnect. Its pure helper tests run in `tests/run.sh`.

On file-capability processes, same-user /proc FD inspection can be denied.
The tool reports PID-specific TCP attribution as NOT PROVEN in that case,
separately requiring no new host TCP/TCP6 listener and no configured TCP
controller. It never upgrades denied inspection into positive absence evidence.
Fixed command output is captured privately with a post-capture size check;
this is not a hard streaming memory bound.

## Exact local checkpoint, 2026-09-09

Implementation head: `335765e9640aa7c70dc94bd3907a0a3c13f150aa`.
Try Omarchy ARM64, Mihomo 1.19.30. Binary SHA256:
`5d9e8e0df4f9076ad5d764a16e36efe32179f1f60112519dbc43e1a3b118f171`.
Rebuilding the committed head preserved that digest.

639 Rust tests passed, four ignored; installed-Mihomo opt-in, clippy, formatting
and parity passed. 289 Python tests passed with no skips, including 17
acceptance-tool helper tests; QML/i18n contracts passed.

Three synthetic native lifecycle cycles each in global, rule and direct passed:
one owned core/TUN, actual mode through the private controller, successful
semantic disconnect, no remaining helper in the service cgroup. A strengthened
three-cycle global run also verified daemon NNP=0/CapEff=0 and core NNP=0 with
network capabilities, owned process group, 54–79 ms connect and 4077–4083 ms
disconnect. No new host TCP/TCP6 listener or TCP-controller configuration;
per-core FD attribution remained NOT PROVEN due to proc permissions.

All temporary services/core/helpers/TUN were absent afterward. An authorization
dialog outlived its requesting helper; the human confirmed it disappeared.
No password handling, policy bypass or provider traffic was performed.

## Staged package directory gate

`tests/staged_native_unit_acceptance.py` is a separate opt-in **no-connect** gate.
It runs `stage-payload.sh`, copies the staged unit into a uniquely named
runtime-only test unit and changes only the executable paths, namespaced
Directory paths and synthetic application environment. The actual user manager
creates the leaf directories; exported directory locations and mode0700 are
verified before writing test sentinels. Environment overrides alone are not
assumed to relocate systemd's provisioning paths.

With the same exact binary above, this gate passed: absent ownership means
read-only/disconnected, socket0600, configuration/state/cache persist through
stop/start, runtime directory is removed/recreated, restart remains disconnected,
service membership contains only the daemon, core/TUN remain zero. Test unit
and unique directories were cleaned up; installed units were not changed.

This is staged-payload and actual user-unit provisioning evidence, **not** a
package-manager installation/update/removal, enabled-login startup conversion,
production ownership handoff or private-profile Full VPN acceptance. No claim
that disabled startup under native ownership was tested: this case deliberately
has no ownership marker at all.

## Rebased candidate: private VLESS Full VPN gate

Harness/source head: `520818bb9dc5d793fc4186df731db6db1d173d86`, rebased onto
main `4975b6388eb4b982d662587c978439495006946e`. Try Omarchy ARM64, Mihomo
1.19.30. The tested binary was built at
`36528c532ebc24d5d9fee6fef017dcb69d3d9121`, with SHA256
`d80166888331fb278136989ab24f294b260d8cf319669f51a3001b0f7b84fdcb`.
Only the acceptance harness and its deterministic tests changed between that
build source and the harness/source head; runtime and packaged-unit files are
identical. This is an explicitly qualified runtime-equivalent binary gate,
not a claim that the binary was rebuilt at the later harness commit.

One existing private VLESS fixture was copied into the isolated test store,
preserving its protocol fields while replacing local identity and detaching
subscription links. The original private store remained untouched. No protocol
feature beyond the actual VLESS fixture is inferred and no private identity is
published here.

| Public case | Result |
| --- | --- |
| Private VLESS, temporary Full VPN/global mode | PASS |
| Bounded HTTPS probe to the generic public probe site | PASS |
| Probe explicitly bound to generated TUN, RX and TX increased | PASS |
| Unprivileged daemon; core network capabilities; owned process group | PASS |
| Exactly one owned core/TUN; responsive private Unix controller | PASS |
| Semantic connect / disconnect | PASS — 109 ms / 4069 ms |
| No new host TCP/TCP6 listener; no TCP-controller configuration | PASS |
| PID-specific TCP listener attribution | NOT PROVEN — restricted proc FD visibility |
| Final service/core/helper/TUN cleanup | PASS |

The TUN-bound probe establishes that bounded request's TUN usage, not all-host
traffic capture, DNS leak protection or physical-network behavior. The gate
uses an isolated temporary user service under the reviewed candidate policy;
it does not install the canonical package, transfer production ownership or
establish login activation. Those #178/R5 gates remain separate. The focused
acceptance-tool suite passes 26 deterministic tests with no private fixture or
network requirement.
