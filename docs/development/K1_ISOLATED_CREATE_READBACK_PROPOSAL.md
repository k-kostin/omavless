# K1 one-create isolated readback — developer integration checkpoint

## Current integration target — 2026-10-05

New development follows the owner-approved
[execution policy](EXECUTION_POLICY.md), reconciled from policy PR #662 at
`b8c967f2039bdad8a385429a212b977318adc9dc`. Old failed experiments and their
observation restrictions remain frozen. A fresh ordinary public-source
`cargo test --offline --locked -p omavless-netguard --lib --no-run` is SOURCE
compilation: it executes the ordinary build/proc-macro graph, but no produced
test main, manager call, privileged unit, namespace change or nft effect.
Relevant bounded public compiler diagnostics may be inspected and corrected.
That classification does not apply to a wrapper that actually staged a unit.

| Completion item | Required behavior / evidence | Current state |
| --- | --- | --- |
| Compile and deterministic controls | Fresh HOME-based compile; fixed witness, worker, manager and identity controls | Corrected compile and six selected Rust pure controls pass; first new compile failure remains recorded below |
| Isolated one-create VM scenario | Original manager Ref/Start, one durable Pending/Arm, original creator complete inventory/readback, exact six frames and known-zero original completion | PASS as historical developer evidence: ROOT's original upload, preparation, publication, manager run and separately selected observer all returned zero; exact evidence below |
| Inventory / acquisition distinction | Same retained socket and complete table/chain/rule/set/object/flowtable inventory; developer administrative origin explicitly separate from canonical acquisition | Existing complete parser and private witness retained; no new production acquisition constructor |
| Kernel persistence / product integration | Production NetGuard service, runtime Full VPN coordination, crash/restart/recovery and installed package gates | Unavailable; this historical one-create scope does not prove them |
| Host-only acceptance | Physical NIC, suspend/resume, armed boot and other declared K1 host cases | Unrun; no physical-host PASS |

The first fresh compile at policy-reconciled source `05661eb1` returned original
exit 101 before producing a test binary. No fixture executed and no uncertain
external effect occurred. This diagnoses that successor's source failure; it
does not inspect or relabel the old stopped six-pure compile, whose cause
remains unknown. The corrected compile returned original zero; the three
private create-witness controls, worker failed-write-order control, manager
exact-frame grammar control and three-unit identity control each passed, with
exact expected counts (3 + 1 + 1 + 1). The six Python source controls,
format check and documentation navigation (101 links) also returned zero.
Produced developer test-binary SHA-256:
`1263be2059343836bb535466951703a6df441cbf644cbb5f6711874c9a501024`.
No ignored/native entry ran during that SOURCE gate. This is ordinary developer toolchain/cache evidence,
not toolchain attestation, canonical authority or installed acceptance.

The additional strict all-target Clippy compile found
`unnecessary_literal_unwrap` in the inherited private witness's synthetic
zero-handle control, returning original exit 101. The test now borrows and
mutates the same optional table metadata; no witness constructor, effect or
parser changed. Strict all-target Clippy then returned zero, and all six pure
controls passed on the corrected test binary, SHA-256:
`ca6bcc6fa998ad1167a0d4096f1b925181260110aac134433a6c064fe2c37e15`.
The prior binary hash is retained as historical source evidence, not the VM
selection. Formatting passed after the pure test change.

The new remote Test job at `c8b68b8c` still failed before Rust execution: the
external launcher's source-only shallow-CI control read the current inventory
module against its older immutable exporter pin. The sole module delta is the
four-line `cfg(test)` witness declaration. The control now admits the exact
successor hash, projects out only that fixed declaration and verifies the
resulting original hash before the unchanged adapter runs. Arbitrary changed
source, alternate declarations and every other pin still refuse. All 14
launcher source controls pass, including a new projection-refusal regression.
The developer exporter still selects immutable `e6488ed5`; no product source,
exported runtime or executable-selection pin was changed by this CI correction.

