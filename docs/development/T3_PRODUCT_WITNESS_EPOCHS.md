# T3 product witness epochs

Status: default-off SOURCE candidate based on `5cfbf2fd`; no product activation,
service/enrollment installation or VM evidence. The existing
[original-image contract](T3_IMAGE_WITNESS.md) and its exact installed-development
checkpoint remain authoritative for the unchanged path. No actual acceptance,
K1 authority or schema1 DNS consent is promoted by this successor.

## First executable boundary

The optional helper feature `product-epochs` selects a distinct class and the
explicit invocation `--product-epoch-service`. Defaults expose no helper.
Existing developer classes keep their single channel, initial thirty-second
wait and fixed UID1000. The opt-in source package adds an explicit runtime flag,
fixed root enrollment writer and inactive helper unit; default public
registration/activation remain absent.

Closed product paths are runtime `/usr/bin/omavless`, managed core
`/usr/lib/omavless-dns/mihomo`, root record
`/var/lib/omavless-image-product/runtime.enrollment`, and endpoint
`/run/omavless-image-product/control.sock`. The record is the original held
root-owned regular single-link0600 file, at most512 bytes, under retained
root-owned nonwritable ancestors. Its exact five LF fields are schema
`omavless-product-current-image-v1`, one canonical decimal nonroot UID other than
`u32::MAX`, nonzero lowercase64hex core and runtime SHA256, and final empty field.
No duplicate fields, caller UID/path/PID, controller ID or receipt is admitted.
One helper admits only the UID selected by this root record, not arbitrary users.

Socket ancestry is root-only writable but searchable by the user; exact named-UID
ACL, kernel-derived original peer and child pidfds, proc-row continuity, all four
UIDs, original parent, PID/user namespaces and fresh executable checks remain.
Client UID is locally observed only for ACL checks; the helper independently
requires the root record's UID. Wire protocol has no UID/PID/path input, fallback
or reconnect. The helper still requires root and exactly CAP_SYS_PTRACE, and
retains its own created listener rather than an inherited-credential socket.
No caps are added to native/DNS/netguard. Ordinary hidden backend allocation and
fatal-loss exclusions remain; the existing64 FD ceiling is unchanged.

## Idle, active context and terminal history

Only this product listener may wait in one-second poll slices with **no provider**
Binding, channel or request context. Each slice has a bounded
two-second original source/root/node recheck envelope. Drift refuses, never
repairs/recaptures. This is not renewal of an accepted operation's budget.
Deadlines are sampled absolute checks, not a promise to preempt a blocked kernel
filesystem operation.

Before accept/peer/child acquisition reserve one of128 nonevicting history slots.
Exactly one channel is active and borrows the SAME retained Roots. Active idle
remains thirty seconds; every request has an absolute two-second deadline.
Every accepted error, expiry, unexpected frame/credential/FD or uncertain send
ends that provider context; there is no automatic next admission after error.
Finish rechecks the same binding/current image, sends its original ACK and checks
the same deadline. All per-channel FDs drop before history is appended and a
next channel is admitted. Entry129 refuses before accept. Shared Roots/listener
are retained once and history adds no FDs. ACK is provider completion, not an
effect receipt or atomic no-exec-through-effect proof.
In particular it cannot observe the native application's counted flights. The
native final flight still exists when it receives Finish ACK. The independent
original Worker drain/owner completion below is mandatory before any NEW Bind;
helper idle or ACK is never a copied native-drain attestation.

## Same-owner caller candidate

The optional runtime feature `product-image-witness` now has a source-only
`bind_current_product_image` factory through the SAME normal current owner. Only
the explicitly compiled/selected `daemon --product-image-witness` uses it.
Ordinary daemon in that same compiled image publishes/routes no close methods.
Typed Disabled/Developer/Product registration is selection data, not authority.
Existing developer selection and fixture tests remain separate. Root/current-manager,
login, desired, TUN/broker, original core/controller and exact close-qualified
pair predicates are unchanged. Enrollment is not a boolean effect permit.

NativeHost reserves its128-entry history before original capture, consumes on
every failure, and retains each original lifetime/session identity without
eviction. Busy refuses before invalidating the prior snapshot. Token capacity
reserves a full128-row snapshot plus confirmation before acquisition; existing
opaque token and128 receipt histories do not evict. First helper Bind/Observe
still occurs detached inside that SAME Session's counted proof flight.

