# K1 inactive durable receipt store

This prerequisite follows [receipt assessment](K1_RECEIPT_ADMISSION.md) and
the [kernel capability experiment](K1_KERNEL_CAPABILITIES.md). It adds only
`omavless-netguard::receipt_store`; no production caller, coordinator hookup,
`KernelPort`, service, package hook, network or firewall operation is added.
K1 remains unavailable. A stored receipt never becomes ownership authority.

The follow-on [crash/orphan disposition proposal](K1_CRASH_DISPOSITION.md)
documents remaining authority gaps and conservative review requirements, with
test-only counterexamples. It adds no automatic recovery or ownership adoption.

## Fixed trust boundary

`ReceiptStore::open_fixed(enrolled_uid)` opens the existing
`/var/lib/omavless-netguard` directory via the root-state adapter. Enrollment
must come from separately verified root configuration, not a client request.
It does not provision the directory. Ancestors must be root-owned and not
group/other writable; the final directory must be root:root 0700. Every open
uses pinned directory FDs with no-follow checks. The held final directory is
revalidated against its pinned parent before observations and publication.

The same nonblocking exclusive directory `flock` used by `RootStateStore`
serializes both adapters. A second marker/receipt adapter cannot open during
the first one's lifetime. Future combined coordination needs a reviewed common
lock owner; it must not drop and reacquire the lock between kernel and storage
steps. No such integration is provided here.

Only three fixed leaves are used:

- `table-receipt-v1.json`: retained receipt;
- `.table-receipt-v1.json.next`: exclusive publication stage;
- `.table-receipt-v1.pending`: exclusive publication-in-progress guard.

Receipt leaves must be root:root 0600 regular files with one hard link and at
most 2048 bytes. Symlinks, FIFOs, directories, wrong ownership/mode, excessive
size, rebindings and I/O errors refuse admission. Parsing uses the existing
strict flat schema plus exact canonical re-encoding and enrollment comparison.
Whitespace/reordered equivalent JSON is deliberately refused by this new
experimental store, though the pure decoder can parse it. No migration or
installed receipt format exists yet.

`Missing` means only safely observed missing storage. `Durable` means storage
admission under this adapter's trusted-writer/crash model, not authentication
of the claimed namespace epoch, operation or table. Unsafe, undecodable,
in-progress or uncertain state maps to `UnsafeOrUncertain`, never Missing.
Neither observation authorizes table adoption/deletion, proves a missing table,
acknowledges a kernel effect, or retires the independent generation marker.

## Publication and failure model

The fixed publication primitive compares the observed receipt with the exact
expected `ReceiptRead`, validates enrollment, and publishes a canonical next
record. It is storage CAS only: **not** a phase-transition policy, operation
allocator, monotonic fence or authorization to fabricate `Live` ownership.
These must be enforced by a later reviewed coordinator before any integration.

Under the shared lock:

1. Exclusively create and fsync the pending guard; fsync its directory.
2. Exclusively create the stage, write all bytes and fsync the stage.
3. Revalidate directory, stage and guard identity; atomically rename the stage
   to the receipt; fsync the directory; verify canonical receipt readback.
4. Revalidate and unlink only this instance's still-bound exclusive guard;
   fsync the directory again; verify final admitted receipt.

The guard is necessary because rename consumes the stage. Without a distinct
durable guard, a crash between rename and directory fsync could be mistaken for
a completed publication on reopen. Any leftover stage or guard refuses all
reads/publications. They are never auto-deleted, promoted or repaired. Existing
unsafe receipts and the generation marker are not overwritten or removed.

Any publication I/O/checkpoint failure poisons the current instance. It returns
no success and subsequent reads remain uncertain. Reopen may admit the old
record if failure preceded mutation, or the new record after guard unlink:
the new receipt's directory fsync already completed before unlink. If power
loss brings the guard back, reopen conservatively refuses instead. No return
value reconstructs whether an interrupted caller received acknowledgement.

Assumptions: local Linux filesystem with working file/directory fsync and atomic
same-directory rename; preprovisioned durable trusted ancestors; every legitimate
writer obeys the same lock and format. This is not protection against malicious
root, unlocked root writers, out-of-band snapshot rollback, failing hardware,
or a filesystem falsely acknowledging durability. Directory/leaf binding checks
detect ordinary replacement, not arbitrary hostile-root atomicity. Unsupported
fsync fails closed. Real reboot/power-cut and filesystem matrices are later gates.

## Deterministic evidence and remaining gates

Unprivileged fixtures use test-only private directories/fixture ownership; they
never open the installed root path. Tests cover canonical decode/enrollment,
oversize and invalid records, unsafe leaf types/modes/owner/hardlinks, lock
contention, directory/stage/guard rebinding, partial stage refusal, retained marker,
stale expected record, leftovers,
and failure at eleven write/fsync/rename/unlink boundary checkpoints for initial
and replacement publication. These model process interruption on a normal local
filesystem; they do not emulate torn writes or storage-controller power loss.

Kernel and filesystem commits remain separate. Canonical host namespace proof,
nft subsystem lifetime, authenticated live table provenance, operation allocation,
pending-phase recovery and orphan disposition remain unresolved. In particular,
this adapter cannot solve a crash after exclusive nft creation but before a Live
receipt becomes durable. Neither a pending record nor matching boot/netns/handle
allows adoption. A later owner-reviewed recovery policy and exact-head isolated
crash/reboot evidence are required before a production adapter can be enabled.