The smallest VM scenario is the fixed ignored manager entry
`manager_retained_lifecycle::adapter::exclusive_create::observe_one_create`,
executed only by ROOT after review and fresh publication. It admits the literal
`/run/omavless-k1-exclusive-create` stage, original executable and exact unit
fragment before original Ref/Start; its new `PrivateNetwork=yes` worker performs
one Arm(7, Full), one exclusive create and original-socket complete readback.
The manager requires the original successful invocation and exact six-frame
result. Success remains historical and explicitly noncanonical; it does not
delete the table, Stop/Unref the unit, or promise custody after process death.
The fixed unit SHA-256 is
`3123aa8e484560fc83b4bde8a09e8e0d192c7d918ff5f8582a982dc6a9f1be52`.
Fresh publication and ROOT's original run subsequently completed at the exact
artifact below; their result remains historical and noncanonical.

## ROOT-executed isolated completion — 2026-10-05

Tested Rust artifact source:
`c8b68b8c1faf7454d1fe0d0b343205809fb69b9f`. The following source-controls-only
successor `ee59522d142ac9e06456b4edb810322a97b412ee` changes no Rust, unit or
artifact bytes. At `ee59522d`, remote Test, x86 packaging and ARM packaging
all passed. ARM's first acquisition failed before compilation on the official
mirror's HTTP 500 for `guile`; one authorized ordinary-infrastructure retry
passed without a workflow or mirror change. The original failure is not erased
or misclassified as a source compile result.

ROOT performed all VM actions. The exact native test binary is
`ca6bcc6fa998ad1167a0d4096f1b925181260110aac134433a6c064fe2c37e15` and the
exact unit is `3123aa8e484560fc83b4bde8a09e8e0d192c7d918ff5f8582a982dc6a9f1be52`.
Fixed public-source wrappers were FULL-read reviewed by ROOT and an independent
reviewer before separately selected dispatch. Their SHA-256 identities are:

| Source role | Exact SHA-256 |
| --- | --- |
| Guest publication/run/file-only observer | `4948a9bded078d8bcd4b9e6c88ee5a5ec2001fc736a3b5f1d3df6978bba20db6` |
| Exclusive root input preparation | `421b5fd4e1e7aa631032e811bf085ebe54c77217440326e1e9a21b190dcdf5a0` |
| Fixed authenticated upload / separate real-TTY dispatch | `5f1846002f973235ada86c1b87a75dc21c60770b1c40821d207a5b2218458898` |

The source-only wrapper controls passed with original zero (16 guest, 7 input,
9 dispatch). The ordinary upload receiver accepted only the fixed three pinned
public bodies into a fresh exclusive directory. Root input preparation made
fresh root-owned copies, then publication created the fixed stage/unit and
completed one original daemon-reload. No existing resource was replaced.

| Original ROOT operation | Original result / bounded evidence |
| --- | --- |
| Authenticated upload `6af1f4` | Exit 0; exact 35-byte completion marker; empty stderr |
| Input prepare `49f2b7` → `33f42c` | Original exit 0 |
| Publish `db7ce8` → `f761f2` | Original exit 0; stored publish result known-zero, both captures empty |
| Manager run `23559b`, original session `63362` → `a2fc73` | Original exit 0; not a copied stored result |
| Separately selected file observer `d33cb0`, session `31660` → `5c7c9d` | Original exit 0; historical_complete true, historical_only true, canonical_authority false |

The observer admitted the run's complete exact 256-byte one-test stdout grammar,
one completion token, one fixed successful footer and empty stderr. Stdout
SHA-256 is `676d5c6fb50b2da61f7eabd62b6399fe7f377a06d578e53f69500b2a4196e1fc`.
It emitted only bounded classes/counts/hash/length, not raw private captures or
kernel/process facts. ROOT retained every original whole outcome as zero; the
stored file outcomes and projection did not substitute for those originals.

This completes the intended one-create integrated developer checkpoint: real
LockedState Pending/Arm, private creator witness, strict all-ACK including END,
same original-socket complete table/chain/rule/set/object/flowtable inventory,
final original-session recheck and exact six worker frames admitted only after
the original successful manager invocation. No Stop, Unref, delete, failure
retry, old-scope query or cleanup was selected. Positive completion is not a
live-owner transfer, kernel lifetime guarantee, persistent production receipt,
installed service or canonical launch acquisition.

Root-owned administrative VM manager/broker, ordinary toolchain/loader paths
and stable trusted administrative home/cache custody are explicit assumptions.
The wrappers do not prove hostile-parent-rename resistance or authenticate the
installed PID1/broker image. Old stopped experiment scopes remain untouched.
No product packet-blocking, crash/restart, boot, physical NIC or suspend gate
is passed by this completion. Main, release and marketplace remain held.

