# T4 retained-manager parent experiment

Status: inactive development proposal, based on exact #649
`b5b0d93293f856f417b346791c2f85ffae31494e`. No production constructor,
activation, delivery, privileged IPC or guest acceptance is introduced.

The separately approved v8 fixed-file observation recorded a returned `eacces`
at `manager_process/executable_open`. This does not identify the kernel policy,
dumpability, current state or cause. The normal CLI's refusal is correct under
the existing contract. All stopped scopes, receipts and frozen originals remain
unchanged. No capability, ptrace policy, timeout or generic inventory relaxation
is proposed.

## Concrete boundary

The experiment must integrate into the existing `Process` capture and recheck,
`StoppedOwner` namespace comparisons and complete same-UID inventory. A manager
exception is bound to one retained original proc directory, PID, start time,
four UIDs, command, comm, executable image and PID/user namespace originals.
Every consultation must reopen the current image and namespaces through that
original privileged parent, compare held and current originals, and refuse
before any reply on mismatch. Original image handles alone are historical;
PID/start, MainPID, InvocationID and configured ExecStart can survive exec and
cannot prove the current image. Other same-UID processes still use strict
ordinary capture, including unrelated nondumpable processes. No global skip,
cached absence, imported serialized identity or caller-selected PID/path is
allowed.

The first cfg(test)-only checkpoint may exercise this integration with actual
kernel `File` originals and a local retained parent. It is not an authenticated
root launcher or a production origin witness. It must remain unreachable from
the normal CLI and must not claim privileged acquisition has been established.

## Required launch and transport proof, not yet established

A future separately reviewed launcher must authenticate the direct system-bus
broker peer and PID1 unique-owner credentials with continuous original process
identity, the effective installed fixed service and executable, and the actual
manager invocation/creator before sandbox/drop. Fixed root UID, PID1, boot ID,
configuration or an inherited descriptor number alone cannot mint this proof.
This is a new privileged trust boundary, not reuse of the ordinary systemctl
configuration projection or the login callback's authority.

Only an original inherited socketpair with a retained owned child is a candidate
transport. Each request must carry kernel SCM_CREDENTIALS for that exact child
PID and post-drop UID; creator-time SO_PEERCRED is insufficient. Each reply must
carry exactly the expected SCM_RIGHTS originals, with exact ancillary types,
counts, truncation checks, bounded frame and nonce/order. No namespace path,
target PID, arbitrary command, serialized authority token or caller deadline is
accepted. Parent and child retain originals on unknown/lost/late outcomes.
One send/reply attempt is permanently sealed; no retry, signal, hidden reap or
cleanup follows uncertainty. A successful transfer still requires fresh parent
consultation whenever the child rechecks current manager state.

## Checkpoint and acceptance plan

1. Compile-time isolated concrete Process integration and negative controls:
   wrong original proc/PID/start/image/namespace, late consultation, permanently
   refused next attempt, exact inventory row routing and unchanged other rows.
2. ROOT and independent architecture review of launch authentication and the
   retained kernel transfer contract; no production availability claim.
3. Only with separate authority: bounded owned-child HOST experiment, exact
   raw-zero-only terminal/reap, and executed credential/ancillary/lost-response
   controls. No primary process, network or stopped guest query.
4. Fresh source/native gates, builds, frozen originals and reviewed fresh
   identity/delivery before any ROOT-only guest attempt. Existing v8 evidence
   cannot be borrowed as this experiment's acceptance.

The prototype does not establish canonical namespace origin, product rollback
availability, normal CLI admission, broad network baseline or security-scan
approval. Engineering review is distinct from the previously blocked scan.

## Initial concrete source checkpoint

`restore_abort_retained_parent_prototype.rs` is included only under `cfg(test)`.
Its private `LocalParent` captures an accessible original with the existing
strict `Process` flow. Current-process tests use real proc directory, image and
namespace `File` originals, not imported descriptor numbers. This local seam
does not authenticate a root parent, bus peer, launcher or recovery child.

The existing `Process` capture/recheck consumes fresh image/name bundles only
when explicitly supplied this test-only parent. Existing `StoppedOwner` manager
capture and both namespace comparisons have the same isolated seam. Its full
inventory still requires the complete PID set, ordinary metadata/status/start
classification, second sweep and final rechecks; only an exact original
manager row routes through the parent. A wrong row falls back to strict ordinary
capture, not an omission. Normal constructors always supply no parent.

