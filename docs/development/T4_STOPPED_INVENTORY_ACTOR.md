# T4 charged stopped-inventory actor candidate

Status: unselected functional SOURCE successor of the separately reviewed
inert checkpoint `3560681f`, based on documentation successor `f76f6d54` of
the exact tested developer stage `d2f58377`. New explicit opt-in developer
flags implement actual fixed canonical-manager/stopped-inventory observation;
no manager query, proc acquisition, child/native body or VM action has been
selected. Compilation and memory controls are not actual acceptance. The
classic PID1/authentication/stage modes retain RLIMIT64, and the legacy
Bundle/StoppedOwner contracts are unchanged. No legacy StoppedOwner is forged.

Owning contracts: [private backup/restore](../roadmap/PRIVATE_BACKUP_RESTORE.md),
[actor service](T4_MANAGER_ACTOR_SERVICE.md), [explicit stopped-owner recovery](T4_FIRST_ABORT_CLI.md)
and [execution policy](EXECUTION_POLICY.md). The complete stage/journal developer
positive and its persistent-evidence limits remain in
[the transaction checkpoint](T4_ACTOR_TRANSACTION_LEDGER.md).

## Concrete choice, not a hidden limit waiver

The existing full inventory retains every original classified PID directory
until final strict checks, and captures another directory plus executable for
each same-UID process. A streaming list of copied PID/start/UID/inode facts
cannot be presented as the same retained-original contract. Do not skip
unrelated UIDs, release uncertain row prefixes, infer canonical origin from
PID1, or call the old local-File capture through a wrapper that loses errors.

The smallest contract-preserving reuse removes the duplicate same-UID directory:
one original directory per numeric PID, and one original executable for each
same-UID row. Classification, full Process semantics and final sweeps borrow
those exact owners. Original canonical-manager image/namespace/identity and
the proc root remain continuously held; no descriptor or proof is exported.
All positive negative-UID rows retain their directory until the whole pass,
not merely a tuple. Complete before/intermediate/final PID catalogues must
match. Existing permission, status/start/image, churn and daemon refusal remain.

| Candidate group | Bound |
| --- | ---: |
| Original PID directories (complete existing maximum) | 4096 |
| Same-UID original executable handles (worst case every row) | 4096 |
| Candidate fixed origin/query/actor/lower roles | 120 |
| Positively reusable scratch, retained on uncertainty | 8 |
| Candidate aggregate product envelope | 8320 |

8320 is a **proposal**, not an active kernel limit or proven canonical-path
total. The 120 fixed roles still need an exact functional source ledger before
any resource selection. The scratch8 is inside8320, not added afterward. There
is no heap/all-kernel-resource quota claim. If canonical/query/lower counting
exceeds120, refuse the candidate or review a new explicit envelope; never
silently borrow slots, evict evidence or accept EMFILE as admission. Keeping64
would require honest early capacity refusal on a small complete inventory,
not a normal-VM compatibility assertion. Exotic hidden kernel-reference stores
and extra custodian processes are not justified here.

## Reviewed inert owner and regression boundaries

The generic private owner reserves bounded row/name storage before any mock
acquisition. Names must be a nonempty sorted unique list of at most4096 positive
PIDs. A real integration must derive this from the original unrestricted proc
root, never a client-supplied subset. Each row acquires in exact catalogue order;
omission, repetition, changed sets and an incomplete final sweep seal forever.
Every row must have a class; SameUid must additionally have its original image.
Neither a negative class nor a memory callback is genuine proc/manager evidence.
This section records the inert356 checkpoint; the functional caller below is
a separately reviewed boundary, not evidence borrowed from these controls.

Charging happens before the backend/open. Every reported positive directory,
image or scratch resource is installed before post-deadline or subsequent
checks. An acquisition, gate, classifier, row recheck or final-pass error seals
the original owner with its reported prefix. No later callback/open, finish or
capacity recycling occurs. The tests use memory Drop counters, not actual FDs.
Fatal owner loss and unreported backend partial resources remain excluded.
The guarded owner latches Result errors only; it does not catch panic/unwind.
The functional actor keeps this owner outside its fallible operation closure,
but has no unwind or fatal-descriptor-survival claim.