For an effect, only the SAME original Worker's completion can permit a later explicit snapshot:
the existing finish path proves helper Finish, definitive Closed phase and zero
counted flights; then Worker drops the entire Session/FDs and its slot BEFORE
publication. Its noncloneable private completion matches the saved cancellation
and original lifetime identity, requires no remaining reservation/poison, and is
consumed only AFTER the original owner finishes its exact receipt. An ACK,
outcome enum or copied marker alone cannot construct it. Unknown/refusal/late or
failed Finish never renews. Receipt replay performs no capture or resend.

Every normal config/start/commit/stop/discard after first use permanently revokes
this candidate factory; initial normal connection setup before first use remains
available. This first cut supports explicit subsequent operations on the SAME
original core incarnation after Closed or proven before-effect retirement, not
reconnect/upgrade adoption. Actual sequential-session evidence is still pending.

An explicit new Snapshot request may now take the old SAME snapshot/session into
an owner-installed Retiring slot. No worker/discovery may be active; old pending
confirmation/rows are consumed. Normal context and same saved session identity
are checked before transfer. Finish/fresh image/controller/qualified pair checks
run outside owner and migration locks under the unrenewed original budget.
Before any new image acquisition/RPC the gate must be BeforeEffect, unrevoked,
uncancelled and have zero flights; exactly the task's one flight may then exist.
Effect authorization/attempt, expiry, panic, cancellation, drift or failed Finish
refuses and poisons. The current FD/count and WHOLE old Session fields drop
before the sealed, noncloneable retirement result is returned.
That result retains the original absolute Session deadline, not merely the later
snapshot expiry. It is checked after whole Session Drop/before publication and
again after owner/context checks before active-clear and new capture. A delayed
positive result cannot borrow the five-second snapshot window after its original
three-second budget expired; failure leaves Retiring poisoned/occupied.

The owner rechecks that exact Retiring identity, old context/revision and original
expiry before consuming the result. Unknown/lost/late/Busy/drift results keep the
slot occupied and cannot renew. On a positive result only the SAME explicit
Snapshot request may proceed to a NEW guarded capture; old rows/tickets do not
authorize it. Receipt capacity is also checked before acquisition, including
refused confirmations that filled history without adding a host epoch. All
receipt/token/lifetime history remains nonevicting; exact replay never resends.

Current capture refuses Busy BEFORE invalidation for an active product epoch;
Snapshot Drop does not positively Finish the provider and cannot grant renewal.
Expired/revoked snapshots remain unavailable, not recreated to Finish.
Three-second snapshot,
five-second confirmation and existing effect budgets/cancellation/per-chunk/final
proofs are not widened or bypassed.

Pure controls exercise the real decoder/admission/terminal functions: nonfixed
UIDs and bounds, cross-schema refusal, idle/no-context versus active/Busy/expiry,
sticky errors and late success, two positive terminals, entry129 refusal and
nonevicting first history. They do not open a privileged listener/core or prove
actual sequential FD behavior. Independent full boundary review and concrete
caller integration precede ROOT's fresh normal-owner two-epoch VM scenario and
wrong-UID/dead/drift/expiry/upgrade tests. Default public registration, packaging,
enrollment and ARM/Nix activation remain OFF/pending.

## Explicit opt-in package / enrollment boundary

The helper's `--enroll-product UID` is root administration only: actual UID/EUID
and GID/EGID must be0. One canonical nonroot UID is explicit input, never inferred
from the caller. No path/hash/profile/receipt input is accepted. It holds and
hashes the two exact fixed root755 single-link native ELF images under the
existing five-second sampled deadline and rechecks those same inputs. It creates
only the fixed root0700 parent and root600 record via exclusive no-follow open;
whole payload/held-name identity and file+directory sync are checked. Existing
record/foreign contents/provider socket refuses. Any partial/error publication
leaves its prefix untouched for explicit ROOT reconciliation, not overwrite or
retry. There is no runtime ownership, service, DNS/network or profile mutation.
The record is read-only image enrollment, not a close permit; native qualification
and SAME-session effect fences remain mandatory on every admission.

The inactive system unit runs the fixed helper with `--product-epoch-service`
as root with CAP_SYS_PTRACE only (bounding+ambient), NOFILE64, no auto-restart
and no Install target. Genuine PID/user/network views remain; only this helper
uses mount hardening because it does not exec the capability-enabled core.
The packaged aggregate task ceiling is2, allowing systemd's temporary pre-exec
idmapping probe in addition to the executor. The synchronous helper itself has
one thread and spawns no children; actual steady TasksCurrent1/Threads1 remain
required. TasksMax2 is not a claim of a kernel task ceiling1, a new channel pool,
self-cgroup write, additional capability or context/budget renewal. Runtime
parent is root0755 and socket ACL selects the enrolled UID. Native/DNS/
netguard caps and unit policy are unchanged. Unit-active is not original binding
or helper/client completion proof. Record or runtime/core inode/hash drift
refuses; no upgrade/restart repair or stale-token adoption is provided.

