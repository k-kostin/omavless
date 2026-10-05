# Static child and original no-policy launch proposal

Source proposal only. No build, child main, namespace, netlink, close, spawn,
wait, VM delivery or publication has been selected for this successor.
The original zero-pin source is preserved at
`81f4fce79282660c4f9e291ed91b5145ac182f13`. The two child pins remain zero here.
ROOT owns build/freeze review and is the sole subsequent VM operator. All
stopped unknown/nonzero scopes remain stopped without query, retry or cleanup.

## Compile-only scope for review

The new closed export name is `netguard-static-child-v1` below the existing
0700 private `/home/kk/.cache/k1-launcher.9tzS8PtY` root. Export refuses an
existing destination. The target must be a newly created 0700
`target-static-child-v1`; scratch remains the existing verified 0700 `tmp`.
No old target or export is overwritten. HOME stays inherited `/home/kk`.

The proposed commands, after ROOT reviews the exact committed source and
external dependency graph, are:

```sh
umask 077
python -B -c 'import sys; sys.path.insert(0,"tests/k1_owned_launcher"); import prepare; prepare.export("/home/kk/.cache/k1-launcher.9tzS8PtY", "netguard-static-child-v1")'
mkdir -m 700 /home/kk/.cache/k1-launcher.9tzS8PtY/target-static-child-v1
TMPDIR=/home/kk/.cache/k1-launcher.9tzS8PtY/tmp CARGO_HOME=/home/kk/.cargo CARGO_TARGET_DIR=/home/kk/.cache/k1-launcher.9tzS8PtY/target-static-child-v1 RUSTC=/usr/bin/rustc RUSTFLAGS='-C target-feature=+crt-static -C linker=/usr/bin/cc' /usr/bin/cargo build --offline --locked --release --target x86_64-unknown-linux-gnu --manifest-path /home/kk/.cache/k1-launcher.9tzS8PtY/netguard-static-child-v1/Cargo.toml --bin k1-fixed-child
```

This compiles the child entry only; dependency build scripts/proc macros are
ordinary compilation work, not execution of child main. The explicit target
keeps host procedural macros out of the target CRT-static setting. A compiler
or linker error is a retained build NONPASS, not authorization to run a dynamic
fallback. The final execution recipe must pin the successor commit, reject a
dirty source tree and verify every executable/dependency pin before this command.
No `cargo run`, test wildcard, ignored selection or produced executable invocation
belongs to the build recipe. Capture stdout/stderr separately with create-only
0600 files and retain the authoritative original exit result.

Read-only toolchain observations for preparing that recipe:

| Input | Identity |
| --- | --- |
| rustc | 1.98.1, commit `48a229ceaefd4985c50990b14116b6d856af0985`, x86_64-unknown-linux-gnu |
| `/usr/bin/rustc` SHA256 | `c916a85b7d4f52d9650399eebd15c672377933e6c2d8e98215757ab0b9b37136` |
| `/usr/bin/cargo` SHA256 | `7af942ecfec176bbd25434d3305772d26fbdd14a4e7118d87fb44361b022eb4e` |
| C compiler | GCC 16.2.1 20260810 |
| `/usr/bin/cc` SHA256 | `8aac907d6fbf40394b424b5fa4a2ad1aa291dac7d3a0315c5b415c1f5ace1287` |
| linker | GNU ld 2.47 |
| `/usr/bin/ld` SHA256 | `4d83828f709f0eade25bcae2f4a2508c47db2f01b58daaca5c1c4cefce897847` |
| external Cargo.lock SHA256 | `5d746cd9bb686f18c8cabdc02bf25f9c6361fc08aa41289a8f5d138a046c5229` |
| fixed child source SHA256 | `b7b01f0b5d65386fc80b4665d0769aae13550f8da5055baaeb2663e3e3297f52` |
| fixed protocol source SHA256 | `0022ee804d0826bd509be8630e3517134049ef3b56f7549ec6a49e41f3da450a` |

Static libc/CRT archives exist locally; their complete resolved link inputs,
Rust target libraries, registry sources/checksums and patched nix/libc graph
still need an exact build receipt. These observations are not that receipt.
`spawn-upstream.json` pins the additional POSIX patch and source to
`c8b35ff287ac406a8bd6aff70ad0117952b46d999eef17f47faa96db8514f8fb`;
the six adapter originals remain pinned to e6488ed. No product Cargo change.

## Inspection and freeze boundary

