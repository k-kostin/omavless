# Review-only external ancillary patch: exact tested source

This is a textual research fixture, NOT a product dependency, fallback,
vendored production library, upstream submission or protocol authority.
No build script or test runner here applies or executes the patch automatically.
The normal CLI and its strict protected-manager refusal remain unchanged.

The patch exports only six source/harness files from the external experiment's
import base `fc2585fa35855fc4751618aaeeb799e874212b95` to the exact tested
`faa069b43224c8b284954c781b28c1e1b041a873`. Later external documentation or
SCM_PIDFD changes are not included and cannot borrow this kernel result.
It expects the licensed crates.io rustix1.1.5 source under `rustix-1.1.5/`;
the unchanged product lock records registry checksum
`891efababe418670775f199f0d233d84843c227a0949a883ce15b37c78d6629d`.
Original `src/net/send_recv/msg.rs` SHA256 before modification was
`d44b02ee8275f12c32ee67508aaf88d4d34954ebee037bfe7168527c98ebd0bb`.
The existing registry manifest/lock/build script are not altered by this patch.
The separate Linux-only local harness uses cached exact dependencies; it is
not an upstream matrix or an instruction to select an ignored test.

`tested-faa069b.patch`: 38916 bytes, SHA256
`3d125bbe375cb27a2eba4082a337c53e2e64c372e38a2211c1a1b10a74654805`.
All original copyright and three license texts are retained beside it,
byte-identical to the imported source. The patch prominently names its new
review-only modules; the modified msg.rs contains only their additive export.
The local reverse-apply check verifies the exported patch against the six
current exact tested source files without mutating or executing them.

| Resulting file | SHA256 |
| --- | --- |
| harness/Cargo.toml | 0784192515eb7100f882908b25961953e2a553e5c25e7663e0b6d34a5c69e94b |
| harness/Cargo.lock | 511703f55570607bb2549882a37230bdd3c58f94e1fdc68042c05ccaade07a27 |
| harness/build.rs | 7e84abbf757a08f4c276304675e9aae71871837e8c21fedf70cd8bd809fbfbfd |
| rustix-1.1.5/src/net/send_recv/msg.rs | a3e5555bd44f0f9c8724f0d8da848437af83da58d41016b36f74f2b355dc69f4 |
| rustix-1.1.5/src/net/send_recv/owned_ancillary_experiment.rs | 908d2e9d6d7549c1a1f7e28fca14f1a805fa6d8d373fff274c0fa4f796d2726a |
| rustix-1.1.5/src/net/send_recv/owned_ancillary_kernel_control.rs | 1d4e47e28074980eeb917c7fefd64da4801597b0bcd991286c7938ef7c1f5fd0 |

## Sole actual HOST result

After FULL ROOT and independent source/recipe review, ROOT alone selected one
linux_raw test, offline/locked, one test thread:
`net::send_recv::msg::owned_ancillary_experiment::kernel_control::fresh_no_child_known_abi_receive_once`.
Author did not invoke it. Authoritative terminal: KNOWN_ZERO `c8b96a`.
Stdout records exactly one passed, zero failed, 47 filtered out. No second
backend, retry, child, VM, root parent or product binary executed.

Corrected immutable recipe SHA256:
`70e155150a965877fe524a3b29d97dde0d80b551a9aade457e52a01d3261411c`;
its review SHA256:
`8b586e80a5a75352a7a293d07c344c70522df69dc7b382eabf1db15aada0ddf6`.
Actual inherited HOME was `/home/kk`, never assigned/exported by the recipe;
TMPDIR/CARGO_HOME/RUSTUP_HOME/CARGO_TARGET_DIR were explicit fixed paths.
The earlier rejected HOME-assigning proposal remained unexecuted.

Original private captures are preserved under
`/home/kk/.cache/ovtmp-root/t4-rustix-kernel-first-v2.AbvZJuwU/`, both0600
single-link uid/gid1000. No raw stderr is exported or read for this report.

| Capture | Bytes | SHA256 |
| --- | ---: | --- |
| first.stdout | 228 | 4c90603aea0ac387aa66807a666aeb2c99484296a67e24c32e0c1893e281b74c |
| first.stderr | 576003 | abd8b86f80cd289c05e9eb331158b3bfcb3051948a5f1b21dbbb084b356ff5ad |

Only three newly owned no-child known-ABI pairs were tested: exact credentials
and SCM_RIGHTS, kernel TRUNC/CTRUNC/CLOEXEC and original-pipe aliases under one
eight-second sampled continuation budget. This is not hard syscall cancellation.
The twelve synthetic private ABI/ownership controls had separately passed on
both backends before selection; libc's actual receive remains untested.
Earlier NONPASS compile/harness attempts remain retained, not converted to PASS.

SCM_PIDFD ownership was NOT implemented or tested at this head. Unknown
descriptor-bearing ABI, corrupt header tails, OOM/panic and backend partial
errors remain gaps. Ordinary frame Drop closes known rights: a protocol must
retain the whole returned frame before semantic/late refusal. No all-partial-FD
retention, root/ancestor authentication, closed protocol, canonical namespace,
product availability, security-scan approval, merge or adoption is claimed.
Any next API/kernel control requires its own exact-head review and authority.

## Separate PIDFD successor and sole actual HOST result

