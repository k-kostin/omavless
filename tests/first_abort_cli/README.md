# Disposable-real-UID first-Abort CLI gate (source scaffold)

The separately versioned [root guard implementation](ROOT_GUARD.md) now supplies
the proposed bounded delivery, bootstrap and normal-CLI orchestration. It is
source-only pending full review, frozen receipts and an explicit VM lease.
The native-scaffold checkpoint described below remains separate evidence.

Not executable acceptance yet. No account, manager, application, VM or network
operation has been run for this scaffold. Its root guard, artifact receipt and
trusted loader still require implementation and complete review. The ignored
native setup, verification and credential/exec helpers are source-only; none
has been invoked. The ordinary CLI checkpoint `7933b9955c4535ad8be086f819650029ca80a4d6`
passed source (565 tests/two expected skips plus JS/QML), full Rust (runtime
1183/39 ignored, DNS broker 52/2 ignored, CLI 25), formatting and strict
workspace/TUI Clippy. Earlier `52595b1` and `18b99a9` failures remain NONPASS.
The separate #605 process-loss evidence is unchanged, not a CLI acceptance.

Native scaffold controls validate credentials and the exact synthetic OLD
archive pair; all four real entries remain ignored. Initial fixture controls
refused an added YAML comment (bundled templates are byte-exact), then a
`direct` startup mode (startup admits rule/global, independently of routing
template modes). Both failed logs are retained; the fixture now uses the exact
bundled global variant with startup disabled. No archive policy was weakened.

## Fixed proposed scope

Use one create-only real account `ov-t4-abort-v1`, UID/GID `48044`, HOME
`/home/ov-t4-abort-v1`, runtime `/run/user/48044`. Refuse any pre-existing name,
ID, HOME/runtime, fixture stage or unsupported ownership. This ID is outside
systemd's dynamic-UID range. It is not an assertion that it is currently free.
No account deletion, manager stop or cleanup is implicit on any outcome.

Only this account's actual packaged `user-runtime-dir@48044.service` and
`user@48044.service` may be started after the reviewed preflight. No linger,
fake manager/bus/PID namespace, unit override, global enablement, application
start, canonical service stop or host network write is part of the proposal.
Global user-unit/generator startup uncertainty must refuse before manager
activation. Actual canonical-unit inactive/dead/MainPID0/ControlPID0 facts are
required afterward; missing units are not replaced with artificial units.
The existing UID1000 canonical runtime remains active and untouched.

The intended smallest case is one synthetic but valid OLD pair and authenticated
NEW archive, staged into a mixed first Intent, followed by the **normal**
`omavless restore abort --confirm-rollback` and a second invocation over that
same retained Abort. There is no reset, fabricated historical admission, fence
retirement or normal owner construction. Preserve both exact case receipts and
the archive. Resolve the actual installed Mihomo path read-only; do not use a
synthetic executable symlink or launch Mihomo. Setup must finish with exact
known exit zero before the CLI's real complete process inventory begins.

## Credential-before-effects bridge

The cfg(test)-only ignored native launcher uses fixed descriptors 198 (its own
frozen helper ELF) and 199 (the frozen normal CLI). A future reviewed root guard
must reserve them only after proving they are unused, duplicate its original
held descriptors, and pass no other extra descriptor. Both copies are UID48044
owned mode0500 beneath its private HOME, outside Cargo. Root retains the source
FDs, hashes, ancestry and exact publication identities before/after execution.
The test-only helper is not installed or reachable through production IPC.

Trusted delivery is a fixed root `setpriv --no-new-privs` Python invocation.
This does not add root capabilities. Direct native child credential arguments
drop to UID/GID48044 with empty supplementary groups. Before any CLI effect,
the helper checks all four UID/GID slots, empty groups, zero permitted,
inheritable, effective and ambient capabilities, and NoNewPrivs=1. It makes no
zero-bounding-capabilities claim. Root supplies the identities of its retained
PID/user/mount/network namespaces; the child pins/rechecks its own matching
namespace FDs. It does not assume unprivileged access to `/proc/1/ns`. The normal
CLI still performs its independent real user-manager/process admission.

The helper verifies both original-FD aliases, fixed path identities, hashes,
regular-file type, ownership, exact mode, single link, no xattrs/file
capabilities/set-ID bits, private non-writable ancestry and ELF magic. It closes
198 and execs `/proc/self/fd/199` with fixed recovery arguments and a cleared,
literal environment. The owned PID is preserved; only read-only FD199 remains
beyond standard descriptors. Private archive/passphrase input is inherited
through stdin, never argv or environment. Libtest's fixed prefix is a harness
record (`--quiet` is required), separate from the normal CLI's exact public
success line.

Pre-exec refusal is fixed NONPASS with no setup or CLI mutation. The ignored
launcher is not exercised by ordinary tests. Pure tests cover credential
malformations without creating users, changing identity or executing the CLI.

## Whole-invocation observer still required

The root guard must publish/fsync its original baseline before account/manager
or fixture mutations. Reuse the sealed nine-category canonical snapshot and
full IPv4/IPv6 preservation rules, including the fixed root-unit observations;
do not mask session churn or address renewal as a permitted countdown. Record
account-database and fixed owned-artifact changes explicitly, without treating
them as unchanged global state. Raw logs/account records remain private.

Use raw exact owned-child WNOWAIT/waitpid, never Popen polling/reaping fallback.
Unknown, timeout, nonzero, malformed evidence or failed preservation is terminal:
no later observation, signal, teardown, retry or automatic account/session
cleanup. Point-in-time original executable-inode absence is not an atomic
descendant-absence proof. New VM execution requires complete source/control
review, exact-head gates, frozen original-FD receipts and an explicit lease.
