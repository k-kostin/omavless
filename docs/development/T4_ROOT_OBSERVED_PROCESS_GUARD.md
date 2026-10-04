# Fixed root observation of the private Abort process-loss matrix

Developer-only successor to Draft #598 at `3110230`. One separately authorized
exact-head invocation is recorded below; the implementation itself grants no
VM authority. Any further invocation requires its own review, sealed artifacts,
explicit staging/host authorization and exclusive lease.
The [earlier report](T4_FIRST_ABORT_PROCESS_REENTRY.md#exact-head-execution-and-retained-nonpass)
is immutable: five inner cases passed, its outer guard failed, and the later
independent root observation did not repair that NONPASS or prove its exception.

## Privilege and provenance boundaries

`tests/first_abort_process/root_vm_guard.py` is a developer-only, one-invocation
observer. It is never installed, dispatched by the runtime, exposed as IPC, or
used as a privileged retry/fallback. Root only stages private evidence, reads
the fixed preservation views and observes executable descriptors. No root
process signals/reaps the fixture's workers, changes services/network state,
or performs application start/stop. Root directly owns and raw-reaps only its
own matrix and fixed read-only inventory-command children.

An authorized invocation must use `/usr/bin/python3 -I -B` and a reviewed root-owned
0500 script in a new root-owned 0700 `/run/ov-abort-root-<32 lower-hex>` directory.
Never invoke root Python on a user-writable script or import tree. This trusted
bootstrap and owner-reviewed sealed-copy operation are explicit preconditions;
the guard is not a self-authenticating root installer. All ancestors are checked
for symlinks, unsafe ownership/write modes and Git metadata.

The root directory contains 0600 `receipt.json`, 0500 `root_vm_guard.py` and a
0500 byte-exact copy of the earlier `vm_guard.py`. The latter stays unchanged in
Git and must match literal SHA-256
`3631492302b3e87beb0b2a801ded9ee5df1235a5c125117b0f4728596067049d` before
its definitions are loaded; its main never runs. Reusing its reviewed raw wait,
snapshot and comparison definitions does not reuse its failed orchestration.
The root-owned receipt retains the strict sealed-copy schema, binding exact head,
new root guard hash and original host build/frozen ELF provenance. Guest lexical
host provenance is not a fabricated guest build directory.

The corresponding new UID/GID-1000 0700 directory under `/run/user/1000` has the
same basename and contains the 0500 UID/GID-1000 frozen `fixture` ELF and original
guard/legacy/receipt staging inputs. No input code is executed from that directory. Root
holds original no-follow read-only descriptors, parent identities and bounded
hashes for every admitted artifact, rejects xattrs/hardlinks and rechecks them.
It creates root-owned 0700 `evidence` using the held root directory descriptor;
0600 evidence files use exclusive relative opens. No user-controlled output
pathname can redirect a privileged write. Files and directory remain root-owned
even on success; later read-only export requires separate authorization.

## One unprivileged child, no launcher intermediary

The matrix is launched directly with native Popen `user=1000`, `group=1000`,
`extra_groups=()`, `umask=077`, `close_fds=True`, a fixed argv and an exact small
environment retaining `HOME=/home/kdk_vm`. There is no shell, PAM/runuser wrapper
or `preexec_fn`. The executable is `/proc/self/fd/N`, where N is solely the
guard's admitted original read-only ELF descriptor—not caller-selected input.
That descriptor is the sole extra inherited FD, besides standard streams. It
contains no root-only data and remains read-only when inherited by workers;
root directory/evidence descriptors are CLOEXEC and not passed.

Before fixture effects, the cfg(test)-only matrix and every worker require all
four UID/GID status values (real, effective, saved, filesystem) to be 1000, no
supplementary groups, zero inheritable/permitted/effective/ambient capabilities,
and the exact HOME. No claim is made that the capability bounding set is empty.
They bind retained `/proc/self/exe` and frozen-path identity to the original
root-provided device/inode plus the sealed hash. Completion repeats retained
identity/credentials and emits a bounded identity receipt. The root checks both
executed and completed identities before opening the five exact case roots.
The new ELF therefore requires the new guard's explicit identity environment;
the old frozen ELF and old invocation evidence remain unchanged.

Root raw WNOWAIT/exact waitpid supervision refers directly to the matrix PID,
not a launcher PID or translated status. Any unknown wait, EINTR/ECHILD, mismatch,
timeout, spawn failure or nonzero matrix result permanently stops the invocation:
no after-query, scan, retry, signal, cleanup or inferred quiescence. A fixed phase
marker is the only public failure detail; private outputs remain retained.

## Whole-invocation evidence and bounded claims

All command argv are literal allowlisted read-only queries and run with the
same dropped credentials, including the actual user's systemd view. The guard
collects the same nine canonical categories, fixed four root-unit states and
full IPv4/IPv6 address/routes-all-tables/rules. It fsyncs the root-owned original
before-baseline before spawning the matrix. Only known successful completion
may validate the five original case receipts, observe executable absence as
root, capture/compare the after-baseline, recheck sealed artifacts and write the
final result. Only the already reviewed address-lifetime countdown is exempted.

The root process inventory is bounded and uses raw start time, stable UID and
original executable FD metadata around each read. Disappearance, permission
failure, changed epoch or inode, or matching executable presence refuses and
latches uncertainty. Stable zombies are checked twice and not represented as
live executable observations. The result claims only absence of the exact
fixture executable inode among current UID-1000 executables at that observation,
not atomic absence of every descendant or arbitrary escaped process. No guessed
argv/name absence substitutes for descriptor identity.

Pure/local tests cover raw uncertainty and nonzero stop boundaries, direct-child
credential kwargs and fixed inherited FD, owner/mode/symlink/hardlink/same-byte
replacement refusals, credential counterexamples, fixed command admission and
executed/completed identity rejection. One harmless local readlink executable
checks Linux FD-exec path resolution, without root or changed credentials. The
ignored five-case matrix is not executed by ordinary gates. This remains
volatile synthetic process-loss evidence, never durability, reboot, power-loss,
installed acceptance or full product T4 PASS.

## Separate create-only root staging

`stage_root_guard.py` is independently reviewed source supplied over trusted stdin
to isolated, no-bytecode root Python, never invoked as a user-writable script.
This trusted bootstrap is an owner-controlled operation, not an automatic sudo
request. Its only inputs are the exact lower-hex nonce and reviewed receipt SHA;
source and destination paths are derived from the fixed paired locations above.
It opens and holds all four user-stage files through their original parent FD,
checks private ownership/modes, single link, absence of xattrs, bounded lengths,
stable path/descriptor metadata and expected hashes, including the fixed legacy
hash. Only then may it exclusively create the root destination. It copies only
the root guard, legacy definitions and receipt into new files, rechecks source
identity, verifies each copied hash through the original output FD, and fsyncs
files and directories. Existing targets or any uncertainty refuse without retry,
cleanup, chown, fallback or executing either the guard or ELF. Partial stages stay
retained. Staging success is not permission to invoke the guard.

Additional main-orchestration tests inject original-baseline write/fsync failure
and prove no matrix starts; case receipt, root observation, after-baseline write
and comparison failures prove no final result/PASS or hidden retry. Loader tests
cover credential/stdin admission, original source replacement, fixed names,
receipt syntax, hash mismatch before privileged creation, existing destination
and partial-copy failures. These controls do not change the reviewed guard's
behavior or convert its pure success control into an executed VM claim.

## Exact-head execution — 2026-10-04

Tested source: `a2eeb423ed202ec6e78af68d7b44e88e04a64acf`, Draft #605,
in the x86_64 Omarchy development VM. This documentation update does not change
the tested guard, loader, fixture or runtime behavior; it is not a new execution
on the later documentation head.

Before the invocation, exact-source local gates passed: 558 source tests /
2 skipped plus JS/QML; full Rust exit 0 with 2,058 aggregate passing executions,
including runtime 1,159 / 38 ignored / 1 filtered; separate cleanup, formatting,
strict workspace/TUI Clippy, terminal and two parity tests. The 28 local
root-guard/loader tests are included. These ordinary gates do not run the ignored
process matrix. Parent-agent and independent incremental source review completed before
staging and execution.

The frozen ELF was compiled at the earlier `2300ddc` component and reused/retested
at the tested head with byte-identical Rust/Cargo inputs, not represented as a
fresh compilation. After all active gates finished, under the owner's standing
development-session authority, the parent agent approved normalization of the
actual original mode 0700 to 0755 under private parents, with original-FD/hash
evidence retained; the frozen copy is 0500. This was not personally attended
release acceptance.
Artifact SHA-256 identities:

| Artifact | SHA-256 |
| --- | --- |
| Frozen test ELF | `d90ce376f5519c0bc95b765ec70f0b13ca27c95d9e5bca238e4ae6be59b100dc` |
| Root observer | `100428aae8271c9489454b5f0384ba3dc75287938c5b586c6f71ec3de8da286c` |
| Unchanged legacy definitions | `3631492302b3e87beb0b2a801ded9ee5df1235a5c125117b0f4728596067049d` |
| Trusted-stdin staging loader | `a3ee5f25b46e4c96e71893f402b440efd4d429ec497a213b9d8a40debe07e98f` |
| Sealed-copy receipt | `0be8615e37acb5a263b2beccbbd0ccbc79715c046692b25d7f010779bb1c5772` |

The sole create-only trusted-stdin staging operation exited 0, followed by one
authorized root-observed guard invocation. The matrix passed in 127.56 seconds;
the outer guard exited 0 with the bounded marker
`T4_ROOT_OBSERVED_PROCESS_LOSS_NOT_POWERLOSS_OR_PRODUCT_PASS`.
Independent review of all five exact-schema case receipts confirmed signal 9
at every requested checkpoint:

| Checkpoint | Fresh re-entry result |
| --- | --- |
| OLD store slot linked | `aborted-still-fenced` |
| First member OLD, template NEW | `aborted-still-fenced` |
| Empty terminal created | `refused-preserved` |
| Full Abort terminal written | `aborted-still-fenced` |
| Final wrapper observation | `aborted-still-fenced` |

The parent agent (Codex) independently verified the complete captured archive structure, three
staged source hashes and original before/after comparisons. All nine canonical
categories and the fixed four root-unit states matched; full IPv4/IPv6 addresses,
routes and rules matched except four approved address-lifetime countdowns.
The root observation inspected 44 current UID-1000 executables and found the
exact frozen executable inode absent at that observation. This is point-in-time
descriptor evidence, not atomic absence of all descendants or escaped processes.

Private root-evidence archive SHA-256:
`34bf2ae91ea47192892606907f74867e343e1bf589ff518282cb6d3819641942`.
Private five-case archive SHA-256:
`a1c8afea33a83b076c4c1241507bdf8448cb962d9ded8f400354fc4fba78dba8`.
Raw logs, case paths, staged files and archives remain private outside Git.

All previous failures remain retained. In particular, the initial `2300ddc`
full-Rust invocation with inherited umask 077 remains NONPASS: five netguard
fixture-mode failures. A separately authorized child-only umask 022 gate passed
without weakening tests. The earlier #598 inner five-case PASS / outer NONPASS
and its later independent diagnostic remain unchanged; this new invocation does
not repair or reinterpret them.

This is bounded synthetic volatile process-loss/re-entry evidence only. Normal
backup/restore registration, installed product acceptance, reboot, durability,
power-loss, full product T4 completion, main merge and release remain unclaimed.
