# T4 charged stopped-inventory actor candidate

Status: inert SOURCE building block from documentation successor `f76f6d54`
of the exact tested developer stage `d2f58377`. The private module is included
only by cfg(test) beneath the opt-in actor service. No production caller,
manager query, proc acquisition, child launch, actor limit change, wire method
or StoppedOwner constructor is added. Original developer RLIMIT64 and the
legacy Bundle/StoppedOwner contracts are unchanged. This candidate cannot
turn fixed PID1 or copied classification facts into canonical manager proof.

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

## Implemented inert owner and regression boundaries

The generic private owner reserves bounded row/name storage before any mock
acquisition. Names must be a nonempty sorted unique list of at most4096 positive
PIDs. A real integration must derive this from the original unrestricted proc
root, never a client-supplied subset. Each row acquires in exact catalogue order;
omission, repetition, changed sets and an incomplete final sweep seal forever.
Every row must have a class; SameUid must additionally have its original image.
Neither a negative class nor a memory callback is genuine proc/manager evidence.

Charging happens before the backend/open. Every reported positive directory,
image or scratch resource is installed before post-deadline or subsequent
checks. An acquisition, gate, classifier, row recheck or final-pass error seals
the original owner with its reported prefix. No later callback/open, finish or
capacity recycling occurs. The tests use memory Drop counters, not actual FDs.
Fatal owner loss and unreported backend partial resources remain excluded.

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
to hide multiple opens/effects in one outer gate. No production callbacks exist
in this checkpoint.

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