The same original directory is borrowed for SameUid image acquisition and all
strict row checks, not reconstructed from metadata. A bounded scratch slot
may be released only after a fully positive operation and its final sampled
gate; any earlier/late error retains that original. Persistent row handles
never recycle. After complete exact intermediate/final sets and every final
original-row check, a separately valid finish may release the completed owner.
Ordinary completed File-close semantics are not per-FD kernel-absence proof.

Scratch is usable during acquisition and the final original sweep. It is
charged before every open and completely empty at each row completion; the
final completion checks it again. Backend implementations must route every
newly returned File through this owner and reserve private output buffers before
fallible postchecks. An immutable callback argument is not a sandbox or permission
to hide multiple opens/effects in one outer gate. No production callbacks existed
in356; the new actual backend must be reviewed separately in full.

Controls cover4096 negative rows with none skipped,4096 same-UID rows plus all
eight scratch at the exact8320 bound, before-open capacity refusal, each
directory/scratch report/gate error, only positive scratch recycling, required
same-UID image, original directory reuse, missing/changed catalogue and ordered
final-sweep refusal, and permanent no-I/O/no-finish reentry. These demonstrate
owner accounting/ordering on memory resources only, not actual StoppedOwner.

## Next functional slice and exact authority distinction

Implement one fixed developer canonical-manager admission before product effects,
using the existing genuine system-manager/user-unit predicates rather than
fake query responses or an imported PID1 witness:

1. Fix target UID through trusted invocation origin (for the first developer
   packet, explicit fixed administrator scope), never arbitrary client PID/UID.
   Retain original proc root and observer namespace handles; prove unrestricted
   proc visibility and current-name/held identity with the existing predicates.
2. Retain root-trusted installed `/usr/bin/systemctl` and
   `/usr/lib/systemd/systemd` path/image originals before checks. The fixed system
   query for `user@UID.service` must return genuine active/running, nonzero MainPID
   and zero ControlPID. Only its positively supervised original child zero plus
   EOF admits the fixed response. Query error retains its reported child/pipes/
   tool prefix and forbids another query, reap/signal or compensation.
3. Capture that actual manager through the same retained owner: all four UIDs,
   start/command/comm/current installed image and original PID/user namespace
   checks, plus continuous fresh actual unit/manager rechecks. Historical image
   FDs, start/MainPID or a private borrow alone are not current image proof.
4. Run whole unrestricted other-row inventory with no skip and exact-original
   reuse as above, require canonical runtime units inactive/dead/zero and the
   unchanged listener predicate. Original manager consultation remains in-actor;
   no caller-local Bundle or fabricated legacy StoppedOwner is constructed.
5. Consume these observations only in the original authenticated operation,
   before/after crypto and every authority effect. Product installed launcher,
   invocation/peer provenance, original migration/singleton/Off/idle/TUN/core
   lease, and full Restore/crash reconciliation remain separate obligations.

The first functional packet should complete genuine canonical-manager identity
plus complete stopped-inventory observation and normal Halt, returning only a
fixed completed developer result. This is not yet product Restore permission.
Its full fixed-role/query resource ledger, real source/artifact/packet and bounded
diagnostics require ROOT/Astra review before ROOT selects any child/native/VM.
The current positive stage's persistent tree cannot be reset via an old recipe.

SOURCE gates at this checkpoint: eight inventory memory controls passed
(`58ea29`), the complete44 actor controls passed (`5cc134`), seven unchanged
capture controls passed (`ad4fdd`), all-target feature Clippy with warnings
denied passed (`c53e8c`), and normal no-feature library check passed (`aab611`).
No proc/query/actor/native body or VM action was selected. These are not actual
row ownership, canonical manager, product-FD-limit or StoppedOwner acceptance.

## Functional canonical developer source (unselected)