`source-only-dedb209.patch` preserves nine external source/harness files from
the same import base to `dedb209c3b5b5df4b62703bc3766dcaea1f4e152`.
It is 64205 bytes, SHA256
`9a34e4bfafdc0192a61cae33c3001e03e6ff216ae056801d25bc905d02dd3c7a`.
The tested `faa069b` patch and its result remain unchanged. No runner applies
either patch; normal dependencies, CLI and production sources are unchanged.

The additive frame now separately owns exact successful SCM_PIDFD originals;
exact negative Linux errno becomes a typed record owning no descriptor.
Malformed lengths/extreme negatives remain visible without adopting a guessed
FD. This follows the separately pinned Linux v6.17 producer at
`e5f0a698b34ed76002dc5cff3804a61c80233a7a`, not current HOST kernel identity.
Six added synthetic controls passed with the original twelve on both backends;
they use exclusive `/dev/null` fixtures, not actual PIDFD/class authority.

The new x86_64 Linux no-child control remains default ignored. Its source-only
checkpoint was subsequently selected once by ROOT as documented below.
Three newly owned nonblocking unnamed pairs check PIDFD-only, credential +
one original pipe RIGHT + PIDFD, and credential-sized control truncation.
Both cfg(test) private backend helpers query the original socket with exact
returned native-int size/value for SO_PASSPIDFD76. The control borrows the
returned original only for CLOEXEC and one zero-time poll; no PIDFD read,
pidfd_open/getfd, proc/path/namespace acquisition, process identity claim,
signal/reap, child or VM action. Whole frames/pairs/pipe enter pre-reserved
ManuallyDrop retention before post-call gates under one sampled eight-second
deadline. No success/failure cleanup or retry is added.

Final compiled/inert gates on the exact successor passed eighteen tests,
zero failed, two actual kernel controls ignored on each backend (35/37 other
tests filtered). These runs do not select either ignored body and cannot
inherit the prior `faa069b` actual result. After FULL ROOT and independent exact
source/recipe review, ROOT selected only the new linux_raw ignored entry once:
`net::send_recv::msg::owned_ancillary_experiment::pidfd_kernel_control::fresh_no_child_pidfd_receive_once`.
Author did not invoke it. The exact clean external head was `dedb209` above;
authoritative selection `77b12e` ended KNOWN_ZERO. Safe post-zero capture checks
confirmed one passed, zero failed, zero ignored, 54 filtered out. No retry,
second backend, child, VM, network configuration or product binary executed.

Fresh fixed recipe SHA256:
`9bc828f658eea6be82bf335ef782af5be9e864a847802533d2de8938f36881d2`;
review SHA256:
`0eda5b911aa7b3f47080d2b0682ba83c22b4792864435697d264bf3f810b626b`.
Original private captures remain under
`/home/kk/.cache/ovtmp-root/t4-pidfd-kernel-first.vntjfV7H/`, both0600
single-link uid/gid1000. Only fixed stdout summary and capture metadata/hashes
were read for this report; raw stderr remains private and unread.

| Capture | Bytes | SHA256 |
| --- | ---: | --- |
| first.stdout | 230 | c4f30f3d887430f00ed623f84b00d124d6358fafa383711a263b0349f69e6bca |
| first.stderr | 576003 | 5806df863605ea02beaf1d1e82fd6ae6b3b230823490a13ee9d7c63e40758b0b |

This establishes only the three own-pair outcomes encoded in that exact test:
PIDFD-only, combined credential/one pipe RIGHT/PIDFD, and actual CTRUNC without
PIDFD at credential-sized capacity, with original-option queries, CLOEXEC,
zero-time poll and whole-frame retention. The sampled deadline is not hard
syscall cancellation. Poll is not descriptor-class, PID/liveness, authentication
or manager proof. Libc actual receive remains untested. The patch keeps its
historical `source-only-dedb209.patch` name and exact bytes; its earlier
source-only checkpoint and the original `faa069b` result are not rewritten.
All unknown-FD/corrupt-tail/OOM/panic/backend-partial retention
and root/manager/product authority gaps remain unchanged.

The export/docs checkpoint passed the full source suite (639 tests, two
declared skips, JS/QML/navigation) and reverse-apply/whitespace checks.
Ordinary product Rust source and dependency inputs did not change; no new
product Rust or host result is inferred from this textual research fixture.

The reverse-apply check validates this exported patch against all nine exact
result files without mutation or execution. Five unchanged outputs retain
their earlier hashes; changed/new outputs are:

| Resulting file | SHA256 |
| --- | --- |
| rustix-1.1.5/src/net/send_recv/owned_ancillary_experiment.rs | 7b06063f7c63f84e009ea00e1c74be7782abe98af7415ff48ed6a82baf11847b |
| rustix-1.1.5/src/net/send_recv/owned_ancillary_pidfd_kernel_control.rs | 6157cdfa60efac0e45e4c2c627ae8179fd45e0201b59e2605b883e2e976f5ece |
| rustix-1.1.5/src/backend/linux_raw/net/sockopt.rs | d7f05676ef512eb60998580a6ff9aafb233a8c80fe19a6db9c4da06ac102679e |
| rustix-1.1.5/src/backend/libc/net/sockopt.rs | 1d63f272119490ff83ff170d04a2ac78f27e56a959257276e40cf537794f6bba |
