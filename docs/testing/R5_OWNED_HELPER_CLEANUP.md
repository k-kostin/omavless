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
