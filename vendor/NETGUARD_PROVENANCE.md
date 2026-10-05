# Private NetGuard safe API boundary

These complete public upstream source trees are an explicitly opt-in private
fork candidate, not an upstream release, system library replacement, or public
publication. Only `omavless-netguard`'s `netguard-service-core` feature consumes
the renamed `nix-netguard` dependency. The registry dependencies used elsewhere
are unchanged. Application code continues to forbid unsafe code.

| Tree | Exact upstream commit | Upstream tree | Original git archive SHA-256 |
| --- | --- | --- | --- |
| nix-netguard | e35c00891f52468979f92b795de2dc1f3dd58a87 | de4b8ba3fb55e94606843607731691ed225cb2ef | f8b4e20a01e929aa0b99d77488accb58b2dbe5be4543d0f78099489faaac6528 |
| libc-netguard | 7b0ab5528dc7f361f3a0b4c06c2cad9971617f75 | fcb98268a6877bcc176aff683eb4b6310f6e62de | af16158ebb50eca5a2f6e60c126fbc766e48a71c85ea0dd1be854de1c865e1b7 |

The retained upstream licenses are nix's LICENSE and libc's LICENSE-MIT and
LICENSE-APACHE. Full upstream source, build scripts, tests and manifests are
preserved, including files not compiled on Linux. The libraries are excluded
from the application workspace and retain their upstream lint/build policy.
This is a reviewed external unsafe syscall boundary, not permission to add
unsafe application code. Linux x86_64 and AArch64 are the intended ABI scope.

Exact modifications beyond the upstream trees:

- `docs/development/patches/nix-readonly-namespace-v2.patch`, SHA-256
  e6229d02e60e20a089a0340f6028f02a436308f5ff031a223c685e7ef31c5c15.
- `docs/development/patches/libc-ns-get-id-02.patch`, SHA-256
  da653498d5daa88c459f5a4e7649e9c64e93a9d2b683d3979c5f9ea94cbdd7ab.
- `docs/development/patches/nix-inherited-fd-v1.patch`, SHA-256
  9309eae91fdfdbab5670db4f84c3ab9b9cae8d21394b4335858350094e05e293;
  applied after the read-only namespace patch, adds only the private inherited
  descriptor duplication API and its pure error/ownership controls.
- The nix manifest pins its libc dependency to `=0.2.190`, path
  `../libc-netguard`, retaining `extra_traits`. No global crates.io patch is used.

The original query APIs accept borrowed descriptors, create no descriptors, perform one
read-only syscall, and return typed namespace kind, nonzero namespace ID or an
exact eight-byte network namespace cookie. No switching, retry, adoption or
kernel ownership is implemented by those APIs. Unsupported kernels refuse.
Only descriptor borrows, scalar values and typed errors cross the fork boundary;
application code must not mix libc structure types between the dependency copies.

The distinct `duplicate_inherited_cloexec` ingress accepts a raw descriptor
NUMBER, not ownership, and a minimum above that number. It performs one fixed
F_GETFD, one F_SETFD adding CLOEXEC to the original slot, then one
F_DUPFD_CLOEXEC. Only the fresh successful kernel duplicate becomes an OwnedFd;
the source is never wrapped, adopted, closed or replaced. A failure is returned
without retry or flag rollback; setting original CLOEXEC may precede a later
failure. Duplicate means same open-file description, not procfs object reopen.
Invalid/missing/replaced sources cannot authenticate origin; the application
must separately ensure startup slot custody and typed namespace identity.
Scalar-only fcntl arguments introduce no caller-supplied pointer or mixed libc
structure. This new external unsafe boundary needs its own full primary and
independent review; earlier query-only approval does not cover it.

The complete final tree/manifests/lock and service delta require primary and
independent boundary review before any VM installation or activation. Previous
patch-only review is not blanket approval of this adoption. Actual canonical
launch, kernel support, isolation, protection and restart checks remain separate.
