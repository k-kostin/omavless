# T4 retained-manager actor service implementation

Status: first developer actor-service integration passed; product activation and
backup/restore acceptance remain unavailable.
Starting source is Draft #658 `f7a4acc472db7dac3ccc844034581c8ad2e55c06`.
The owner-approved execution policy at PR #662
`b8c967f2039bdad8a385429a212b977318adc9dc` selects the new availability-oriented
service boundary. The previous descriptor-free model and all stopped actual
experiments retain their original contracts and outcomes.

This feature continues #658 rather than adding a sequence of prerequisite PRs.
Normal daemon, CLI recovery, backup/restore and package activation are unchanged.
The legacy caller-local ancillary Bundle contract is neither implemented nor
weakened by this service. Its partial-error custody limitations remain recorded
in [the parent experiment](T4_RETAINED_MANAGER_PARENT_PROTOTYPE.md).

## Completion matrix

| Scope | Required behavior | Deterministic gate | Real integration gate |
| --- | --- | --- | --- |
| Developer actor acquisition | One original child, positive original pidfd acquisition, authenticated post-exec connection, READY before manager acquisition | Pure transition/parser/order cuts; compile exact service feature | ROOT's exact `0d1c7dec` first disposable-VM run returned original zero |
| Fixed manager operation | Capture and fresh recheck of fixed PID1 process/image/PID+user namespaces, proof consumed within original actor borrow; only completed reply | Source identity/comparison tests; exhaustive acquisition cuts remain open | One observation and separate normal Halt completed in the original actor; no canonical-manager/backup authority |
| Fault boundary | Unknown operation consumes capability; live reported originals retained; actor/channel loss permanently revokes context and late replies | Wrong nonce/sequence/peer/late/EOF and permanent-reentry controls | New reviewed disconnect and actor-loss cases; not run |
| Aggregate bounds | Capacity reserved before launch/open; no uncertain-owner eviction or new-context bypass | Slot/FD/allocation limits, refusal before effect | Saturation with one actual quarantined actor; not run |
| Product recovery | Canonical manager origin, full inventory, stopped-owner admission, backup/restore/reconciliation | Separate integration contract | Not supplied by the first scenario; remains unavailable |

The opt-in service implementation is present. Exact-head ordinary source gates
passed 12 new pure Rust controls (10 protocol/channel-policy controls and two
retained-prefix capacity/reentry controls), feature/binary `cargo check`,
compile-only `cargo build`, normal no-feature library check and feature Clippy
with warnings denied at implementation head
`0d1c7dec5b8df05cc073cf13bc1c5d2f63c02b85`.
The separately bound VM packet's eight memory/AST controls also passed.
The first actual result is bound below. Old model
tests, prior own-pair results and successful source builds cannot supply the real
service gate. No physical-host networking, service installation, package action,
old stopped-resource query, main merge or release is authorized here.

## First actual service checkpoint

ROOT and the independent reviewer FULL-reviewed the service, fixed packet and
ordinary-copy/real-TTY transport before ROOT's separate selections. ROOT reports
these exact original outcomes; the implementation author did not inspect the
private captures or issue any VM command.

| Independent selection | ROOT receipt | Original outcome |
| --- | --- | --- |
| Two public input files uploaded into the fresh user-owned fixture | `697094` | zero, exact 35-byte upload marker, empty stderr |
| Fixed root-owned actor byte-copy preparation | `32a2d9` / terminal `2375b1` | zero, fixed prepare token |
| One supervisor/actor service run | `8bb317` / terminal `6180d5` | zero |
| Separately selected, pre-scoped two-file observer | `96be02` / terminal `26b9ca` | zero |

The selected artifact was 58,124,768 bytes, SHA-256
`59205342db0539b232cee37afcb0f8da0d7dccf86c58f6a41f61e054eebe2a18`,
compiled from exact `0d1c7dec`. The fixed packet SHA-256 was
`2b9b6b55eb18c6a6d62e9d49bab43ee1cba4500a4e2b7b07e3b7e0065723890b`.
The observer projected supervisor output as 179 bytes, seven closed literal
frames, last `t4_service_completed`, SHA-256
`209511e916a4680fb0433c8bb567eb7b432b102c2b5714435863c314c9afc5b5`;
actor output as 618 bytes, 23 closed literal frames, last
`t4_actor_identity_checked`, SHA-256
`55053d4868abc50a30016a361ffe13cfca934ca2ded5365fcdff92569c2845e4`.
These projections alone are not ordering, custody or whole-run proofs. Original
run zero together with the reviewed program binds the normal Halt reply and
original child's terminal zero. No second service invocation had been selected
at this checkpoint; positive fixture retirement is recorded separately below.

