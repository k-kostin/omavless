# Encoder/libm copy admission proposal

This developer-only successor uses the exact #632 static observation at
`5f4f5ca583fb2759e90b13413c53e6f3632ea8f0`. The whole wrapper completed
known zero with canonical/private/service/core/TUN/resolver/network preservation.
The separately reviewed fixed-file collector also completed known zero and
validated 21 original regular files, all eight source pins, the complete finite
receipt-bound event sequence and recorded network comparisons. Its private
report SHA-256 is
`0e7e77200644b3062b68a3bd218a6cefd83508de7ee48360ec1682c20606528c`;
the probe's canonical public result SHA-256 is
`0f1af9f222a6f5e48eba3ac2b82dfd547f6b382394c4d5edc53ee3ba56aeb305`.
No raw private logs, network snapshots, settings or credentials are included.

## Exact finite delta

The proposed immutable table adds only two original canonical objects:

- `/usr/lib/libbrotlienc.so.1.2.0`: device31/inode8052, 780768 bytes, root0:0,
  mode0100755, nlink1; original Brotli1.2.0-1 package membership.
- `/usr/lib/libm.so.6`: device31/inode3444, 1268336 bytes, root0:0,
  mode0100755, nlink1; original glibc2.44+r24+g16be1518495f-1 membership.

Their original hashes, ELF headers, declarations, original metadata, package
pins and recursively closed five-object static graph are embedded as public
evidence. Encoder needs libm/common/libc; libm needs libc/loader; the measured
common, libc and loader remain exact predecessor objects. Local package records
do not prove package signatures or globally unique package ownership.

All previous 18 logical paths/17 original objects and decoder evidence remain
unchanged. The table becomes 20 logical paths/19 original objects. Observed
SONAME/interpreter aliases are evidence only: no additional alias, fallback,
search path or dynamically discovered object becomes a copy target.
Static `DT_NEEDED` closure does not prove arbitrary `dlopen` behavior.

`admission.py` changes only the exact manifest hash and exact counts from its
reviewed predecessor. It retains all canonical originals and ancestors before
any caller could copy; installed root must appear as unmapped65534 in the
already validated private namespace, unlike namespace-root-owned copies.
Identity, hash, full metadata/path/parent checks, bounds and permanently sealed
failure behavior are unchanged. It has no mounts, process launch or guest entry.

## What this does not establish

This is an explicit source proposal, not automatic allowlist adoption from an
observation, installed package approval, loaded-object identity or T3 acceptance.
Original per-file FD stability is not an atomic multi-file snapshot. Historical
nonpasses and their parked namespaces remain unchanged and uncleaned.

The next distinct launcher must bind these exact bytes, retain the original
FD/copy/proc anchors, compare actual maps against read-only copied device/inode
identities twice, retain zero-only terminal supervision, complete positive
daemon shutdown and baseline gates, and pass full parent/independent review.
No old fixture is retried or relabelled by this proposal. Production runtime,
frontend, packages, main, RC and primary-PC networking are unchanged.

The inert controls repeat the retained-source, metadata/path/ancestor,
replacement/change-reversion/deadline/permanent-refusal matrix for all19 objects,
check both new object shapes, reject old/altered/alias-expanded manifests before
source open, and verify the complete exact canonical result hash without losing
u64 precision. They do not execute an ELF, daemon or guest.