The following proposal and pre-compilation checkpoints are retained history,
not the current compilation/native status. This was a successor to Draft #657.
The compiled `83f42593` extraction has only three pure synthetic test results.
No source here authorizes effects in the stage6 read-only namespace, host
namespace, installed service or an earlier attempted fixture.

## Intended evidence and limit

Exercise exactly one existing `LockedState` Arm request, causing durable
`pending_create`, `FixtureCreator::full(None)`, the private exclusive-create
witness and complete same-session inventory readback. The result is historical
evidence of that completed operation in a new disposable VM namespace. It is
not a live owner transferable to a later invocation, canonical authority or
proof that protection survives worker exit or manager lifecycle changes.

The worker must never replace, disarm, delete, inject a generation cut, reopen
an uncertain creator or enumerate processes to reconstruct custody. Existing
`manager_retained_lease` runs all those additional positive/negative scenarios
and is therefore not the proposed entry point. Its old stage is never reused.

## Namespace origin and manager boundary

Use a new literal developer fixture identity, unit, stage and ignored test name.
The fixed unit must request `PrivateNetwork=yes`, have no namespace-sharing
source, and be admitted as never started before its one StartUnit call. A
different inode or loopback-only inventory is a negative isolation check, not
proof of original acquisition or freshness on its own.

Reuse the existing retained manager admission graph: fixed original system-bus
connection, pinned unique manager owner/version, exact unit fragment bytes,
executable identity and typed effective properties. This graph trusts the
disposable VM's administrative bus/manager origin; it does not independently
authenticate the broker or PID1 installed image. That missing canonical
provenance is not supplied by a unique-name string. The outer owner retains its
original reference before StartUnit and records the original returned job and invocation
under the existing bounded observation rules. No supplied PID, arbitrary unit,
namespace path, caller-produced receipt or post-failure lookup substitutes for
that acquisition. Review the complete reached admission graph before adapting
it; this proposal does not assert that a new name alone passes those checks.

The root adapter retains original host/current namespace descriptors and
publishes a fresh pinned negative witness through the existing helper. The
worker first captures its inherited original host anchor, its current namespace
and that original witness, then rechecks exact credentials, distinct namespace,
loopback-only device inventory and the existing no-switch restriction before
opening its netlink session. The session's namespace identity must match the
retained worker namespace. These witnesses establish only the developer
isolation boundary; the fixture epoch remains explicitly synthetic.

## One effect-bearing path

1. Retain the fixed stage, isolation witnesses, one fresh root state store and
   one creator in the existing explicit held-owner pattern before their use.
   Acquire the state lock and require complete absent inventory with no prior
   creator effect. Partial acquisition remains a separately reviewed boundary;
   no complete-owner claim follows a constructor that has not returned.
2. Call exactly one `LockedState::request(Arm { generation: 7, mode: Full })`
   with the existing synthetic namespace vocabulary. Do not hand-write a
   Pending JSON file: the actual locked state machine must persist it before
   the callback, and `FixtureCreator::pending_at` must observe both Pending and
   the held lock as it already does.
3. The existing callback must run the `83f42593` path: original mutable lease,
   fixed generation-conditioned exclusive batch, complete strict ACK including
   END, same socket full table/chain/rule readback and final lease recheck. Keep
   its shortened original deadline; do not renew it after the effect. No second
   parser, copied tuple, supplied classification or Boolean can mint a witness.
4. Require the existing successful response, one effect, and the handle retained
   by this creator. Recheck the original isolation/session before accepting the
   worker result. No extra effect or different socket is required to prove the
   already completed same-session readback.

## Fixed output and failure boundary

Proposed finite worker vocabulary is `ISOLATION_OK`, `STATE_READY`,
`CREATE_BEGIN`, `CREATE_READBACK_OK`, `FINAL_RECHECK_OK`,
`COMPLETE_NOT_CANONICAL`, each with one fixed prefix and LF. `CREATE_BEGIN`
precedes Arm and does not claim Pending already exists. `CREATE_READBACK_OK`
follows the positive returned Arm and all internal witness checks. No frame
contains PIDs, handles, paths, rule dumps, exception text or private data.

Every frame needs one safe write attempt, exact full byte count, and sampled
pre/post bounds. A partial/late/error output is uncertainty, not another output
attempt. The complete ordered grammar plus original known-zero worker and
manager command outcomes is required; any prefix is only progress. Ordinary
blocking-syscall/runtime assumptions and the outer budget must be stated, not
claimed hard preemption.

