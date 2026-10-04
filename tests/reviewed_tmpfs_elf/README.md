# Reviewed private-copy admission proposal

Developer-only, source-only preparation following the manually reviewed static
capture at `9c71ecdbb90b3b93748a7549c8873ee804532ba9`. There is no launcher,
wrapper, subprocess, mount, guest invocation or production integration here.
The earlier measured fixtures and their manifests remain unchanged.

`copy-manifest.json` has exactly the original 16 logical paths plus
`/usr/lib/libbrotlicommon.so.1.2.0`: 17 logical paths, 16 canonical objects.
It does not add the observed canonical loader alias or a Brotli SONAME alias.
All original path/mode/hash rows are preserved. The manifest separately records
the 16 original-FD identities and public package name/version/files/desc hashes
from the manually reviewed typed result, whose SHA is pinned in the manifest.
These are historical provenance facts, not a new package-database observation.
Neither future observations nor unknown mapped objects can extend the table.

The inert `admission.py` verifies exact manifest bytes and opens each canonical
source relative to retained directory FDs from `/`, with no-follow at every
component. It requires the private chroot root's namespace owner 0 and installed
`/usr` ancestors' unmapped owner 65534, non-writable-by-group/other directories,
and the exact reviewed source device/inode/size/mode/nlink. All source FDs and
ancestor FDs stay open together; bounded ELF-header/hash checks and complete
FD/path/ancestor identity checks include mtime/ctime. No copy can be requested
through this helper: admission failure closes only its own FDs and returns no
usable source set. Namespace validation remains a required caller precondition,
not something this inert module claims to establish.

The first recheck error permanently seals the source set before propagation.
Recovering metadata or advancing the deadline cannot permit another read/hash,
entry, or FD accessor call. Closing also seals before cleanup and cannot turn an
empty set into a successful admission. Cleanup attempts each owned FD number
only once, including an ambiguous close error; repeated close is inert. This
corrects the retry/empty-after-close gap in source-only checkpoint `60c8265`,
which was never executable or VM-eligible.

## Remaining integration contract (not implemented or executable here)

A separately reviewed fixture must validate the frozen containment first, then
admit every source before any overlay. It must copy from these same retained FDs
into a new private tmpfs, with create-only destinations, existing 32-MiB/file and
128-MiB aggregate bounds, exact byte count and destination hash, namespace-owned
uid/gid 0, exact mode and nlink 1. It must recheck every original source and full
ancestor identity after copying and before publishing any bind. Temporary copy
creation must not be mistaken for a loaded-object identity proof.

The whole tmpfs superblock and each file bind must become read-only before
launch. No writable copy FD may survive. The frozen strict tmpfs filesystem and
mount-policy checks remain required; read-only bind flags alone are insufficient.
Retained copy FDs must be matched to each actual `/proc/PID/maps` device/inode,
then opened original copy metadata/hash rechecked. Installed Btrfs device values
must never substitute for copied tmpfs mapping identity. Unknown/deleted paths,
changed maps, wrong device/inode, or ambiguous process state refuse. No pathname
hash alone is described as loaded ELF proof.

Tests currently exercise exact one-object manifest admission, original-entry
preservation, all-source FD retention, strict original identity and parent
ownership, same-byte replacement, actual change/reversion, parent replacement,
deadline/read bounds and unknown-fstat FD closure. These tests are not a VM gate.
New copy/map integration, its negative controls, a separately sealed outer guard,
full parent/independent review and an explicit exclusive lease are still needed
before any private bus/resolved invocation. Broker/core/DNS acceptance requires
its own later review; this proposal authorizes none of those effects.