Explicit inert Arch mode:

```text
build-local-package.sh BUILD_DIR RUNTIME_ELF SOURCE_SHA --product-image-witness HELPER_ELF
```

It reuses normal payload/PKGBUILD/makepkg topology, checks both native ELF shapes
and records both byte hashes under schema4 opt-in identity. It includes the fixed
helper/system unit, requires the managed DNS package, and preserves the ordinary
runtime/login user-unit bytes. No supplied ELF is executed and no compile,
install, enroll, enable or start occurs. Three-argument and RC/stable modes remain
unchanged. Prebuilt provenance is caller-supplied, not toolchain attestation or
released distribution acceptance.

Required real gate after full primary/peer review: ROOT seals product runtime/
helper/package, explicitly installs/enrolls/starts in the disposable VM, uses
standard HOME/current login/manager/original broker and genuine TUN, then proves
two explicit same-owner epochs and pre-effect cancel, selected EOF/nonselected
echo, unchanged desired/replay/FD plateau/original statuses. Wrong-UID/dead/drift/
expiry/startup/reboot/upgrade checks remain separate and pending. Old scope5 or
passive/memory callbacks are not promoted to that result.

Opt-in integration SOURCE checks:18 helper pure controls;11 selected product
runtime controls (including the preexisting nonpromotion case) and the explicit
developer unsupported-host socket control passed. The new ordinary product-build
socket control originally exposed method-specific parsing before the disabled
registration gate; the gate now rejects before parsing/admission, and both
ordinary no-advertise/no-route controls pass.12 offline package controls pass
(one root-only refusal control skipped for the ordinary test UID), including
actual inert makepkg archives with public `/usr/bin/true` as prebuilt fixture,
both hashes, opt-in-only members and unchanged user-unit bytes. Those fixtures
are not actual product binaries. Strict helper/feature/default all-target Clippy,
feature/default checks,14 static retention guards, format/shell syntax/whitespace
pass. No root enrollment writer, helper/service activation, package install or
VM scenario has executed at this successor.

New caller controls run the real owned controller/Worker/receipt functions with
memory image-provider callbacks, not a privileged helper or qualified product
package: original publication enables next admission after complete Drop; Busy
preserves the original snapshot; unknown/failed Finish refuses renewal; replay
does not write again; capacity reserves before acquisition. Real second capture
through product enrollment/class, FD plateau and genuine
package/reboot/upgrade acceptance remain unrun.

First-cut source results:16 product-feature and10 unchanged developer-feature
library controls passed; default helper check, strict all-target product and
developer Clippy, workspace formatting and whitespace checks passed. The first
compile/style failures were corrected before these results. No helper/native/VM
execution or source-to-product acceptance is claimed.

Positive caller source results:3 original-worker/admission controls and6 existing
off-lock image/cancel/drift/partial/final controls passed. Product feature check,
strict all-target product-feature and normal default Clippy, normal default
runtime check,13 static source-retention controls, formatting and whitespace
checks passed. The first new static-order guard selected an earlier spawn-failure
slot Drop; its scope was corrected to the actual worker closure before passing.
These are exact candidate source results, not actual product helper/second-epoch
or default registration acceptance.

Retirement controls additionally exercise the real take/task/return functions:
one-shot original before-effect Finish outside the lifetime gate; old confirmation
unavailability and concurrent admission Busy; refusal at the NEW fixed product
capture guard in this deliberately non-product fixture; Finish error, expiry,
late cancellation, panic and observed context drift before/after Finish stay
sealed; restored bytes cannot repair poison; receipt exhaustion admits no source
or helper acquisition. These controlled callbacks are not root service/package
or actual two-session acceptance. Read-only observation descriptors may drop on
failure as in the baseline; original core remains under its owner and no fatal
descriptor-survival/product recovery claim is introduced.

Retirement successor source results:8 epoch/retirement/capacity behavioral
controls plus6 existing image-RPC fault controls passed (the broader `product_`
filter also includes one preexisting receipt-is-not-product test). Strict
all-target feature/default Clippy, feature/default checks,14 static retention
guards and explicit formatting of the included Rust tests passed. Independent
review found that the first retirement head lost the original3s deadline in its
returned result; the opaque deadline and post-Drop/owner-consumption guards now
refuse a deterministic delayed result while snapshot expiry remains future.
Four focused retirement controls and strict feature Clippy passed on that narrow
successor. The original finding/head remains preserved, not accepted product
evidence. A new static
guard initially selected the admission constructor instead of the dispatcher;
its function boundary was corrected before passing. No old accepted helper
primitive was rerun for this prose/caller change and no product VM gate ran.

