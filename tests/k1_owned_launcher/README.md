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

## Explicit owned spawn successor (still zero-pinned)

The historical dbdd56c used std Command. Pinned Rust 1.98.1
`48a229ceaefd4985c50990b14116b6d856af0985` has internal partial-pipe drops and a
fork fallback that can wait/reap before returning a Child. Retaining only a
returned Child cannot govern those internal outcomes. This successor replaces
that call with two explicitly retained original nonblocking CLOEXEC pipe pairs,
fixed safe nix posix_spawn attributes/actions, exactly three dup2 actions to
0/1/2, an empty environment and the same original-FD executable path. The private
non-Copy/non-Send/non-Sync OwnedChild takes its PID only from that actual spawn
return, never from a caller, discovery or a serialized receipt. No std fallback
or arbitrary spawn API is exposed.

Stderr now shares the PRIVATE bounded protocol output pipe; unexpected bytes
refuse and are never printed. The parent's copies of the child's two pipe ends
are deliberately held until READY plus full original live-image/acquisition
verification. Only then does a one-shot gated handoff close those copies;
otherwise the retained parent writer would prevent real EOF. Parent I/O ends,
the owned child and all other originals remain held after unknown/late closure.
The libc spawn implementation itself remains a synchronous noncancellable
boundary. This is not retention of every internal libc allocation or a hard
deadline through libc/kernel execution. An explicit no-retry failure contract
still applies to the whole attempted instance.

Before using the safe spawn builder, the additional external
`nix-posix-return.patch` corrects 17 POSIX result checks in the exact upstream
spawn module. POSIX returns an error number directly; the old -1/errno helper
could treat ENOMEM as success before reading uninitialized output. Both
initializers and signal getters now construct values only after exact zero.
Failed destroy/reinitialization or unwind suppresses a second destructor on an
uncertain/already-destroyed object; successful reinitialization restores normal
Drop. The public ABI/API is unchanged. Five actual private-helper controls cover
positive/unknown/negative errors, uninitialized error outputs, original success,
both reinit failure cuts, unwind and exactly-once normal destruction. They invoke
no libc initializer, child or kernel spawn. Existing upstream AIO deprecation
warnings during all-feature dev-dependency compilation are not new patch warnings.

`spawn-upstream.json` fixes upstream HEAD e35c008 (also current in the read-only
remote check), original and corrected spawn module hashes and the patch digest.
The namespace/libc patches and their older immutable caches remain separate.
The exporter requires the corrected source hash and exact Git HEAD before
writing its fresh workspace. No upstream submission or product Cargo change is
authorized. Upstream-declared MSRV is 1.69; a new actual MSRV/cross-target gate
for this added patch has not yet run. Full independent review and complete
constructor/hand-off/finish fault controls remain required before any child run.

The patch's first artifact control rejected Git's configured i/w diff prefixes;
the exported review patch was normalized to a/b, repinned, and all ten source
controls then passed (`70c97c`). This was source tooling, not a spawn outcome.

Successor gates: five external POSIX helper tests known-zero `67e3b6`; normal
export library/binary strict Clippy known-zero `f4ef30`; nine inert protocol,
return-owner and ELF tests known-zero `f77747`; full source 698 tests/two skips
and JS/QML known-zero `7bc81c`. The final source and nine-test gates inherited
the ordinary HOME unchanged and used only the separate fixed HOME-cache target
and scratch paths. No child main, actual spawn, namespace or nft operation ran.

## Checked close and executed owner-boundary controls

