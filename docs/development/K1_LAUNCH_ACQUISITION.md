# Inactive K1 original-launch acquisition boundary

Current successor: [developer live-owner service core](K1_SERVICE_CORE.md)
adds an opt-in private fixed installed-launch constructor/verifier and private
safe-library adoption under owner authorization. Default builds retain the
inactive boundary below. This page preserves the historical prerequisite
stage; its statements about no normal feature-enabled constructor/dependency
are superseded only by that explicitly selected successor. No installed/VM
canonical-launch acceptance or product availability is implied.

This successor to [external safe API validation](K1_NAMESPACE_API_VALIDATION.md)
changes the existing [authority composition](K1_AUTHORITY_COMPOSITION.md), not
the product availability boundary. K1 remains unavailable. No installed unit,
provider, dependency adoption, privileged IPC or VM operation is introduced.

`AuthoritySession::from_admitted` now requires an opaque `AcquiredCreator<C>`
instead of accepting a bare `CanonicalCreator`. Its fields cannot be supplied
by other modules. The only constructor and original-resource verifier are
under `cfg(test)`. Normal code therefore has no route to acquire this owner.
This is a compiled obligation for a future authenticator, not an available
authenticator or proof that a canonical launch happened.

The owner retains original safe namespace Files, a creator OwnedFd, verifier
and creator together. They are never reconstructed from integers, serialized
receipts, paths, root UID or PID 1. It is non-Copy and not Send/Sync, records
the actual acquisition thread, and supplies lifetime-bounded descriptor borrows
to the verifier. The same original owners remain held through each callback.
The latch is set before precheck, callback and postcheck; any error or unwind
permanently refuses this instance. The generic callback is module-private;
other modules can call only fixed epoch/observe/create/replace/delete methods,
never receive `&mut C` or replace the paired creator via `mem::replace`/swap.
Normal drop retains all owners through ManuallyDrop; only synthetic teardown
can release them. This is process-lifetime retention, not crash persistence.

Existing provider epoch admission/fences and observe/create/replace/delete now
go through that owner. Conditional effects and their immediate provider return
check run in one acquired callback. Original owners remain retained across the
entire accepted client exchange; existing before/after accept, receive and reply
fences still run. There is no new retry or compensation. Acquisition loss after
an effected operation leaves the actual LockedState Pending record untouched.
A post-reply refusal still cannot retract bytes or relabel a committed receipt.

## Deliberately unresolved production acquisition

The future constructor must originate inside a reviewed installed system-manager
launch, establish package/unit/manager provenance, open original descriptors
before sandboxing, and bind the **actual exclusive creator socket used by C**.
Owning a second socket with matching numbers or cookies is not sufficient.
The current synthetic constructor pairs ordinary `/dev/null` files and a local
Unix socket with synthetic effects: these are real descriptor lifetime tests,
not namespace or kernel-creator evidence. No real provider implements this path.

The private verifier interface has no production implementation. That missing
implementation must consume adopted safe namespace type/ID and socket-cookie
APIs on the borrowed originals, validate current thread binding and retain the
trusted launch lifetime. Numeric equality alone cannot implement provenance.
The syscall-library patches remain external review artifacts; Cargo is unchanged.
Likewise !Send and repeated checks do not prevent same-thread switch-and-return:
reviewed launch restrictions and the complete session-owning code path must
structurally forbid namespace transitions. No setns, repair, reopen, inherited
arbitrary-FD input or cached root observation is added here.

## Configuration evidence only

`launch_acquisition.unit-contract` is an exact-byte **non-installed sketch**.
It is not a supported package unit, complete bootstrap description or systemd
acceptance receipt. `ConfigurationEvidence::validate` checks the fixed proposed
package/executable labels, entire sketch bytes, exact two-element argv and
empty supplied drop-in list. Changed namespace restrictions, extra commands,
environment, alternative root, joining namespace, argv and drop-ins refuse.
Exact bytes avoid implementing another partial systemd parser.

These are supplied configuration observations, not an installed package audit.
There is no discovery, trusted package signature/ELF identity, effective manager
configuration query, descriptor delivery or proof that the drop-in list is
complete. Successful validation cannot construct or convert to AcquiredCreator.
Effective unit/manager provenance, mount/proc/container view, trusted original
descriptor transfer, bind-before-access listener and safe API release/adoption
remain separately reviewed prerequisites. Physical/host acceptance remains
under [KILL_SWITCH](../roadmap/KILL_SWITCH.md); the old #631/#643 fixture evidence
does not transfer to this new native source.

## Controls and boundaries

Native controls use original local descriptor owners with a synthetic verifier:
same owner before/after, wrong actual thread ID, pre/post errors and panics,
callback error/unwind, permanent refusal and retained drop. Existing real
private listener/store controls now use the acquired wrapper; a new create,
replace and delete cut retains actual Pending without compensation or retry.
No nft datagram, namespace transition or root-manager query is needed by these
new controls.

The Rust gate also compiles the unchanged actual acquisition module against
inert imported-interface stubs. A positive compile exercises all five fixed
method signatures before negative Send/Sync/Copy, private callback/creator
replacement, absent normal constructor and configuration-to-acquisition
conversion checks. Produced metadata is not run.
The harness is not a replacement implementation or kernel acceptance.

The first native compile correctly found a missing result-type annotation in
the test's deliberately unreachable panic callback. It was corrected in the
test without changing production behavior. Exact checkpoint gates belong to
the PR; no source-only success implies installed authority.

ROOT review of the initial `09d6529` checkpoint identified that its crate-visible
generic callback could replace C despite preventing borrow escape. That checkpoint
remains preserved, not retrospectively called immutable creator pairing. The
successor makes that callback private and supplies only fixed semantic methods;
actual-module compile controls reject callback and direct-field replacement.
This restricts callers, not malicious internals of a future trusted C itself:
the missing production constructor/verifier must still bind C's actual exclusive
socket and audit its lifetime-preserving implementation, not a matching second
socket or copied labels.
