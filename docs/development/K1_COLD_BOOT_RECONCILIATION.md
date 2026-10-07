# K1 opt-in cold bootstrap: bounded design

Status: opt-in SOURCE implementation awaiting independent review, on `dev/k1-cold-boot-reconciliation`
from the Native57 evidence checkpoint. It does not change the frozen Native57
source/ELF/pins or repair the preserved old Armed192/stale-Live image. No VM,
physical/default/product/whole-K1 result is claimed. ROOT and a separate T4
reviewer approved the design direction and must review the complete affected
source before VM. No installed or reboot result is claimed by source controls.

## Narrow new producer, not cold adoption

Add an explicit default-off `netguard-cold-bootstrap` feature requiring the
existing service core. Keep the normal wire enum and default/private legacy
ports unchanged. A root/internal startup path owns the SAME enrollment/state
directory lock and independently acquired canonical creator before any group
control socket publication. Startup Drop/error retains the whole graph.

For trusted Armed plus terminal Live from a DIFFERENT actual boot, require
matching enrollment, stable marker/receipt, independently proven complete
current fixed-table absence, original canonical namespace/subsystem lifetime
and the qualified early network gate. Checked old operation+1 precedes writes.
Publish current-epoch PendingCreate under that original lock; call the existing
private `LiveCreator.full(None)` exclusive new-create/ACK/readback primitive;
publish Live ONLY with its same-invocation causal FullVpn identity and complete
final rechecks. Never call `replace_owned`/`delete_owned` with the old handle.

Armed intent/generation and any high-water fence stay unchanged: no marker write,
manufactured Closed, zero/reset or generation replay. Numeric handles may be
reused across boots; a different handle is not the proof. The independently new
epoch and actual exclusive create are. Pending/unknown never promotes on restart.
Do not invoke the old cold classifier after NEW ownership was genuinely earned;
the new startup result is a private one-use transfer, not a decoded Live grant.

Reuse fixed FullVpn rather than extend Emergency/schema semantics. The existing
private primitive installs/verifies its fixed firewall policy without starting
a core/TUN; it does NOT claim a connected VPN, Rule/provider traffic or physical
enforcement. Its existing policy/namespace/conditional-create guards stay exact.

## Completion matrix

Every row requires trusted storage/enrollment/current canonical originals.

| Marker / receipt / fixed table | New effect and record | Client/READY outcome |
| --- | --- | --- |
| Missing / Missing / completely absent | No mutation; verify stable absence | Fresh Disarmed readiness |
| Closed / Retired in current epoch / absent | No mutation or fence change | Verified Disarmed readiness |
| Armed / terminal Live in DIFFERENT boot / absent, early gate proven | PendingCreate(oldop+1,current epoch) → exclusive NEW FullVpn → Live(same op/new causal identity); marker unchanged | Armed firewall readiness after all guards |
| Armed / Live in same boot, including absence or policy-equal orphan | None; no restart/orphan adoption | Refuse, no client publication/READY |
| Any present unowned/foreign/unreadable table | None; no shape/handle adoption or cleanup | Refuse |
| Any pending/unsafe storage/invalid marker/enrollment or unproven epoch | None; preserve all remnants | Refuse |
| Stale Closed/Retired or unsupported cross-record combination | No implicit epoch retirement/Closed manufacture | Refuse; separate disposition remains open |
| Operation exhaustion, binding drift or missing early fence before Pending | No mutation | Refuse |
| Any uncertain Pending/create/ACK/readback/terminal/final boundary | Retain same original graph, actual Pending/publication prefixes and causal history; no retry | Park, no READY/accept |

## Real early-network failure fence

The current Type=exec plus Before/Wants network-pre sketch does not wait for
policy verification and is NOT sufficient. The opt-in bundle must include:

- A separately selected exact notify NetGuard unit, `NotifyAccess=main`, with
  READY emitted only after startup policy/state verification and retaining
  authority assembly. `TimeoutStartSec=infinity`, no watchdog and `Restart=no`
  avoid a timer kill/retry of an uncertain graph. Existing capability, namespace,
  inherited-anchor, package/group and no-extra-exec controls stay strict.
- A fixed opt-in NetworkManager dependency (`Requires` and `After` NetGuard),
  qualified both from pinned root-owned installed files and current effective
  manager properties. No automatic install/enable or default host-policy change.
  Missing/overridden dependency must refuse the mutating cold path.
- Actual fixed-table module/subsystem prerequisites ready through normal OS
  module loading before this unit; no module/namespace/policy guard waiver.
  The opt-in bundle includes the fixed `nf_tables` modules-load member plus an
  effective Requires/After systemd-modules-load dependency. Complete original
  kernel absence, generation/conditional-create ACK and full readback remain
  mandatory; module file presence does not waive those guards.

Only the mutating stale-Armed path requires that NetworkManager has not already
started and the original notify invocation is still activating. Normal no-effect
disarmed readiness need not be mislabeled genuine armed boot restoration.
The SAME trusted launch verifier brackets every effect; readiness is not a
serialized token or an arbitrary recipient/path/manager-epoch argument.
Dependency/timeout semantics and actual Omarchy unit ordering require source
tests plus the later real VM gate, not prose or a sleep-based assertion.

All final cold-policy/state/current-manager guards precede listener publication.
Prepare the SAME retained session/listener graph, consume the startup phase,
then send READY with no remaining fallible cold-admission step. A late bind or
notification failure can leave a named listener prefix: retain it without
accept/retry or uncertain unlink; do not assert that its pathname never existed.
Positive READY cannot be unsent. Ordinary active-session original fences remain,
but NetworkManager-not-started is not re-admitted after the phase is consumed.
The installed failure gate observes actual policy/order, not socket absence
after a post-bind failure.