The 06b6138 handoff used File Drop, which cannot report close failure. Pinned
[Rust OwnedFd Drop](https://github.com/rust-lang/rust/blob/48a229ceaefd4985c50990b14116b6d856af0985/library/std/src/os/fd/owned.rs#L179)
explicitly ignores the libc return. This successor uses the existing safe
`nix::unistd::close(File)` from exact e35c008: it consumes the ORIGINAL File
through IntoRawFd, calls close once and returns its result. No application raw
descriptor is supplied, adopted or reconstructed, and no new unsafe code or
library patch is required for close. The unchanged reached `src/unistd.rs`
is part of the same external pinned nix source graph.

[Linux v6.17 close](https://github.com/torvalds/linux/blob/v6.17/fs/open.c#L1483)
removes the descriptor-table entry before flush and does not restart close on
EINTR. Therefore any error is terminal rather than grounds to retry the numeric
descriptor. The private Handoff owns both original ends, latches Attempted
before checking READY or time, and marks Complete only after both consuming
closes return exact success within their original pre/post budget. Failure,
late return or unwind leaves untouched ends retained and permanently denies
another handoff. Already-consumed ends cannot be recovered or described as
still open. The actual child, parent I/O ends and other originals stay retained.

The constructor now executes a shared private fixed sequence with the actual
safe-library adapter: two pipe pairs, attribute/action initialization, exactly
three fixed dup2 actions, original image recheck and one spawn. Its synthetic
backend replaces only those private operations for tests, never a public
acquisition provider. Every successfully returned owner is held before the
next gate; fault controls cut every nine-operation return and all 18 pre/post
gates, including error and panic. Exact dup2 order, invalid stdio alias and
invalid returned PID refuse without dropping previously returned owners. This
does not prove ownership inside an unreturned failed libc operation.

The actual READY parser now has its own permanent attempt/completion latch;
failed or late READY cannot enable handoff. The actual finish adapter shares
the tested fixed FINISH, DONE, original WNOWAIT-zero, EOF, image and sole exact
zero-reap sequence. Six-operation and 12-gate fault controls include late zero
before EOF/reap and late actual-reap return: the latter cannot undo a completed
reap but cannot report success. An observed live child may only continue under
the same remaining budget; unknown/nonzero observations never reap. These are
executed tests of the exact private sequence bodies, not string assertions or
a kernel/child execution claim. Nested ELF/proc metadata failures, libc internal
partial errors, installed origin and lifetime compile-fail coverage remain
separate review obligations.

The first checked-handoff test export failed compilation on an ambiguous test
integer (`acac4c`); its source/export is preserved. The corrected combined
boundary export passed 19 inert tests (`e2bf70`) and normal library/binary strict
Clippy (`c8c4d2`). No child main, close syscall, pipe/spawn/wait syscall, namespace,
netlink or VM was selected by these new synthetic controls; the inherited
returned-owner tests still open their own harmless /dev/null Files.

The first full-source attempt passed Python 698/two skips but ended NONPASS
(`2a5131`) when a JS scratch creation reached an incorrectly specified missing
private TMPDIR. The retained capture identifies that environment error; no test
or product condition was relaxed. The final gate uses the existing 0700 scratch
directory and repeats the source suite. Historical prior compile/test captures
are not promoted to evidence of this final source.

Final exact executable-source gates: 19 inert Rust controls known-zero
`d4601d`, strict external library/binary Clippy known-zero `d9266b`, full source
698/two skips plus JS/QML known-zero `3c5007`. These inherited HOME unchanged and
used the verified 0700 private scratch directory. The external export predates
the source commit: matching source bytes are not a frozen artifact or a claim
that an executable was built at the later commit.

## Next opt-in actual no-policy gate (proposal, not invocation authority)

Keep this zero-pinned checkpoint immutable. First prepare a fresh export/build
identity for the fixed child alone, with static ELF admission (no PT_INTERP),
exact toolchain/patch/lock/source pins, normal strict Clippy and inert gates.
Do not overwrite any prior export/target/frozen original. ROOT alone may freeze
the verified original child into a new 0500/single-link HOST artifact after full
source/freeze review. Its held-original SHA/size/receipt then authorize a NEW
explicit source checkpoint replacing the two zero admission literals; old zero
source and failed attempts remain preserved. This is a reviewable provenance
change, not a runtime bypass or a mutable public pin override.

A separate fixed opt-in parent gate must enter only private open_fixed plus
finish, never inventory/effects or a product entry point. It must use the exact
adapted acquisition body and actual creator-owned socket, not a matching second
socket or a model. A fresh ROOT-reviewed VM publication scope admits the frozen
child through original-FD hash/metadata/ancestor checks and a root-owned 0555
guest copy; the HOST frozen original stays unchanged. One ROOT-only invocation
would test READY, original child image and creator/namespace binding, checked
handoff, FINISH/DONE/EOF and exact owned zero reap. First unknown/nonzero stops
the scope without retry, query-adoption or cleanup. Fixed public phase evidence
must be captured from the original call, not obtained through failure queries.

That prospective gate proves only a locally constructed, retained no-policy
owner relationship under a trusted test launch. It cannot prove installed
package/unit/system-manager origin, authorize policy, construct CanonicalCreator
or expose production availability. The installed-origin verifier remains a
distinct unmet boundary even if the local child gate later passes.

The [concrete static build proposal](BUILD_PROPOSAL.md) now specifies the fresh
child-only CRT-static build and provenance/freeze review. An external opt-in
parent binary uses the exact adapted module tree and only open_fixed plus finish;
it reports fixed original-call phases. Its feature is disabled by default and
the child pins remain zero. Ten unchanged adapter source controls pass after
the exporter addition; the new parent/export has NOT been compiled or invoked.
The follow-up removes buffered phase output: one safe rustix write on borrowed
stdout must return the exact full length inside one outer phase budget. Short,
Interrupted, other errors or lateness stop without another output/operation.
The earlier buffered-output proposal remains preserved as unapproved history.
Sixteen actual-module type-control sources (one positive, fifteen negatives)
are prepared in a separate closed-name export but have not been compiled.
Full ROOT/independent build/freeze/runner reviews remain explicit prerequisites,
not implied by source checks.
