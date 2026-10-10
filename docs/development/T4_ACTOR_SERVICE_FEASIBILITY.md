# Actor-service feasibility, not a T4 custody substitute

Current direction, 2026-10-05: the owner-approved execution policy in PR #662
selects a separate availability-oriented SERVICE fault boundary. The
[active implementation and first real scenario](T4_MANAGER_ACTOR_SERVICE.md)
continue #658. The observations below remain historical feasibility research;
the original caller-local Bundle requirement is not relaxed, accepted or
implemented by that service alternative. No service/VM gate has run yet.

We can preserve a useful architectural direction without changing the accepted
T4 contract. I inspected the retained local-reply boundary, the descriptor-free
model and the pinned Linux acquisition/exit paths. The model's six positive
controls establish only its own Rust transitions. They do not implement the
acquisition or lifetime property that an actual actor would require.

## Observed feasibility boundary

Linux v6.17 [fork.c](https://github.com/torvalds/linux/blob/v6.17/kernel/fork.c)
reserves a pidfd and can write its user output slot before remaining fallible
construction. Installation follows the final construction failure boundary
before wakeup. A negative parent return may therefore leave an uninstalled
scalar; we cannot adopt that scalar or reconstruct authority from a PID.
Positive original return capture in userspace still needs an independently
reviewed implementation.

Fatal exit is a different boundary. In
[exit.c](https://github.com/torvalds/linux/blob/v6.17/kernel/exit.c), `do_exit`
calls `exit_files` before exit notification. In
[file.c](https://github.com/torvalds/linux/blob/v6.17/fs/file.c), that operation
detaches the task's file table and releases it; final table release closes its
files. Holding a pidfd or leaving the task unreaped is not retained table custody
for an isolated actor with an unshared table. We cannot turn a model quarantine
label into survival through SIGKILL, abort, OOM or another fatal exit.

The accepted unconditional partial-error custody requirement remains unchanged.
A userspace actor alone does not satisfy it across fatal death. If that property
must remain at this boundary, we need a separately designed durable external
or kernel custodian; additional Boolean-model tests do not supply one.

## Candidate alternate service boundary — unimplemented

A separate actor service could provide fixed completed operations rather than
returning a local `OwnedFd` Bundle. The original parent would acquire one owned
pidfd only from a captured positive original acquisition return, and retain its
original private channel. No `CLONE_FILES`, thread/shared-VM substitute or
reconstructed identity could stand in for that process. An exact authenticated
READY barrier would precede every possibility of descriptor import; unknown
acquisition must never enable receive.

We would keep receive, complete manager-only capture/recheck, executable and
namespace comparison, and every dependent proof-consuming operation inside
the same exclusive actor borrow. There would be no descriptor forwarding,
arbitrary command IPC, namespace switching, copied live-image proof or replacement
handle. A typed response would describe the operation already completed.
A later operation would need a fresh consultation on that same original actor,
not adoption of a saved response or numeric object ID.

For a cooperative, nonfatal actor, a returned uncertain receive/validation or
operation could consume its request capability, issue no further requests and
leave its whole table internal while it remains alive. This is a proposed
conditional property, not current physical custody evidence or permission to
query, reap, reconstruct or clean up an uncertain actor. EOF, death or an
unknown channel/acquisition outcome would make the service permanently
unavailable with no fallback or local descriptor adoption. We would explicitly
claim no preserved descriptors after fatal death.

That availability-oriented service is a different interface and fault boundary.
It is not compatible as a drop-in implementation of the existing caller-local
Bundle or common ancillary ownership contract. ROOT/owner review must decide
whether a separate inactive service prototype is useful; neither this document
nor the source fixture changes canonical acceptance.

## Minimum next executable slice and prerequisites

I recommend an original-acquisition/READY barrier slice before an actual
descriptor import. It would need one reviewed launch primitive, positive parent
return envelope, same original pidfd/channel, exact bounded READY grammar, and
a demonstrated refusal before import for every unavailable/unknown acquisition.
This is an implementation recommendation only, not native authorization.

Before a manager service could be used, we still need the actual broker/PID1 and
invocation/install origin, per-message credentials and nonce binding, whole
other-row inventory, namespace/no-switch provenance, original launch lifetime,
resource bounds and an end-to-end proof-consuming operation retained inside the
same borrow. Process/runtime construction and fatal-death behavior remain
separate obligations. No current executable provides these prerequisites.

The value of this option is that unknown installed ancillary descriptors need
not be guessed or individually adopted by a parent; they stay in an isolated
table for as long as the service lives. The cost is a new trusted service,
latency and lifetime/resource burden, plus permanent unavailability on death.
There are no measurements yet. We should compare those costs and the interface
change against an external durable custodian before committing to production.

## Evidence and compatibility

The [research fixture](../../tests/research/actor-state/README.md) preserves the
unchanged model and exact six-pure developer receipt. Its inert relocated
templates have no autorun or bindings and require a new selection review.
No backend, product API, dependency, descriptor receiver or accepted contract
changes here. #653's generic backend error/unreported-FD limitations and
canonical authority gaps remain. Model preparation, same-borrow promotion and
label poisoning are useful synthetic state properties only.
