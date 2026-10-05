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
