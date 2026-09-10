# R5 isolated subscription probe: pure plan checkpoint

## Boundary and reference

Python `backend.py` still owns production subscription latency testing:
`probe_proxy_yaml`, `probe_core_config`, `collect_probe_response`,
`run_mihomo_probe`, and `probe_subscription`. This checkpoint adds only
`omavless_mihomo::probe_plan`; it is not registered with the daemon, CLI or QML.
It does not resolve DNS, open sockets, start a core, write files, update latency
caches or change connection state. Python cannot be removed on this evidence.

The plan borrows existing `CanonicalProfile` instances, reuses their canonical
Mihomo renderer with a typed IP server override, and preserves original TLS/SNI,
transport and protocol semantics. There is no second profile parser or store
schema. Profile results are positional; a future owner maps positions back to
its admission-fenced internal record IDs. Neither private configurations nor
plans implement `Debug`.

Bounds: 256 profiles, at most four distinct pinned addresses per profile, at
most 64 target aliases per chunk, at most 16 chunks and a 4-MiB configuration cap
per chunk. Address values must already have passed the future resolver's public
address and provenance policy. `IpAddr` prevents hostname/config injection but
is deliberately **not** a DNS/pinning/public-address verification claim.

Each chunk uses opaque `p0000a0`-style aliases, a relative private Unix controller,
the reference routing mark, and no TUN, DNS server, inbound or TCP controller.
Generated contents remain credential-bearing private material: the future
executor must use a disposable 0700 directory and atomic 0600 file, never argv,
ordinary IPC, logs or diagnostic bundles. The plan creates no such file itself.

## Schedule correction and aggregation

The Python executor requests each of three public HTTPS URLs once for the entire
group, with a 5000-ms HTTP timeout and expected status 200–299. It does **not**
request three repetitions of each URL for each target. The previously unused
Rust `probe_schedule` helper incorrectly represented nine samples per target;
this checkpoint corrects it to three projected group memberships. Those
memberships are not individual HTTP requests. The executable plan uses the
three fixed `PROBE_URLS` rounds per chunk.

The collector admits each chunk/round once in order and reuses the existing
`merge_probe_response` implementation. Thus accepted 504 all-timeout responses
are meaningful empty samples, malformed/rejected responses are not successful
probes, and no completed round is an operation failure rather than evidence of
an unreachable provider. At most three delay values accumulate per alias.
All successful address samples for one profile feed Python-compatible median
rounding (nearest integer, ties to even). Unresolved and resolved-but-unreachable
remain distinct. Incomplete collection cannot be reported as finished.

Chunking is a deliberate new bounded-resource constraint; the Python reference
uses one unbounded target group. It can change total batch timing, not alias,
rendering or per-profile reduction semantics. No scheduler timing claim is made.

## Differential evidence

`crates/omavless-mihomo/tests/probe_plan_differential.rs` reuses accepted cases
from the existing canonical VLESS and non-VLESS corpora, with no addresses, one
documentation IPv4 address, and an IPv4/IPv6 documentation pair. The test passes
only synthetic inputs through stdin to `tools/probe_plan_parity.py`.

The adapter calls actual Python config rendering and actual
`probe_subscription` aggregation, replacing all store, resolver and execution
boundaries with in-memory stubs. It compares exact config-shell fingerprints,
asserts reuse of canonical proxy rendering, and reuses the established canonical
semantic fingerprint oracles for each overridden proxy (YAML quoting and map
ordering are not protocol semantics). It compares bounded result classifications;
no URI, generated config or raw backend exception is
printed. It also inspects the actual executor's public-URL loop without running
the executor. Unit tests exercise maximum chunking, unique aliases, no-TUN
shape, address/profile bounds, duplicate/out-of-order responses, unresolved and
timeout distinctions, ties-to-even medians, and empty plans.

Synthetic canonical cases are **not** live experimental-protocol evidence.
V0/#30's unavailable fixture families remain unavailable.

Local validation on the current-main-based candidate:

- Seven new pure-plan unit tests and the corrected schedule unit test PASS.
- 153 differential cases (51 canonical cases × three address modes) PASS.
- Existing 14 response-merge differential cases PASS.
- `cargo clippy -j1 -p omavless-mihomo --all-targets -- -D warnings` PASS.
- Workspace formatting, oracle Python compile and `git diff --check` PASS.

Tests used the dedicated `profile-details-target` sequentially with one Cargo
job and incremental compilation disabled. No full Python/full Rust repeat or
live fixture run was performed for this unreachable pure slice. The final
post-test source adjustment removed a redundant `must_use` attribute flagged by
clippy; it does not change plan or aggregation semantics.

## Mandatory blocker before any executor or UI activation

Current native host observation counts all processes named `mihomo`:
`NativeLifecycleHost::visible_core_count`, fresh disconnect observations and
production ownership checks. Starting an isolated probe beside the live VPN
would currently look like an unowned duplicate core; disconnect cleanup expects
zero cores and could fail. It is unsafe to wire a process launcher to this plan
without first resolving that ownership model.

The next checkpoint must define and test a supervised auxiliary child identity
(PID plus process start identity, exact private config/controller ownership),
registered under the existing operation/batch owner. Unknown cores must still
block; do not ignore arbitrary names or suppress duplicate-core checks. Auxiliary
work must cancel and reap before connect/disconnect ownership transactions,
without waiting while holding the runtime owner mutex. Child death, cancellation,
daemon restart and UI close must not alter the requested tunnel or leak children.

Further prerequisites: pinned resolver policy, bounded controller group-delay
adapter, supervised child startup/shutdown and signal cleanup, generation-fenced
subscription result publication, and exact-head Try Omarchy active-VPN plus
disconnect acceptance. No new sudo/pkexec path or privileged arbitrary command
channel is permitted. No host smoke is required for this unreachable pure module;
host smoke becomes mandatory when the future executor is wired.