## Smallest exact installed scenario, after source CLEAR

On a NEW qualified disposable image, never an edited old cold image:

1. ROOT admits the complete opted-in image/unit/dependency/module/enrollment
   bundle and genuine fresh Missing/Missing absence. No private state seeded.
2. One fixed enrolled-user request through existing checked transport performs
   normal Arm192 and returns original0 with genuine Armed192/Live operation1.
   Preserve its whole before-reboot image/evidence; no synthetic record write.
3. ROOT selects a genuine normal reboot. The new original root creator earns
   current namespace lifetime, proves fixed-table absence and re-creates FullVpn
   before NetworkManager starts; no old actor/table ownership is imported.
4. Exact readback: Armed192 marker bytes/fence unchanged, NEW current-epoch Live
   operation2 and causal table identity/FullVpn policy; actual notify/dependency
   start ordering and normal typed status. No connection/physical PASS inferred.

Source controls first cover every matrix refusal, checked exhaustion, all receipt
publication cuts, before/after exclusive-create and lost-ACK/readback uncertainty,
root marker immutability, original lock contention/unwind, no pre-proof socket/
READY and missing/wrong typed dependency/notify properties. Only those affected
paths need rechecks; existing native/profile/UI gates are not restarted.

Owning seams: `LockedState` startup before its existing request path;
`AuthoritySession`/bound effects retaining startup-before-listener transfer;
`launch_service_origin` exact opted-in unit/current manager and early fence;
`service_core` readiness after verified policy; separate source unit/dependency
members. Existing `receipt::assess` epoch refusal and old cold classifier remain
unchanged for their original callers. Closed/Retired stale-epoch disposition,
arbitrary corrupt-state emergency and fatal-loss/physical boot acceptance remain
separate, explicit gates.

## Implemented SOURCE boundary

`StartupAuthority` retains the original `LockedState` plus `BoundEffects` before
its first read/effect. Its preparation is one-attempt-only; canonical admission
must be proven, not merely return a constructor `Ok`. After exact terminal/state
and original launch rechecks it consumes the early-manager phase and transfers
that SAME graph into a sealed `AuthoritySession`. No caller can accept or recover
before the sole READY attempt succeeds. Terminal/notification uncertainty leaves
the new Live or Pending state honest and the original lock/creator retained.

The fixed opt-in unit is installed under the existing canonical service name
only in a separately admitted experiment bundle. The default exec unit and
default feature selection are unchanged. The bundle additionally installs
`NetworkManager-netguard-cold.conf` at the fixed NM drop-in leaf and
`omavless-netguard-cold-modules.conf` at the fixed modules-load leaf. No installer,
enable command, generic root IPC, unit override or production Python is added.
The trusted installed-manager verifier pins those exact root-owned bytes and
the unchanged original normal NM fragment, checks the same manager/unit path,
effective Requires/After ordering, main-only notification, infinite start timeout
and no restart. For the mutating phase it checks the SAME NG invocation still
activating/start and NM inactive/dead, PID0 and no previous exec-start timestamp.
That last condition is consumed; ordinary operations do not demand NM inactivity.

Notification uses one retained datagram descriptor connected to the fixed
root-owned manager endpoint; no caller-selected recipient or READY payload exists.
Only `READY=1` is attempted, after all preparation and listener assembly, without
a fallible post-send cold admission. Success means that send returned its exact
length, not independent installed proof that PID1 processed it or NM ordering
worked. Those remain the actual VM gate.

Source tests use synthetic canonical/kernel/manager providers and ordinary
HOME-backed file/socket fixtures; they are not canonical production authority or
real packet acceptance. The independent full-wire decoder additionally verifies
there is no conntrack-state/established or CT-mark grant: the single fixed mark
exception loads the current skb mark and the final rule drops. No conntrack
clearing, absent TUN connectivity or physical enforcement is inferred.

### Qualified package and smallest fixture plan

After complete SOURCE review, compile/seal the exact new service head with
`netguard-cold-bootstrap` (which includes `netguard-service-core`). Admit the
whole new NetGuard image and three exact unit/dependency/modules members, plus
the unchanged original normal NM fragment. Do not pair this ELF with the old
exec unit or replace only one unqualified installed member. Default package
selection and default activation remain unchanged. Use a separately owned fresh
disposable image, preserving the old Armed192/stale-Live whole image untouched.
Any pre-existing extra NM drop-ins refuse this fixed experiment; do not erase
them to manufacture the expected catalogue.

The minimum enrolled sender uses the existing explicit `netguard-client-candidate`
`FixedClient`, after ROOT independently earns UID1000/package-group and canonical
user/mount/network/current-login prerequisites. It first performs the existing
checked Status and requires fresh verified Disarmed with no closed generation;
then ONE fixed `Arm { generation: 192, mode: Full }` through the same normal
credential-checked service, requiring verified Armed192 and the original sender
exit0. There is no root impersonation, direct marker/receipt publication, new
runtime API or copied native57 fixture authority. Freeze/review that tiny fixed
SOURCE sender and outer literal preflight before any VM selection; its current
unimplemented selector is not implied by this service source.

Preserve the whole positively armed image/records, then select one genuine
normal reboot. A separately bound read-only observer compares exact unchanged
Armed192 marker bytes/metadata, current boot/epoch, Live operation2/new causal
Full policy, original service result and actual PID1 READY/NM start ordering.
One NEW checked client may query Status after readiness; it cannot reuse a
pre-reboot peer or receipt as permission. No native57 roundtrip, DNS fixture,
provider/UI result or physical PASS is borrowed. A late/missing policy proof,
terminal prefix or notification stops the scenario and preserves evidence;
no helper start, retry, disarm, reset or cleanup manufactures completion.
