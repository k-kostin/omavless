# Original-launch complete-inventory lease successor

Inactive successor to #654. Its exact stage5 native evidence remains
`8e902d664c8922bb7c897f6d7c4c135a8fd22e42`, not this changed source. No inventory
or new resource-owning executable has run. No product crate, dependency or constructor
changes. The existing no-policy entry still selects open/finish only.

## Concrete change

The old external `ActualCreator::inventory` discarded table metadata and
generation before acquisition post-verification. This successor borrows the
existing #641 `LocalInventoryLease` instead: the exact complete parser, table
metadata, remembered generation and exclusive original-session borrow remain
alive through lease recheck, original acquisition-verifier recheck, then another
lease recheck. Only their complete success releases the borrow and restores the
creator/acquisition operation latches. No new socket, request encoder or rule
parser is introduced. The adapted constructor takes the original launch deadline;
it does not acquire a new one-second inventory allowance.

Dropping an incomplete internal inventory scope poisons the original session
without any I/O, descriptor close, mutation, retry or cleanup. Acquisition is
sealed before its first verifier operation. Error or unwind therefore cannot
restore this acquired owner. A private fixed three-step sequence is shared by
the actual adapter and pure fault controls. The outer constructor and kernel
parsers remain separate coverage obligations, not proven by those mocks.

This is a lexical internal lease, not a newly transferable user-facing token.
Its public-facing result remains only `LocalPolicyInventory`. Parsed table and
generation data are retained inside the existing lease; raw netlink transcripts
are not retained or exposed. Internal trusted adapters can borrow the original
socket for verification but cannot receive a mutable session through this API.
No `CanonicalCreator`, `EffectPort`, `OwnedVerified` or durable receipt conversion
is supplied. OtherUntrusted is deliberately not renamed Foreign: failure to
recognize ownership is not affirmative proof of foreign ownership.

## What absence and completion mean

The unchanged fixed table reader accepts absence only from a matching kernel
ENOENT reply with the original sequence, port and exact echoed request. A second
table observation must match the first, with equal nonzero GETGEN observations
bracketing the whole operation. Silence, an ACK alone, a missing dump or a parser
error cannot yield TableAbsent. When the table is absent, child-object dumps are
not sent; this is the existing complete-table-absence branch, not proof that the
host ruleset is empty. It grants neither canonical-host authority nor permission
to create, and says nothing about policy after the observation window.

For a present table the existing chain/rule/set/object/flowtable parsers must
complete. Their bounds, exact kernel sender/sequence/port, multipart flags,
zero-status final NLMSG_DONE, padding and trailing-message rejection remain
unchanged. Generation equality and local retained-ID checks are consistency
checks, not nft subsystem reset continuity. Lease recheck does not send another
GETGEN or claim a fresh atomic kernel snapshot. Same-thread switch-and-return
and privileged concurrent mutation remain outside the local witness.

## Exact-head compilation and pure controls — 2026-10-05

At source `81adf5b7b39e36ab13fdc54be9c4eb3d775d9592`, ROOT and an independent
reviewer read the seven-file private recipe, full exporter/type-controls graph
and reached inventory/lease source. Five in-memory recipe classifier controls
passed before the separately selected actual recipe returned terminal zero.

The fresh private export compiled its positive actual-module example first.
Six compiler-only negative examples then returned the expected complete Cargo
JSON failure: escape `E0515`, overlapping borrow `E0499`, private session
`E0616`, and non-Send/non-Sync/non-Copy `E0277`. No unexpected diagnostic or
application execution was accepted. The standalone sequence module's two pure
tests passed; they include error/unwind cuts and permanent re-entry refusal.
They neither include nor execute the descriptor/netlink/namespace adapter.

The closed recipe used offline/locked Cargo, two build jobs, fresh HOME-cache
target/TMPDIR, exact clean-source checks, and the unchanged successful build
input catalog before and after. Compiler-only expected failures are not native
owner failures. Private build capture paths, raw transcripts and executable
objects are deliberately not committed. This records compilation and pure
controls, **not** a new launch or complete read-only kernel inventory witness.
The old stage5 result still belongs only to its earlier exact source.

## Remaining actual gates

The 11 focused exact-source Python controls completed with known zero
(`93c141`), using inherited HOME and a fresh private HOME-cache TMPDIR. They
import only source-tool definitions and execute no Rust/export/native path.
Pure fixed-sequence and actual-module type controls now have the bounded
exact-head evidence above. Any later
ignored read-only kernel witness needs a new fixed entry, fresh original scope,
separate ROOT authorization and original launch/finish accounting. There is no
new actual selector or permission to reuse a stopped scope here.