Six focused controls passed on the initial source checkpoint (final focused
terminal `4d6ce7`): actual capture/recheck/namespace originals, closed namespace
names, exact inventory row routing, wrong PID/start/proc/image/namespace, late
entry and permanent refusal. They use only the current test process and fixed
public HOST source objects; no child, root acquisition, unit query, transport,
network mutation or guest action. This is not an executed full inventory or
full `StoppedOwner` admission. Exact initial checkpoint
`1f419fbcf081560f9c8cddba9c8b38664016165c` subsequently passed FULL source
639/2 declared skips plus JS/QML/navigation (`f034d0`), strict runtime/test
Clippy (`ca7d84`) and FULL Rust (`03629b`:1199 passed,44 ignored, one isolated
filter, followed by the separate isolated/parity gates). ROOT and independent
FULL source review cleared only this inactive local-original seam.

The parent reuses the existing strict Process observations and their sampled
budget checks; this checkpoint is not a hard syscall-cancellation or every-IO
deadline guarantee. A failed consultation preserves the parent's historical
originals for its retained owner lifetime; it does not yet implement transport
quarantine of every partially acquired fresh FD. Canonical launch and actual
strict SCM_CREDENTIALS/SCM_RIGHTS transfer remain deliberately unimplemented.
Neither these tests nor normal binary compilation can upgrade that missing
authority.

## Separate owned-child smoke proposal: stopped NONPASS

The new cfg(test)-only kernel-smoke module uses a fixed inherited unprivileged
HOST socketpair, one fixed request/reply and three actual `File` originals.
It distinguishes kernel per-message child credentials from creator-time
SO_PEERCRED, retains its small owner graph until test-process exit, and only
an exact owned nonreaping zero could authorize one raw-zero reap. It does not
authenticate a root launcher, supply a transport parent to `Process`, execute
the complete inventory or admit `StoppedOwner`. Child and parent sampled
limits are not hard cancellation or a shared child-lifetime guarantee.
Inner Process budgets are clipped to each caller's original eight-second end;
capture/recheck/namespace entry gates cannot renew that grant. Existing inner
Process IO checks remain sampled, not an every-syscall enforcement claim.

The first ONE owned-child attempt returned NONPASS (`cb968a`): nine tests,
seven passed, one failed at the parent's request-receive result, one ignored.
No returned errno, child status, underlying cause or completed exchange was
established. No child query, signal, reap, retry or cleanup followed refusal.
That positive smoke is now ignored; a corrected fresh child attempt requires
a new immutable source checkpoint and exact ROOT/independent review first.
The captures remain separate from the successful initial seam gates.

A separate NEW pair with no child passed (`30f75d`) and projected only
`OWNED_LOCAL_PAIR_ADDRESS_PRESENT=false`. It did not access the failed pair or
child and does not establish why the earlier attempt refused. Pure visible
frame-shape controls reject wrong/missing/duplicate credentials, extra bytes,
rights mismatch and truncation; they cannot observe hidden ancillary headers.
After adding the clipped-budget control, the bounded local module passed
`ed5b5b`:nine passed, two ignored (failed positive child and fixed child worker).
This ran only the six original current-process controls, two pure controls
and a NEW no-child pair, never the failed child or its retained resources.
FULL source/Rust evidence above remains attached only to `1f419fbc`, not this
successor smoke proposal.

Independent review of `e80f73c` identified a newly cloned endpoint outside its
retained owner at the post-clone gate and a plain returned bundle before partial
unwraps. The successor retains the cloned endpoint before that gate and the
returned bundle plus incrementally built file vector before extraction. This
does not solve inherited partial acquisition inside `Process` or `LocalParent`;
the opening comment now states that limitation instead of claiming all originals.

A second NEW no-child pair enables SO_PASSCRED on BOTH endpoints, as the failed
child proposal did. Its finite projection was address-present true (`cabe00`),
where the receiver-only probe was false. This demonstrates a HOST counterexample
to assuming an original credential-enabled pair always returns no address.
It is not the failed child's returned error or underlying cause. Unrelated
owner-return edits occurred during that compile, so its binary is not attributed
to the final successor source. A fresh unchanged focused gate subsequently
passed `e51227`:ten passed and the same two child entry points ignored.
The actual child remains ignored and the receive refusal criterion unchanged;
any future accepted address policy requires exact original-socket proof and
independent review, not pathname/address-based authority or a blanket bypass.

