# Proposed upstream namespace API patch — not adopted

The [patch artifact](patches/nix-readonly-namespace.patch) targets nix upstream
`e35c00891f52468979f92b795de2dc1f3dd58a87`. It is a concrete review proposal,
not a Cargo dependency, vendored implementation, upstream submission or safe
API approval. OmaVLESS's unsafe-code prohibition and Cargo.lock stay unchanged.

It adds fixed borrowed-descriptor `namespace_type` and `namespace_id` functions
under Linux/ioctl, and a Linux socket `NetnsCookie` getter. The library contains
three small unsafe syscall sites, each with fixed operation, initialized output,
borrowed FD lifetime and documented safety reasoning. There is no selectable
opcode, pointer, length, path, socket creation or namespace transition API.
Unknown namespace kinds and zero identifiers refuse. Socket output length must
be exactly eight bytes; unlike nix's generic GetStruct path, unexpected length
returns EINVAL rather than panicking. Ordinary syscall errors are preserved.

The ID request uses nix's target ioctl encoding for Linux UAPI
`_IOR(0xb7, 13, __u64)`. It does not substitute an inode, SO_COOKIE or netlink
namespace-relative ID. Kernel headers/source and both architecture ABIs still
require independent review before adopting a reviewed immutable upstream version.

Five focused upstream-library tests pass on x86_64 Omarchy kernel
7.2.5-3-omarchy, Rust 1.98.1. They cover ordinary-file refusal, distinct network
and PID namespace types, current network namespace ID, socket-cookie matching,
and zero/malformed-length refusal. The tests open only current namespace files
and an ephemeral loopback UDP socket, send no packets and change no namespace
or firewall. Existing upstream aio pin-utils deprecation warnings remain.

```sh
cargo test --lib --features ioctl,socket netns_cookie
cargo test --lib --features ioctl,socket nsfs
```

This is not completed dependency acceptance. Still required: upstream API and
maintainer review, license/security review of the consumed immutable release,
the library's MSRV and ARM64 checks, unsupported-kernel execution (the current
test allows ENOTTY but did not exercise that kernel), closed-FD lifetime checks,
and isolated mismatched socket/namespace fixtures. The test of malformed lengths
exercises the post-syscall validator, not a kernel returning a short result.

Even an accepted wrapper would prove only local descriptor relationships.
Trusted system-manager launch, safe fixed descriptor adoption, structural
prevention of namespace transitions, session poisoning, full rule/object
ownership and conditional effects remain separate product prerequisites.
No Canonical or EffectIdentity token is introduced by this artifact.

## Isolated descriptor-relationship continuation

The `dev/k1-namespace-review-vm` patch continuation adds opt-in ignored tests
inside the same upstream-library proposal, not production OmaVLESS code.
The outer test requires `NIX_NAMESPACE_REVIEW_VM=1`, invokes a fixed test
worker through `unshare --user --map-root-user --net`, and checks the pinned
parent/current namespace identifiers again after that worker exits.
The worker retains namespace A and a socket created in A, transitions only its
own disposable thread to namespace B, and verifies all of these relationships:

- A's retained descriptor and socket cookie stay bound to A, not current B;
- B's namespace ID differs and a newly created socket reports B's cookie;
- comparing the old socket with B gives a genuine mismatch;
- a safe duplicated namespace descriptor retains A after the original File drops.

No datagrams are sent, interfaces/routes/firewalls are configured, parent
namespace transitions occur or service/network settings change. The explicit
worker guard refuses direct ordinary invocation. These tests validate local
descriptor mechanics, never canonical-host authority or switch-and-return
prevention in a production owner.

At exact upstream source `e35c00891f52468979f92b795de2dc1f3dd58a87`, updated
patch SHA-256 is
`520b6f8d4ef72f122c9970c5be436823a2fcf0414faf7f000467e7a63c326d62`.
The five ordinary API tests pass; the two disposable tests remain ignored by
default. The explicitly delegated x86_64 Omarchy Dev VM passes outer+worker
with transferred test-binary SHA-256
`a38df040ab3e3042db7ce2aba2e4c8d1a915d7fe17f127958ea6b42d6327159d`.

```sh
NIX_NAMESPACE_REVIEW_VM=1 /absolute/review-test-binary \
  --ignored --exact sys::nsfs::tests::isolated_namespace_cookie_relationships \
  --test-threads=1
```

MSRV/ARM64, unavailable-kernel execution, upstream/security/license adoption,
canonical manager launch and all installed K1 gates remain unpassed. The
library proposal is still neither submitted nor adopted; workspace dependencies,
unsafe prohibition, production namespace facts and package payload are unchanged.