## Initial packaged service pre-exec refusal and narrow successor

ROOT installed the exact432 runtime/helper and complete three-patch LegacyMeta
pair, verified their hashes/core capabilities, and performed genuine DNS/Product
image enrollment. This did not establish a product witness epoch. The first
normal helper unit start command returned0, but systemd's original execution
failed233/RUNTIME_DIRECTORY before the helper executable: special directory
setup reported Resource temporarily unavailable. No Bind/Observe/native close
or helper listener startup is claimed, and that attempt stays NONPASS.

The installed systemd261.2 source path calls is_idmapping_supported from
setup_exec_directory before the UID0 branch, through userns_acquire_self_root /
userns_acquire / pidref_safe_fork. A TasksMax1 unit cannot accommodate executor
plus this transient probe; the error propagates as directory setup failure.
The package-only successor changes TasksMax1→2. CAP_SYS_PTRACE-only bounding /
ambient sets, NOFILE64, NNP, Restart=no, original PID/user/net views and all helper
Rust/channel/epoch logic stay unchanged. See the primary
[execution setup source](https://github.com/systemd/systemd/blob/v261.2/src/core/exec-invoke.c)
and [namespace probe source](https://github.com/systemd/systemd/blob/v261.2/src/basic/namespace-util.c).

The successor package reuses the exact432 compiled images, not a newly built
Rust head: runtime8731544/d6da91a95f37024fdc7a1fdab1cdae82f7b93ed84595e289680f7ab3cb29a0d8;
helper604760/43294cedccd858ea68402a33c0295581a45bd3a3e29d922e5f4f0471168ea877.
New package/source identity and unit hash remain separately recorded. Genuine
existing image enrollment hashes do not change or need authority-copy repair.
No override, administrator cgroup write, automatic startup retry or record edit
is supplied. ROOT must separately admit the failed pre-exec filesystem prefix
and select the new packaged unit after source/packet review; actual TasksCurrent1,
Threads1, cap0x80000, NOFILE64 and unchanged namespaces remain pending gates.

## First two-epoch attempt: final-wire fixture defect, not product PASS

ROOT installed the bounded TasksMax2 package and observed genuine helper
TasksMax2/TasksCurrent1. Supervisor3 preparation returned0. The selected first
two_closed run (original session77754) reached checkpointB: same original native
process/core/Meta observations succeeded. It then parked at checkpoint-1 and
remains NONPASS. A separately reviewed fixed capture reader returned0: the flow
contained only the exact refusal phase before; runtime stdout/stderr and flow
stderr were empty. No unobserved response bytes or provider completion are claimed.

Cause in the frozen harness: it checked only the internal retained-row DTO's
three fields, while actual RuntimeServer final dispatch additionally inserts
instanceId. A genuine successful snapshot therefore fails that local validation
before any Prepare/Confirm can be issued by this immutable flow. This proves no
targeted conditional DELETE from that caller, not no effects, zero, drain or
recovery: normal connect/snapshot had already acquired resources.

Change/check: the successor requires the complete final four-field result and
exact original server instance. A tiny DATA-only final projection function is
actually called by dispatch; its success_response encode/decode regression
checks the shared public synthetic wire fixture consumed by the harness tests.
This makes wrapper fields part of the executable contract rather than a
self-confirming handwritten internal DTO mock. It adds no permission, capture,
query, replay, lifecycle or budget behavior. Future diagnostics retain only
finite method/code/state/outcome and phase in memory, print only on failure,
and never emit response bytes, opaque tokens, profiles or exception messages.
Successful capture grammar remains unchanged.

ROOT separately selected the supported SAME owner's ordinary disconnect once;
its original CLI0 and independently parsed Off/empty profile/Meta absence were
observed. This earned genuine Off through cancellation and owned stop, not a
close retry or desired-file edit. Original77754 stayed parked/nonpass; archived
evidence and any later disposable-VM administration are not product cleanup.
Fresh source/ELF/package/enrollment provenance and boot binding are required
before a new two-epoch selection. Older scope5/c9 UI or worker evidence does not
qualify this successor. Unchanged comprehensive suites are not rerun for this
small serialization change; focused producer/harness/static/strict gates apply.
