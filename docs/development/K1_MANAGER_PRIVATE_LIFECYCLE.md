# K1 manager-created private lifecycle fixture

Developer-only candidate, based on #603 documentation head
`a8a82d2dbdb1ca01b9753c88dd9b5ee723c3d73c`. The actual paired filter
result on `b6ab5b6` is preserved; it is not execution evidence for this fixture.
The separately authorized VM attempts and their NONPASS outcomes are recorded
below. No installed production service, package, production caller or main/RC
change is supplied by this fixture.

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

The `manager_private_guard.py` and `manager_private_stage.py` require complete
parent-agent review and an explicitly transferred VM lease before publication or
start. The attempt below grants no authority to retry. The create-only loader admits
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

## Exact-head attempt and retained NONPASS — 2026-10-04

Tested source: `c54697a9a3ffb7af082cd33ea43b87a19da2acdd`, Draft #604,
in the x86_64 Omarchy development VM. The documentation follow-up changes no
guard, loader, unit or native source and claims no execution on its later head.
The ordinary source gate passed 578 tests / 2 existing skips plus frontend,
JS/QML checks; ten pure observer/loader/boundary tests, two pure native tests and
the full Rust gate passed. The actual native fixture is ignored by ordinary
gates. Native source and the frozen ELF above remain tied to `28bdba2`, not a
new native build at the later Python-source head. The first source-gate
generated-pycache path false-positive and its failed private log remain retained.

After independent source review and separate authorization, the original guard
exited 2: **NONPASS, native lifecycle not started**. Effective-property admission
refused before native execution. The scalar observation exposed `WatchdogUSec`
as `infinity`, rather than the guard's expected `0`; the expected
`StandardOutputFile` and `StandardErrorFile` getters were absent. These are
representation/admission failures, not proof of the requested effective I/O or
watchdog configuration. No native create/delete lifecycle or packet protection
PASS follows from publication of the fixture unit.

A separately reviewed read-only diagnosis (`8a2…`) retained those observations.
A subsequent reviewed exact-matching manager dump (`aad803…`) exited 0 but
returned a typed empty string. It supplied no effective configuration facts.
Unit garbage collection is a possible explanation only, not an established
cause or permission to reconstruct absent facts from the unit source.

A separately authorized exact known-never-started cleanup (`04e15…`) then
exited 2 in the after-baseline phase: **cleanup NONPASS**. Its retained evidence
confirms that the exact own link was unlinked, the fixture unit was not found,
and the exact probe executable was absent from the observed UID-0 and UID-1000
process inventories. That bounded removal/absence evidence is not whole-run
preservation or a successful native lifecycle. The removed link is not retained
in place; all prior staged artifacts, logs and archives remain retained.

The parent agent (Codex) compared the retained original before-baseline with
the later after-baseline offline. All categories matched except the activation
inventory's transient session change (`153` to `183`) and four address-lifetime
deltas: two decreases and two IPv6 router-advertisement lifetime increases of
21 seconds. The increases are not approved countdowns; these differences must
not be normalized into preservation PASS. There is no claim of complete
original-baseline preservation.

Private archive SHA-256 identities (raw paths, logs and contents stay outside Git):

| Capture | SHA-256 |
| --- | --- |
| Read-only diagnosis | `fc788c3bff81d8601c298e1e0a88b17de1ffb3c03c9c8116069dd86c3affa53f` |
| Exact-matching empty dump | `7a72817d1666761e0b74ab87124d07f85d7f34412d9e60efc67836ccfad3f53b` |
| Exact-inode cleanup evidence | `37d84d798446f086b47b0e43c8f4a92bd3f1284276629cdfadcabb971570d5c5` |
| Retained original before-baseline capture | `07413cb12d02c8c2532eca0813dc8e62c1e81cfca578b359a6cf1b333c5b17d4` |

Original invocation NONPASS and separate cleanup NONPASS remain independent,
immutable outcomes. Neither diagnosis nor removal repairs either invocation;
there was no automatic retry. Further research needs separate reviewed source
and authorization. No whole K1 PASS, installed protection, release, main merge,
or completion of the owning K1 contract is claimed.
