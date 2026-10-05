# K1 external namespace API validation — review only

Historical external validation follows. The later owner-authorized
[service core](K1_SERVICE_CORE.md) introduces narrowly scoped private immutable
fork adoption; the earlier unchanged-main-Cargo restriction describes this
external-validation stage, not that later explicit authorization. No upstream
publication, installed-kernel or canonical-launch acceptance transfers.

This follow-up implements and tests the external syscall-library candidate;
it does **not** adopt a dependency or enable OmaVLESS K1. The original
[namespace patch](K1_NAMESPACE_PATCH_REVIEW.md), #546 evidence and accepted
#643 retained-lease fixture remain unchanged and do not transfer acceptance.

## Exact source and interface

On 2026-10-05, read-only upstream Git metadata resolved nix HEAD/master to
[`e35c00891f52468979f92b795de2dc1f3dd58a87`](https://github.com/nix-rust/nix/tree/e35c00891f52468979f92b795de2dc1f3dd58a87).
It is still the original patch base, version 0.31.3, declared Rust minimum 1.69.
The source contains neither these namespace query functions nor `NetnsCookie`.
This is a checked immutable source observation, not a claim about every future
release or alternative crate.

The [v2 patch](patches/nix-readonly-namespace-v2.patch), SHA256
`e6229d02e60e20a089a0340f6028f02a436308f5ff031a223c685e7ef31c5c15`,
applies to that exact nix source and changes only three files. It exposes fixed
read-only `namespace_type(BorrowedFd)`, `namespace_id(BorrowedFd)` and
`NetnsCookie: GetSockOpt`. Only nix contains the three fixed FFI call sites.
There is no local unsafe helper, generic ioctl request, caller output pointer,
setter, namespace transition, process launcher or authority constructor.

Private `FnOnce` seams exercise the same initialized output and validation path
used by the real wrappers. Errors retain errno without retry, even if an error
also wrote output. Unknown namespace kinds, unexpected success return codes,
zero IDs/cookies and short/oversized cookie lengths refuse. Socket output is
an initialized aligned `u64` with exactly its capacity supplied to the kernel;
the descriptor is borrowed throughout. `SO_NETNS_COOKIE` is not `SO_COOKIE`.

The [compile-only consumer](patches/namespace-api-compile.rs) forbids unsafe code,
typechecks the real three APIs, checks x86_64/AArch64 Linux generic ioctl encoding
and `u64` size/alignment at compile time, and includes two owner-drop rejection
doctests. Successful compilation is not kernel execution or canonical origin.

## Measured checks

All tools and build outputs used a fresh exclusive private HOME cache, outside
the repository and outside earlier Cargo/frozen evidence. No VM or existing
namespace/socket test was executed in this follow-up.

| Check | Exact scope | Result |
| --- | --- | --- |
| Rust 1.98.1 nix unit tests | four new inert result/seam/ABI controls | PASS |
| Rust 1.98.1 rustdoc | namespace-type owner-drop compile-fail | PASS |
| Rust 1.69.0 library + consumer | x86_64-unknown-linux-gnu | PASS |
| Rust 1.69.0 library + consumer | aarch64-unknown-linux-gnu, cross-check only | PASS |
| Rust 1.69.0 rustdoc | namespace-ID and cookie owner-drop compile-fail | 2 PASS |

The official Rust 1.69.0 channel manifest SHA256 was
`78c25eb61c3964bac91e6a0ddbf9746c8c908e84d7d4db2f6a5592b1c4e2c692`.
Downloaded compiler/Cargo/two target std archives matched its individual SHA256
values before private extraction. Compiler identity:
`rustc 1.69.0 (84c898d65 2023-04-16)`, Cargo 1.69.0. No system install or PATH
change occurred. The first two offline old-Cargo attempts failed resolving its
registry cache; vendoring the exact already cached dependencies resolved the
tooling mismatch, without changing their versions or weakening source checks.

The external consumer used nix 0.31.3 path source plus patched libc 0.2.190
and separately named patched libc 1.0.0-alpha.5 to compile-check both public
constants, with bitflags
2.13.2, cfg-if 1.0.5, cfg_aliases 0.2.2, memoffset 0.9.1 and autocfg 1.5.1.
Upstream's MSRV job checks the library, not all its development dependencies.
The new inert unit tests ran on the installed compiler; they are not claimed
as MSRV tests. Existing upstream deprecated pin-utils and old-Cargo check-cfg
warnings remain warnings, not silently suppressed findings. Current libc also
emits legacy-compiler unknown-lint and opaque-FFI warnings; this is not claimed
as a warning-free upstream gate.

Reproduction in a separate external checkout: apply the v2 patch, run
`cargo test --lib --features ioctl,socket namespace_api` and
`cargo test --doc --features ioctl,socket nsfs::namespace_type`.
For the MSRV consumer, use package name `namespace-api-compile-review`, edition
2021, `rust-version = "1.69"`, the supplied file as `src/lib.rs`, and a path
dependency on the patched nix with only `ioctl,socket` features. Add the exact
patched libc 0.2 source as an external-consumer-only `[patch.crates-io]`, and
the patched main source as `libc-main = { package = "libc", path = "..." }`.
The nix unit/doc commands likewise need the external libc path override.
Run locked,
offline `cargo check --target` for both targets and x86_64 `cargo test --doc`.
The main OmaVLESS Cargo graph must remain unchanged.

## Explicit remaining prerequisites

This is not ready for dependency adoption or upstream submission. Nix's
[conventions](https://github.com/nix-rust/nix/blob/e35c00891f52468979f92b795de2dc1f3dd58a87/CONVENTIONS.md)
require missing libc constants to land there first. Fresh libc main
[`b739c733e06a5f184bae7e296919c15d6ccd8ec8`](https://github.com/rust-lang/libc/tree/b739c733e06a5f184bae7e296919c15d6ccd8ec8)
has `NS_GET_NSTYPE` and `SO_NETNS_COOKIE` but no `NS_GET_ID`; its main package is
1.0.0-alpha.5, not nix's 0.2 dependency. Two separate tiny patches implement
that prerequisite for review:

- [libc main](patches/libc-ns-get-id-main.patch), base `b739c733e06a5f184bae7e296919c15d6ccd8ec8`,
  SHA256 `df90aa94dcb199ac45a00b15e2247adeea601c4cc5ae8b32dea43a3ea82ca999`;
- [libc 0.2 backport candidate](patches/libc-ns-get-id-02.patch), base
  `7b0ab5528dc7f361f3a0b4c06c2cad9971617f75`, SHA256
  `da653498d5daa88c459f5a4e7649e9c64e93a9d2b683d3979c5f9ea94cbdd7ab`.

Each adds only `NS_GET_ID = _IOR::<__u64>(NSIO, 13)` beside existing nsfs
constants and the symbol in `libc-test/semver/linux.txt`. The v2 nix wrapper
uses `libc::NS_GET_ID`, not a duplicate request definition. The consumer checks
both public symbols against the target ABI on both targets. The full upstream
libc semver/C-header harness is not claimed run. Main-first review,
maintainer-controlled backport/release and separately authorized immutable
dependency adoption remain prerequisites; nothing was submitted.
The Linux [UAPI declaration](https://github.com/torvalds/linux/blob/a90ee4305c4a5df72c11b31dacfdc76e00fcf78a/include/uapi/linux/nsfs.h)
specifies `_IOR(NSIO, 13, __u64)`. No library dependency was silently changed.

Remaining: independent full patch/ABI review, upstream-compatible constant
provenance and changelog, feature/architecture matrix beyond these two targets,
actual supported/unsupported-kernel behavior and lifetime/mismatch integration
on separately authorized fresh fixtures. ARM64 compilation is not execution.
Mocked ENOTTY/ENOPROTOOPT is not old-kernel acceptance.

Even accepted APIs relate original descriptors only. Neither namespace type,
matching namespace ID/socket cookie, `/proc/1/ns/net`, a copied observation nor
a serialized epoch establishes canonical authority. A trusted system-manager
launch that structurally prevents namespace transitions is still required,
alongside the retained original namespace/creator socket, same-session complete
inventory, generation-conditioned effects and listener/session lifetime guards.
No available canonical provider is manufactured by this patch.
