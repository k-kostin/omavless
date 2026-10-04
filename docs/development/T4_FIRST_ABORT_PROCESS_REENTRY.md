# First-Abort fixed-current process-loss fixture

Base: reviewed `ffe6116dfa937e0e1f9e8713bee1bcd601717bf0`, Draft #594.
The exact-head execution disposition below separates inner fixture evidence,
failed whole-invocation guarding and a later independent observation.

The stable cfg(test)-only `restore_first_abort_process_tests.rs` module adds an
explicitly ignored parent matrix and worker. The ordinary build has no hook,
environment fault selector, dispatcher, CLI or IPC registration. A thread-local
checkpoint executes only after actual owner gate observations and retained
checks, plus the final pre-return boundary. The pure default-inert/thread-local
test runs in ordinary source gates; the process matrix does not.

The worker calls the exact private `current` constructor: fresh encrypted
archive authentication, fixed current paths, actual `ObservationOnlyNativeHost`
and existing lease. HOME is unchanged. Child-only OMAVLESS_HOME and XDG state/
runtime paths point into a new exclusive fixture subtree, never existing
canonical application paths. Real /proc and /sys observations remain intact;
uncertainty is refusal, not a fabricated successful checkpoint. The absent
managed-pair selector is not replaced by a fake receipt. A symlink to the frozen
test ELF supplies only executable-path resolution; it is not Mihomo and is never
executed as a core or treated as managed package authority. No application
start/stop or host network mutation is involved. VM execution requires its own
explicit lease and authorization.

Archive ancestry policy rejects /var/tmp and /tmp because they are writable by
other users. Instead, the fixture requires a create-only random private subtree
under `/run/user/<uid>`, with every ancestor checked for owner/mode, symlinks and
Git metadata before writing. The archive and schema-1 marker are private,
bounded files. Archive path, bytes and identity are retained across child exec.
This volatile fixture proves neither reboot persistence nor power-loss durability.

Before execution, build the reviewed exact head once and copy its test ELF to an
exclusive private location outside Cargo's target tree. Record commit and SHA-256,
chmod 0500, and invoke only that frozen executable with the exact ignored matrix
name. The harness and every child verify current executable path, ELF magic,
read-only private mode, single link, bounded size, SHA-256 and exclusion from the declared
host build-target tree. The canonical host source path is recorded and validated
on the build host; a copied guest receives only normalized lexical host provenance,
not a claim that this build directory exists in the guest. Original O_NOFOLLOW descriptor and parent identity stay held;
path/descriptor metadata are checked before and after the bounded streamed hash.
A retained /proc/self/exe descriptor must match that same inode, including in
each worker, rather than accepting a same-byte replacement as executed identity.
Do not run the matrix from Cargo or while changing its frozen
artifact. The synthetic passphrase travels only over inherited stdin.

The five requested SIGKILL checkpoints are: OLD store slot linked, first member
renamed to OLD while template is NEW, empty terminal created, full Abort terminal
written, and final wrapper observation before return. Parent and child both
verify the requested phase. The parent signals only after raw WNOWAIT proves its
exact unreaped child still live. Final exact WNOHANG waitpid must match the
previous observed terminal PID and status, including raw signal 9 for crash.
No Child polling/waiting/signalling convenience API participates. Any unknown
observation, EINTR/ECHILD, mismatched PID/status, failed signal/reap, missing
checkpoint or timeout permanently quarantines the whole matrix: no further
spawn/query/signal/reap/cleanup. The original Child handle/pipes are retained
even on unwind, without hidden destructor reaping. Recovery timeouts therefore
preserve potentially live children for explicit owner diagnosis, never silently
kill them. Four cases must freshly reopen to AbortedStillFenced and exact OLD;
the empty-terminal case must refuse before observer/checkpoint and retain the
same empty inode. Existing Abort re-entry must retain terminal identity. Archive
identity, ordinary startup refusal and pending fence are checked independently.

Reports are bounded private schema-1 files, with private stderr. No automatic
cleanup is attempted; root reviews exact fixture paths before removal. Keep the
first failure and its phase disposition rather than silently rerunning. Root
code review and exact source/Rust/clippy gates are required before the matrix is
authorized. Installed acceptance, power-loss testing, product registration and
merge/release remain separate.

The initial `5860c4f` fixture passed its ordinary source (521/2 skipped) and full
Rust (2051 aggregate passing executions) gates but FAILED root source review:
Child convenience supervision and writable/path-only ELF provenance were not
adequate. Its process matrix was never executed. Those gate results do not
authorize the fixture or erase this review NONPASS. Corrections require a new
exact head, pure uncertainty/FD-substitution tests and repeated full gates.

