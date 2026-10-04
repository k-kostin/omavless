# T3 composed DNS-channel wire gate

This bounded development result belongs to code
`6026f085db8b8886c8b669e05558b10a8663655a`, stacked on
[#555](https://github.com/k-kostin/omavless/pull/555),
`8d6877c49d400aa723e01032c62a95d6559fecc3`. It closes an explicit
**synthetic wire** gate; it does not change normal DNS ownership or authorize
main/RC merge, installation, release or marketplace publication.

## Executed inputs

| Input | Exact identity |
| --- | --- |
| Mihomo source | `ab405bad5beeeac8b003bb01f60f134f6df54471` |
| sing-tun source | `b50ae28a1409c7bce8e96e6c6966cf57d8ace754` |
| DNS patches and Rust example source | `c4e800425243c1b02165f82153e4bf418fe465e6` |
| Frozen Rust example | `673b9bcaccd9fe294e3c7541041ba8cafe16ab0a42c8dbf83579e42e76b55332` |
| Pinned golden corpus | `51fef4516f3c56410118965ebe141d977412a463833473e4845458e604e2f8fc` |
| Test-only socket overlay | `d0132ae4758ddc9baadca7f0826e37fbbed9667436e965c9a5eee65121fa3385` |
| Retained second-run Go test ELF | `e8c6484d3a4f8c0fcb9d39df710c57eed9adb11a260a94292608d9e66c3d2ad7` |
| Final composed core, both runs | `3b1da75d3c9fd8440216f9c256c6c59da812faae88debc936f3c72fef9724544` |

The Rust example was built cold in a fresh HOME target from an isolated
object-only export of the exact DNS source, using `cargo build --locked
--offline -p omavless-dns-channel --example channel_fixture`. The executable
was frozen outside Cargo and checked as an x86_64 ELF with no capabilities.
Existing dirty upstream working copies were not consumed as source bytes.

Observed local toolchains: Rust/Cargo 1.98.1 and
`go1.27.0-X:nodwarf5 linux/amd64`. Executable digests:
Go `3144268876ba974458f06ab523581fabac9e967855e2a1bb9de9ba950167b542`,
rustc `c916a85b7d4f52d9650399eebd15c672377933e6c2d8e98215757ab0b9b37136`,
Cargo `7af942ecfec176bbd25434d3305772d26fbdd14a4e7118d87fb44361b022eb4e`.
These identify the local compiler artifacts; they are not independent compiler
provenance, package attestation or supported-toolchain distribution evidence.

## Actual results

Two independent full composition invocations completed successfully:

- Each executed all seven conditional-close cases x20 with race instrumentation
  and all eleven ordinary managed-DNS cases x20. The default interop skip remains
  explicit in that ordinary matrix.
- The separate opt-in executed acquire/release, acquisition refusal,
  recovery-required release and loss-after-Ready x20 each: **80 actual named
  wire subcase results per invocation, 160 in total**. Parent results are not
  added to that count. Every parent iteration requires all four distinct
  subcases, not merely matching global counts.
- Each rebuilt the exact unchanged final core, exercised the existing private
  TCP conditional-close fixture and twenty private SOCKS5 UDP close/reconnect
  fixtures. No external provider, installed core, service or routing was used.
- The second invocation retained the raw JSON wire receipt outside Git:
  69,610 bytes, SHA-256
  `2248328de14904e3e10ac817f0e5ddd9998801cc9c8bee9686a7e8f60e70e9dc`.
  Independent readback confirms each named subcase has exactly twenty `run`
  and twenty `pass` events, no failure/skip, and one package completion.
- Both invocations removed their exact temporary composition directories;
  frozen artifacts and private receipts remain outside Git for review.

CPU gates: twelve artifact/receipt/supervisor guards PASS; source suite
351 reported, two existing opt-in skips, zero failures; frontend/native and
documentation navigation gates PASS; diff whitespace check PASS.
CI remains a separately recorded exact-head gate, not inferred from local PASS.

## Review-driven hardening

Receipt validation rejects missing, skipped, duplicate, foreign and reordered
events, malformed JSON and count-preserving redistribution between iterations.
The socket budget includes all ten possible decimal `os.MkdirTemp` suffix
digits. Checked input descriptor bytes are copied exclusively without inherited
modes/xattrs/capabilities and rechecked after execution.

Go compilation is separate from execution: a small execution-output file limit
must not accidentally constrain compiler/linker objects. The private Go test
ELF is checked before and after execution. The supervisor uses private file
capture, bounded runtime and an unreaped session leader; cancellation signals
the owned process group before leader reap, avoiding PGID-reuse targeting.
A real hanging same-group descendant was killed in the CPU timeout test.
Cleanup assumes the reviewed unprivileged fixtures stay in the inherited group;
it is not a general daemon or installed broker supervisor.

The socket overlay is reversed before final core compilation. The two pinned
production DNS patches, conditional-close patch and final core bytes remain
unchanged; the wire gate is opt-in rather than a silent change to default PASS.

## Still open

The peers exchange Unix credentials and descriptors using an ordinary
regular-file proof and **synthetic** Ready/Released acknowledgements. No real
TUN/device ownership, resolved operation, DNS restoration, installed broker,
matching immutable companion package or canonical host authority was exercised.
Normal T3 operation admission, package/distribution proof on both architectures,
installed healthy-session behavior and owner-attended acceptance remain gates
under the owning roadmap/contracts. This result cannot supply those permissions.
