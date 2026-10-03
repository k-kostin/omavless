# P4 pinned AWG source-engine timer boundaries

Developer-only, CPU-only continuation of [the AWG fixture](P4_AWG_LOOPBACK_SMOKE_PLAN.md).
Base #558: `a8fa1711db5b2b9e69c9548078f199d1360ad46e`.
Executed Go test/runner source: `88414a1c524c9d7ae17cb8ccfcff53febe4be5ad`.
Receipt-only follow-up: `0dcf2ec11309c9cf7d8b6c5ec417c253e455a36c`.
That follow-up changes Python receipt guards, not the Go overlay. No VM lease, guest
operation, host networking/configuration, installed profile or Cargo build was
used. Rust remains the normal runtime owner; P4 activation remains fenced.

## Execution and provenance

The dedicated [runner](../../tests/fixtures/p4_awg_peer/run_timer_overlay.py)
reuses the existing pinned-export/bounded-child helpers without changing their
API. Exact official engine `b5928efb6ca19f0153958460c3d141f04abc5c2e`
(`v3.1.20260828`) is freshly exported and verified before/after execution.
Checkout bytes are not executed. An add-only virtual overlay supplies the
existing in-memory Bind/TUN support and [four timer cases](../../tests/fixtures/p4_awg_peer/upstream-tests/timer_boundaries_test.go).
It replaces no upstream implementation. No sockets, TUN FDs, namespace, unsafe
or reflection operations are added. Synthetic identities stay solely in silent
test memory, and raw subprocess output is withheld on refusal.

The clean offline compiler environment disables inherited Go configuration,
workspace, flags, toolchain downloads, proxy and sumdb. Cached pinned modules
must pass `go mod verify`. Executed overlay bytes and runner/helper inputs are
checked after execution; source hashes and exact nonempty Go JSON pass receipts
are required. Private HOME scratch/cache is used; temporary exports/overlays
are removed on success/refusal. No peer/core executable or package is built.

Toolchain: `/usr/bin/go`, `go1.27.0-X:nodwarf5 linux/amd64`.
Invoke the dedicated Python runner with explicit `--source` (clean exact-pinned
checkout), `--scratch` and `--cache` (owned 0700 HOME directories),
`--module-cache` (owned HOME pinned dependencies), and `--count 50`;
use `--count 20 --race` for the race run. The runner refuses missing/unsafe
prerequisites and never downloads them or repairs the source checkout.
Executed artifact hashes:

| Input | SHA-256 |
| --- | --- |
| Official source archive | `716c0eec8a7557485555397f1217e0cac56317af8b41fefc287027d0c74ce00d` |
| Dedicated runner executed for the 200/80 cases | `df4d594db3890943052cbd263b92dd17f851ff3eec92ddffe290a7fd0e170bb5` |
| Unchanged export/child helpers | `55fa8597b183a5b8deb0b0aabf25740ee12dfc221054e04e62ac017d1b86cc6e` |
| Existing in-memory support overlay | `40983d9632f4f92d3e157d25e17c91e5703b53e49aad39f559332d5e423a1e36` |
| Timer overlay | `82dd5560c2ec7414d39bc6288f98d48ecf6b60ffff3cada7bb287d9cf1b2efbd` |

## Actual bounded results

- Final normal 50 repeats: **200 subcases PASS**. Race 20 repeats:
  **80 subcases PASS**. Four cases and their parent must each pass exactly the
  requested repeat count, plus exactly one package PASS. No-tests, missing,
  duplicate, skipped, failed, extra-test and wrong-package receipts refuse.
- `defaults`: actual engine helpers match the fixed default retransmit,
  keepalive, new-handshake, send/receive refresh, keychain expiry, minimum retry
  and maximum-attempt values.
- `ranges`: each repetition samples actual configured timing/attempt ranges
  128 times. Values stay within bounds; new-handshake uses keepalive upper bound,
  receiving refresh subtracts keepalive/retry lower bounds, keychain expiry uses
  reject upper bound, and a negative receiving refresh clamps to zero. This
  proves sampled bounds, not randomness quality or every possible value.
