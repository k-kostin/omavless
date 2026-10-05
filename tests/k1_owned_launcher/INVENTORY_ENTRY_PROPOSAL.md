# Fresh original-launch read-only inventory entry — source only

Successor to exact f04f6345c717c325c0a9f63de10bbb02ce0c2067. No test,
compiler, parent, child or VM invocation by this author. The earlier stage5
8e902 launch result and 81adf type/pure results retain their exact scopes;
neither covers this executable. All nonzero/unknown old scopes stay stopped.

## Fixed operation and lifetime

The explicit external export adds only `k1-owned-readonly-inventory`, behind
`owned-launch-readonly-inventory`, with the sole fixed argument
`--fixed-owned-readonly-inventory`. No paths, FDs, PIDs, observation values,
deadline values or operation selectors are taken from a caller. The ordinary
product manifest, lock, runtime and acquisition authority remain unchanged.

Entry samples one five-second deadline before argument admission and output.
`Prototype::open_fixed_before` accepts only an earlier positive allowance no
greater than five seconds, then uses it unchanged through child admission,
creator acquisition, launch, READY, inventory and positive completion. The
older open_fixed wrapper still derives its own five seconds for its separate
entry. The new entry does not use that wrapper or acquire a second allowance.
This is pre/post sampled time, not syscall preemption or a hard execution bound.

The actual owner is stored under ManuallyDrop before the post-open budget check
or next phase. Exactly one `Prototype::inventory` retains the #641 shared
complete inventory lease through the #656 lease/owner/lease sequence. It yields
only TableAbsent, ExactUntrusted or OtherUntrusted. All three are successful
complete read classifications, not canonical ownership, foreign ownership or
permission to mutate. An error, partial dump, late budget, output failure or
unwind permits no finish, later output, query, retry, cleanup or adoption.
The original owners remain retained. The positive path alone calls the existing
FINISH/DONE/EOF/exact-original-zero completion. Any whole nonzero remains STOP,
including failure to write the last phase after positive completion.

The fixed private nine-step sequence is shared by the actual entry and two
standalone pure controls (no adapter inclusion or descriptors). It records six
phase frames: OPEN_BEGIN, OPEN_OK, READ_BEGIN, exactly one inventory class,
FINISH_BEGIN, FINISH_OK. Each frame is one safe rustix write on original stdout,
with exact returned length and deadline checks; no buffered flush, EINTR or
short-write continuation. No raw rule, namespace, descriptor or policy bytes
are printed. Synthetic error/unwind cuts cover every step and permanent reentry
refusal; they are prepared source, not executed evidence.

## Fresh executable and publication scope

The fixed parent child lookup is now solely
`/run/omavless-k1-owned-inventory-v1/child`. Root0755/0555 and /run0755 predicates
are unchanged; this NEW publication leaf is0755, child0555/root-owned/single-link.
No old stage5 directory or stopped stage is read or reused. The child contents
stay exactly 1,475,200 bytes / SHA256
7838d1c3b1b26fa247d0bb16608153f576477c442817f8e1528f6f9c85fe6311;
the child contains no publication path and needs no rebuild for this change.
Parent compilation and dedicated immutable freeze are required anew.

Future stage6 must have fresh absent user/root staging paths, fixed closed
four-member catalog, whole input hash/size validation before publication,
independent exact-source/compiled-image/runner reviews and separate ROOT
transfer/root-copy/publication/native selections. No selector, root staging,
capsule or binary pin is fabricated here. Old positive transport does not
authorize using its destinations for this scope. ROOT alone supplies actual
sudo input through the reviewed prompt-only no-echo controller.

## Compile and witness recipe design (not an executable authorization)

After freezing the source SHA, a NEW private recipe must pin that exact clean
head and current SOURCE input catalog, retaining inherited HOME and fresh700
target/TMPDIR, closed loader/child environment and Cargo-config absence.
Use existing exact export dependencies/lock without adoption into product.
Export only the new fixed `netguard-inventory-entry-v1` into an absent private
path; compile offline/locked -j2 the explicit new bin/feature for static GNU
x86_64. Do not run that binary. Validate ELF as data (no interpreter/NEEDED,
bounded segments/stack) and freeze a dedicated single-link0500 parent with
provenance before constructing the stage6 catalog.

Separate pure selection may compile only inventory_gate_sequence.rs as a
standalone module and select exactly its two synthetic controls; source Python
controls now include shared-deadline/retention/order/opt-in assertions. ROOT
must review a concrete closed recipe before any such selection. The earlier
full-inventory lifetime type controls remain relevant but require recompilation
against materially changed exported source before a fresh native witness.

Future native result requires original transport/process known zero, exact six
frames, one closed inventory class and no opaque output. TableAbsent proves
only the existing fixed-table ENOENT/generation-consistency observation window;
no chain/rule dumps are sent in that branch. Present-table completion requires
all existing multipart parser/terminal-DONE rules. None proves empty host
ruleset, reset continuity, installed origin, canonical ownership, effects,
or whole K1 acceptance. No nft add/delete, policy installation, activation,
packet-policy tests or primary-PC action belongs to this scope.

## Exact compile, offline freeze and source-suite checkpoint

The preceding preparation status is historical. ROOT and an independent
reviewer approved the concrete private recipe for exact source
`b893b06a839ead60ebd0defdd07d090c6b4f0660`. Four in-memory ELF controls passed
(`390ab50`) before the original compile scope (`558af2` / `10b215`) returned
known zero. Its receipt (`028255`) records the static parent build, both pure
entry-sequence tests and bounded ELF data validation. Parent bytes are
1,644,536, SHA-256
`7d00964c959b098f432627bde8ebf013cbd26971e39bc07108c4a9327c0430c8`.
Neither this parent nor its unchanged child was executed by those gates.

After separate full ROOT and independent review, four offline-freeze controls
passed (`2eae52` / `eda521`). The separately selected freeze (`940d37`) returned
known zero; its readback (`f94282`) confirmed a dedicated mode0500/single-link
parent with those exact bytes and mode0400/single-link provenance. Writable
output descriptors were positively closed before readonly readback. This is
an offline artifact-custody result, not guest publication, installed origin or
an executed inventory witness. Private paths and raw captures are not committed.

The first full source suite at b893 hit an unrelated existing DNS package test
socket-path limit: 701 tests, two skips and one error at the fixture's bind.
That failure remains recorded. Fixture-only successor
`00a1d8e5432fa58daff3745651c8d7402d694526` shortens its temporary-directory prefix,
preserves the actual stale-socket/symlink assertions, and adds a length/private-
directory counterexample; no K1 Rust bytes, production guard or socket suffix
changed. ROOT's fresh full `./tests/run.sh` (`5bc32b` / `c35b1d`, readback
`b67269`) then passed 702 Python tests with two existing skips and all JS/QML
contracts. The compiled/frozen artifact still belongs to b893, not a newly
compiled fixture-only or documentation head.

Stage6 transfer/publication, original-parent native invocation and actual
complete read-only kernel inventory remain **UNRUN**. No table classification,
canonical creator, policy effect, installed activation or whole K1 acceptance
is established by this checkpoint. All stopped scopes remain stopped.
