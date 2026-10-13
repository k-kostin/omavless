# P4: private captured-input staging and completion model

Status: **SOURCE ONLY, checks pending; no validator or namespace executed**.
This successor to [#764 capture](P4_CANDIDATE_VALIDATION_CAPTURE.md) prepares
the next resource-aware boundary. It does not enable candidate v4, replace the
production v3 validator, or introduce a second lifecycle owner.

## Exact private seam

`isolated_validation::candidate_validation` is a private sibling behind the
existing explicit `p4-candidate-preparation` feature (and unit tests). Its real
`stage` consumes the same `CandidateValidationSnapshot` **by value**. Core,
config and mandatory resource bytes come from that carrier, not another path,
store/cache/template read, caller-rendered config/manifest, digest or boolean.
The carrier still owns its recognized template and prepared intent generation
and mode; staging does not reinterpret canonical mode semantics.

There is no public execute/validate/spawn/success API, NativeHost/startup call,
lease conversion, installed-tool acquisition or production launch callsite.
The tiny input/child/clock traits are private synthetic-test seams, not public
alternate constructors or command/environment injection APIs. A synthetic
input fixture is NOT an eligible candidate, core ELF or valid MRS dataset.
Normal fake exit 0 proves only the completion model, never namespace or engine
validation. Actual launcher/kernel qualification remains a separate boundary;
no caller permission token or boolean pretends to close it.

The existing default-v3 `capture`, `validate`, sandbox builder, runner and
Scratch body remain byte-identical; only a feature-gated sibling declaration
is added to their file. Existing connected-v3 input requirements, core-path
reread and historical ignored kill/wait outcomes are not silently corrected
or accepted by this new source slice.

## Staged byte and lifetime proof

The new candidate-only initializer uses exclusive `0700` directories under
an absolute private parent. It saves each acquired directory FD before the
next fallible operation, including partial initialization. This narrow separate
initializer is necessary because the old initializer returns a fixed error
without transferring its partial FD anchors; its default behavior is unchanged.
The established Scratch file writer still creates core `0500` and config/MRS
`0400` exclusively, replacing writable handles with same-inode read-only anchors.

Prelaunch and post-completion proof check exact held-FD bytes against the same
owned carrier, owner UID, fixed modes, ordinary single-link files, lengths,
stable dev/inode/gid/time metadata and path-to-held-inode equality. Directory
membership is exact, read through held directory FDs and bounded by the
recognized snapshot membership. There is no recursive removal or adoption of
foreign/replaced entries. This is not hostile same-UID atomic-tree attestation:
the caller still owes concurrency/ownership fences; pre/post checks do not
eliminate every check-to-use race.

The fixed candidate command differs from the old command by exactly one
`--as-pid-1` option in bwrap's prefix **before** `/input/core`. Mandatory
user/network/PID isolation, disabled further user namespaces, cleared
environment, null stdin/outputs, capability drop, read-only input/root, 16 MiB
work tmpfs and `core -t -d /work -f /input/config.yaml` are preserved. No shell,
host-network fallback, prerequisite installation, fetch or provider probe exists.

Why PID1 matters: the reviewed upstream bubblewrap source at
[`0f8073dd9b547e15e8ec9451371f9729830b083d`](https://github.com/containers/bubblewrap/blob/0f8073dd9b547e15e8ec9451371f9729830b083d/bubblewrap.c)
reports the initial application's status through eventfd while namespace init
can still be reaping descendants. `--as-pid-1` suppresses that eventfd/init-fork
path and waits for the actual namespace-init/core child instead. The reviewed
Linux source at
[`24a19b953e2a3d0ea5a8c4951a0ab8f32720c907`](https://github.com/torvalds/linux/blob/24a19b953e2a3d0ea5a8c4951a0ab8f32720c907/kernel/pid_namespace.c)
documents descendant shutdown before namespace init can be reaped. These are
architecture references, NOT admission of this host's installed bwrap,
interpreter/libraries or kernel, nor actual candidate-core PID1 compatibility.

## Finite original-child model, not effect admission

The three-second execution budget starts before launch. The model checks the
deadline before and after each `try_wait`, and after its post-tree proof. Late
exit 0 remains failure, with retained scope; an already reaped child is not killed.
There is no unbounded `wait`, process-group/name/PID lookup, alternate launcher
or regeneration/retry. On timeout it calls kill at most once on the original
owned Child, then polls that same handle for at most one second of grace.
These are execution/grace polling budgets, **not a hard whole-call deadline**:
staging, spawn, filesystem calls and signal delivery have separate limitations.

An armed retained owner is constructed before any new resource acquisition.
It records the intended scratch path before creation and owns each returned
FD immediately. Staging/supervision/completion/failure move the same armed
payload, never plain Scope/Child locals across fallible callbacks. The original
child is stored immediately when launch returns, before clock/wait/proof calls.
Unwind and consumer Drop retain this payload; only explicitly proved exact
cleanup disarms it. No production catch-and-retry or broad panic policy is added.
A callee that has not returned a child handle cannot establish that handle's
ownership here; installed launch integration remains closed.

Spawn/wait/kill/nonzero/signal/timeout/late completion or tree-proof refusal
retains the input carrier, every staged/partial FD anchor and any original
child handle. Even a reaped launcher after timeout does not establish namespace
closure and cannot authorize cleanup. An opaque fixed/redacted failure owns
them in the armed `ManuallyDrop` owner: consumer/result Drop neither kills/cleans nor discards
the only live child handle. There is deliberately no recovery/cleanup/retry API.
This intentionally retains allocations/FDs and private disk scope per failed
attempt; it is not a global memory/disk budget or crash-survival supervisor.

Private source-model finalization requires completion plus another exact tree
proof before the existing exact Scratch cleanup. Cleanup errors, including
partial deletion, retain all remaining anchors and never become success/retry.
That model has no production launch caller. A live validation success route
must first close the selected installed launcher/namespace assumptions; a mock
zero or preparation eligibility cannot do so.

## Controls and next selected gate

### Retained source-review refusal

Original source-only checkpoint `a43a74025e1b75480564cb083ccd12b1e85e6300`
was refused in independent Astra review (P2): `ManuallyDrop` existed only on
explicit Failure results, while panic in staging/launch/try-wait/clock/proof
could drop ordinary Scope/Child locals and lose the only child handle/anchors.
No Cargo/test/engine result existed at that head. The narrow successor arms
ownership before acquisition and keeps it armed through every phase and result,
disarming only after proved cleanup. Prepared unwind controls reread retained
directory/file FDs and check input/child Drop counters after caught **test-only**
panics and consumer Drop. That failed source checkpoint is preserved, not
retroactively accepted by the correction; checks/review remain pending.

Prepared normal controls are file/mock only: exact owned staging; byte/length/
mode/owner/link/membership/replacement drift; sole PID1 argument delta; retained
child/input/partial anchors after Drop; late zero; deadline before launch and
slow spawn; finite grace and retained launcher zero; spawn/wait/kill/nonzero
refusals; post-completion drift and partial-cleanup retention. No test in this
new module spawns a child or runs a core/bwrap command. Three additional controls
cover staging directory/file-anchor unwind, launch/obtained-child/wait/clock/
kill/pre/post/cleanup-proof unwind and staged/completed consumer Drop. All twelve
controls remain unrun. The old v3 execution
tests retain their existing separate effect prerequisites.

Before Cargo, freeze the exact source and obtain Root/Astra scoped review.
Then separately select serial bare-default/feature checks, strict clippy and
these normal controls, retaining every original failure and stopping dependent
commands on nonzero/unknown. Authentic core DATA staging, valid resource-bearing
MRS and native offline validation are separately admitted rows, not implied by
these mock fixtures or inherited #764 capture results.

Future owner integration must still bind revision, lease/generation, exact
desired/candidate/template inputs and trusted host-empty observations before
effects and after completion. No quiesce, active replacement, startup/controller
registration, real key, VM, service, route, DNS, provider, TUN or production-v3
default change is included. Existing R6 evidence is not rerun or broadened.
