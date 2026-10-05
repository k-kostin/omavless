# Fixed core profile persistence exclusion — SOURCE proposal

This changes only this developer fixture's literal `CORE_CONFIG`. It sets
`profile.store-selected: false` and `profile.store-fake-ip: false`; no normal
product configuration, core code, native bytes or image admission changes.
The immutable code-image allowlist still refuses a mutable database mapping.
There is no data-path exemption, unknown pathname open or diagnostic fallback.
The current tracked launcher still names stopped review3 and is NOT eligible
for execution. Both FULL reviews, a separately reviewed unused stage/source
graph and ROOT's separate selection are prerequisites for any new experiment.

## Shutdown verification and diagnostics — October 5 successor

Fresh review5 used `e5cca64364226ffe8ae1553bbeb71b24598d1cc8`. All 24
separately selected source/native stages returned original zero. Whole
`3a7a7b/session2562 -> 971ce4` returned original exit 2 and remains stopped.
Its separately FULL-reviewed fixed-file observer `623249` returned zero. The
finite projection `453df1` reported 266 literal frames (83 Bridge/183 Session),
last `before_core_shutdown`, no opaque tail, empty inner result, absent outer
result and no launcher marker. Child stderr length/hash:
12509/`42d51bd4ae97f43f984b58c348999e7ea841b0ae0719a65e9f05ea8921366251`.
No live query, signal, reap, cleanup or whole-PASS inference followed.

Independent source review identified a shutdown workload/budget hotspot, not
that stopped run's cause. The fixed native table is 115,843,333 bytes. One
artifact recheck hashes three complete copies; core shutdown does two such
rechecks plus packaged images/maps, at least 695,059,998 native bytes, formerly
all inside five seconds. Existing internal fifteen-second budgets were clipped
by this outer cap. The successor uses an already admitted fifteen-second
read-only verification/maps stage, followed by a fresh five-second original
live/namespace/single-signal guard. The six-second exact-zero settlement and
enclosing absolute Session fence are unchanged. No hash, map row, original
owner check or refusal predicate is skipped, and no earlier result is cached.

All four shutdown roles have one-shot before/after stages for initial live
check, full verification, complete inventory, final live guard, single signal
and settlement. One closed category identifies the local versus enclosing
Session fence; resolved alone adds its two credential stages. No timestamp,
PID, exception, map or private payload is printed. First output failure seals
before continuation, including after the signal; no retry/second signal or
failure cleanup is added. This adds 54 complete-path attempts and 58 vocabulary
labels: conservative Session complete288/cap289, Bridge cap128, lexical cap417.
The old stopped scopes retain their old diagnostic grammars and outcomes.

Source regressions cover slow-but-in-budget verification, a fresh short signal
guard, absolute clipping, every new core diagnostic refusal cut and resolved's
two credential cuts. The fresh full source gate passed 1172 controls with two
existing opt-in skips plus JS/QML; the additional resolved control's affected
gate is recorded at the final checkpoint. The new reviewed VM recipe/results
must still be recorded before acceptance.

## Explicit bootstrap scratch binding

The approved [execution policy](../../docs/development/EXECUTION_POLICY.md)
applies to new experiments; it does not reopen stopped predecessors. Fresh
review4 used the frozen `446f5e6cfae0af11230c7483101a9d365d7e7793` fixture
source and unchanged native pins. Its 24 individual source/native transfers
returned original zero; its whole invocation returned original exit 2. That
scope remains stopped. A separately FULL-reviewed fixed-file observer returned
original zero and reported empty child stderr, empty inner result, no outer
record or launcher completion marker. No live process/namespace query or
failure cleanup was selected, and no whole acceptance is inferred.

Independent source inspection then found an unrelated composition defect:
the stage-derived launcher supplies its scratch directory through `TMPDIR`,
while the lifecycle module still required a hard-coded review3 scratch path.
Containment's pre-chroot mount utilities run before positive isolation, so a
fresh launcher and that unchanged Session cannot satisfy the old equality
check. This is a source-supported incompatibility, not proof of the stopped
run's actual cause.

Session now requires an explicit internal `bootstrap_scratch` construction
argument from the pinned launcher. The lifecycle module contains no old-stage
default. The argument is a bounded, absolute, normalized path; it is not
selected from argv, a private receipt or the environment. The launcher also
requires its admitted source graph to name the same stage before loading it.
The environment remains an equality check before any temporary file or utility
spawn. Post-isolation `/tmp` selection still requires the existing positive
directory-FD check; no path bypass, cleanup or product authority is added.

