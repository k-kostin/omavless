# K1 original-session inventory lease — inactive

This follows the [authority composition](K1_AUTHORITY_COMPOSITION.md) without
inventing a canonical provider. K1 remains unavailable. No dependency, unsafe
binding, installed caller, privileged IPC, service or namespace transition is
added. The [safe namespace API and trusted launch prerequisites](K1_NAMESPACE_API_PREREQUISITE.md)
remain unresolved.

`LocalReadSession::borrow_policy_inventory` runs the existing complete fixed
inventory: generation, table identity, chains, rules, sets, objects, flowtables,
table identity again and final generation. Its non-Copy `LocalInventoryLease`
exclusively borrows the actual session containing the original namespace File
and netlink OwnedFd. It retains the observed classification, table metadata,
generation and original deadline. There is no public field constructor or
conversion from serialized labels. The classification accessor deliberately
returns only `LocalPolicyInventory`, whose matching policy remains untrusted.

Recheck verifies the original session, remembered generation and observation
coherence under the original budget; failure permanently poisons that session.
Dropping the lease neither closes the owner's descriptors nor sends anything.
It does not authenticate a canonical namespace, prove switch-and-return cannot
occur, establish nft subsystem continuity, or create durable ownership.

The existing cfg(test) conditional deletion now owns this lease instead of a
separate session/generation/deadline tuple. The actual cfg(test) creator's full
create/replace path likewise retains its exclusive session borrow from complete
readback through conditional send. A split-borrow sender accesses the same
session and existing fault controls; no alternate socket or executor is added.
Generation-fenced kernel transactions, ACK handling, uncertainty refusal and
subsequent readback remain the existing mechanism. All mutation paths remain
cfg(test). A copied tuple adapter exists only for legacy diagnostic test reads,
not the create/replace/delete preparation paths.

Controls cover exclusive-borrow and non-Copy compile failures, cancellation
without descriptor/sequence change, original-budget expiration and permanent
poisoning, and changed remembered generation/shape. Runtime controls open local
original namespace/socket descriptors but send no nft datagram and change no
namespace or policy. Synthetic lease contents in those unit tests cannot be
promoted to a real kernel inventory result. No new VM or host acceptance is
claimed; prior private fixture evidence remains tied to its exact tested head.
