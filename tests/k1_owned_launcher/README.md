# Isolated actual-creator/owned-child implementation checkpoint

Review-only source, not ready for execution or VM delivery. Nothing here is an
installed package or a production constructor. No Cargo dependency or source
under the product crates is changed. The existing #651 local pair, stopped v1,
fresh v2 proposal and native e648 input remain separate and immutable.

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

1. Fixed published child executable hash/ancestor/original-FD admission and
   exact child protocol, including launch/exec race and original child outcome
   checks. The current fixed pathname and metadata predicate are insufficient.
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
