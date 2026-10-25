# Explicit Product image upgrade

Status: implementation proposal, not installed admission. This extends only the
existing `product-epochs` helper's root administration. The current installed
create-only enrollment remains untouched. Public/default activation, enrollment
repair, restart of an uncertain context and schema1/2/3 installer promotion are
outside this boundary. The [Product epoch contract](T3_PRODUCT_WITNESS_EPOCHS.md)
continues to own effect authority.

## One normal upgrade

The complete journey is owned Off and no pending close → normal owner and helper
stop with original completion/descriptor custody → qualified retirement of that
original helper's stale socket → one normal checked package replacement → explicit
root enrollment transition → genuinely new helper and normal Product owner →
selected-set close and normal return/preservation. No old operation is resent.
Package hooks never enroll, transition, enable or start the provider.

The existing helper executable gains closed root-only administration commands:

* `--product-enrollment-seal UID`: read-only predecessor seal. The old helper
  cannot provide this new selector: preserve the exact old record identity and
  bytes in the pre-replacement baseline, then use the newly installed helper to
  seal that SAME unchanged record and compare against that baseline. This read
  does not use `Roots::capture`, which intentionally rejects old image hashes
  after replacement. No staged helper is executed at the installed boundary.
* `--upgrade-product UID PREDECESSOR_SEAL NEXT_CORE_SHA NEXT_RUNTIME_SHA`:
  one exact predecessor-to-successor attempt at the existing fixed paths.
* `--inspect-product-upgrade UID OPERATION_ID`: read-only original disposition.

The canonical operation ID is derived from the versioned UID/predecessor-seal/
next-hash tuple before ANY mutation, including initial lock creation. It is
available without receiving administration output, so lost replies never require
resending the transition. Repeated invocation refuses another exchange.

These selectors accept canonical public identity data, never a path, PID, raw
record, profile, effect permit or arbitrary command. Actual UID/EUID/GID/EGID
must all be zero. The running administrator image must match the fixed installed
helper. The predecessor seal covers the exact held/named enrollment identity,
payload and root ancestry, not only equal bytes. Its enrolled UID must match.
Live held/name comparisons retain the complete kernel device number. Durable
seals and journal identities instead bind the descriptor-observed Btrfs
filesystem/subvolume FSID, inode and full ownership/mode/link/size/timestamps.
All fixed image files and retained ancestry directories must satisfy the
explicit Btrfs profile; distinct Btrfs subvolumes retain their own bound FSIDs.
Zero FSID or another filesystem type
refuses before fresh enrollment's first mkdir or any upgrade publication.
This follows the Btrfs-specific [kernel statfs implementation](https://github.com/torvalds/linux/blob/v6.17/fs/btrfs/super.c#L1732-L1742),
not a generic promise about statvfs identifiers. Device renumbering never relaxes
a same-process comparison. Existing journals are not rewritten or migrated.
Actual same-VM reboot qualification of this persistent profile remains required;
synthetic device renumbering and a same-boot fixture do not establish it.
The new core/runtime are the fixed root755 single-link native ELF images, bounded
and hashed under one absolute five-second sampled deadline. Both explicit next
hashes must match; an unchanged pair refuses. No old image or record is repaired.

Only the existing `/var/lib/omavless-image-product` namespace is used. The root
writer holds an exact NONBLOCKING exclusive maintenance lock under that SAME
five-second deadline and rechecks its held/named identity before exchange.
Product readers acquire/validate the same shared lock BEFORE pending inspection
or Roots capture, retain it until listener/Bindings/Roots have dropped, and refuse pending
administration. The provider never creates or repairs this lock. Fresh explicit
enrollment creates it; the upgrade administrator may introduce it only after all
old-record/new-image inputs and socket absence have been validated. Developer
classes, wire protocol, service capability/thread/FD limits and ordinary units
remain unchanged.

The bounded root history reserves before publication and never evicts; an
unfinished prefix, foreign entry, duplicate transition or entry129 refuses.
Each slot contains an immutable identity manifest, an exclusive staged record,
the predecessor and a completion marker. All entries are fixed-shape root-owned,
single-link objects below retained no-follow root directories. The active
pending marker inhibits Product admission. Incomplete or foreign history also
refuses without a well-formed pending marker. No service, package or network
operation occurs inside this fixed filesystem primitive.

After durable staging and an attempt claim, one atomic exchange installs the
new record while preserving the exact predecessor inode. There is no ordinary
overwrite, directory reselection, re-enrollment or new provider namespace.
Preserve the immutable OriginalBefore seal; the owned exchange may change ctime,
so postconditions prove the same predecessor inode/bytes/ownership and that owned
rename transition rather than demanding unchanged pre-rename ctime.
Original held/name, payload, image, lock and deadline checks surround publication.
All required file/directory syncs and the completion record precede removal of
the pending fence; parent-directory sync follows its removal. An uncertain
attempt is never exchanged again. Read-only
inspection distinguishes retained incomplete state from an observed committed
transition; it does not reconstruct authority or repair a prefix. A late
administration error does not retract an already published valid generation and
is never reported as the original command's success. Inspection reports observed
committed only after all current committed predicates are read back; this does
not supply the original missing durability proof or replay permission.

## Proof required before the live transition

Use the same production filesystem operations in finite owned public fixtures:
wrong predecessor/UID/hash/image, unsafe ancestors, symlink/hardlink/identity
drift, concurrent service admission, live socket, capacity, interruption at each
publication boundary, retained predecessor and lost administration reply. The
nonroot source fixture is not actual root enrollment. A separately admitted
same-VM root fixture must exercise the inert temporary tree before touching the
installed namespace. Production has no caller-selected test paths or fault flags.

The real journey additionally requires exact package/source/helper provenance,
fresh root and original actor/socket custody, full private-state preservation,
new owner/opaque-token identity and rejection of old requests. Native proof,
per-member outcomes, receipt-only resolution, original TCP EOF/other-stream echo,
FD plateau and ordinary Off remain independently required. Source, package or CI
success does not establish these installed results. The actual existing-namespace
record transition requires its own concrete admission choice after this kit is
implemented and reviewable; this proposal grants no such effect.

Two explicitly ignored, test-binary-only root fixtures prepare and inspect the
same retained inert tree across a real reboot. They require actual root and one
public `OMAVLESS_IMAGE_UPGRADE_REBOOT_FIXTURE` path constrained below `/var/lib`
with the fixed `omavless-upgrade-reboot-fixture-` prefix. Preparation is create-only;
inspection requires a different boot, preserves the original preparation result,
and performs no commit, repair or cleanup. These fixtures bypass fixed-helper
self identity only in test code, so they prove filesystem transaction custody,
not actual provider enrollment or native effect authority. Production accepts
neither this path nor this test selector/environment setting.
