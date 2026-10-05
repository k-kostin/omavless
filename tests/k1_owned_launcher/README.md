# Isolated actual-creator/owned-child implementation checkpoint

Review-only source, not ready for execution or VM delivery. Nothing here is an
installed package or a production constructor. No Cargo dependency or source
under the product crates is changed. The existing #651 local pair, stopped v1,
stopped v2, successful scoped v3 local pair and native e648 input remain separate.

The pure adapter binds six exact e648 source hashes and reuses the actual
AcquiredCreator private callback and fixed semantic methods. Originals no
longer contains a separate matching socket: its verifier borrows directly from
the actual exclusive LocalReadSession owner through private CreatorOwner.
The callback remains module-private and cannot expose &mut C to callers.
ActualCreator does not implement CanonicalCreator or EffectPort. Its only
operation is the existing complete generation-bracketed inventory, returned
as LocalPolicyInventory, explicitly untrusted.

The new constructor source creates its own fixed current-thread namespace
originals and actual NETLINK_NETFILTER socket before spawning one fixed child.
No supplied descriptor, PID, namespace ID, replacement socket or creator is
accepted. All owners are non-Send/non-Sync and retained using lexical owners
and ManuallyDrop. The child gets no netlink descriptor across exec. A retained
owned-child WNOWAIT check, fresh current image, original namespace handles
and actual creator socket cookie are checked through the same acquisition
and session path. A failed callback or verifier keeps acquisition sealed.

The original private five-second launch deadline now gates the four shared
inventory send/receive paths before and after their actual leaves. No extra
request or mutation sender is added. Ordinary exported sessions with no launch
owner preserve their old behavior; the actual creator branch additionally
checks its own namespace FD and actual socket. Kernel operations are not
cancellable: these are cooperative sampled bounds, not atomic authority or
protection against hostile root or same-thread switch-and-return.

## Current evidence and mandatory unfinished work

The exact-source export compiles against copied review-only patched nix/libc
in its own private HOME cache. The first compile completed known zero30247a;
the leaf-integrated compile completed known zero ccd0ff without warnings.
No namespace API, socket, child, inventory, ELF main or VM was executed.
Six source controls check exact pins, immutable fixed acquisition operations,
actual socket borrowing, all four existing readback leaf pairs, full image
metadata fields and closed developer-export names.

This is an early compilable checkpoint, not a finished launch proof. Before
any invocation it still requires:

1. Review and pin the separately built static child and its complete publication
   graph. The new admission/protocol below is implemented, but its SHA and size
   deliberately remain zero and refuse before I/O. No execution readiness follows.
2. Executed inert fault controls of actual ownership, partial constructors,
   late open/spawn/readback, error/panic, FD closure, replacement/lifetime
   compile failures and permanent refusal. Source controls are not these tests.
3. Independent full review, locked exact dependency provenance and full gates;
   separately reviewed build/freeze/fresh delivery, with ROOT sole executor.
4. A separately authenticated installed-launch origin boundary. Root UID,
   PID1, configuration, fd numbers, retained local IDs or matching cookies
   cannot turn this prototype into canonical authority. Existing
   AuthoritySession remains unavailable for ActualCreator.

The developer exporter only creates a fresh closed-name HOST directory and
never executes its Rust contents. A draft export-name shadowing bug produced
a missing-manifest failure2bf4e5; its output was retained, the parameter renamed,
and a fresh corrected export compiled. This was not a runtime or guest result.
Do not run the constructor or inventory to infer acceptance from compilation.

The pre-launch follow-up preserves the observer's local nsfs/metadata/link,
procfs and socket-address predicates while placing original-budget checks
around each leaf. It no longer calls the nested legacy identity/session helper
under one outer check. Newly opened proc/current descriptors are retained before
late classification. These extra retained originals remain process-lifetime
resources, not cleanup or canonical authority. A first compile exposed an AsFd
type mismatch for ManuallyDrop (88a327); that failed export remains untouched.
The corrected safe borrow compiled in a fresh export, known-zero 130f08.
This is compilation plus source coverage only; injected actual-path deadline,
error and partial-owner controls remain required before invocation.

## Fixed original child and protocol implementation