Seven new pure controls cover exact constructor forwarding for both scopes,
graph-stage mismatch, environment mismatch before any file/process acquisition,
missing/malformed bindings and unchanged post-isolation scratch behavior. The
fresh full source gate passed 1169 Python controls (two existing opt-in skips),
the JavaScript matrices and QML contracts. The earlier regression gate failed
on the missing constructor API; two intermediate source tests exposed stale
mock signatures and were fixed before the passing gate. These results are
source-only. Any successor VM recipe must use a fresh stage, refreshed affected
pins and its own reviewed entry; no stopped recipe is retried or adopted.

## Source-supported incompatibility, not actual cause

ROOT supplied the review3 whole outcome `2e6ade/session38126 -> 9f6e68 EXIT2`;
All 24 earlier separate stages were original zero. That scope is permanently
STOP: no query, signal, reap, retry, archive or cleanup. The separately reviewed
fixed-file observer `ae1fac` returned original zero; its finite projection
`d2ea2e` matched 234 literal frames (83 Bridge + 151 Session), last
`before_core_initial_inventory_first_parse_reject_named_path`, with no opaque
tail. Inner result empty, outer absent, all 18 source/four native pins matched.
The recorded child length/hash is 10658/SHA
`6e2285d08791ec8f3299e1f65fedd580db083eacb0f6dce5f4a34facaa72f67a`.
These are ROOT-supplied bounded facts, not author capture reads, raw maps,
cause/current-preservation evidence, ordering or whole/native acceptance.

