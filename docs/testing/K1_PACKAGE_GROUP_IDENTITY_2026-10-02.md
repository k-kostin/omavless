# K1 inactive package-group identity — 2026-10-02

This candidate narrows the [first-listener publisher](K1_FIRST_LISTENER_PUBLICATION_2026-10-02.md):
its fixed-path entry point no longer accepts an arbitrary numeric GID. It
reads a local group named `omavless-netguard` from a bounded, pinned
`/etc/group` file, requiring a unique non-root numeric GID and no duplicate
name or GID. The root-owned file must be a regular, single-link, non-writable
by group/others and must not be a symlink. File identity and bytes are checked
again before opening group access to the socket.

Deterministic tests cover duplicate/invalid group entries, in-place mutation,
replacement and symlink refusal. Mutating the group record at either of the
two publication checkpoints leaves the newly created socket directory at
0700. No group was created and no user membership or current-session access
was changed on the host or Omarchy Dev VM.

This is **not** proof that the chosen group is installed, correctly populated
or active in the service's user session. Administrator provisioning, group
membership, old-socket restart handling, installed service/package ownership
and all K1 firewall/host acceptance remain open. Neither this draft nor its
predecessor makes the kill switch available in OmaVLESS.

## Unrelated local-account compatibility follow-up

The inactive first-enrollment parser still admits only an exact, eligible
requested local login. It now avoids rejecting the entire `/etc/passwd` file
because an **unrelated** account uses an ordinary mixed-case, dotted,
trailing-dollar or leading-digit name, or a system-account home/shell field.
All records still need seven fields and canonical numeric UID/GID; duplicate
numeric UID or duplicate requested login still refuses enrollment. Control
bytes in unrelated names also refuse the file. Synthetic parser tests pass;
this does not create enrollment, a group, a listener or a host service.