`child_executable.rs` opens the fixed root/run/fresh-stage chain through original
directory FDs. It retains each successful return before late classification;
root-owned 0755 ancestry, full original eleven-field metadata, no xattrs and
root-owned 0555 single-link child are mandatory. Exact size and SHA-256, checked
from the original bounded pread stream with EOF and before/after metadata and
ancestry checks, must match nonzero build pins. No supplied path or descriptor
is accepted. The first artifact is static little-endian ELF64 x86-64 with no
PT_INTERP; the pure structural parser does not replace exact whole-file hashing
or validate all loader semantics. An ordinary dynamically linked build is not
an acceptable artifact. SHA2 0.10.9 and rustix 1.1.5 (only safe fixed flistxattr)
are locked dependencies of this external export only, not OmaVLESS adoption.

Execution uses `/proc/self/fd/<held-original>` with FD >= 3 and fixed argv0,
not a later open of the admitted pathname. No unsafe pre_exec/fork wrapper is
introduced. The pinned Linux
[alloc_bprm/open sequence](https://github.com/torvalds/linux/blob/a90ee4305c4a5df72c11b31dacfdc76e00fcf78a/fs/exec.c#L1418-L1434)
obtains the executable before
[begin_new_exec closes CLOEXEC descriptors](https://github.com/torvalds/linux/blob/a90ee4305c4a5df72c11b31dacfdc76e00fcf78a/fs/exec.c#L1117-L1135).
This supports original-object selection, not same-OFD transfer into the new
image, hostile-root immunity or authenticated package provenance. Exact Rust
spawn implementation/stdio descriptor behavior and complete fixed launch graph
remain review prerequisites before invoking this source. Existing creator and
namespace descriptors are CLOEXEC; none is passed to the child.

The separate `fixed_child.rs` performs no namespace, nft or policy operation.
It sends READY, remains alive waiting for FINISH, then sends DONE and exits zero.
Both ends retain their original pipes; reads/writes are nonblocking and fenced
before/after each returned operation. Only WouldBlock permits continuation;
unexpected bytes, EOF, errors and late output refuse. The child has its own
five-second defensive budget; it cannot extend the parent's original five-second
deadline. No hard syscall cancellation or scheduler guarantee is claimed.

Parent image verification starts only AFTER READY. Every live creator check
requires the same owned unreaped child to be live and its original output pipe
idle, then checks current image against the retained hash-verified ELF and
actual same-session namespace/socket ownership. `Prototype::finish` first
verifies the lease, permanently seals acquisition and poisons the actual creator,
then sends FINISH. DONE alone is not completion: only exact owned WNOWAIT exit
zero, original output EOF and original executable recheck authorize the sole
matching zero reap. Nonzero, signal, late or unknown results retain the remaining
owners and cannot retry, compensate, query-adopt or reopen operations. A late
post-reap refusal cannot undo that already completed reap. No NFT effect API
or canonical conversion is implemented.

The shared `retain_after` primitive is used by actual child spawn and each ELF
admission opener. Its executed controls retain real harmless `/dev/null` Files
on synthetic late and panic gates, and distinguish original error from an owner.
These prove this returned-owner primitive, not every partial constructor path
or a real delayed spawn. Pipe tests execute the exact protocol using in-memory
I/O; static-ELF tests cover truncations, bounds, wrong kinds and PT_INTERP. No
test runs child main, Prototype, namespace calls, netlink or VM. The complete
constructor/finish fault matrix, lifetime/escape compile-fail controls and
independent full graph review remain mandatory before any invocation.

The first protocol export exposed a Vec type inference error; an all-target
attempt also reached inherited test-only support outside the export. Those
failed exports remain retained. Normal library/binary compilation and fixed
inert binary tests are distinct scopes, not a claimed full product Rust gate.
The original #654 CI failure at 210fe14 was missing BASE in a shallow checkout:
source controls now admit the current six unchanged originals by their SAME
exact pins without fetching or skipping. Developer export still requires BASE.

Final scoped gates: external locked library/binary strict Clippy passed
(`983187`); nine fixed protocol/return-retention/ELF controls passed (`ec9466`).
Python 3.14 full source checks passed 696 tests with two opt-in skips, followed
by JS/QML checks (`544e9a`). Python 3.12's full run remained NONPASS (`2f7d3d`):
an unrelated inherited DNS test expects os.pidfd_open, absent from that local
standalone interpreter. That fixture was not changed or skipped here; the
eight exact-source adapter controls pass independently on Python 3.12.
These are not full product native acceptance or a real child invocation.
