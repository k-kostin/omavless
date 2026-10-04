# First-Abort fixed-current process-loss fixture

Base: reviewed `ffe6116dfa937e0e1f9e8713bee1bcd601717bf0`, Draft #594.
This is source implementation pending review, not an executed acceptance claim.

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
start/stop, host network mutation or VM action is involved.

Archive ancestry policy rejects /var/tmp and /tmp because they are writable by
other users. Instead, the fixture requires a create-only random private subtree
under `/run/user/<uid>`, with every ancestor checked for owner/mode, symlinks and
Git metadata before writing. The archive and schema-1 marker are private,
bounded files. Archive path, bytes and identity are retained across child exec.
This volatile fixture proves neither reboot persistence nor power-loss durability.

Before execution, build the reviewed exact head once and copy its test ELF to an
exclusive private location outside Cargo's target tree. Record commit and SHA-256,
chmod 0700, and invoke only that frozen executable with the exact ignored matrix
name. The harness and every child verify current executable path, ELF magic,
private mode, single link, bounded size, SHA-256 and exclusion from the declared
build-target tree. Do not run the matrix from Cargo or while changing its frozen
artifact. The synthetic passphrase travels only over inherited stdin.

The five requested SIGKILL checkpoints are: OLD store slot linked, first member
renamed to OLD while template is NEW, empty terminal created, full Abort terminal
written, and final wrapper observation before return. Parent and child both
verify the requested phase. The parent signals only its exact owned child, waits
for raw signal 9 and refuses an early exit/missed checkpoint. Recovery children
have a bounded deadline; timeout kills only that known child and preserves all
artifacts. Four cases must freshly reopen to AbortedStillFenced and exact OLD;
the empty-terminal case must refuse before observer/checkpoint and retain the
same empty inode. Existing Abort re-entry must retain terminal identity. Archive
identity, ordinary startup refusal and pending fence are checked independently.

Reports are bounded private schema-1 files, with private stderr. No automatic
cleanup is attempted; root reviews exact fixture paths before removal. Keep the
first failure and its phase disposition rather than silently rerunning. Root
code review and exact source/Rust/clippy gates are required before the matrix is
authorized. Installed acceptance, power-loss testing, product registration and
merge/release remain separate.
