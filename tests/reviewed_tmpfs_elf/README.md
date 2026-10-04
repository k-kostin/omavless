# Reviewed private-copy admission proposal

Developer-only, source-only preparation following the manually reviewed static
capture at `9c71ecdbb90b3b93748a7549c8873ee804532ba9`. A fixed launcher, strict
receipt validator and whole-invocation wrapper are now source-only proposals.
There has been no guest invocation or production integration. Source tests
never execute the mount/namespace/daemon entry point.
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

## Successor copy/map integration (source only)

The separate successor branch based on sealed `9e7fd08` adds `bridge.py`; the
original #602 admission checkpoint and all historical measured fixtures remain
unchanged. The launcher supplies the exact frozen containment and admission
modules through a fixed-pin loader, not caller-selected implementations.
No module discovery or fallback loader exists in this integration module.

`Bridge.prepare` validates the original private namespace mappings/proc/root
boundary, then creates the fixed bounded tmpfs. All sixteen original sources
and their ancestors are admitted together before the first copy. Copying reads
only their retained FDs into exclusive indexed destinations. The bridge retains
only read-only copy FDs, checks content and exact namespace-root metadata, then
performs full original-source rechecks both after copying and after freezing the
whole tmpfs, before the first bind. Every staged name is matched to its retained
copy and rehashed before publication. The store FD and its complete identity
are retained through later verification, as well as every copy FD.

Whole-superblock and per-file mount checks require tmpfs, RO, nosuid and nodev;
the writable-FD check retains its enumeration FD and treats every unknown stat
as a refusal rather than assuming a transient closed directory FD. Actual map
device/inode must match a retained known copy before target open/hash. Each map
pass compares before/after snapshots and known child liveness. The fixed launcher
invokes two complete passes for each actual child and compares their results.
Unknown or deleted mappings cannot extend the fixed table. Prepare, verification
and inventory use a terminal state latch; first uncertainty and close never
permit retry. Descriptor cleanup is single-attempt and does not query processes.

Thirteen pure integration controls cover ordering, whole-superblock policy,
exclusive real temporary copies of synthetic non-executable bytes, retained
read-only FDs, short writes, destination shape/hash failures, replaced store,
wrong/unknown/deleted/conflicting map identities, two map passes and late drift,
unknown FD inventory and permanent refusal. No test calls a real mount command,
namespace operation, daemon, candidate ELF or guest transport.

## Preserved fixture contract (execution not authorized)

A separately reviewed fixture must wire this integration to frozen containment,
then admit every source before any overlay. It must copy from these same retained FDs
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
The fixed-pin launcher, both complete child map passes, strict typed receipt
validation and whole-invocation outer guard require full parent/independent
review and an explicit exclusive lease
before any private bus/resolved invocation. Broker/core/DNS acceptance requires
its own later review; this proposal authorizes none of those effects.

## Fixed source-only wiring and supervision paths

`probe.py` accepts only the fixed outer opt-in; its internal child invocation has
a fixed executable, filename and root, plus owned original namespace facts.
Its literal dependency pins cover containment, admission, bridge, original
inventory and new copy manifest. All staged modules are loaded from bounded
original FDs with exact hashes and no repository fallback. Both input-directory
ancestries, file ownership/modes/link counts/xattrs and stable identities are
checked. The new fixed stage is
`/home/kdk_vm/.cache/t3-reviewed-tmpfs-copy-review-1`; directories are 0700,
code 0500 and data 0600, with create-only outputs.

Only private dbus and resolved launch after full copy verification. Each child
gets two complete map passes, requiring an unreaped known-live direct child,
matching PID namespace and `/proc/self` PID view. Kernel address ranges,
ordering, permissions, offset/device/inode grammar and anonymous mappings are
checked. Unknown public paths retain a bounded typed refusal from already-read
data only; they are never opened, copied or admitted automatically.

The reachable frozen support is `tests/real_resolved_binary/probe.py`, SHA
`2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592`.
Review must inspect its actual `child_status`, `wait_child`, `supervise`,
`reap_child`, `quarantine`, `stop`, `command`, `isolate` and `OwnedProcess`
paths in this new composition; prior approval alone is not this fixture's gate.
`probe.main` calls `base.supervise(child, 65)` exactly once for its owned unshare
session. The frozen supervisor uses a retained exact WNOWAIT group anchor for
known live/exited subtree containment, including its bounded known timeout
path. That containment is not proof of recovery after an inner refusal.
Unknown wait/signal/reap state latches quarantine: no later process query,
signal or reap is authorized. Nonzero/unknown outer completion stops before
result reads or after-state work. There is no GNU timeout or fallback supervisor.

The observer deliberately has no finally-stop branch: after first failure it
performs no later child query, signal, reap, cleanup or diagnostic read. Normal
`base.stop` occurs only after both successful map passes and final credential/
copy checks, and records genuine integer exit statuses. Existing namespace-init
exit/outer known-anchor containment may terminate the isolated subtree; this is
distinct from claiming successful explicit cleanup after an unknown inner state.

`validate_receipt.py` rejects duplicate keys, nonfinite numbers, unknown/extra
fields, booleans as integers, string collections, altered source or copy
identities/hashes, missing executables and unequal map passes. It requires all
sixteen unique copies and two known-live maps for each child, tied to exact pins.
Only known validated success allows `vm-guard.sh` after-state queries: canonical
epoch, eight baseline categories, all IPv4/IPv6 JSON fields (only independently
confirmed address countdowns may differ), and fixed-form process/root absence.
Failed probe/validation terminates immediately. No automatic archive, cleanup,
retry or reuse of old stages exists. New shell tests execute the actual terminal
fragment against synthetic failed/malformed receipts, without any guest calls.