After a known-zero build, inspect the produced file without executing it.
Require an ordinary single-link file, bounded size, ELF64 little-endian x86-64,
at least one PT_LOAD, no PT_INTERP, and no DT_NEEDED. Use the existing exact
static_elf parser as an additional offline check, not a loader emulator.
Read program headers/dynamic entries with readelf; never use ldd (which may run
an ELF loader). Record actual SHA/size and complete build-source provenance.
An unexpected interpreter/dependency is a failed candidate, never waived.

ROOT alone freezes the inspected bytes into a fresh exclusive HOST artifact;
no hard links, overwritten target or reused artifact name. Re-read/hash the
held original, verify one link/no xattrs/full metadata, then remove write access.
The unexecuted review copy is 0400; only a separately reviewed execution artifact
may use 0500. A recipe and receipt must themselves be immutable 0400/single-link
before full ROOT and independent review. Do not infer immutable file identity
from a filename or chmod alone: record original metadata/hash and ensure no
writable descriptor remains. No freeze implementation or freeze result is
claimed by this document.

Only that accepted frozen child receipt can supply a NEW explicit source
checkpoint replacing SHA and SIZE in child_executable.rs. No environment,
argument, mutable manifest or compile-time override supplies those pins.
Keep all zero-pinned commits and previous failed capture/export identities.

## Separate fixed parent gate

The exporter builds `parent_main.rs` from the exact copied library module tree
plus `include!("no_policy_gate.rs")`. The opt-in binary requires the explicit
`owned-launch-no-policy` Cargo feature. No library public acquisition API is
added. The entry admits exactly `--fixed-owned-no-policy`, calls private
Prototype::open_fixed once and then consumes that original through finish once.
It never calls inventory, an effect, installed daemon or product CLI.

Original-call output consists only of OPEN_BEGIN, OPEN_OK, FINISH_BEGIN and
FINISH_OK fixed lines. OPEN_OK means the existing constructor returned after
READY, original image/acquisition checks, checked handoff and second lease.
FINISH_OK follows the actual sealed FINISH/DONE/WNOWAIT-zero/EOF/image/zero-reap
sequence. It is not installed provenance. Missing output or nonzero exit stays
unknown/refused; no subsequent status query can fill a missing phase. Owners
are retained before output. Each phase uses one safe rustix write on the original
borrowed stdout descriptor; only its exact full-length return before the fixed
outer deadline succeeds. No write_all, line buffering, flush, EINTR retry,
short-write continuation, raw-FD adoption, duplication or pathname reopen occurs.
Short/error/late phase results stop without another operation or output.
The runner must supply private regular-file stdout/stderr. One outer five-second
phase budget begins before argument admission/OPEN_BEGIN. Prototype retains its
separate internal five-second acquisition budget, starting within open_fixed;
the outer wrapper gates its call/return, not each internal leaf. Neither clock
is reset for FINISH. A late open result is retained before phase refusal, and
the inner budget cannot authorize continuation after the outer refusal. This
is sampled continuation gating, not syscall cancellation or a claim that every
inner leaf uses the earlier outer deadline.

The future parent build/freeze has its own exact static artifact receipt and
strict compilation gate; child identity cannot stand in for parent identity.
First compile-check this zero-pin source with the feature explicitly enabled,
then run only the existing 19 inert child-bin controls and source suite. No
parent binary may be invoked during those checks, even to demonstrate zero-pin
refusal. A new VM publication/runner proposal is still required, reviewed in
full twice, with ROOT the sole executor.

The prospective guest scope publishes a new root-owned 0555/single-link child
at the exact fixed path, under root-owned 0755 ancestors with no xattrs. It
must establish absence before publication and preserve the frozen HOST original.
The one invocation uses only the no-policy gate; first unknown/nonzero stops
without query, retry, kill/reap or cleanup. A successful local relationship
does not authenticate installed manager/package origin or activate K1.

## Still mandatory before invocation

The 19 controls prove shared sequence/adapter cuts, not every nested metadata,
namespace, procfs or libc-internal acquisition outcome. `type_controls.py` now
prepares a separate `netguard-types-v1` export of the actual complete adapted
module graph. One positive bin checks fixed signatures without executing them;
15 negative bins check Send/Sync/Copy for all three actual owner types, escaped
socket borrows, private callbacks/originals, creator replacement, absent supplied
constructor and no CanonicalCreator conversion. Run each only through explicit
`cargo check --bin type-CASE`, never a binary or broad `--bins`/test selection.
The positive must compile first; each negative must have its specified diagnostic
and no unrelated compile error. These controls are prepared but UNEXECUTED and
cannot be inferred from token checks or the older unadapted harness.
Installed origin and no-switch provenance remain distinct unavailable gates.