After an effect-bearing error, timeout, panic or unknown return, preserve the
same held state lock, creator/socket and namespace witnesses; do not query,
retry, delete, StopUnit, UnrefUnit, signal or reap to obtain a result. A handled
uncertain worker parks without further operations, as the existing fixture
does. Fatal process death can destroy custody and is not a preservation proof.
The outer owner must likewise retain its graph and stop continuation.

On positive completion, the worker may return normally, making its proof
historical. No live-owner claim is made after that boundary. Any subsequent
positive-only manager Stop/Unref must be an explicit separately reviewed stage,
never inherited from the old create/replace/delete coordinator or run after an
uncertain outcome. Retaining a live owner instead would require a different
protocol and is outside this proposed one-create checkpoint.

## Historical pre-compilation source checkpoint

The uncompiled successor adds closed `Fixture::ExclusiveCreate`, its unit,
`kernel_exclusive_create_fixture.rs` and `manager_exclusive_create_fixture.rs`.
The manager lane does not enter the old lifecycle coordinator: it retains its
new original Ref/Start job, accepts only positive completion of that invocation,
then reads the exact six-frame `create.frames` file through an original bounded
pin. It never calls the old native absent/retired verifier or Stop/Unref methods.
The legacy coordinator explicitly refuses the new identity.

The worker uses the existing isolation helper with a new fixed witness identity,
one locked state and one creator. The unchanged state machine also performs its
existing post-effect observation on that creator; this is not a second effect
or an external readback supplied to the witness. Its internal inventory budgets
remain distinct from the new sampled five-second worker envelope. No hard
preemption or universally shared syscall deadline is claimed. The manager has
one sampled 45-second envelope and at most 450 original-job observations.

Two Rust pure controls remain UNRUN. At `c7fe444c`, the first six Python
source-wiring controls returned five passes and one fixture failure (`542d6d`):
the frame check split at `];` inside the Rust array type before reaching its
values. The successor narrows that source-only extraction to the array value
delimiters. Its separately authorized six-check run completed with explicit
exit tracking and status zero (`332001`); the corrected test SHA-256 is
`31992fed9b37273ee51d8a8a77e052b44e83bc2c2fd4f2e0f8c01ce6fed087fe`.
At that gate the Rust source was exactly the uncompiled `c7fe444c` source.
The subsequent source-only identity-control update (`d6fee996`) covers all
three fixed units and their pairwise stage/unit/fragment separation; it remains
UNRUN. Documentation
navigation passed (93 local links), and the whitespace check passed. No Rust, formatter or
native fixture was selected. The controls cover finite frames, failed-write
continuation, fixed unit and existing Pending/witness wiring; they do not prove original acquisition or
kernel effects. Dedicated negative lifetime/type controls remain pending.

The same source successor replaces the witness's explicit Result match with
`result?`, preserving the `InFlight` error/unwind poisoning. This is a source
simplification, not a diagnosis of Draft #657's negative CI result. The compiled
three-pure evidence remains tied to the unchanged historical `83f42593` head.

ROOT and an independent reviewer must read the complete new reached graph,
including original manager acquisition and all failure paths, before fresh
compile/export/freeze and VM publication recipes can be proposed. Exact
executable hashes, stage publication, manager properties, sampled budgets and
original capture custody remain unprepared. No command in this document is an
executable recipe; no namespace, service or native fixture has been selected.

## Historical reviewed formatting-only checkpoint

ROOT reported eight mocked recipe controls and eight separate stdin-to-stdout
formatter invocations at original zero. All original source remained unchanged
until ROOT read the complete outputs. Only the worker, manager and identity
outputs differed; their reviewed changes are whitespace and import ordering.
Those exact three outputs were applied, leaving the five identical files alone.
The Python wiring assertions now accept the corresponding whitespace and an
optional trailing field comma without relaxing their operation/order checks.

The subsequent six Python source controls, documentation navigation (93 local
links) and whitespace check completed with explicit exit tracking and zero
(`a86ec7`). This is formatting/source evidence, not compilation or execution of
the new Rust controls. The next separately reviewed compile recipe will select
six pure controls: three witness controls, the worker and manager frame controls,
and the three-unit fixed-identity control. All new ignored/native entries remain
UNRUN; the historical compiled witness evidence remains at `83f42593`.