This is a real fixed PID1 identity observation, not a fabricated model result.
It does not prove canonical manager origin, whole process inventory, stopped
owner admission, private transfer, journal recovery, first Restore/Commit,
normal-owner integration, fatal descriptor survival or product activation.

## Next integration matrix and reset boundary

The next source change must add closed developer scenarios, not a generic
command/target/FD endpoint or a new admission path. Each runtime scenario needs
FULL source/recipe review and an independent ROOT selection. Pure controls may
exercise the exact service coordinator using memory I/O and mock acquisitions,
but cannot supply the real acquisition/custody rows.

| Next scenario | Required runtime result | Epoch disposition |
| --- | --- | --- |
| Positive capacity | Three fresh observations retain 51 originals; valid Halt releases them before Closed; original child exits zero | Eligible for separately reviewed positive-only fixture retirement |
| Fourth request | Capacity refusal before any fourth acquisition; all 51 reported originals remain in the live quarantined actor; no further operation or Halt | Reserved; no automatic retirement or replacement actor |
| Channel loss after a completed acquisition | Original pending capability revoked permanently; late completion cannot restore authority; actor's recorded resources retained only while alive | Reserved; separate disposable-fixture recovery required |
| Malformed or stale request/reply | Wrong nonce/sequence, partial/EOF, duplicated or late reply permanently revokes the original context; no resend | Reserved on uncertainty |
| Actor fatal loss | Original channel/context unavailable, no reconnect or replacement capability; no claim that descriptors survived death | Reserved; independent persistent-evidence reconciliation required |
| Same-epoch supervisor reentry | Existing reservation refuses before listener/child/manager acquisition, including while another actor is quarantined | No bypass by a new upload directory or caller context |

The first successful epoch may be retired only by a separately reviewed,
ROOT-only fixed-name administration packet tied to the exact original run zero,
normal Halt/original-child zero and exact two-file observer receipt above. It
must verify the entire fixed epoch member set and original/current identities,
the selected actor bytes, exact reservation bytes and exact capture hashes
before its first unlink. It must unlink only these admitted names relative to
its retained directory, fsync and remove that exact empty directory, never
recursively delete, follow names, query a process, signal, adopt an uncertain
scope or use mere absence as positive completion. Every partial retirement
refuses further effects; there is no cleanup-on-error. This is explicit
developer-fixture administration, not product rollback or a reusable reset API.
An uncertain fault epoch is ineligible
for this positive-only retirement regardless of a visible completion label.

ROOT subsequently selected the FULL-reviewed four-file positive-only retirement
packet after its 11 mocked controls returned original zero (`5fa31d`). The
independent selection `bc01c2`, original terminal `341b6c`, returned zero and
exactly seven closed outputs ending `T4_FIRST_POSITIVE_EPOCH_RETIRED`. It removed
only the five admitted first positive members and exact empty epoch, with
descriptor-relative unlinks and directory sync. The private raw captures were
deleted and have no promised recovery; the bounded hash/status receipts above
remain durable and the public user upload remains unchanged. No old stopped
scope, process query, signal or retry was involved. This is one known-positive
developer fixture administration result, not product rollback or a reset
permission for any uncertain actor or transaction.

The next source coordinator exposes only fixed opt-in developer arguments:
`--capacity-three`, `--capacity-fourth`, `--wrong-nonce-after-first`,
`--partial-after-first` and `--disconnect-after-first`. They use the same
single sentinel, original child, private READY channel, 64-FD ceilings, 51-held
admission and 15-second whole supervisor budget. The actor receives the same
closed observation/Halt protocol, not a configurable fault instruction. Three
capacity observations each freshly acquire/recheck 17 originals. The fourth
attempt still refuses in the existing retained owner before its first open;
even an unexpected fourth completion cannot become scenario success or Halt.
The malformed and loss scenarios first complete one real observation, then
make the pre-scoped fault cut with those 17 originals already retained.

The exact exchange coordinator is now memory-tested over every 0..63 request
write and reply-read cut. All I/O/decode/deadline errors revoke the original
pending context before return; subsequent begin/late completion performs no
next write/read and cannot restore it. A completed reply is additionally bound
to its original pending operation kind: Closed cannot complete ObserveManager,
and Completed cannot complete Halt. These are deterministic source tests, not
real fault/custody evidence. Initial successor source gates passed 16
protocol/coordinator controls and the unchanged two retained-prefix controls,
plus feature Clippy with warnings denied. Fresh artifact, packet, FULL source
reviews and ROOT selection remain required for all five new scenarios. No
fatal-loss hook or private transfer operation is added by these flags.

