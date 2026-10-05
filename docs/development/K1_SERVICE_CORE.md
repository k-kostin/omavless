# K1 developer live-owner service core

Status: opt-in development implementation, not installed/activated or accepted
as a working product feature. This is the integrated successor to the preserved
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

## Exact source and dependency boundary

`omavless-netguard` has an explicitly selected `netguard-service-core` feature
and a fixed `omavless-netguard serve|recover` developer binary. Default workspace
and product runtime paths do not enable it. No Python production fallback,
generic command/path/namespace/mark/interface/handle input or new unsafe
application code is introduced. The workspace unsafe prohibition remains.

The complete upstream nix/libc trees, licenses, original commit/tree/archive
identities and exact modifications are retained in
[private fork provenance](../../vendor/NETGUARD_PROVENANCE.md). The only
additional unsafe call sites are the previously reviewed fixed read-only nix
patch. The paired path libc supplies its exact native `NS_GET_ID` constant.
Registry nix 0.30.1 and libc 0.2.189 remain unchanged; Cargo.lock adds only path
nix 0.31.3 and path libc 0.2.190 plus dependency disambiguation. No global
registry override or cross-copy libc structure exchange occurs. This is private
fork adoption, **not** upstream acceptance or a released safe API.

`cargo fmt` checks current workspace members. `--all` additionally formats
local dependencies, including upstream libc's different formatter policy, so
the Rust gate no longer rewrites/checks immutable external trees with the
application formatter. The full vendor diff remains exactly the two supplied
patches plus nix's paired-libc manifest pin; no formatter delta is admitted.

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
thread namespace, current thread and canonical manager view. The exact
installed systemd's opening behavior still needs source/VM evidence before
selection; literal unit text alone is not that evidence.

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
That Linux mechanism was tested historically, but this service's actual
process-death packet protection is still unmeasured. Losing socket ownership
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
no delegation/FD retention/descendants and installed-manager behavior need
actual restart/isolation evidence. Manager removal never authorizes deleting
durable `/var/lib` records or nft policy. Handled uncertainty parks with the
graph held; no watchdog or failure exit drives an automatic restart. Fatal
death and an explicit owner-selected SIGKILL remain separate boundaries.

## Narrow next VM matrix (expectations, NOT PASS)

| Exact selected scenario | Required evidence |
| --- | --- |
| Installed original canonical manager launch | original unit/binary/library hashes, actual FD0 origin, effective configuration and same current invocation; direct/nested/substituted launch refuses |
| Fresh enrolled Arm → Status | original create causality, durable Live+Armed, complete policy and same retained creator; wrong UID/frame cannot mutate |
| Actual Full VPN policy | unmarked IPv4/IPv6 and direct UDP/TCP DNS blocked in supported VM paths; fixed bypass/TUN exceptions preserved; foreign sentinel unchanged |
| Same-owner explicit Disarm and root live Recover | exact owned handle conditional delete, absent readback, Closed+Retired fence; root recovery separately identified |
| Explicit service SIGKILL while armed | policy and available packet-blocking evidence survive actual process/socket loss, no successful orphan claim |
| New installed invocation | manager retires only dead invocation IPC, no stale socket adoption; bounded recovery-required response; no policy/record mutation, no automatic adoption |

Fewer complete actual rows take priority over a new orchestration framework.
VM absence of a physical family/route is unavailable, not PASS. Product runtime
desired/core/TUN/mark coordination, packaging/provisioning/upgrade/removal,
boot/network ordering, suspend/interface changes, cold-orphan adjudication and
host/physical acceptance remain incomplete under [KILL_SWITCH](../roadmap/KILL_SWITCH.md).
This checkpoint does not advertise K1 or declare whole-feature closure.
