# K1 developer live-owner service core

Status: opt-in development implementation with scoped installed developer-VM
evidence at exact application source
`d3b24a364c1fdc5b07a866edb54ec89994cd1086`, not whole K1 or shipped-product
acceptance. The actual original launch, live transactions and conservative
automatic-restart refusal matrix is recorded below. Documentation checkpoints
do not change the tested application, executable or unit.

The historical `0dd01010` VM install and original Start returned zero,
but the first status exchange did not complete and the service remained parked
without control/recovery sockets. No Arm was selected. At `c2007781`, ROOT's
fresh disposable boot, installation, 47 pre-start predicates and first original
Start returned zero. The current-invocation trace refused at ORIGINAL_NAMESPACES
before creator assembly or READY; no client/Arm was selected. The new fixed
manager-descriptor ingress below addressed that launch incompatibility without
capability expansion and reached original-namespace admission and READY in the
actual `d3b24a36` VM cycle. Earlier failed/unknown receipts remain historical,
not upgraded by this successor. This is the integrated successor to the preserved
[#661 isolated create/readback checkpoint](K1_ISOLATED_CREATE_READBACK_PROPOSAL.md).
That checkpoint remains exact `858b528722480f165891ef62ae805c9dcbd22a9a`; its
private-namespace native success does not attest this new service or launch.

The owner authorized a private, immutable syscall-library adoption and a real
VM service/protection slice with conservative cold restart. No main merge,
upstream publication, marketplace/release, physical acceptance, host network
change, or automatic orphan adjudication is authorized by this source change.
ROOT alone selects VM installation/manager/effect scenarios after complete
primary and independent boundary review. Ordinary source gates follow the
[approved execution policy](EXECUTION_POLICY.md).

The separate default-off [cold-bootstrap SOURCE successor](K1_COLD_BOOT_RECONCILIATION.md)
adds only different-boot Armed/terminal-Live plus independently complete absence
→ NEW causal FullVpn ownership before notify/NM startup. It does not alter the
default exec unit, old cold classifier, same-epoch orphan refusal or historical
developer-VM results below. Its exact installed/reboot ordering is unaccepted.

## Exact source and dependency boundary

`omavless-netguard` has an explicitly selected `netguard-service-core` feature
and a fixed `omavless-netguard serve|recover` developer binary. Default workspace
and product runtime paths do not enable it. No Python production fallback,
generic command/path/namespace/mark/interface/handle input or new unsafe
application code is introduced. The workspace unsafe prohibition remains.

The complete upstream nix/libc trees, licenses, original commit/tree/archive
identities and exact modifications are retained in
[private fork provenance](../../vendor/NETGUARD_PROVENANCE.md). The only
additional unsafe call sites are the fixed read-only nix patch and the distinct
bounded inherited-descriptor ingress described in the provenance. The latter
has a separate full review obligation before activation. The paired path libc
supplies its exact native `NS_GET_ID` constant.
Registry nix 0.30.1 and libc 0.2.189 remain unchanged; Cargo.lock adds only path
nix 0.31.3 and path libc 0.2.190 plus dependency disambiguation. No global
registry override or cross-copy libc structure exchange occurs. This is private
fork adoption, **not** upstream acceptance or a released safe API.

`cargo fmt` checks current workspace members. `--all` additionally formats
local dependencies, including upstream libc's different formatter policy, so
the Rust gate no longer rewrites/checks immutable external trees with the
application formatter. The full vendor diff remains exactly the three supplied
patches plus nix's paired-libc manifest pin; no formatter delta is admitted.
The registry nix `dir` feature is enabled only by `netguard-service-core` to
inventory fixed inherited slots with its safe owned directory iterator. No new
crate version, registry patch or default product activation is introduced.

## Trusted installed launch and original resources

`launch_service_origin.rs` is the sole normal private factory. Its authority
assumption is the trusted root package/install and the original canonical
system-manager launch of the fixed installed unit—not matching unit text, root
UID, PID 1, environment, cached manager data, receipt or namespace integers.
The supported developer deployment must be independently admitted by ROOT;
the binary cannot cryptographically distinguish a hostile root or a fabricated
canonical installation. Such an invocation is outside the threat model, not
made canonical by these consistency checks.

The retained system-bus connection pins the unique manager owner and verifies
its root/PID-1 peer, actual GetUnitByPID mapping, current MainPID/ExecMainPID,
nonzero original InvocationID, cgroup, installed root-owned executable/current
exe and exact argv, original root-owned unit identity/bytes and no drop-ins.
Typed effective-unit checks fence standard streams, command list, namespace
restrictions, root/proc/mount/bind/join/PAM settings, capabilities, runtime IPC
lifecycle, no delegation/FD store/watchdog and actual current mount/user/PID
namespace identity. Bounded reply decoding rejects duplicate properties,
wrong types, oversized replies and received descriptors. Zbus's upstream
receive allocation limit remains distinct from the tighter decode limit.

The literal unit supplies `StandardInput=file:/proc/1/ns/net`. Safe stdin
duplication retains the same open file description; this is descriptor
delivery, never self-authentication. Adopted safe namespace type/ID and actual
netfilter socket namespace-cookie calls compare the original anchor, original
thread namespace, current thread and canonical manager view. Exact installed
systemd v261 opening/transfer behavior was reviewed from primary source and
exercised by ROOT's `d3b24a36` original launch below; literal unit text alone is
not that evidence. This does not attest arbitrary manager versions or
fabricated launches.

The v261 API has no readable StandardInputFile property: that name is a
transient setter, while StandardInputFileDescriptorName describes named-FD
input, not a file path. The origin therefore requires readable StandardInput
to remain `file`, the retained exact installed unit/no drop-ins, and original
FD0 namespace fences under the independently admitted manager launch. It
does not substitute an unrelated getter or claim pathname self-authentication.
The pre-start WatchdogUSec diagnostic and runtime gate are intentionally
different: v261 initializes the never-started original timeout to infinity,
then copies literal WatchdogSec=0 before starting the invocation. Only exact
unsigned zero is admitted by the activating/active runtime origin. See the
primary [property mapping](https://github.com/systemd/systemd/blob/v261/src/core/dbus-execute.c),
[timeout accessor](https://github.com/systemd/systemd/blob/v261/src/core/service.h)
and [start initialization](https://github.com/systemd/systemd/blob/v261/src/core/service.c).

### Fixed manager-delivered namespace set under minimal capabilities

The actual `c2007781` trace placed refusal inside its original-namespace block,
not a property/RPC timeout. It did not expose an errno. Primary Linux v6.18
[`proc_ns_get_link`](https://github.com/torvalds/linux/blob/v6.18/fs/proc/namespaces.c)
uses PTRACE_MODE_READ_FSCREDS, and
[`cap_ptrace_access_check`](https://github.com/torvalds/linux/blob/v6.18/security/commoncap.c)
requires the target's permitted capabilities to be a subset of the caller's
effective capabilities in the same user namespace, or CAP_SYS_PTRACE. Therefore
CAP_NET_ADMIN-only root cannot generally reopen PID1's namespace links when
PID1 retains broader capabilities. This is a structural incompatibility, not
an assertion that the trace proved a particular syscall error.

The successor keeps CAP_NET_ADMIN alone. The trusted fixed unit adds exactly
three read-only OpenFile entries for `/proc/1/ns/user`, `mnt` and `pid`, with
literal names `k1-manager-userns`, `k1-manager-mntns`, `k1-manager-pidns`.
FD0 remains the original manager-opened network namespace. Primary v261
[`exec-invoke.c`](https://github.com/systemd/systemd/blob/v261/src/core/exec-invoke.c)
collects these files before capability reduction and packs inherited descriptors
from 3; read-only flags prohibit writable/graceful/truncate/append variants.
Typed `OpenFile` a(sst) must match the exact ordered three entries/flag1, and
`ExtraFileDescriptorNames` must be empty. The unchanged retained exact unit,
installed launch and no-drop-in gates remain prerequisites; LISTEN metadata,
descriptor labels/slots or matching IDs cannot authenticate an arbitrary root.

Before bus creation, threads or other FD allocations, the private constructor
requires current LISTEN_PID, count3, the exact ordered names and exactly original
slots0–5. Its own directory inventory FD must be above slot5 and is excluded
by its retained descriptor, so a missing source cannot be filled by inspection.
The safe private ingress marks source slots0,3,4,5 CLOEXEC and duplicates each
above slot5 without adopting/closing/replacing any source. All inherited source
slots remain process-local, unexported and never closed/reused by application
code for its lifetime; owned same-OFD aliases form the retained resource graph.
Unknown/missing/extra slots, names/counts, types/IDs, flags or duplicate errors
refuse. Source CLOEXEC can remain set after partial setup failure; this ordinary
bootstrap metadata change is not a policy effect, cleanup or ownership grant.

The held user/mount/PID aliases are checked against newly opened current-thread
namespace descriptors by exact kind and full nonzero kernel ID at initial and
every original-owner recheck. The original FD0-derived network alias, current
thread namespace, held socket cookie and actual FD0 retain their original
typed equality checks. **All** later `/proc/1/ns/*` reopens are eliminated;
no PID1 path check is replaced by a manager-property equality assertion, and
no broad CAP_SYS_PTRACE grant is added. Preassembly temporary duplicate owners
can still close on ordinary refusal; after AcquiredCreator assembly the entire
manager-anchor/creator graph stays retained by the existing ManuallyDrop model.

Source controls exercise fixed metadata, inventory and namespace-equality
predicates plus one-attempt scalar fcntl error/ownership seams. They do not
execute the new ingress on inherited namespace descriptors or attest actual
installed manager opening, transfer, slot ordering or minimal-capability
admission. ROOT's separately selected exact `d3b24a36` VM launch exercised that
fixed boundary; source controls are not substituted for its original receipts
or generalized to other environments.

`HostEpoch` records the actual boot UUID, zero-extended kernel namespace ID and
namespace device/inode only as held-original consistency projections. They
cannot reconstruct a creator. The actual creator socket and safe duplicate
aliases are retained together, CLOEXEC and unexported. There is no second
matching socket, namespace transition/helper, descriptor swap, reopen repair
or retry. The acquisition latch covers pre/effect/post checks on the owning
thread. After AcquiredCreator assembly, refusals/unwind retain its graph through
ManuallyDrop; AuthoritySession likewise assembles its retained owner before
setup checks. Earlier ordinary temporary read-only/new-unowned-socket FDs and
pre-publication IPC FDs can close on setup failure: no policy effect exists at
those stages, and their closure attests no kernel ownership/recovery. This is
not a blanket promise to retain every pre-assembly temporary descriptor.

The service keeps primary Group=root for root:root durable files and admits
only literal SupplementaryGroups=omavless-netguard for owner chgrp of IPC.
Typed manager-name-list and actual kernel supplementary-group checks bind it
to the pinned unique package-group entry (optionally primary root membership,
no unrelated/duplicate groups). CAP_CHOWN is not added. PrivateUsers remains
the legacy boolean property and must be false; no version-wide support for
the separate PrivateUsersEx string property is assumed.

## Developer startup trace and historical refusal boundary

The owner selected a disposable-VM fresh-boot boundary for the historical c200 experiment,
not a service rollback or cleanup justified by the old descriptor snapshot.
Current manager/namespace/enrollment/state observations at `0dd01010` matched
the closed predicates; its six observed descriptors contained no creator
aliases or state handles. Those later facts do **not** establish historical
acquisition, syscall success, no-effect history, or safe orphan retirement.
All original failed/unknown receipts remain preserved outside Git. A VM reset
explicitly ends volatile unknown custody; it cannot claim product recovery.
No writer performs that reset, service stop, installation, start or policy action.

The successor keeps every origin/kernel/ownership/cold-state predicate. Only
the fixed developer unit's stderr changes from null to journal, with the exact
typed property expectation updated to match its retained literal unit bytes.
`startup_trace.rs` emits only `K1_DEV_STARTUP_V1` frames: finite phase/event
labels and optional compile-time numeric property IDs (the ordered literal
`PROPERTY_KEYS` catalogue, unknown internal key ID99). No caller/manager
name, value, error, PID, namespace number, profile, payload or Debug/Display
formatting is emitted. The panic hook exports only the fixed PANIC refusal,
not panic payload/location/backtrace. There is one original stderr-descriptor
write attempt per frame; missing/short/failed writes park **in place**, with
the entire currently held stack/resource graph, no unwind/exit/retry/progress.
The single syscall has no hard time bound. Complete trace delivery is never
kernel authority or proof of a completed manager action.

The phases cover entry/anchor, inherited-anchor ingress, bus owner, original files, effective unit,
actual original namespaces, creator open/assembly, locked-state open, control
publication and READY; RPC decode/unavailability/deadline and property
missing/type/decode/mismatch have fixed categories. READY follows assembled
AuthoritySession plus root recovery listener, before accepting clients. After
READY, startup frames are disabled; no new per-request tracing or privileged
diagnostic IPC is introduced. A separately selected journal observer must bind
the exact unit and **current original InvocationID**, parse only this allowlist,
and never publish raw messages or treat an incomplete trace as a grant.

The proposed activating-watchdog race was rejected by primary v261 source:
service_start assigns configured watchdog_original_usec before entering the
condition/start-pre/start/spawn path, not after activation. MainPID and
ExecMainPID are set by service_set_main_pidref before the manager returns to
its event loop; the Type=exec EOF event later completes activation. Existing
activating/active and exact unsigned-zero watchdog predicates remain unchanged;
zero/other/wrong-typed MainPID/ExecMainPID still refuse. This instrumented cycle
exposes the actor's actual failed boundary rather than assuming later external
observations explain its historical refusal.

## Live effects and conservative restart

The normal private `LiveCreator` starts with **no** created-table history. A
present table is Foreign regardless of name/shape/userdata/Live receipt. It
cannot be adopted, replaced or deleted. Fresh creation is reached only through
LockedState after its durable Pending publication under the original sole
state lock. The existing PreparedCreate holds complete absent inventory and
the same original socket lease through one exclusive atomic send, every ACK
including END, complete table/chain/rule/set/object/flowtable readback and final
original-session check under the original deadline. Only that causal result
sets the live handle. Subsequent replace/delete require that same live handle,
identity and complete current inventory, nonzero generation preconditions,
one fixed atomic batch and full resulting-policy/absence readback. Unknown
effects poison the instance; no resend, compensation, adoption or cleanup.

The wire uses owner,persist (flags 6): creator death must preserve policy.
That Linux mechanism was tested historically. ROOT also selected one actual
original-pidfd SIGKILL of this armed service and observed original task
termination, automatic startup of a new invocation, conservative cold refusal,
and continued unavailability of the narrow public SSH path. This is not an
all-packet/family/DNS or physical protection result. Losing socket ownership
does not preserve proof of application ownership for a successor.

At cold start, any Live/Pending/Armed/unsafe/incoherent durable state seals
mutations immediately and returns bounded `manual_recovery_required` errors
without promoting or retiring records. Missing/fresh or Closed+Retired may
proceed only to ordinary independently bound absence/epoch checks. A present
table still refuses. Closed generation fences are retained. No absence, boot
change or matching receipt revives a retired generation. No healthy/armed
claim is returned for an orphan; no orphan recovery guarantee is promised.

## Fixed enrolled and root administrative exchanges

The normal control socket retains the administrator enrollment and unique
package-group binding, authenticates the enrolled UID using SO_PEERCRED and
uses the existing bounded one-client status/arm/disarm protocol. Normal Disarm
still requires the user runtime's independently coordinated core-cleanup
preconditions; this service does not prove Mihomo cleanup itself.

The live emergency command is **separate**: real console TTY is a client UX
guard, server SO_PEERCRED=root is authority. The fixed literal Recover frame
contains no UID, generation or arbitrary command. Under the existing retained
lock, it reads the current generation and permits Disarm only after independent
live-owner admission. Cold/Pending/orphan state refuses, preserving fences and
kernel policy. Success prints a distinct administrative direct-connectivity
restored token, never a normal core-cleanup proof or enrolled-root impersonation.
Each accepted connection has one bounded exchange, with original-owner,
listener and enrollment gates before and after. Failed delivery is not replayed.

Control is `/run/omavless-netguard/control.sock` (root:package-group 0660,
parent 0750). Recovery is a new root-only `admin` subdirectory (0700), with
root:root `admin/recovery.sock` 0600. No existing socket or admin directory is
adopted/unlinked. The manager must create a fresh empty root-private parent for
this original invocation; an empty directory label is not freshness evidence.
The original parent/entry FDs remain pinned through publication and use.

RuntimeDirectoryPreserve=no removes **only runtime IPC**, after the previous
invocation/control-group is positively terminated. Fixed KillMode=control-group,
no delegation/FD retention/descendants and installed-manager behavior remain
separate admission conditions. The actual automatic successor reached fresh
control/recovery publication and READY; this is not an independent historical
old-cgroup-empty or orphan-cleanup proof. Manager removal never authorizes deleting
durable `/var/lib` records or nft policy. Handled uncertainty parks with the
graph held; no watchdog or failure exit drives an automatic restart. Fatal
death and an explicit owner-selected SIGKILL remain separate boundaries.

## Exact developer-VM checkpoint, 2026-10-06

Tested application source is **only**
`d3b24a364c1fdc5b07a866edb54ec89994cd1086`. The public executable is 56,326,304
bytes, SHA256 `3b484c21f0076b89a73a0ee1700f6ac686658479d28d8bf5392361fc467caa39`;
the exact 876-byte unit is SHA256
`7fc59fa8ae3915a65a0470909b7ff7d908ecca26f7c5fbfe1630e47d4368a94c`.
Guest boot was `875aff36-2a4e-4309-b80b-f0d4d1081ee1`. Full PRIMARY and
independent boundary reviews preceded ROOT's separate selections. Original
captures and finite source/scenario receipts remain outside Git; no private
profiles, credentials, provider URLs or raw private logs are retained here.

| Selected row | Actual evidence | Qualification |
| --- | --- | --- |
| Fresh deployment and canonical launch | Fresh-boot, upload, prepare/archive-both, reload and first Start originals0; all49 strict pre-start property predicates; stable current-invocation trace through inherited anchors, original namespaces, creator/state/listeners/authority and READY | Fixed installed minimal-capability developer boundary, not arbitrary-launch or hostile-root attestation |
| Fresh status | Exact stored response, completion marker and returned shell observed | Explicit original child exit was not sampled; stored success is not original-exit0 proof |
| Arm7 | Visible original phase exit0 and exact response; narrow public SSH path unavailable | Underlying SSH255 is generic unavailability, not an nft-cause or all-packet claim |
| Same-owner update7 | Exact stored armed7 response; child later known returned | Original exit remains unobserved after stale graphics; no replay. This fixed repeated Arm7 reaches reinstall/verify, not a different configuration |
| Disarm7 | Visible original phase exit0, exact response and restored public SSH marker with originalSSH0 | Owned conditional-delete/absence source path; no coordinated Mihomo/core-cleanup proof |
| Arm8 and root Recover-current | Both visible original phase exit0; armed path unavailable, recovered public marker originalSSH0 | Separate live-owner administrative recovery, not cold/orphan recovery or normal cleanup proof |
| Arm9 and final wrong UID | Both visible original phase exit0; armed path unavailable | Wrong-UID refusal was last normal live-owner row; no retry or subsequent normal mutation |
| Explicit fatal old process | One original pidfd SIGKILL, original task termination, fatal marker and explicit original phase exit0; separate historical observer original0 | Task termination is not cgroup retirement, product cleanup or successor authority |
| Retirement observation | Original retirement exit0, stable current epoch, live PID/nonempty procs and threads/NRestarts nonzero; dead/zero-PID expectations false | Correctly barred manual cold Start; the plan's no-restart expectation was mistaken, not an application failure |
| Automatic successor | Original pf_manager0 and pf_trace0; stable active/running current epoch, MainPID=ExecMainPID nonzero, NRestarts nonzero, Restart=on-failure/Result=success; all projected startup phase pairs through READY, ignored0 | Current invocation evidence, not historical creator reconstruction; literal READY alone is not protocol/kernel authority |
| Automatic cold status and Arm10 | Both explicit original phase exit0 and exact `manual_recovery_required` responses through the unchanged reserved frames | No manual cold Start selected/admitted; no successful adoption, normal mutation, replacement/delete or healthy armed claim |
| Post-fatal public path | cold9 observer wrapper0, underlying SSH known255, no exact marker | Narrow path remains unavailable after the automatic successor; no all-family/route/DNS/physical result |

Original first Start was `5420e0/session9131 -> 9a4a33`; initial READY trace
`10572b/session57139 -> 08c55f`. Stored arm7/update7/disarm7 observation was
`a426dc` original0; visible Disarm7 exit0 was `4d6414`, root Recover-current
exit0 `6f5de3`, wrong-UID exit0 `cd8d81`. The selected fatal was `382cfe`,
marker `8ff635`, explicit fatal exit0/empty bash `ba3df8`; historical fatal
observer `37cbd5/17b8e6` original0; retirement `a75acb/33a5ca` original0 with
the false admission predicates above. Final originals were pf_manager0
`d7d34a`, pf_trace0 `ae7250`, cold_status0 `18bbb4`, cold_arm10=0 `c57536`;
cold9 path sample was `b13344`. These identifiers locate retained scoped
receipts; they are not authority tokens.

### Automatic restart versus the disqualified manual-cold plan

The tested unit explicitly specifies `Restart=on-failure` and `RestartSec=2s`.
The initial49 configured-property diagnostic did not inspect Restart or
NRestarts. Its initial current-invocation trace's NRestarts0 was accurate before
the fatal selection, but cannot be reused unchanged for a successor. The later
fault plan incorrectly expected a dead service/no restart. Its retirement
diagnostic completed with false predicates and therefore did **not** admit a
manual cold Start. The actual useful continuation tested the automatic new
invocation and existing reserved cold refusal frames instead. No signal,
Stop, forced restart, compensating Disarm/Recover, retry or cleanup was selected
to manufacture the original plan's expectation.

MainPID is a live reference; systemd v261 retains ExecMainPID as historical
execution status after death. Requested zero predicates were projected
separately, not used to treat historical PID metadata as a live-task claim.
The unused named cold wrapper adds a redundant explicit outer installed-input
check: unchanged `retained_command` already begins with `installed_inputs()`.
It does not repair or allege an unchecked prior Start path.

All five CI jobs at application `d3b24a36` completed SUCCESS, including Test at
2026-10-05 20:06:39 UTC. Unchanged application/vendor/source gates are not rerun
locally for this documentation-only evidence checkpoint. CI, source tests, stored
receipts and public path samples keep their distinct scope.

## Remaining product/physical matrix (NOT PASS)

| Exact selected scenario | Required evidence |
| --- | --- |
| Unsupported/direct/nested/substituted launch | Independent negative launch matrix; measured fixed installed launch is not blanket acceptance |
| Product runtime coordination | Actual desired/core/TUN/mark and coordinated cleanup prerequisites; fixture Disarm does not prove core cleanup |
| Actual Full VPN policy | unmarked IPv4/IPv6 and direct UDP/TCP DNS blocked in supported VM paths; fixed bypass/TUN exceptions preserved; foreign sentinel unchanged |
| Broader crash/boot/network lifecycle | Core/runtime failure, armed boot and network ordering, suspend/interface changes and relevant physical packet probes |
| Packaging and orphan disposition | Provisioning/upgrades/removal and any new cold-orphan adjudication guarantee require their own scoped decisions/evidence |

Fewer complete actual rows take priority over a new orchestration framework.
VM absence of a physical family/route is unavailable, not PASS. Product runtime
desired/core/TUN/mark coordination, packaging/provisioning/upgrade/removal,
boot/network ordering, suspend/interface changes, cold-orphan adjudication and
host/physical acceptance remain incomplete under [KILL_SWITCH](../roadmap/KILL_SWITCH.md).
This checkpoint does not advertise K1 or declare whole-feature closure.