Diagnostics add only fixed `t4_service_before_fault_request` and
`t4_service_before_channel_disconnect` literals. The complete supervisor
vocabulary is nine labels with at most nine frames; three actual observations
emit at most 69 actor frames under the unchanged 14-label vocabulary. There is
no raw nonce, sequence, target, FD, exception, PID or process metadata output.
Late/partial or phase-emission failure cannot become a completed-operation
receipt. The fixed observer must be separately rebound to these new bounds
before ROOT selects a new scenario; the first packet and its actual result are
not reused as that recipe.

After these availability/resource rows, the service must compose the existing
T4 private transfer, staged transaction, journal classifier and crash-prefix
reconciliation contracts. Manager proof acquisition/consumption remains within
the same original actor operation; private bytes and typed completed results,
not descriptors or reusable proof tokens, cross the private channel. Recovery
after fatal loss first classifies persistent evidence under the existing fences;
it cannot infer completed publication from a lost reply, discard journals,
evict an uncertain owner or re-enable a pending capability. Those operations
need a separately reviewed source design and a whole synthetic transaction/crash
scenario before product activation. The legacy Bundle contract remains separate.

## Smallest executable scenario

An opt-in Rust Cargo feature and separate developer binary implement both the
supervisor and its fixed actor entry. Ordinary builds contain no service command
or automatic actor launch. No Python runtime or generic command/FD/PID/path IPC
is added. The exact executable and VM fixture paths are bound by the separately
reviewed ROOT recipe, not chosen by a client request.

The first run is an explicit trusted-administrator developer scenario. ROOT
launches one fixed binary in a fresh private disposable VM fixture. This origin
is not product PID1/system-bus/install/invocation authentication. The target of
the first operation is the fixed visible PID1, with no namespace switching or
claim that a result establishes full `StoppedOwner` admission.

1. Reserve the one aggregate actor slot and its descriptor allowance durably
   before listener/child acquisition. The fixed admission sentinel is exclusive
   and remains on any uncertainty. A new supervisor or context cannot overwrite,
   rename around or ignore it. A fresh directory name is not a reset capability.
