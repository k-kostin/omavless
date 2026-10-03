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