Pinned rustix1.1.5 exposes safely owned SCM_RIGHTS, but silently skips unknown
ancillary kinds and accepts oversized credential payloads. Draining all
visible rights before post-receive refusal preserves those originals; it does
NOT discharge the exact closed-frame contract. No application RawFd adoption,
O_PATH reconstruction, second authority copy or dependency substitution is
introduced. The next separately scoped experiment is an external review-only
safe-library patch with non-lossy shape/error reporting and owned-rights
retention, retaining upstream licenses and testing malformed/truncated/drop
ownership. No product dependency change or upstream submission is authorized.

## External known-ABI research checkpoint

The exact external source `faa069b43224c8b284954c781b28c1e1b041a873` now has
a [pinned textual patch and retained licenses](../../tests/research/rustix-owned-ancillary/README.md)
for cross-machine source review, not a dependency substitution or runnable
product fixture. The source adds a Linux-only owned receive frame, ordered
unknown/malformed header records and exact credential-size checking while
preserving legacy drain behavior. Known SCM_RIGHTS originals remain owned in
the returned frame; protocol refusal must retain that whole frame.

After separate FULL ROOT and independent source/recipe review, ROOT alone ran
ONE exact ignored no-child linux_raw control, authoritative KNOWN_ZERO
`c8b96a`: one passed, zero failed, 47 filtered out. Only three newly owned
local pairs were checked for credentials/SCM_RIGHTS, actual truncation flags,
CLOEXEC and original pipe aliases. No second backend, child retry, guest,
root-manager authentication or product owner executed. Its original private
captures and recipe hashes are recorded in the research fixture; raw stderr
remains unread and outside Git.

The earlier stopped child remains NONPASS and untouched. This external result
does not establish its cause, the protected guest manager's current identity,
SCM_PIDFD, unknown descriptor ABI, corrupt-tail/OOM/backend-partial retention
or production availability. The pinned patch is exactly the tested head,
not later external documentation or a future PIDFD successor. No ordinary
Rust source, dependency, trust predicate or CLI path changes in this checkpoint.

## Separate PIDFD successor and narrow actual result

External `dedb209c3b5b5df4b62703bc3766dcaea1f4e152` is preserved as a
separate textual research patch in the same fixture. It adds exact successful
SCM_PIDFD ownership and typed negative-errno records, plus a default-ignored
own no-child receive control with exact native-size SO_PASSPIDFD option helpers
in both backends. Eighteen synthetic tests pass on each backend; both actual
kernel entry points remained ignored in those inert runs. The earlier
source-only checkpoint cannot borrow `faa069b`'s result.

Its three own unnamed pairs check PIDFD-only, combined credentials/
one original pipe RIGHT/PIDFD, and control truncation. Whole owner graphs are
retained before post-call gates. CLOEXEC and one zero-time poll borrow only the
returned original; no PIDFD read, PID reconstruction, namespace acquisition,
signal/reap, child, old scope or VM action is added. Poll is empirical behavior,
not descriptor-class, process-liveness, root authentication or manager proof.
After FULL ROOT and independent immutable source/recipe review, ROOT alone
selected that exact clean `dedb209` linux_raw ignored entry once. Authoritative
selection `77b12e` ended KNOWN_ZERO: one passed, zero failed, zero ignored,
54 filtered out. The [research receipt](../../tests/research/rustix-owned-ancillary/README.md)
pins the fresh recipe, private capture sizes/hashes and unchanged textual patch.
No retry, second backend, child, VM, configuration or product execution occurred.
Only own-pair PIDFD-only, combined credentials/one original pipe RIGHT/PIDFD
and credential-capacity CTRUNC/no-PIDFD behavior were checked. Libc actual
receive remains untested; neither poll nor this result proves class/process/
manager authority or broader retained-original ownership.
All broader ownership/authority gaps and
the original stopped child NONPASS remain unchanged; no normal runtime,
dependency, CLI, accepted candidate or publication path changes.

## Separate preallocated successor: bounded inert evidence

External `1971b705d65b721f3319de48bbd76f75de0ca72a` has a separate
[textual research export and exact receipt](../../tests/research/rustix-owned-ancillary/README.md).
It fallibly reserves all bounded control/record/known-owner storage before
receive, then consumes the prepared context around one borrowed original socket.
Successful bounded copy/decode does not grow heap vectors; semantic refusal
retains all later well-bounded known owners and structural failure keeps only
the known prefix without guessing a tail. Whole-frame caller retention remains
mandatory. The unprepared entry still allocates as before.