2. Create one private Unix listener, retain its original, then spawn only the
   fixed actor executable. Capture the original positive `Child` return before
   using its PID. Keep that child unreaped; obtain an original owned pidfd from
   a positive `pidfd_open` return. No guessed scalar, PID lookup, reconstructed
   child, fallback process or second acquisition can stand in for either owner.
   This is a fresh standalone Linux exec, not an entry callable in an arbitrary
   daemon process. It has one thread, no ignored/caught SIGCHLD disposition,
   no concurrent waiter/reaper and no subsequent signal-handler installation.
   Linux's exec signal reset clears SA_NOCLDWAIT; the fixed supervisor is the
   sole reaper. See the pinned
   [signal reset producer](https://github.com/torvalds/linux/blob/v6.17/kernel/signal.c).
3. The actor connects after exec. Accept once and compare kernel SO_PEERCRED to
   that original child PID and expected UID/GID; check the original pidfd for
   loss. Challenge/READY binds the same original channel and fresh nonce.
   No manager descriptor is acquired before the complete READY barrier. An
   inherited socketpair's creation-time SO_PEERCRED is not used as post-exec
   credentials. No ancillary receive is used anywhere in this initial protocol.
4. Send one closed `ObserveManager` request with exact context nonce/sequence.
   The actor owns all originals and performs capture, current-image and namespace
   comparison and a final proof-consuming validation within one exclusive
   operation borrow. No `File`, numeric FD, copied live proof or token leaves
   the actor. Only an exact fixed `ManagerIdentityChecked` completed response can
   be reported; it is an observation, not reusable authority for another action.
5. Verify the completed response against the same original channel/context and
   original actor availability. A late, malformed, lost or incomplete result
   consumes the outstanding capability permanently. No resend follows. A next
   admitted operation must independently capture/recheck inside that same actor.

The initial wire format is fixed-size and versioned, with a closed operation
enum, nonce and monotonically bounded sequence. No producer-defined authority
field, arbitrary string, operation deadline or raw private error is accepted.
The service uses ordinary `read`/`write`, not recvmsg: an unexpected SCM_RIGHTS
message cannot install descriptors in the supervisor. Protocol shape failures
return only fixed categories. Partial I/O and deadline failure cannot become a
completed response or reset the context.

## Live custody and resource accounting

Reserve one actor, one in-flight operation and at most 64 actor descriptors
plus a 64-FD supervisor ceiling. The fixed epoch reservation is
`/run/omavless-t4-actor-development/reserved`, outside every caller-selected
input/staging directory. It is not deleted even on normal completion in this
first developer scenario; a second invocation refuses the same epoch.
These bounds cover admissions through this fixed trusted-admin service, not a
hostile administrator who can replace binaries or launch arbitrary processes.
Set the actor's RLIMIT_NOFILE before READY/acquisition and verify the applied
limit. In addition, explicitly reserve application-held descriptor slots and
bounded buffers before each operation; the kernel limit is not a substitute
for admission accounting or a heap limit. Every successfully returned owned
descriptor is inserted into the retained owner before a post-call validation,
metadata read, comparison or deadline gate. Refusal retains the full recorded
prefix in that live actor, including transient current-image originals.

The existing `Process::capture` cannot simply be called and advertised as this
property: its local partial acquisitions drop on an error. The implementation
will add a separate retained-owner acquisition path using the same strict
process/status/start/image/namespace predicates. Shared pure comparators and
parsers are reused; ordinary constructors and acceptance are unchanged.
Acquisition primitives with internal unreported resources remain explicit
library/backend boundaries, never guessed ownership.

The retained application vector is reserved for 51 originals. One observation
uses exactly 17 slots: one proc root and two snapshots of eight descriptors
(directory, stat, status, cmdline, comm, executable, PID namespace, user
namespace). Up to three completed observations retain their full prefixes;
a fourth refuses before opening anything. The fixed first supervisor sends
only one observation then a separate Halt. Normal Halt releases the recorded
originals before its fixed reply; ordinary `File` close behavior is a backend
boundary, not per-FD kernel absence proof. The supervisor's final result also
requires the original child's terminal zero, after the positive Halt response.

On a nonfatal uncertain operation, revoke dependent work and leave the live
actor quarantined with resources it actually owns. No descriptor eviction,
automatic close, retry, adoption, signal, reap or compensating manager operation
follows uncertainty. Capacity is not released to admit a fresh context.
On actor/channel loss or fatal death, permanently revoke the original context
and all pending capabilities, reject late replies and report unavailable. No
descriptor survival after fatal death is claimed. Persistent effects, if added
later, need explicit reconciliation before any fresh service admission.

The first operation is observational and performs no manager/systemd/network
mutation. Any normal completed teardown recipe must be independently reviewed;
the admission sentinel is not removed automatically on a failure or merely from
absence of a process. An uncertain old scope is never reopened by this design.
The practical first VM resource reset, if required, is an owner-authorized
disposable-fixture recovery boundary, not product rollback acceptance.

## Review and evidence plan

ROOT and Astra independently review the new launch/origin/channel, descriptor
acquisition, retention, aggregate admission and all affected failure paths
before any actual child/native selection. Pure code/parser/model controls and
compile-only gates may run earlier under HOME with bounded planned public-source
diagnostics; a routine compiler error is not privileged acceptance or an unknown
manager effect. No actual service body is selected by ordinary test discovery.

The executable packet has independent `prepare`, `run` and `observe` actions,
never automatic chaining. It binds one exact compile-only artifact SHA/size,
copies public bytes into the exclusive fixed root epoch, rechecks the destination,
and only its separately selected `run` redirects two fresh 0600 captures then
execs the one fixed binary/argument. No caller program, target PID or input path
is accepted. ROOT must separately admit upload/SSH/source bindings; packet
presence and its pure gate do not authorize a VM action.

Diagnostics were designed before the first service run: seven fixed supervisor
stdout labels and 14 actor stderr labels. The first operation emits at most
23 actor frames (root, first marker, nine first-snapshot stages, second marker,
nine second-snapshot stages, final comparison and completed observation).
The pre-scoped observer reads only the two exact capture files, each bounded
to 4096 bytes with original metadata/identity stability checks. It publishes
only presence/size/SHA, closed grammar/count/last literal, never raw payloads.
These phases are before-effect boundaries, not ordering, cause, preserved FD,
absent-resource or whole-PASS proofs. Actor stdout is null; its fixed stderr
is inherited solely into the supervisor's private capture. Unexpected stderr
remains private and gives only a grammar refusal projection.

After compile/preflight, ROOT selects the smallest positive VM scenario first,
then separately reviewed fault cases and the bounded integration matrix. Useful
phase/error diagnostics are part of that recipe from its first run, not a new
observer per token. Report exact heads and actual reached operations. Preserve
nonPASS receipts, do not infer original causes from later observations and
reassess with the independent reviewer after two unsuccessful diagnostic cycles.

Product activation still requires canonical launcher/broker/manager invocation
origin, whole other-row inventory with no skipped processes, authenticated
application integration and installed exact-head acceptance. Fixed root UID or
PID1 alone proves none of these. The legacy restored-owner and K1 fail-closed
guarantees remain separate and unchanged.

Relevant safe-library source APIs are the pinned
[rustix 1.1.5 pidfd interface](https://docs.rs/rustix/1.1.5/src/rustix/process/pidfd.rs.html)
and [nix 0.30.1 resource interface](https://docs.rs/nix/0.30.1/nix/sys/resource/index.html).
Their positive owned returns do not discharge upstream partial acquisition,
libc/loader or fatal-process safety. No signal API is part of this service.