`--observe-canonical-stopped` selects a distinct supervisor scenario and
`--actor-canonical` selects its private actor entry. Both inherit the same
exclusive fixed epoch reservation, original child/pidfd/private-channel
authentication, challenge/READY and permanent context revocation. Only this
explicit scenario sets its reviewed candidate hard/soft ceiling to8320 before
launch; the classic supervisor and actor keep64. The canonical actor reserves
all4096 row/metadata/name entries, fixed-owner storage and bounded read/directory
buffers before READY. It accepts only one ObserveStopped request followed by a
separately valid normal Halt. Other requests, repeated observation, missing
completion, late/wrong replies and Result errors park the original owner without
queries, signals, reap, eviction, retry or reset. Fixed completion12→13 has no
FD, PID, path, proof token or serialized authority payload.

Target UID1000 and the normal `/run/user/1000/omavless/control.sock` are fixed
trusted-administrator VM scope, not client input. Root actor identity is checked
against its fixed private entry arguments; it is not the legacy recovery CLI
self exception. Other same-UID rows still receive complete daemon-candidate
checks, including deleted image names, renamed executables and cross-network
namespace processes. Alternate runtime paths, product launcher/broker/session
origin, Off/idle/singleton/TUN/core authority and genuine Restore permission
remain unavailable. The default runtime path and canonical fallback coincide;
this mode does not guess an alternate HOME socket or claim arbitrary-path
listener absence.

The installed root/usr/bin/lib/systemd path and systemctl/systemd images are
retained before checks and rechecked by current named/held identity. Root-owned
safe executable admission uses the existing strict predicates, not an arbitrary
path or imported FD. Execution uses the original systemctl FD with fixed args,
closed environment and fixed bus addresses. Ordinary installed dynamic
loader/libraries and std process/pipe/close internals remain trusted backend
assumptions, not strong toolchain/all-FD/fatal custody attestations.

Every returned Child is installed in the outer canonical owner immediately;
its reported stdout is transferred before the post-spawn gate, and its original
pidfd is retained before post-open checks. Exclusive actor reaping and the
unchanged single-thread/SIGCHLD prerequisites exclude another waiter. Only
actual EOF and exact original WNOWAIT exit0 permit waiting/reaping that original
child. Nonzero/signaled/wrong-child/unknown/read/parser/deadline failures retain
the reported prefix and permanently forbid another query or compensation.
After original zero, strict response parse, tool recheck and final gate, the
known-positive query handles may release. Stdio null/partial constructor
resources unreported by std remain ordinary backend behavior, not owned-error
evidence. Query output and process command bytes never enter diagnostics.

There are seven finite fixed queries: initial `user@1000.service`, then two
rounds of that unit plus `omavless.service` and `omavless-runtime.service`.
The actual active/running/nonzero/ControlPID0 manager result determines the
captured process; all four UIDs, sole namespace PID, start, command/comm,
installed systemd image, original PID/user namespaces and original live pidfd
are checked. Both runtime units must genuinely be inactive/dead/zero. All query
children finish before the first frozen PID catalogue; final queries finish
before the final row sweep and final catalogue. This ordering prevents this
observer's own systemctl children from creating artificial inventory churn.
Queries remain original observations, never a substitute for manager origin.

The backend reuses the charged owner from356: one original directory per PID,
and one original executable for each same-UID row. Every row is classified and
its original status/start/image/command/comm/current name is checked. Other-UID
directories remain held through the final whole pass. Text reads use one
positively reusable retained scratch slot; pre/post current-name checks, bounded
read-at from offset0, strict parse and final gate all precede release. Any
failure retains scratch and the row prefix. Exact before/intermediate/final
numeric catalogues come from the same original unrestricted proc root, with
at most16384 charged next operations including EOF per traversal and at most4096
numeric rows; malformed numeric names, overflow, duplicate, changed or missing
rows refuse. No std read_dir duplicate proc FD or growing directory buffer is
used. Proc visibility, original observer namespaces, live manager and fixed
listener checks remain required. The complete observation is bounded by the
existing2-second/16MiB budget clipped to the authenticated5-second operation;
supervisor whole15 seconds is unchanged. These are sampled guards, not syscall
preemption, atomic global snapshots or a hostile-manager guarantee.