After FULL ROOT and independent source/recipe review, ROOT alone selected seven
inert Rust controls at that exact clean head: `4837ca` KNOWN_ZERO, seven passed,
zero failed, zero ignored, 45 filtered out. They check capacities/pointers,
deterministic preparation cuts, flags and synthetic boundaries/ownership;
synthetic PIDFD slots use `/dev/null` originals, not class/identity proof.
The actual receive method compiled but was NOT called; no kernel control,
child, VM, native product or privileged action ran. Older dedb source/recipes,
actual receipts and stopped scopes remain unchanged.

The narrow successful-decode allocation guarantee does not recover descriptors
installed but unreported on backend error, future unknown descriptor kinds or
malformed structural tails. It is not universal OOM/panic/process-death safety,
hard cancellation, current-kernel attestation, manager authentication or normal
owner adoption. No product Cargo/API/dependency or CLI path changes. Any actual
receive control requires its own fixed source/recipe review and authorization;
this inert result cannot inherit either older own-pair kernel result.

## Prepared own-pair successor: narrow actual composition

External `0a0c958cd823b5be70abd38cf8def36b1554a089` is preserved in a
separate eleven-file [textual export and exact receipt](../../tests/research/rustix-owned-ancillary/README.md).
The prepared receiver is unchanged apart from the isolated default-ignored
control module. ROOT's separately reviewed compile/inert gate `029d9d` /
`059988` passed seven tests, zero failed, zero ignored, 56 filtered out; the
new ignored body compiled without being selected.

After distinct FULL ROOT and independent source/immutable-recipe review, ROOT
alone selected that new linux_raw own-pair control once: `30990f` KNOWN_ZERO,
safe post-zero `9fe085`, one passed, zero failed, zero ignored, 62 filtered out.
All reservations precede acquisition of one fresh nonblocking unnamed pair
and two pipes. One prepared receive checks exact credentials/two RIGHTS/PIDFD
shape, original option queries, CLOEXEC, zero-time poll, pipe aliases and
unchanged returned vector pointers/capacities. Whole owners are retained before
post-call gates. No PIDFD read, child, old scope, VM, configuration, product
binary, retry or second backend ran; author did not select Cargo. Older exact
source heads, patches, receipts and permanently stopped scopes remain intact.

This composes bounded successful receive/decode only. It does not authenticate
a manager/launcher, integrate transport into `LocalParent`, prove descriptor
class or close backend-installed-but-unreported error ownership, unknown-FD,
malformed-tail or broader OOM/panic/process-death gaps. No product dependency,
API or ordinary CLI changes, actual libc receive, adoption or publication are
implied. The sampled deadline remains distinct from hard cancellation.

## Prepared LOCAL reply: separate private composition evidence

The [inert source export and exact evidence](../../tests/research/prepared-local-reply/README.md)
preserve private `68f618c6c601d3fb9145cb90e585af1779f64eb9` with the separate
prepared-rustix shim `0a0c958`. The textual two-file Rust patch, exact private
resolver lock and non-executable relocated driver template are not applied by
normal tooling. Ordinary runtime source/manifests/lock/API remain unchanged;
the relocated template is uncompiled and cannot inherit private evidence.

The slice-typed heterogeneous-array regression fixes an intrinsic pure-test
type mismatch; it is not a proven cause of the earlier stopped compile.
ROOT's fresh exact private compile ended zero, then separately reviewed eight
pure entries and one ignored current-process own-pair composition ended zero.
The latter retains the whole prepared frame before checking exact credentials/
three RIGHTS, nonce/name, borrowed-original executable/namespace equality and
unchanged capacities. It does not authenticate a root parent or supply a normal
manager transport. Original received descriptors never become authority.

Internal acquisition/unwind, backend installed-but-unreported FD errors,
unknown future classes, malformed tails, broader OOM/panic and root/child
authentication remain gaps; sampled deadlines are not hard cancellation.
Stopped scopes and older evidence stay intact. Exact public 9ac CI remains
nongreen at a separate DNS-broker singleton metadata assertion; private slice
results are not whole public Rust acceptance, T4 closure or release authority.
