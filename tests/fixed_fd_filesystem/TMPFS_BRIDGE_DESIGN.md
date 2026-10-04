# Proposed private read-only ELF-copy bridge — not implemented or authorized

The existing inventory and `90cca2a` refusal stay frozen. A new inventory-only
fixture would preserve strict maps device/inode matching by moving the reviewed
public ELF inputs onto private tmpfs, not by redefining Btrfs identities. It
would run only private bus/resolved until separately reviewed; broker/core and
DNS cases remain a later, separately authorized integration.

## Fixed input set and provenance

Use all **16 logical entries** from the unchanged SHA-256-pinned
`real_resolved_binary/guest-inventory.json`, representing **15 unique resolved
targets** (two loader aliases share one target). Verify the manifest's literal
key/path/hash set; reject additions, removals, ambiguous aliases and hash changes.
Do not discover or admit dependencies by executing `ldd`, observing maps, or
copying paths from a failed run.

Inside the already verified private user/mount/PID/network boundaries and root,
create a dedicated bounded tmpfs copy store. Before any overlay bind or daemon
launch, open every fixed resolved source no-follow, verify installed root-owned
provenance (UID 65534 in the namespace), regular shape, exact expected mode,
size bounds, ELF header and pinned content hash through the same stable FD.
Copy into exclusive no-follow namespace-root-owned files; verify full bounded
writes, destination size/hash and source before/after FD identity. Keep source
and destination receipts distinct. All 15 copies must complete before proceeding.

Bound each file and the aggregate bytes, number of FDs and total elapsed time.
Preserve all alias targets unchanged; create no arbitrary path selected by input.
Reject source rename/replacement or changing content rather than adopting new
package bytes. These are byte-identical test copies, not installed executables.

## Freeze before launch

Close every writable copy FD. Remount the entire private tmpfs store read-only
(not merely a read-only bind leaving an exposed writable alias). Verify the
superblock filesystem type/magic, ownership and exact RO/nosuid/nodev policy.
Bind each fixed copied file at its corresponding resolved `/usr` target inside
the private chroot, and enforce RO/nosuid/nodev on each bind. No host mount,
canonical `/usr`, source package inode, service or network is changed.

Reopen each final target no-follow and prove its device/inode equals the retained
copy receipt, filesystem is tmpfs, metadata/hash matches, and no writable copy
FD remains. Recheck the complete fixed target table and mount policy before
forking a daemon. Preserve the existing read-only base `/usr` and config masks.
Bootstrap helper invocations retain current unknown-child/quarantine discipline.

## Actual mapped proof and scope

For each observed bus/resolved file-backed mapping, require a known copied target,
maps device/inode equal to the opened target and retained copy identity, stable
FD metadata, and pinned digest. Copy ownership is explicitly namespace root 0,
not a global relaxation of the installed-file UID 65534 predicate. Recheck maps,
credentials, copied mount policy and identities at final observation. Unexpected
pathname spelling, deleted mappings, non-copied libraries or any unknown state
refuse. A new dlopen dependency requires independent provenance and a separately
reviewed new immutable manifest; observing it never grants admission.

This bridge proves the identity of mapped **copies of pinned packaged bytes**.
It is not installed acceptance and does not prove every possible future dlopen.
Do not reuse the full broker/core fixture until the new exact containment/source
hashes, helper pins, pure gates and independent whole-invocation guard are reviewed.

## Required counterexamples before a VM proposal

- Manifest additions/removals, duplicate target with conflicting hashes, alias drift.
- Symlink/special source, non-root source, changed mode, oversized aggregate,
  same-byte path replacement, short reads/writes, source mutation and timeout.
- Existing destination, replaced destination, changed copied bytes, writable FD
  survivor, writable store alias, wrong filesystem, wrong target bind or mount flags.
- Same bytes on the wrong mapped inode/device; unknown/deleted mapped path;
  final mapping/mount change; child observation or cleanup uncertainty.
- All copy checks finish before the first bind/daemon; a first refusal prevents
  every subsequent copy/mount/launch action except known-safe owned cleanup.

The immutable outer guard still needs canonical epoch/artifact checks, all eight
baseline categories, full IPv4/IPv6 comparison, bounded whole invocation,
independent quiescence, retained archive and a new exclusive VM lease.