## Whole-invocation VM guard

`tests/first_abort_process/vm_guard.py` is developer-only, separately reviewed
before a lease or invocation. A private schema-1 sealed-copy receipt binds full
head, original host build/FD metadata, frozen host copy and guest ELF hash plus
guard hash. The guest validates lexical host provenance separately from its real
guest original descriptors; it never creates a dummy build directory. Staging
and ELF descriptors/metadata/hashes are checked before and after, mode 0500.

The guard reuses T3 #597's nine literal preservation checks: CANONICAL_EPOCH,
PRIVATE_FILES (four hashes), USER_SERVICE, EXECUTABLE, NAMESPACE, CORE_INVENTORY,
TUN_INVENTORY, RESOLVER and RESOLVCONF. It pins the approved active PID938 epoch,
and snapshots full IPv4/IPv6 address/routes-all-tables/rules. The sole comparison
exception is the precise approved address valid/preferred lifetime countdown.
Fixed read-only root unit metadata also preserves omavless-dns-broker.service,
systemd-resolved.service, omavless-k1-namespace-filter-fixture.service and
omavless-k1-generator-filter-fixture.service; neither presence nor inactivity
confers authority to change them. No activation traversal policy is imported.

All guest inventory commands and the one matrix child use raw WNOWAIT/exact
waitpid with permanent uncertainty quarantine and no hidden Popen reap. A
nonzero matrix may represent unsettled descendants: stop without after-query,
signals or cleanup, retain private evidence, and request separately authorized
diagnosis. Only exact success may collect five bounded case receipts, verify
no residual executed-ELF inode, compare the complete baseline and recheck staged
objects. The guard is not automatically run by source gates; tests use only
pure mocked process outcomes and local synthetic file substitutions.

## Exact-head execution and retained NONPASS

Tested source: `3a2504052d24a1f699e40933292976e3b0d80642`, Draft #598.
Ordinary gates passed: source 530 tests / 2 skipped plus QML; full Rust 2057
aggregate passing executions, including runtime 1158 / 38 ignored / 1 filtered,
separate cleanup test, formatting, strict workspace/TUI Clippy and parity.
These counts do not include an automatically executed process matrix.

One separately authorized invocation used the sealed ELF SHA-256
`0189c495cccfaf3c32da75da0aac2f40996adaa656e8af9474dccf71b6d84bb0`, guard
`3631492302b3e87beb0b2a801ded9ee5df1235a5c125117b0f4728596067049d` and receipt
`41a198502f40c1f3de4000b88afb4e6b85a74718226c050ebd07099591ea9803`.
The inner matrix reported 1 passed / 0 failed in 125.95 seconds. All five private
schema-1 case receipts were independently inspected: signal 9 at each checkpoint;
linked, mixed, full and final reported `aborted-still-fenced`; empty reported
`refused-preserved`. Fourteen captured worker stderr files were empty.

**The outer guard exited 2: NONPASS.** Its before-baseline exists, but its
after-baseline and final result do not. The confirmed failure interval is after
inner matrix completion and before after-baseline persistence; the original
exception and exact failing operation remain unproven. There was no automatic
retry, signal, reap, cleanup or after-query following the failed invocation.

A separately authorized read-only user diagnostic copied only the five exact
named fixture roots and bounded stage evidence. Its process inventory stopped
on `PermissionError`: independent quiescence was UNKNOWN, not established by an
empty partial inventory. Private capture archive SHA-256:
`500bcb8fdd70ff43fcae1fd396da49c02fdb9cc4e13b2a2e69c12911f43be443`.

A subsequent, separately reviewed and authorized fixed read-only root diagnostic
(`f487f144f90572b443972a516f1975eb1633f4cddab6986c3916e81b115ae492`)
exited 0. It observed 44 current UID-1000 executables using stable original
descriptors and found the exact fixture inode absent at that observation. Its
comparison against the original before-baseline matched all nine canonical
categories plus the fixed root-unit category; full IPv4/IPv6 matched except the
already approved address-lifetime countdown. Independent private result SHA-256:
`00e45ce530859c53ea7b2025f8418d0c7cc5dc74383e2b8167c0e1655e16cac2`;
independent after-baseline SHA-256:
`35daccecc937e78aac7c062a4e89f907620c24847d5bf76a2880a22cffb9ed97`.

This later point-in-time observation is neither atomic general descendant
absence nor evidence of uninterrupted preservation across the original guard.
It does not repair that guard's NONPASS or establish its original exception.
All guest originals, private archives and earlier failures remain retained.
Inner synthetic process-loss cases PASS; whole-invocation acceptance NONPASS;
product, installed, reboot and power-loss acceptance remain unclaimed.
