# P4 default residue runner: source-only recipe proposal

Status: NOT BUILT / NOT EXECUTED. Source-only successor to the separately
reviewed overlay/receipt checkpoint f774697b667315a4ae657916f69a3c0a4bdbee0d.
ROOT and independent FULL source, graph, environment and recipe review are
required before any Go command or engine selector. No normal P4 activation,
real socket/TUN FD, VM, Rust runtime owner, profile, merge or release is covered.
The [owning design](P4_DEFAULT_EXPIRY_EXHAUSTION_DESIGN.md) retains the case
causality and closure contract; this proposal does not manufacture elapsed proof.

## Exact source graph

The new [runner](../../tests/fixtures/p4_awg_peer/run_default_residue_overlay.py)
adds separate build/execute phases; it never calls the old rekey runner. Its
seven captured inputs are runner, unchanged supervisor, unchanged export helper,
strict residue parser, frozen #589 support and the two tagged new overlays.
The full upstream source remains read-only at
`/home/kk/.cache/ovtmp-p4/amneziawg-go-upstream`, exact
`b5928efb6ca19f0153958460c3d141f04abc5c2e`; git archive SHA-256
`716c0eec8a7557485555397f1217e0cac56317af8b41fefc287027d0c74ce00d`.
Every build exports attested archive bytes, never executes filtered checkout
bytes, and adds virtual device test files only. Original engine files/defaults
remain byte-identical. Fixture inputs must be tracked, committed and unchanged.

Supervisor SHA-256 is
`00fa64cacdf72d65fcdd772208eca1bc4ca40ca954ef94444ff11eec65b729ec`;
helper is `55fa8597b183a5b8deb0b0aabf25740ee12dfc221054e04e62ac017d1b86cc6e`.
The new loader attests complete bytes before compiling any definitions. It
selects the original five supervisor function bodies, ownership class and two
retained containers verbatim, then binds the separately attested helper module.
It does not execute the supervisor's path-reloading helper prefix; no original
file is patched. The selector/count/receipt policy is new; ownership command,
reap, members, private-directory and save bodies remain the exact original graph.

Build receipt binds fixture commit, archive/input/tool/test-ELF hashes, selected
case, ordinary/race mode, fixed time/resource upper bounds and false network/VM/
intrinsic-heap-cap flags. The physical test ELF resides outside build caches.
Execute checks retained inputs and exact build receipt, then consumes an
exclusive `execution-attempt.json` before the only test2json command. No count
or diagnostic override exists. A failed/uncertain snapshot is never retried.

## Offline environment and resource envelope

Only `/usr/bin/go` and, for a separately reviewed fresh race build, the fixed
`/usr/bin/gcc`/`g++` tool paths are used. Actual tool versions/hashes, compiler/
runtime dependencies and the offline module snapshot must be verified by ROOT
before selecting a build. Prior successful tool metadata is not fresh evidence.
Module cache must be an owned0700 HOME directory with a separately reviewed
offline dependency copy; never pass the user's mutable shared module cache.
Build cache, scratch and artifacts are also distinct owned0700 HOME paths;
the artifact directory must be empty at build selection. No `/tmp`, dependency
download, provider/profile data or shared-cache mutation is in the recipe.

Environment is explicit: PATH `/usr/bin:/bin`, HOME retained owner, LANG/LC_ALL C,
GOENV off, GOTOOLCHAIN local, GOWORK off, empty GOFLAGS, GOPROXY/GOSUMDB off,
GOOS linux, GOARCH amd64, GOMAXPROCS2, CGO0 ordinary/1 race, fixed CC/CXX,
GOMODCACHE private snapshot, GOCACHE private cache, GOTMPDIR/TMPDIR private
scratch. `go mod verify` has30s; `go test -c -p=2 -trimpath -mod=readonly
-tags=p4_cookie_overlay,p4_default_residue_overlay -overlay ... ./device` has120s
and executes zero engine cases. Build output must be empty and status known zero.

The proposed runner lowers only its own/inherited-child SOFT limits, preserving
any stricter inherited bound: NOFILE<=512, NPROC<=1024, CPU<=660s,
FSIZE<=256MiB, CORE0. Hard limits are not raised or changed. NPROC is a per-UID
kernel limit, not a precise count of this experiment's goroutines; failures are
NONPASS, not permission to raise limits. Source fixture bound remains<=222
engine/fixture goroutines plus Go runtime/background/finalizers, with NumCPU1..32
admitted separately; GOMAXPROCS2 does not reduce NewDevice's NumCPU worker count.
There is NO address-space/RSS/heap cap. Linux RSS rlimit is not an enforced heap
bound and race reserves large virtual ranges. Engine pools have no intrinsic
allocation cap. ROOT must accept this explicit resource envelope or require a
separately reviewed stronger one before compilation/selection; no memory-cap
or isolation PASS is claimed by these constants.

