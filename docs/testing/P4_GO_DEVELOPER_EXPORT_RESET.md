# P4 developer Go-tree export: checked directory reset

The separately reviewed fixed exporter completed one fresh whole export with
known-zero original status. It did not execute Go, another tool, a test ELF or
an engine selector. This is UID 1000 developer custody of official archive
bytes, not the former installed root-owned tool predicate or normal P4
acceptance. The [provenance record](../../tests/research/p4-go-export/provenance.json)
contains exact input/source/recipe/receipt hashes without private paths or logs.

## Mechanism and narrow correction

An owned directory can have per-open enumeration state distinct from its
current inode metadata. In the reviewed upstream
[Linux v6.17 Btrfs source](https://github.com/torvalds/linux/blob/v6.17/fs/btrfs/inode.c),
`btrfs_opendir` records a per-open last index; readdir uses it, and
`btrfs_dir_llseek` refreshes it before the generic seek. This is conditional
source reasoning, not attestation of the actual HOST kernel or filesystem.
Even a SEEK_CUR probe would be an operation, not a passive observation.

The reviewed [CPython v3.14 source](https://github.com/python/cpython/blob/v3.14.0/Modules/posixmodule.c)
duplicates the supplied directory FD for `scandir(fd)`, without an initial
rewind. Its fd-backed iterator rewinds on exhaustion/close. That behavior
does not refresh the original per-open view before the first enumeration.
It also does not justify a generic stale-offset explanation for all scans.

The [four-line correction](../../tests/research/p4-go-export/root-members-reset.patch)
performs `Store.call(os.lseek, original_root, 0, SEEK_SET)` and requires an exact
integer zero before each unchanged whole-root membership scan. The original
retained root remains the owner; no reopened or reconstructed directory is
adopted. All three original scans use this same function: before sealing,
before receipt creation, and after receipt creation. Exact wanted-set equality,
uniqueness, bounds, sampled deadlines and successful iterator close stay intact.
No failure cleanup, retry or weakened predicate is added.

The portable [function export](../../tests/research/p4-go-export/directory_membership.py)
has no CLI, acquisition or export entry point. It delegates every operation to
the caller's checked Store boundary and alone cannot admit a tool tree. The
[ordinary controls](../../tests/test_p4_go_export_membership.py) use fabricated
iteration/seek/close adapters only; they do not access any old FD/tree or prove
kernel behavior. The public function is not the entire externally selected
exporter and cannot inherit whole-export evidence as a new execution.

Public checkpoint source gates: seven synthetic regression controls passed;
the normal source suite passed 383 Python controls (two existing skips), all
JS contracts and QML contracts. These are developer-source results only,
separate from the earlier fixed export recipe's 22 controls and actual export.

## Exact positive scope

The fresh two-round minimal fixture completed both checked resets, wanted-entry
enumerations, original equality/close checks and final source recheck:
selection `a620cb`, exit 0, fixed projection `8cb869`. This distinguishes its
positive result from earlier permanently stopped scopes. It does not assign an
actual cause to any earlier export failure.

After both full source reviews and 22 separate in-memory controls, ROOT selected
the fresh full exporter once (`83f840`, original session 71814, terminal
`2db384`, exit 0). The original fixed marker was
`P4_OFFICIAL_GO_DEVELOPER_TREE_RESET_KNOWN_ZERO`; stderr was empty. Its retained
receipt was 2090803 bytes, SHA-256
`fe42cfda1b8dfdaed64bd2f23de917aa68ed64120abdaa0273628975b6f43f8e`,
mode 0400, single-linked, UID/GID 1000. The receipt schema was
`p4-official-go-developer-tree-reset-v1`, with 17353 members and metadata records
and 10 fixed executable-mode tools. These are ROOT-supplied exact original
results, not new author queries of the tree or captures.

The full exporter retained the original archive/raw-tar/catalogue pins and
complete hierarchy/type policy, fixed modes, every member hash/readback,
no-symlink/single-link and no-attribute checks, sealing order and 1200-second
sampled whole budget. That budget is not a hard cancellation mechanism or a
disk quota. Ordinary UID 1000/admin custody is explicit; hostile mutation or
atomic filesystem attestation is not claimed. The successful receipt says no
tool/native execution and no prior installed-tool adoption.

## Remaining engine gate

The [default-residue runner proposal](P4_DEFAULT_RESIDUE_RUNNER_PROPOSAL.md)
remains NOT BUILT / NOT EXECUTED. The exported developer tree is only a possible
input to a future separately reviewed fixed GOROOT/GOTOOLCHAIN-local, closed
environment and offline-module build proposal. Compiler/runtime dependencies,
retained resource limits, build output/ELF provenance and execution ownership
still need exact review and separate selections. Neither 540-second engine case,
race run, normal P4 activation, installed compatibility nor cleanup is proved.
Every earlier nonzero scope stays stopped and unqueried; this positive result
does not authorize a retry, Go execution, VM use, merge or release.
