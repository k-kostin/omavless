# R5 owned controller permission normalization

Independent runtime fix based on main, discovered during native diagnostics UI
acceptance on Try Omarchy ARM64 (Mihomo v1.19.30). Connection readiness accepted
the owned Mihomo Unix socket at 0666 under its private 0700 directory, but strict
diagnostic reads require 0600 and returned `capability_unavailable`.

After configured readiness and before successful startup admission, normalize
only the authenticated owned child socket to 0600. The existing private parent
is pinned with O_PATH, the socket inode is pinned with O_PATH/NOFOLLOW, and peer
credentials must match the current user's UID and unreaped child PID. Mode is
changed through the held descriptor's fixed procfs path, not a re-resolved
caller pathname. Parent/socket identities are rechecked; failure keeps the
child under existing rollback ownership. No new dependency, unsafe code,
privileged command or relaxed diagnostic permission check is introduced.

Tests cover idempotent 0600, wrong peer/UID, unsafe parent, symlink/regular file,
and replacement immediately before chmod. A replacement file remains untouched
and admission fails. The installed-Mihomo no-TUN test now exercises actual
start/commit, checks 0600, reads the strict diagnostic summary and retains the
existing child-death observation assertions.

This is a Rust-specific integration correction, not parity with permissive
socket modes. Python remains untouched as migration reference. Full workspace,
clippy and installed exact-candidate host results belong in the PR; this does
not complete R5/R6 or merge unfinished UI into main.