## Concrete proposed ordinary selections

The following are future literal paths, not directories created or commands
selected by this source checkpoint. ROOT first prepares and independently
verifies separate private cache/module/scratch/artifact directories beneath
`/home/kk/.cache/p4-residue-ordinary-reject-review` and
`/home/kk/.cache/p4-residue-ordinary-exhaust-review`. Do not reuse prior #589 or
failed artifact/cache scopes. Each case has its own build receipt and ELF.

From the exact frozen #655 worktree, the proposed reject BUILD argv is:

```text
/usr/bin/python3 -I -B tests/fixtures/p4_awg_peer/run_default_residue_overlay.py
--phase build --case reject
--source /home/kk/.cache/ovtmp-p4/amneziawg-go-upstream
--scratch /home/kk/.cache/p4-residue-ordinary-reject-review/scratch
--cache /home/kk/.cache/p4-residue-ordinary-reject-review/cache
--module-cache /home/kk/.cache/p4-residue-ordinary-reject-review/modules
--artifacts /home/kk/.cache/p4-residue-ordinary-reject-review/artifacts
```

Execute is a separate reviewed selection with the same exact argv/inputs,
changing ONLY `--phase build` to `--phase execute`. Exhaust uses `--case exhaust`
and its separate `p4-residue-ordinary-exhaust-review` directories. These are
single ordinary cases, not retries or evidence for each other. Fresh race
artifacts require a separate reviewed environment/path selection and `--race`;
an upstream race report is NONPASS and must not be patched away in engine source.

The only execution argv is `/usr/bin/go tool test2json -p
github.com/amnezia-vpn/amneziawg-go/v3/device -t <frozen-ELF>
-test.v=test2json -test.run=^<selected-fixed-test>$ -test.count=1
-test.timeout=600s`, owned-supervisor timeout620s. Case body/closures remain570s
max and shared closure deadline5s. Coordinating tools must yield/report at most
60s, never block for the entire selector.

## Settlement and retained evidence

The original supervisor retains child/selector/channel graph, uses WNOWAIT
anchor observation, and requires group quiescence plus both EOFs before exact
raw reap/status. Its output bound is2MiB. The strict parser then requires the
complete eight-event grammar, both elapsed values and the selected causal
receipt. Failed complete output is private; unknown supervision emits only a
fixed refusal JSON with `complete_output_retained=false`, no invented capture.
First unknown latch retains the graph and forbids further ownership effects.
No query/signal/reap/close retry follows an
unknown scope. CPython Popen internal constructor failure/partial-pipe behavior
is not governed or proven by the returned-graph supervisor contract.

There is NO export or artifact cleanup path, even after positive compilation.
All source exports/overlays remain available after nonzero, refusal, drift,
constructor uncertainty or positive completion. The source export is bounded
by the8MiB archive and three64KiB overlay-input bounds; build-cache total disk
usage is not quota-bounded by the per-file FSIZE limit. Complete known-zero
events/ELF/source/input/tool checks precede provisional flushed output and the
final success-receipt save. No check/query/output follows that final save.
Failed print yields no success receipt; inherited save/write/fsync/close errors
may leave partial OR full bytes but cannot yield an accepted receipt without
the original CLI's known-zero terminal. Neither a saved receipt nor a printed
provisional line alone authorizes execution/retry. ROOT must separately review
complete known-zero build closure and select execution; failures stop that
snapshot, never trigger receipt-based recovery. That receipt is channel-only source
engine evidence; callback/log stop literals alone are not physical queue joins,
Go heap erasure, OS TUN, installed compatibility or normal-owner adoption.

The [inert runner guards](../../tests/test_p4_default_residue_runner.py) attest
input/definition reuse, frozen object predicates, offline/fixed case fields,
exclusive attempt placement and resource-limit upper-bound behavior using mocks.
They call main ONLY under mocks for commands/tool bytes/resources/export and
control synthetic coordinator paths; they never spawn Go or an engine selector. This source recipe
is not an actual resource, teardown, ordinary or race outcome.

Earlier4786 source gates were7 runner/17 residue controls and full370; its
independent review identified refusal-time cleanup and premature receipt blocks.
That checkpoint and immutable source graph remain fixed, not relabelled.
The source successor removes cleanup and moves every gate before final issuance;
13 runner controls cover mocked mod/compile nonzero/throw, each input/commit/tool/
source/owner drift cut in both phases, print/save failures and last-action order.
No real runner CLI, Go compiler/artifact, engine selector, native/VM command or
actual limit change occurred. Final exact-head source gates are recorded below.

Refusal-path successor source gates:13 mocked/inert runner guards and17
residue/source controls PASS. Both Python3.14/3.12.13 full376 controls (two
existing skips), all JS/QML contracts and `git diff --check` PASS. Mocked main
paths are synthetic coordinator tests, not real runner CLI/Go/resource evidence.
