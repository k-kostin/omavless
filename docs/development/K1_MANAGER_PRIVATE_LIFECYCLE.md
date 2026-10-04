# K1 manager-created private lifecycle fixture

Source-only developer candidate, based on #603 documentation head
`a8a82d2dbdb1ca01b9753c88dd9b5ee723c3d73c`. The actual paired filter
result on `b6ab5b6` is preserved; it is not execution evidence for this fixture.
No VM, installed service, package, production caller or main/RC change occurs.

## Native composition

`kernel_manager_private_fixture.rs` is nested under the existing cfg(test)-only
creator module. One explicitly ignored test reuses `FixtureCreator`, the
retained `LocalReadSession`, complete generation-bracketed inventory and real
`LockedState` writers. It never runs the older unshare-based test launcher.

The fixed non-installed unit asks the system manager to create an anonymous
`PrivateNetwork=yes` namespace before `RestrictNamespaces=yes` applies to the
executable and its threads. Only CAP_NET_ADMIN is in the effective/permitted/
bounding capability sets, with no inherited/ambient capabilities, supplementary
groups or CAP_SYS_ADMIN. NoNewPrivileges must actually be set. Root identity is
fixture identity, not production authority.

The manager supplies read-only FD3 from its fixed `/proc/1/ns/net`. The native
test retains it before its first file open, checks nsfs/procfs and `net:[inode]`
shape, and requires agreement with the visible PID1 namespace but disagreement
with its own retained current-thread namespace. It also requires only loopback,
exact process credential/capability facts and EPERM from setns on a verified
`/dev/null` descriptor with zero flags. No namespace descriptor is passed to
setns. These checks run **before opening any netlink socket**, then repeat at
transaction boundaries. This specifically refuses systemd's possible graceful
PrivateNetwork fallback. Effective unit properties alone cannot authorize a
write. The root launch review must establish no private PID/user view, joining
path or extra syscall filter, and inheritance of restrictions across all threads.

The inherited FD is only a negative isolation witness: this is not a safe
production namespace-FD adoption API, NS_GET_NSTYPE/ID, SO_NETNS_COOKIE proof,
canonical host provenance or nft-subsystem continuity. The existing modeled
`NamespaceObservation::Canonical` seam remains explicitly synthetic, entirely
under cfg(test), never a newly authenticated authority value.

One lifecycle performs absent inventory → atomic exclusive complete FullVpn
create under durable Pending and the held lock → complete inventory → second
socket ownership refusal → conditional same-session delete under Closed/Pending
→ absent inventory and terminal state. There are exactly two mutation attempts
on success; no replacement/retry/compensation is added. The original creator
asserts durable pending and competing-lock Busy at each effect. A second socket
cannot acquire creator causality from identical rules, port/handle or receipts.
The normal production crate/API and all older fixture artifacts are unchanged.

## Retention, launch and remaining work

All fixture state lives under the new fixed root-owned 0700
`/run/omavless-k1-manager-private-lifecycle`, outside installed NetGuard paths.
State allocation and the native receipt are exclusive; there is no cleanup.
After admission, any error or panic parks the test thread indefinitely while
the actual creator, lock and original namespace/socket FDs remain retained.
No destructor closes those retained owners on that path. The unit has no
automatic restart, start/runtime/stop deadline, ExecStop or automatic kill.
Successful completion leaves the unit active/exited through RemainAfterExit.
These intentional retention semantics require an exclusive development VM.

**Not ready for invocation:** a separately reviewed fixed outer observer/loader
must still be supplied before any publication or start. It must seal/pin the
ELF, unit, guard and root stage, reject existing evidence/unit/link/cgroup,
check complete effective properties (typed empty arrays where necessary), and
capture the existing full host baseline. It must issue only fixed
`systemctl start --no-block` and bounded observations, never a synchronous
unbounded Start wait. Unknown/nonzero/deadline stops all further queries and
effects without signals, cleanup or retry. A fully verified native success,
Closed/Retired records, absence, zero manager PIDs and exact unchanged own-unit
identity are prerequisites to explicit known-success teardown. Only then may
the observer compare its complete after-baseline and publish whole-run PASS.
Neither native receipt nor a later independent observation repairs a failed
outer invocation. The pinned old #603 guards are reusable read-side mechanisms,
not authority to execute this new unit unchanged.

The native receipt is synchronized, bounded and private; /run remains volatile.
This is not power-loss or reboot durability. Pending installed/provenance,
safe namespace API, orphan recovery, package/runtime, physical NIC/DNS/boot
gates in [KILL_SWITCH](../roadmap/KILL_SWITCH.md) remain open.

Ordinary gates run pure credential/isolation rejection and launch-source
boundary tests only. The ignored native test must never run on the primary PC,
from cargo, or without the reviewed observer and explicit VM lease.
