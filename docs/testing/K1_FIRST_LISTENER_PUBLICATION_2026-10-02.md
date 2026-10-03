# K1 inactive first-listener publication — 2026-10-02

This records an **inactive candidate**, stacked after the enrolled
single-session/path-admission drafts. No root service was installed, no fixed
`/run/omavless-netguard` directory was created on the host or VM, and no
firewall, route, VPN, package, or user's network state was changed.

The first-publication candidate creates a new private mode-0700 directory,
binds a fixed Unix socket with backlog 8, pins the original directory and
socket entries, sets the socket's group/mode while the directory is private,
then grants group traversal only as its final publication step. Final admission
compares the pinned entries to the current path. It no longer accepts an
arbitrary already-bound descriptor by checking only `local_addr` and a fresh
path lookup. Existing directories are refused rather than adopted or removed;
an interrupted first publication leaves private artifacts for explicit
recovery, not automatic deletion.

Deterministic tests cover successful private-bind publication and a synthetic
status exchange, an interrupted publication, an existing or symlinked
directory, and socket/directory replacement during publication. The directory
replacement test uses an otherwise valid 0750/0660 alternate listener, not
merely a malformed placeholder. The publisher still has no installed caller.

Not established here: root-package group identity/membership, an existing
socket's safe restart/retirement, authorization service policy, network
namespace provenance, actual nftables authority, boot ordering, kernel
firewall behavior or the mandatory host matrix in the [K1 contract](../roadmap/KILL_SWITCH.md).
Filename-based socket mode/owner changes are rechecked against the pinned
entry before and after, but a concurrent trusted-root replacement remains
outside this uninstalled candidate's guarantee. No K1 availability or
kill-switch claim follows from these tests.

## Umask-independent synthetic fixtures — October 4 continuation

Draft #592 code head `a375ec0432855bd7142bb18c011b5e63faa37941`
changes only three directory-creation sites in `locked_state_session_tests.rs`:
create private 0700, then explicitly set the intended synthetic 0750 mode.
The previous `DirBuilder::mode(0750)` was masked to 0700 under umask 077.
No production permissions, admission assertions or process-global umask change.

The frozen predecessor binary genuinely produced 18 passing / 5 failing focused
tests under 077, and all 23 passed under 022. The corrected binary passes all
23 under each umask in separate child processes. Four failures had prevented
admission setup; the fifth exposed the alternate-directory mode mismatch.
These were permission-fixture failures, not socket-path-length failures.

Retained October 3 full Rust output reaches the final successful strict clippy
and matched R0 parity result without a failed Rust result. It includes the
workspace, serialized helper cleanup and DNS suites, TUI terminal checks,
normal and TUI clippy/check gates from `tests/run-rust.sh`. The process session
was no longer available after the owner pause; this is inspection of completed
output, not a newly recovered process exit status.

The first source gate is retained as FAILED: 512 tests, three V0
`private_file_inside_git` errors, two skipped. The corrected October 4 run
explicitly places TMPDIR outside Git and exits zero: 512 tests, two skipped,
plus JavaScript and QML contracts. No privacy guard was weakened. Logs and
build outputs remain outside Git; logs are private mode 0600.

All three GitHub checks (Test, x86_64 package, ARM64 package) passed on the exact
#592 code head above and on #587 report head
`e78efc427c72832acb4717db81aaeeb974271a40`. Documentation added afterward
does not relabel those identities. No new VM, installed service or network
gate was run. Namespace/socket provenance, normal ownership adoption, root
service and boot-policy acceptance remain open; K1 remains unavailable.