The exact public Mihomo source is
[`ab405bad5beeeac8b003bb01f60f134f6df54471`](https://github.com/MetaCubeX/mihomo/tree/ab405bad5beeeac8b003bb01f60f134f6df54471),
tree `bf1d3f0efdc504a883ea89705f274324bc353628`. A fresh SOURCE-only Git fetch
and clean detached checkout verified those identifiers; no Go/compiler/engine
was selected. The former fixture omitted profile fields. The pinned
[config defaults](https://github.com/MetaCubeX/mihomo/blob/ab405bad5beeeac8b003bb01f60f134f6df54471/config/config.go)
set StoreSelected true; ApplyConfig calls updateProfile, which invokes
[patchSelectGroup](https://github.com/MetaCubeX/mihomo/blob/ab405bad5beeeac8b003bb01f60f134f6df54471/hub/executor/executor.go)
before checking whether any proxy needs a restored selection.

[Cache()](https://github.com/MetaCubeX/mihomo/blob/ab405bad5beeeac8b003bb01f60f134f6df54471/component/profile/cachefile/cache.go)
lazily opens the database. With the unchanged literal `-d /home/core`,
[Path.Cache()](https://github.com/MetaCubeX/mihomo/blob/ab405bad5beeeac8b003bb01f60f134f6df54471/constant/path.go)
names `/home/core/cache.db`. The pinned
[go.mod](https://github.com/MetaCubeX/mihomo/blob/ab405bad5beeeac8b003bb01f60f134f6df54471/go.mod)
selects bbolt `v0.0.0-20260706163408-d4ec34ad7c48`.
[DB.Open](https://github.com/MetaCubeX/bbolt/blob/d4ec34ad7c48/db.go) reaches
mapping; its [Unix implementation](https://github.com/MetaCubeX/bbolt/blob/d4ec34ad7c48/bolt_unix.go)
uses the original database FD with PROT_READ/MAP_SHARED. Mapping failures may
have an inherited heap fallback; this is not a claim that every run maps it.
A genuine valid row for that fixed mutable data file is refused by unchanged
`images.PUBLIC`, regardless of read-only map permissions. Source proves this
conditional incompatibility; it does not identify the actual rejected row.

## Complete singleton caller closure for this fixed case

The pinned source has twelve production `cachefile.Cache()` sites in seven
files. All importing files were also inventoried to exclude alias imports.
The additional fakeip cachefile adapter receives an existing CacheFile but
does not initialize the singleton; a pool test is not a production caller.

| Source file / sites | Reached prerequisite and fixed-case exclusion |
| --- | --- |
| `hub/executor/executor.go` /1 | updateProfile calls patchSelectGroup only when StoreSelected is true; the explicit false prevents that call. |
| `component/fakeip/pool.go` /1 | New chooses cachefileStore only when Persistence is true; false chooses memoryStore. restoreState/StoreState operate on cachefileStore only after a type check. |
| `component/resource/vehicle.go` /2 | HTTPVehicle ETag read/write; this fixture has no HTTP provider resource and no ETag override. |
| `adapter/provider/provider.go` /2 | External ProxySetProvider Initial/subscription response; no external proxy providers are configured. The default builtin CompatibleProvider uses only baseProvider, not ProxySetProvider. |
| `hub/route/storage.go` /3 | Explicit storage GET/PUT/DELETE handlers; the fixed controller never calls them. |
| `hub/route/proxies.go` /2 | Explicit proxy update/unfix handlers; the fixed controller never calls them. |
| `hub/route/groups.go` /1 | Explicit group delay handler; the fixed controller never calls it. |

The unchanged config has empty proxies and proxy-groups, no proxy-provider or
rule-provider definitions, direct mode and only a literal loopback nameserver.
Config creates the builtin default CompatibleProvider and GLOBAL selector,
neither of which requires persistent storage during this fixed case. Fake-IP
remains enabled with the same prefix and in-memory pool; its persistence was
already false by the pinned default, now made explicit. The controller reaches
only GET `/connections/conditional-capabilities`, GET `/connections` and POST
the private retained target's `/close-conditional`; it cannot select an arbitrary
storage/proxy/group path. Stream CONNECT, DNS broker lease and reset-while-held
are unrelated to cross-run selection persistence.

The exact public OmaVLESS c4
[`c4e800425243c1b02165f82153e4bf418fe465e6`](https://github.com/k-kostin/omavless/tree/c4e800425243c1b02165f82153e4bf418fe465e6)
was separately fetched by its literal object ID from the public origin.
Its production DNS patch hunks and the retained conditional-close production
patch hunks were read: they add no cache imports/callers and do not change the
profile/default or fakeip persistence selection. The same native artifact and
all broker/descriptor policy remain unchanged. Test-only DNS socket patches do
not add a production cache role. This inventory is for this fixed source/config
and literal request graph, not a global prohibition for arbitrary Mihomo use.

## Inert regressions and remaining gates

The predecessor literal config is 529 bytes, SHA
`acc9e60bca550b5b3ce593bc8290f01a09c7177f48e931e5c922b2ce2e0ec8da`.
Three pure controls require exactly one two-false profile block and otherwise
byte-identical predecessor config; refuse a genuine read-only fixed cache map
without any open/read; and preserve unknown/deleted/alias-path refusals while
the original artifact row still parses. They do not parse config in Go, create
a database, map a file or execute the core. Future native execution could still
refuse for unrelated reasons; no successful outcome or acceptance is inferred.

The anonymous parser policy is also unchanged. The pinned
[Go 1.27.0 naming source](https://github.com/golang/go/blob/go1.27.0/src/runtime/set_vma_name_linux.go)
copies the five-byte ` Go: ` prefix into an 80-byte buffer, leaving byte 79 zero;
[its Linux allocator](https://github.com/golang/go/blob/go1.27.0/src/runtime/mem_linux.go)
names anonymous private zero-offset mappings. Linux v6.17
[get_vma_name/show_map_vma](https://github.com/torvalds/linux/blob/v6.17/fs/proc/task_mmu.c)
wraps an anonymous name in `[anon:%s]`. Existing controls accept only the bounded
Go/plain-bracket grammar with zero device/inode/offset, while spaced glibc,
foreign, oversized and path-bearing labels still refuse. This producer review
does not establish actual mappings, kernel support or any earlier failure cause.

## Exact SOURCE checkpoint gates and transitive pins

The first full SOURCE gate (`811e40/31754 -> 184ac9 EXIT1`) ran 1162 controls
with two existing skips and five graph-admission errors: the tracked positive
source hash had not yet been updated. After synchronizing only that graph pin,
the second (`38c2f1/7335 -> b5b2f1 EXIT1`) had one launcher graph-hash error.
Both NONPASS SOURCE results are retained; neither selected a native candidate.
The launcher graph admission constant was then synchronized to the exact graph
bytes, with no admission predicate or authority change. A tracked-reference scan
found no other stale predecessor positive/graph pins. Pure source inspection
verified all 14 core graph pins and both launcher source pins. The external outer
guard, manifest and index require a separately reviewed fresh recipe; none is
adopted or modified by these tracked metadata updates.

The fresh full gate (`b5a00d/29536 -> 2ebd30 EXIT0`) passed 1162 controls, with
the same two existing skips, plus all JavaScript and QML checks. The three new
focused controls were also original zero (`e48a95`). An initial source-catalogue
inspection assumed every graph member was co-located and encountered a SOURCE
ENOENT for the borrowed bridge; the corrected explicit five borrowed paths
verified all 14 hashes. This was not a failed runtime-scope inspection. These
results are synthetic/source-only; no Go config parser, core/database, native
binary, namespace or VM was selected. Python 3.12 is not on this author PATH and
has not been claimed as a new checkpoint gate.

## Review6 stopped scope and fixed composition descriptor inventory

Review6 used source `3506b566c988211c8c4d927d39c1acece2a6081e` with all
24 separately selected preparation stages returning original zero. The whole
selection `e4ca6c/session59304 -> 26da4c` returned original EXIT2. It remains
STOP: no query, signal, reap, retry, archive or cleanup. The separately reviewed
fixed-file observer `1fee68` returned original zero. Its bounded projection
`17ed14` reports 270 literal frames (83 Bridge, 187 Session), last
`before_core_shutdown_verify`, with no opaque tail; inner empty, outer absent
and completion marker false. Child stderr is 12731 bytes, SHA
`6f945f1d67d63bec491879caf224d1607ead12590631dd65ca30fe82bdba08fe`.
These are lexical file facts, not cause, custody, kernel absence or acceptance.

Source inspection narrowed the failure surface: `Images.verify` first invokes
the Bridge verification and its writable-copy descriptor inventory. The
standalone Bridge scanner admits at most128 entries, whereas this retained
PID1 composition deliberately sets both NOFILE limits to512 and retains source,
image and native descriptors. That is a supported composition mismatch, not
evidence that the stopped process actually crossed128 or that its timeout was
not responsible. Both independent source reviewers agreed with the bounded
correction; no timing budget or image/namespace/hash predicate is relaxed.

The launcher now selects a composition-only fixed512 scanner after isolation,
before the first Bridge acquisition, never as a retry or refusal fallback.
It requires exact integer soft/hard512 before opening, enumerates to EOF with
live scandir duplicate, requires distinct canonical numeric descriptors below
512 and its own directory entry, and checks every descriptor on the copy
device is read-only. The standalone128 body and Images source stay unchanged.
The temporary scanner directory is closed under ordinary backend semantics;
this is not a close, release or custody claim for any retained original. The
scan still needs descriptor headroom and safely refuses exhaustion.

Six new in-memory controls cover full129/512 suffixes, standalone129 refusal,
limit/type errors before open, malformed/duplicate/oversized/self-missing names,
writable/unknown/iteration suffix failures and hook selection ordering. The
focused launcher gate `8672e3` passed25 controls; complete source gate
`cea832/session37831 -> 566ef9` returned zero, 1179 Python controls with the
same two existing opt-in skips, plus JavaScript and QML contracts. Launcher
SHA `91bf0105dff0ef9fb76b633992e0fb64ab7d54a495ac8093168c47d0e82991be`,
test SHA `8872b8efc6ecd21c0e74b4199dbd5e7a7cb4fb89400fba501d10b4afa953d021`,
unchanged Images SHA
`c7a821f478a7102f1c128fb450c4e172f9cdfc30db1d69b4c69d6a08d5f333bb`.
These gates are source-only. A fresh reviewed recipe is required before any
successor VM selection; all predecessor stopped scopes remain ineligible.

## Review7 stopped scope and broker stop contract

Review7 selected `4c19cd58f2a485aaf79a355d57a4162f5ed8fd18` with the
composition512 scanner. All24 separately selected source/native preparations
returned original zero. Whole `2b5dc8/session42131 -> 40f7aa` returned original
EXIT2; this scope is STOP, with no live query, signal, reap, retry, archive or
cleanup. The separately reviewed fixed-file observer `d67bab` returned zero.
Its bounded projection `0f0ca8` matched294 literal frames (83 Bridge/211
Session), last `before_broker_shutdown_settle`, no opaque tail, inner result
empty, outer absent and completion marker false. Child stderr is13953 bytes,
SHA `e897ef761d22148a7b10e002b65c0d4c273ed7e51ddda24cabbc6290d960f4af`.
These are file facts, not the actual wait status, cause, current custody,
kernel absence or whole acceptance.

Inspection of the exact pinned public broker source
`c4e800425243c1b02165f82153e4bf418fe465e6` found a supported incompatibility:
`server::serve` has an infinite accept loop and no graceful signal branch;
`main` returns zero only when `serve` returns Ok. This fixture sends one
SIGTERM after verified core completion and requires an actual normal-zero
broker exit. Unhandled SIGTERM is not normal-zero. That source finding does
not prove this stopped run's actual wait status or cause.

A successor needs a separately reviewed orderly-stop contract and fresh
native/source pins. It must not reinterpret signal death as zero, weaken the
exact-zero settlement, use a wrapper to forge success, clean unknown DNS
state on termination, or retry this scope. A stop request may not itself
establish settled release or authorize removal of retained lease evidence.
