# Live FD enumeration successor — source only

This separate generation preserves every file in `reviewed_tmpfs_elf` and its
measured head `1c1a8a4711dd29ad281a799139c063295a23900d`. That invocation
returned NONPASS; no successful after-baseline or compatibility is claimed.
Two later independently authorized read-only file diagnoses retained the exact
eight source pins. The second recorded errno 9 (EBADF) and the fixed frame
`bridge.no_writable_fds` at its `os.fstat(fd)` call. These file observations
are not process-state, cleanup or preservation evidence.

Before this fix, an inert local procfs reproduction demonstrated one closed
internal FD in `os.listdir(retained_directory_fd)`'s returned names while the
original directory FD was still valid. CPython's
[POSIX directory implementation](https://github.com/python/cpython/blob/3.14/Modules/posixmodule.c)
duplicates an FD for directory enumeration. Unlike listdir's completed list,
scandir retains that duplicate while yielding each entry, closing it at EOF,
error or context exit.

The successor checks each entry inside the live scandir context. It does not
materialize the iterator, ignore EBADF, retry, or skip any yielded FD. Canonical
bounded numeric names, uniqueness, at most 128 entries and presence of the
original directory FD are required. Every same-device FD must be read-only.
Any real uncertainty still permanently seals the enclosing bridge. This is a
single-threaded owned-process inventory, not an atomic global FD snapshot or
support for concurrent descriptor mutation.

Before-effect labels are fixed ASCII literals emitted only to the already-owned
private child stderr. They contain no exception text, paths, arguments or data.
The emitter caps output at 128 labels and permanently seals on malformed input,
short write or uncertainty. A label means only that a boundary was about to be
attempted, never that it completed. No post-failure diagnostic operation is
introduced. Parent/independent review must inspect these new emission paths.

## Fixed dependency graph and execution boundary

The new probe, bridge, strict validator and whole-invocation guard live here.
The unchanged admission and manifest come from `../reviewed_tmpfs_elf/`;
containment and guest inventory come from `../real_resolved_binary/`.
The loader has literal hashes and filenames with no fallback. The stage is
`/home/kdk_vm/.cache/t3-live-fd-tmpfs-review-1`, distinct from every measured
generation; create-only 0700 directories, 0500 inert code and 0600 data remain.
No stage has been created or invoked for this successor.

All sixteen sources and ancestors must be admitted before copies, followed by
retained original-FD rechecks, read-only tmpfs superblock and file binds, no
writable copy FDs, and full copy verification before private bus/resolved
launch. Strict actual mapping device/inode binding, two complete map passes,
fixed manifest, credentials, namespace/proc ownership and typed successful
receipts are unchanged. No new ELF, alias, search fallback, broker/core or DNS
setter is admitted.

The unchanged reachable containment is
`../real_resolved_binary/probe.py` with SHA
`2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592`.
Review its `OwnedProcess`, `child_status`, `wait_child`, `supervise`,
`reap_child`, `quarantine`, `stop`, `command` and `isolate` in this composition.
The new probe calls `base.supervise(child, 65)` once. Known WNOWAIT-anchored
containment is distinct from unknown wait status; namespace-init death is not
successful cleanup proof. Unknown or nonzero supervision stops before receipt
reads, after-queries, archives, retry, signals or reaping. Normal owned-child
stop occurs only on the completely known successful observation path.

Only whole-wrapper known zero permits separately approved private export and
independent exact-fixture absence checks. No VM staging or execution is
authorized until this exact generation's source gates, full parent and
independent reviews, frozen pins and exclusive lease are complete.

Source controls include real local procfs iteration, both writable access modes,
real-FD injected uncertainty and permanent refusal, malformed/duplicate/
overflow inventories, per-entry ordering, bounded finite breadcrumbs and
failure-before-effect controls, plus the complete inherited copy/map and strict
receipt/terminal-wrapper counterexamples. They never mount, enter namespaces,
launch a daemon or access the guest.