- `retry_exhaustion`: invoke the real expiry callback directly at `attempts ==
  max` and then `max + 1`. The final retry emits an actual protected initiation
  into the in-memory Bind; exhaustion drains a real staged pooled packet,
  cancels keepalive, emits no retry and schedules residue cleanup at three times
  the configured reject upper bound (66 seconds). An existing cleanup timer
  is retained without renewal. **The 66-second interval is inspected, not waited.**
- `elapsed_key_expiry`: actual Noise initiation/response and symmetric-session
  derivation populate the real keypair/index. A second real partial exchange
  and staged pooled packet are also retained. The real `Timer` fires at a
  **test-shortened 1 ms delay**, with a two-second completion deadline, and
  invokes the official zero-key-material callback. The current keypair,
  indexed partial handshake, ephemeral state and staged packet are removed.
  Other key slots are checked empty but were not populated by this case.
  No global constants or upstream implementation are changed. This is elapsed
  test-timer cleanup evidence, not elapsed product `RejectAfterTime` acceptance.
- Initial ordinary source suite: **313 tests reported, two existing skips, zero
  failures** (311 executed); JS/native/QML contracts pass. Three added pure
  receipt guards run in ordinary CI; actual Go overlay execution remains opt-in.
  Go formatting and diff whitespace pass. No local full Rust result is inferred;
  all Rust/Cargo inputs are unchanged and the Draft's normal CI remains separate.

Independent review then found that a malformed event could be ignored among
valid PASS receipts. The receipt-only follow-up rejects missing/non-string/
unsupported `Action` and explicit null/non-string `Test`. A fourth pure guard
tests these forms, with four focused guards PASS. The final local source suite
reports **314 tests, two existing skips, zero failures** (312 executed), plus
JS/native/QML contracts and documentation navigation PASS. Its runner SHA-256 is
`d01681788a19837f382ef4fd55eb12fa3862e889f12f5e6bfa2d724c5554a468`.
No Go test execution is inferred for that changed runner; the 200/80 engine
results above retain the exact earlier executed runner and unchanged Go inputs.

Initial exact-head CI at `d175b8a0da5c9ddde2c8c59c6d69211c980e1da6`
[FAILED](https://github.com/k-kostin/omavless/actions/runs/37098664387/job/111133686765)
in the unchanged Rust `concurrent_initializers_never_replace_or_publish_partial_success`
test at `fresh_setup_cli.rs:376`: a fixed unsafe-path/permissions error was
returned instead of the expected migration-lock-contention error (that test
binary: nine passed, one failed). No Rust/Cargo diff exists against #558.
The cause is not established; no runtime fix, timing relaxation or unchanged-head
rerun was performed. Later CI applies independently to its exact head.
Native package jobs do not apply to this test-only diff under the existing
workflow path filters; no architecture/package PASS is inferred.

An earlier normal 20-repeat run also passed 80 cases before runner provenance
checks were tightened; it is excluded from the final 200/80 totals. Four earlier
development invocations, including two diagnostic re-executions, refused during
fixture preparation. The direct Noise setup initially omitted receive-worker
H1/H2 normalization and active peers. The corrected fixture supplies both;
those failures are not counted PASS or attributed to a protocol defect.

## Remaining gates

No Mihomo-path timer evidence is added. Real elapsed rekey/retry/keepalive,
full retry exhaustion, packet rejection after key expiry, autonomous recovery,
timer shutdown/concurrency and performance remain open. Header/wire ranges,
MTU boundaries/large payloads, IPv6 inner/outer traffic and independent real
servers require their own exact-artifact transport cases. Source timing ranges
are not broad AWG header-range acceptance.

The official peer revision is distinct from Mihomo's embedded engine; these
results do not attest that implementation or the installed core. Full/Routing/
Direct, DNS/provider, normal v4 owner/lifecycle admission, core/flavor refusal
before quiesce, active replacement/deletion compensation, migration/restart/
autoconnect, installed rollback and core/privacy review remain unclosed.
R6, AUTO-1, V0 and the incomplete security scan retain their existing outcomes.
No main/RC merge, release or marketplace publication is authorized here.