| Reported canonical actor roles | Maximum |
| --- | ---: |
| Installed path/image Files (root + six children) | 7 |
| Original proc root | 1 |
| Observer directory/stat/status/command/comm/image/two namespaces | 8 |
| Observer four namespace originals | 4 |
| fdinfo/mountinfo/net/unix Files | 4 |
| Actual manager directory/text/image/two namespaces | 8 |
| Two original proc-visibility text handles | 2 |
| Continuously held manager pidfd | 1 |
| Actor stdio + original channel | 4 |
| Active original query stdout + pidfd | 2 |
| Reported fixed actor maximum | 41 |
| Future lower ledger reservation (not acquired here) | 36 |
| Unallocated fixed-envelope margin | 43 |
| Reserved fixed total | 120 |

The actual fixed File owner refuses BEFORE a35th File open. Only one query is
active; its stdout/pidfd never borrow row/scratch slots. The conservative current
reported upper bound is8192 row/image +41 fixed +one active row scratch =8234,
below8320. Queries and row scratch do not overlap: the tighter query-side bound
is8233, and the row-side bound is8232. The conservative generic owner reserves
120 fixed +all8 scratch even though only one scratch is used by this backend.
Std's hidden spawn/stdio internals are not counted as reported originals; no
proven89/128 all-internal-role claim is made. No heap/kernel-reference quota or
implicit future lower integration is implied by unused margin.

The frozen b42b SOURCE diagnostics were closed public literals: nine fixed surrounding
phases, two query phases repeated exactly seven times on the complete path,
and one actor completion, at most24 actor frames. The supervisor has seven
frames. No row count, raw proc bytes, unit result, dynamic path, errno or exception
text is emitted. Phase emission failure is the same original refusal; no
secondary failure-time log/query runs. A last frame does not establish cause,
completion, ordering, current ownership or Restore authority. A separately
reviewed fresh packet will pre-scope only its exact two capture files and this
bounded vocabulary before ROOT may select it. The old stage tested head and
its packet remain unchanged.

Before any new canonical selection, ROOT reported an explicitly authorized
disposable-VM reboot/reset with new boot prefix `02c0a195`, preserving persistent
disk/profiles and deliberately discarding the known-positive T4 synthetic run
tree. This is VM administration, not the old retirement recipe, uncertain
product reset or Restore/crash-recovery acceptance. No author inspected that
scope or the new VM state. A new fixed packet still requires separate review
and ROOT selection; this note is not permission to reuse an old invocation.

