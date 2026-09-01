# Try Omarchy R2–R4 Rust candidate acceptance — 2026-09-01

This report records local acceptance of the stacked Rust migration candidate on
`codex/rust-mihomo-foundation`. It does not claim a production runtime cutover:
Python remains the installed plugin owner until R5/T1.

## Candidate chain

- base `origin/main`: `8dda785c1b2239e56db3cc2a4d11156844729564`;
- R2 non-VLESS adapters: `4005d54741257c269d76bee668c0b654e24b21d6`;
- R3 routing foundation: `516df520e70c8d8142572e2268ae2d870d4c4124`;
- R3 store migration foundation: `0507b81`;
- R4 Mihomo foundation after replay: `03ee7f5`;
- R3 completion batch: `d98651b`;
- R4 diagnostics/observation batches: `24ad90c`, `d5852d2`.

## R3 boundary

The candidate owns language-neutral, credential-safe semantics for store
relationships and migration defaults; active/last/favorite/startup state;
subscription refresh planning and identity preservation; strict unified import
classification; routing rules and modes; deterministic profile/template/private
controller assembly; and bounded, symlink-refusing, same-user atomic `0600`
store replacement.

Remote subscription fetching remains with the Python compatibility runtime as
allowed by R3. The Rust plan is fully validated before its caller performs one
atomic commit; it does not partially mutate a live store.

## R4 boundary

The candidate owns bounded primitives for Mihomo discovery, fixed-argv config
validation, private Unix-controller HTTP/JSON, diagnostic redaction and
rule/provider projections, traffic samples, deterministic isolated-probe
scheduling and response merging, process-family/TUN/user-service observation,
Arch/NixOS classification and host readiness.

The installed-core gate used Mihomo Meta v1.19.30 on Linux arm64. A private
temporary `0700` directory and `0600` config were validated, an isolated
no-TUN core was started, `/version` was read through its Unix controller, and
the child was reaped. No TCP controller was configured. No private profile or
subscription fixture was used.

## Acceptance

- full Rust workspace, including Python/Rust differential corpora: PASS;
- installed-Mihomo Rust opt-in integration: PASS;
- strict workspace Clippy and rustfmt: PASS;
- Python suite with installed-Mihomo opt-in gates: 259/259 PASS;
- QML/i18n contracts: PASS;
- Python compilation and shell syntax: PASS;
- manifest JSON and Omarchy plugin validation: PASS;
- `git diff --check`: PASS;
- private data audit: PASS; only synthetic reserved examples are checked in.

R5/T1 remains the owning change for the daemon, mutation serialization,
desired/actual reconciliation and QML/CLI cutover. These are intentionally not
part of R3 or R4.
