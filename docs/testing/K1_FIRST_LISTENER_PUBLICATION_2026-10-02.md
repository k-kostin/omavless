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