Functional SOURCE gates:13 canonical/row memory controls passed (`9d9922`),
37 actor/protocol/transfer/stage memory controls passed (`54183a`), seven unchanged
capture controls passed (same `9d9922` command's separate gate), all-target feature
Clippy with warnings denied passed (`c09d9c`), and normal no-feature library check
passed (`57f66b`). Initial compile `ee7c10` failed on a private test-constant scope
and forbidden unsafe readlink; it remains NONPASS. The successor uses safe
bounded rustix1.1.5 `readlinkat_raw` and the exact private scope, with later
compilation/pure gates zero. No installed systemctl/manager/proc or actor body
was run by the author. Actual canonical observation, whole-inventory stability,
reported FD custody and product authority were then untested.

### First canonical actual: NONPASS, and scoped query successor

The exact tested SOURCE was `b42b97e507356a1ee761f1dd71b1f310a9469f13`,
compiled artifact SHA256
`eb78dd23af3af48ccbc7f61217b9e54aea53c73a9dfbbe841d522f41ca39008a`
(63724016 bytes), with the separately FULL-reviewed `QH9TOgeV` packet.
ROOT upload `88732c` returned original0; prepare `a7747c`/`30f868` returned
original0. Whole run `6db95b`/`afb109` returned original2: **NONPASS**.
Only the separately approved two-file observer `62144d`/`470e16` returned
original0. Its bounded projection reported:

| Exact capture | Bytes / literal frames | Last public phase | SHA256 |
| --- | --- | --- | --- |
| Actor | 314 / 9 | `t4_actor_before_canonical_query` | `6fad388db3a531a0eb714dc5a8f117fdb2bde493bd8a5b450e9464b9fbfdeaf4` |
| Supervisor | 122 / 5 | `t4_service_ready` | `8dd70e5ec01210a4c9bad017c2670784fbe66b0611f579b12f79a632779f56fb` |

This is compatible with the source path reaching the first user-unit query
after completed system-manager queries and manager/boundary checks. It does
not disclose the actual query errno, prove a causal explanation, establish
current retained custody, or confer StoppedOwner/Restore authority. The original
actor epoch remains permanently stopped for agent actions: no failed-tree,
capture or process inspection, retry, signal, reap, cleanup or second invocation.
The immutable old SOURCE, artifact and packet remain the tested NONPASS objects.

Independently, primary systemd v261 SOURCE establishes a definite producer
contract incompatibility in that old query design. `systemctl show` requests
BUS_MANAGER ([systemctl-show.c](https://raw.githubusercontent.com/systemd/systemd/v261/src/systemctl/systemctl-show.c));
local manager acquisition selects the direct systemd transport
([systemctl-util.c](https://raw.githubusercontent.com/systemd/systemd/v261/src/systemctl/systemctl-util.c)).
For user scope, the configured XDG runtime directory selects its
`systemd/private` socket, whose peer must have UID0 or the client's effective
UID ([bus-util.c](https://raw.githubusercontent.com/systemd/systemd/v261/src/shared/bus-util.c)).
An euid0 client against the intended UID1000 manager fails that check; the
supplied session-bus address is not a fallback for this peer-credential refusal.
This source counterexample is not proof of the retained actual failure cause
or an attestation of the guest's installed systemd version.

The successor changes only the four fixed `--user` query children to safe
`CommandExt::gid(1000).uid(1000)`. The three system-manager query children and
root actor UID/primary GID, namespaces, original tool FD, manager predicates,
query arguments/environment, captured ownership and EOF/original-zero/strict
parse requirements remain unchanged. No alternate bus, user, unit, path or
shell fallback is added. These credentials are fixed developer origin, not
client-selected product authority.

Before channel connection, READY, canonical proc captures or any queries, only the
standalone canonical actor clears its own supplementary groups using safe
`setgroups(&[])` and verifies `getgroups()` returns empty. Either failure refuses
startup; there is no fallback or host account/group-database mutation. This is
a narrow actor-local credential change, not a change to the supervisor or
classic64-FD actor. No new FD role is acquired. Linux's ordinary syscall and
std constructor behavior remain backend assumptions, not fatal/unwind custody.
Rust's [safe setter contract](https://doc.rust-lang.org/stable/std/os/unix/process/trait.CommandExt.html)
and [1.99.0 implementation](https://raw.githubusercontent.com/rust-lang/rust/1.99.0/library/std/src/sys/process/unix/unix.rs)
apply child GID, clear supplementary groups, then apply UID; the implementation
deliberately ignores group-clear EPERM. The earlier explicit actor clear/empty
verification avoids relying on that ignored error to remove inherited groups.
Child credential failure remains a spawn refusal, never a root-query retry.
Std's unreported partial spawn resources remain outside the owned-error proof.

The prospective query diagnostic ledger is fixed and one-shot:
`before_canonical_query` → `before_canonical_query_spawn` →
`canonical_query_spawned` → `canonical_query_stdout_eof` →
`canonical_query_original_zero` → `canonical_query_parsed` →
`canonical_query_completed` (all have literal `t4_actor_` prefix and LF).
Spawned is emitted only after returned Child/stdout/pidfd retention and
nonblocking setup. EOF is emitted once after original stdout read0. Original
zero requires exact WNOWAIT originalPID exit0 plus EOF, and precedes the positive
wait; parsed follows the unchanged strict record parser. Completed still follows
original wait0, parse, original tool recheck and final budget gates. These are
milestones, not authority or guarantees about a failed phase emission.

There are seven such frames for each of seven queries plus nine fixed
surrounding phases and one actor completion: at most59 actor frames (previous
24 is historical, not the new cap). The new canonical vocabulary has17 literals;
the supervisor remains seven frames. Each emitted phase uses the unchanged
shared2-second/16MiB budget clipped to operation5 seconds. Ordering/duplicate or
emission refusal seals the progress ledger and the original operation returns
refusal without secondary diagnostics. No dynamic count, raw output, errno,
exception, private path or failure-time probe is logged. A fresh packet must
admit these exact labels/bounds and a new reconciled epoch only after separate
ROOT and independent FULL review; this SOURCE successor is not VM permission.

Successor SOURCE gates:17 canonical/row memory controls passed
(`a5134c`/`eba691`),39 actor/protocol/transfer/stage memory controls passed
(`be660d`/`4e0240`), and all-target feature Clippy with warnings denied passed
(`d3e527`/`4f09ef`). The six new controls exercise fixed credential selection,
exact seven-frame/59-whole bounds, every emission-error prefix, out-of-order and
expired-phase terminal refusal, and clear-before-empty group admission/error
ordering. They do not invoke actual credential changes, proc, systemctl, child
spawn, the actor binary or VM. This is synthetic SOURCE coverage, not proof that
the changed original user-query child executes or completes in the guest.
The unchanged seven legacy capture memory controls passed (`70d753`), and the
normal no-feature library check passed (`90ddaf`). Git fetch/prune succeeded;
this candidate branch still has no remote counterpart. No push, new packet,
guest upload, reset, installed tool query or native actor invocation is selected.

### Second canonical actual: query progress, inventory NONPASS

Exact tested SOURCE `3c4942061ad8e9b32c433a3af729343d131c6418`, artifact
63789896 bytes/SHA256
`be7b3f34ef93ffba5ccc2ff6ede2d5f5d7d871d99ffe35cd4133850097b1ab3c`,
and sealed `5s10HgSg` packet were separately PRIMARY/independent FULL-cleared.
ROOT verified fresh boot `875aff36-2a4e-4309-b80b-f0d4d1081ee1` via SSH
original0 `954baa`, after explicit isolated reset `a6a881`/fresh LUKS `36468a`.
The previous unknown b42/c200 volatile epoch was disposed under explicit
disposable VM administration, not product recovery/custody/cleanup PASS.
No writer inspected either stopped actor scope.

Final22 packet memory controls `df29d0` returned original0. ROOT upload `9fae4f`
returned original0 with35-byte fixed marker and empty stderr; prepare
`2e8b5a`/`aabe26` returned original0. Whole run `7f5ba0`/`229d78` returned
original2: **NONPASS**. Separate scoped file observer `7d684a`/`e26d3d`
returned original0, projecting only its pre-scoped two captures:

| Exact capture | Bytes / literal frames | Last phase | SHA256 |
| --- | --- | --- | --- |
| Actor | 1164 / 33 | `t4_actor_before_canonical_inventory` | `29ec6672cbc5af6bb2a4ff50dea1d4641bd63413637af0edf1e2afb444603b39` |
| Supervisor | 122 / 5 | `t4_service_ready` | `8dd70e5ec01210a4c9bad017c2670784fbe66b0611f579b12f79a632779f56fb` |

The source-compatible complete prefix includes the initial four query
completions, including both corrected user queries, before inventory. This is
progress in this exact scope, not proof of the earlier scope's errno/cause or
whole PASS. The inventory interval includes initial catalogue traversal,
charged-owner admission, all row checks and intermediate catalogue/sweep
admission:33 frames alone cannot select one refusal. Original3c SOURCE/artifact/
packet stay immutable tested NONPASS objects. Its actor epoch is parked, with
no retry/query/signal/reap/cleanup or author capture/tree inspection permitted.

### Inventory first-refusal SOURCE successor

The smallest added diagnostic is actor-private `InventoryDiagnostic`, with one
attempt bit and a closed23-case `InventoryFailure` enum. Each hook consumes only
the already returned original Result or already evaluated predicate. There is
no new open, metadata/proc/directory/process query, private value serialization,
raw errno/message, PID/count or per-row output. Original success is unchanged;
original failure propagates unchanged to the existing live parked-owner path.
The attempt bit sets BEFORE the one label output. Output failure/expiry cannot
change that original failure or trigger a retry, fallback or secondary label.
Nested outer hooks preserve the first inner read/parse category, not relabel it.

All labels have fixed `t4_actor_inventory_` prefix and `_refused` suffix/LF:
catalogue_seek, catalogue_next, catalogue_name, catalogue_limit, catalogue_set,
owner_admit, row_directory, status_read, status_parse, stat_read, stat_parse,
classify, image_open, image_shape, command_read, command_parse, comm_read,
comm_parse, link_read, daemon, row_current, row_complete, sweep_set. Read cuts
also include the unchanged original scratch admission/metadata/current-binding/
positive release prerequisites; image_shape includes original metadata/type.
These categories identify an existing refused suboperation, not a specific
errno or independently established process fate. Uninstrumented invariant or
expired budget refusals need not yield a category; no failed-time probe follows.

Success remains at most59 actor frames. One failure-label attempt gives a
conservative maximum60;17 existing +23 refusal labels give40 distinct actor
literals. Longest literal times60 fits the existing4096-byte capture bound.
Supervisor seven frames is unchanged. Each attempted failure label uses the
SAME original2-second/16MiB budget clipped to operation5 seconds. An expired
budget cannot be extended to make diagnostics succeed. No parser, equality,
visibility, catalogue limit,8320/classic64 FD envelope, original child outcome,
group/user credential admission or StoppedOwner/Restore contract changes.

Typed text roles merely replace the four existing fixed string names status,
stat, cmdline and comm; caps, flags and predicates remain byte-for-byte choices.
Scratch and row Files still install before every fallible postcheck. A parse
failure retains the positive directory/scratch prefix; owner revocation blocks
any later acquisition, scratch release or Halt/finish. Diagnostic memory state
does not claim to replace those actual owner guards or prove fatal/unwind custody.

The separate start-time-zero compatibility question remains unresolved for
product policy. Linuxv6.17 [proc stat producer](https://raw.githubusercontent.com/torvalds/linux/v6.17/fs/proc/array.c)
converts task start_boottime to clock ticks, after the
[fork producer](https://raw.githubusercontent.com/torvalds/linux/v6.17/kernel/fork.c)
records the boot-time nanoseconds. A synthetic well-shaped zero-tick stat sample
matches the [integer clock conversion](https://raw.githubusercontent.com/torvalds/linux/v6.17/kernel/time/time.c)
when a supplied start time is less than one clock tick; no positive lower bound
is added by that conversion. The sample
is still rejected by the unchanged shared positive-start parser. This does NOT
establish a zero-start task in either failed scope or authorize parser relaxation.

Six new memory controls cover23 unique fixed categories/original-result
preservation, one attempted label even if output fails, expired/no-output refusal,
whole60/40/4096 bounds, three different refusals behind the same33-frame prefix,
retained positive row/scratch prefix with no downstream IO/finish, fixed text
roles and the still-negative zero-start sample. They do not use actual proc,
credential/child/backend/actor/VM resources. A new artifact/packet requires
fresh exact-head build/gates,40-literal/60 cap admission and separate FULL ROOT
and independent review before a reconciled NEW epoch may be selected.

Successor focused SOURCE gates returned original0:23 canonical/row memory
controls `57c76e`/`64eb48`,39 unchanged service/transfer/stage controls
`0d1b64`/`96d47b`, seven legacy capture controls and normal no-feature library
check `5e3026`, all-target feature Clippy with warnings denied
`749023`/`2e96a2`, and diff whitespace check. Initial focused no-run compilation
`76f4f2`/`755335` also returned0. No actor binary, installed tool, credential
syscall, actual proc backend or guest action was run by the writer.

### Third canonical actual: original image-open category still unknown

ROOT selected exact3671084292672c979ca0886641fa774919e5ae8d actor
63854648 bytes/SHA256
`861eccc7e0749c3c68258ab68aa729c1291a1c3b947addccd68866a493479533`
with a separately reviewed fresh-boot-bound packet. Upload `94d996` returned
original0 with the fixed35-byte receipt/empty stderr; prepare `86d816` returned
original0. Whole run `79601`/`6a066f` returned original2: **NONPASS**.
Separate pre-scoped two-file observer `7ceb65` returned original0:

| Exact capture | Bytes / literal frames | Last phase | SHA256 |
| --- | --- | --- | --- |
| Actor | 1202 / 34 | `t4_actor_inventory_image_open_refused` | `4bb3c447cd795bd0f86d17653273e2e13d9893edd91120e1e1496df462281d2f` |
| Supervisor | 122 / 5 | `t4_service_ready` | `8dd70e5ec01210a4c9bad017c2670784fbe66b0611f579b12f79a632779f56fb` |

This outer category covers the pre-open owner/gate/charge, original SameUID
executable open and post-positive budget gate. It does not identify the original
errno, process fate, zombie state or actual cause. The actual scope stays
permanently stopped for agent actions: no failed tree, capture/process query,
retry, reap, signal or cleanup. Separately authorized disposable administration
is not product recovery.

### Original image-open errno SOURCE successor

This separate branch starts from exact367; it does not change the later private
authentication or synthetic-stage branches. Shared legacy `magic_file` and its
cfg(test)-only reporter remain unchanged. A canonical-private helper makes the
SAME single original `openat` of fixed `exe`, O_PATH|O_CLOEXEC/Mode::empty, and
keeps its typed Errno. Positive File ownership still transfers immediately into
the existing charged Owner before the unchanged post-open gate.

On the original Err only, the acquisition callback records a stack-local closed
category and returns the same Unavailable. It emits nothing, performs no probe,
metadata/path read or ambient-errno resampling. After `Owner::executable` returns,
its existing guard has revoked on Err and retained the reported prefix. Only then
does the existing one-attempt InventoryDiagnostic select a finite label. A
charge/gate/invariant refusal without an open Err keeps generic image_open_refused.

The six static suffixes are image_open_enoent_refused, image_open_eacces_refused,
image_open_eperm_refused, image_open_emfile_refused, image_open_enfile_refused
and image_open_other_refused (all t4_actor_inventory_ prefix and LF). Exact
ENOENT/EACCES/EPERM/EMFILE/ENFILE map respectively; every other Errno, including
EINTR, is OTHER without retry. No raw integer/PID/UID/path/count, private text
or copied live authority is serialized.

Six new failure literals make46 total, not a second failure frame. Successful
whole59/conservative failed60, supervisor7, capture4096 and8320/classic64 bounds
are unchanged. Attempt consumption still precedes output using the same budget;
diagnostic failure cannot change the original Err or permit downstream
IO/finish/release. Missing labels remain possible on expiry. No Class/start-time,
required regular image, daemon, UID/GID/namespace, inventory equality, listener
or backend fault predicate is relaxed. ENOENT is only an original returned
category, not proof that skipping a zombie is safe; EMFILE/ENFILE confer no
permission to raise limits or evict uncertain owners.

Four new controls use memory resources only: exact mappings/OTHER/no-success
label, revocation before one diagnostic and no later IO, positive image retention
on a late postgate without an errno label, and pre-callback refusal without an
open. Existing all-failure/expired controls cover29 failure labels and the
unchanged60-frame/4096 bound. Focused memory gates passed:27 canonical/inventory
controls (`2a0bbe`),39 service/protocol/transfer/stage controls
(`0d8763`/`1def2e`), and seven capture controls (`4eeab7`). Strict feature-library
Clippy with warnings denied (`276068`/`a85c73`), normal no-feature library check
(`50f46a`/`de0750`), formatting and diff whitespace checks passed. These are
library-only compile/memory controls, not actor binary or installed-proc evidence.
No actor build or VM action is selected by this implementation. Fresh artifact/packet/
vocabulary and separate review/boot/runtime admission precede any future ROOT
actual choice; old367 remains immutable NONPASS, not a retry target.
