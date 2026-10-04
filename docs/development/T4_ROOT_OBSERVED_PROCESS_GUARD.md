# Fixed root observation of the private Abort process-loss matrix

Source-only successor to Draft #598 at `3110230`. No VM invocation is authorized
by this implementation. Root source review, exact-head gates, sealed artifacts,
explicit staging/host authorization and a new exclusive lease remain required.
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

The future invocation must use `/usr/bin/python3 -I -B` and a reviewed root-owned
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
same basename and contains only the 0500 UID/GID-1000 frozen `fixture` ELF. Root
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
