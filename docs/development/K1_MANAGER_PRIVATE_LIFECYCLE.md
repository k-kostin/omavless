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

## Retention and source-only launch composition

All fixture state lives under the new fixed root-owned 0700
`/run/omavless-k1-manager-private-lifecycle`, outside installed NetGuard paths.
State allocation and the native receipt are exclusive; there is no cleanup.
After admission, any error or panic parks the test thread indefinitely while
the actual creator, lock and original namespace/socket FDs remain retained.
No destructor closes those retained owners on that path. The unit has no
automatic restart, start/runtime/stop deadline, ExecStop or automatic kill.
Successful completion leaves the unit active/exited through RemainAfterExit.
These intentional retention semantics require an exclusive development VM.

**Not ready for invocation:** the new `manager_private_guard.py` and
`manager_private_stage.py` require complete root review and an explicitly
transferred VM lease before publication or start. The create-only loader admits
all four fixed artifacts into memory from stable original user-owned FDs before
creating any root artifact. Its literal source directory has exactly those four
members plus the loader; no caller path, argv, environment dispatch or generic
privileged interface exists. Each root file is exclusive and synchronized.
The loader pins the guard; the guard pins the unit, original #603 read-side
query definitions and native ELF. This is an acyclic pin graph. The native ELF
is the ordinary test build of `28bdba2ee398db2e3da75a8ab11a7465ba7beef8`, SHA-256
`a3f224c7f9e0e288a2e93a64b428c65c4b03207388368cfc6e13572bc0cffa28`;
rebuilding is not permission to substitute another artifact.

The observer retains original root artifact FDs and a root evidence-directory
FD. It refuses pre-existing evidence/unit/link/cgroup, synchronizes its full
before-baseline before publication, and rejects unknown or missing effective
properties. Exact typed Service properties cover empty additional Exec lists,
EnvironmentFiles, SystemCallFilter, mount/view overlays, the sole read-only
OpenFile anchor and no extra passed descriptors. Requires permits only the two
order permutations of the exact sysinit.target/system.slice pair. Fixed scalar
properties cover credentials, capabilities, environment, no joining paths,
drop-ins or auxiliary dependencies, timeouts, kill/restart and standard I/O.
The fixed ExecStart must still have never-started metadata. This is deliberately
fail-closed on a manager version with an unfamiliar representation.

The observer reuses the hash-pinned old #603 guard's **definitions only** for
bounded owned subprocess waiting and the complete existing host/network/
installed-state baseline. It never invokes that guard's original main or runner.
It issues fixed `systemctl start --no-block`, then bounded coherent own-unit
observations, never a synchronous unbounded Start wait.
Unknown/nonzero/deadline seals this invocation permanently and stops all further
queries and effects without signals, cleanup or retry. A verified native success,
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
boundary tests only. They also execute the actual observer control flow with
mocked external observations, asserting permanent refusal and no stop/after-query
following baseline, launch, observation or evidence failure; they do not prove
real manager behavior. Loader tests exercise actual ordinary UID1000 file FDs,
wrong hashes/modes, symlinks and hard links without privileged staging.
The ignored native test must never run on the primary PC,
from cargo, or without the reviewed observer and explicit VM lease.

Primary manager representation references: systemd v261
[Service properties](https://github.com/systemd/systemd/blob/v261/src/core/dbus-service.c)
and [execution properties](https://github.com/systemd/systemd/blob/v261/src/core/dbus-execute.c).
No actual result for this new fixture is recorded here.
