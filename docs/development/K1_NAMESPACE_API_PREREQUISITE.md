# K1 namespace and socket API prerequisite

Status: bounded source/API research, 2026-10-02, following the
[live Emergency creator experiment](K1_LIVE_EMERGENCY_OWNER.md). This records
the concrete dependency and launch requirements for a Rust authenticator.
It adds no implementation, dependency, authority token, privileged operation
or installed service. K1 remains unavailable.

## Exact missing boundary

The current crate uses `nix 0.30.1`; workspace `Cargo.lock` also contains
`rustix 1.1.5`. Inspection of their installed source finds no ready-made safe,
typed wrapper for `NS_GET_NSTYPE`, `NS_GET_ID` or `SO_NETNS_COOKIE`.
The current upstream sources were checked at
[nix e35c008](https://github.com/nix-rust/nix/tree/e35c00891f52468979f92b795de2dc1f3dd58a87)
and [rustix 287214b](https://github.com/bytecodealliance/rustix/tree/287214b889865d8e1406a0ee71cc409b6f6191c8),
with the same relevant gap. This is a claim about these inspected dependencies,
not proof that no safe implementation exists anywhere.

`rustix::net::sockopt::socket_cookie` queries **SO_COOKIE**, the socket cookie;
it is not the namespace cookie. The generic `rustix::ioctl::ioctl` call is
explicitly unsafe, while nix ioctl macros produce unsafe functions. Constants
in libc/Linux UAPI and custom syscall-binding macros are not an already
reviewed typed namespace API. The workspace's `unsafe_code = "forbid"` remains
unchanged. Hiding a new binding in a local helper crate or executing the
developer Python fixture from production would not satisfy this prerequisite.

The exported nix `sockopt_impl!` macro with `GetStruct<u64>` is a possible
custom `SO_NETNS_COOKIE` binding to review, not an absent macro capability.
Its expansion performs unsafe libc access and `GetStruct::assume_init` asserts
the returned length before reading the value. A successful syscall with an
unexpected length therefore panics rather than returning the required bounded
refusal. A local macro invocation would still need explicit binding and failure
semantics review; it does not provide the two missing namespace ioctl wrappers.
It is not adopted here as a shortcut around the safety contract.

## Minimum reviewed dependency API

The following are proposed signatures, **not available APIs or implementation**:

```rust
fn namespace_type(fd: BorrowedFd<'_>) -> io::Result<NamespaceType>;
fn namespace_id(fd: BorrowedFd<'_>) -> io::Result<u64>;
fn socket_namespace_cookie(fd: BorrowedFd<'_>) -> io::Result<u64>;
```

The dependency must safely contain the syscall boundary: borrow the actual
descriptor for the whole call; expose no caller-selected opcode, pointer,
buffer or option; use the native ioctl ABI for both supported architectures;
validate the exact eight-byte socket result; return syscall errors without
substituting inode, socket cookie, zero or a parsed procfs string. Namespace
type must distinguish `CLONE_NEWNET`; unknown types remain unsupported. Test
all three functions on ordinary files, wrong namespace kinds, unsupported
kernels and valid network namespace/socket descriptors.

The narrow next dependency work is adding these fixed read-only wrappers to
an audited syscall library and consuming a reviewed immutable version. This
document does not authorize publishing an upstream PR or changing dependencies.
No syscall wrapper alone authenticates the canonical host or table ownership.

The kernel source basis is explicit: [nsfs ioctl handling](https://github.com/torvalds/linux/blob/v6.18/fs/nsfs.c)
returns namespace type and `ns_id`; [socket option handling](https://github.com/torvalds/linux/blob/v6.18/net/core/sock.c)
returns `sock_net(sk)->net_cookie` with an eight-byte result. Network namespace
initialization assigns the cookie from the namespace tree ID in
[net_namespace.c](https://github.com/torvalds/linux/blob/v6.18/net/core/net_namespace.c).
These are version-pinned mechanism references, not installed-kernel acceptance.

## Canonical host is a separate launch obligation

The existing observer checks `PROC_SUPER_MAGIC` on its pinned proc directory
and `NSFS_MAGIC` on the namespace descriptor. Those are filesystem kinds, not
host provenance. A substituted bind mount of a real namespace remains nsfs;
a procfs mounted for another PID namespace remains procfs. Reopening matching
device/inode labels merely confirms agreement within the supplied view.

`/proc/1/ns/net` is not an unconditional host anchor. PID 1 may be a nested
namespace's init, and a pidfd pins the selected task without proving it is the
host service manager. Root UID inside a user namespace, `INVOCATION_ID`, a
caller-supplied descriptor or matching self/PID-1 namespace IDs likewise do
not establish trusted launch. Do not promote any of these alone into
`NamespaceObservation::Canonical`.

The intended production root service needs a reviewed system-manager launch
and package/unit boundary establishing the canonical namespace reference
before accepting clients. That review must cover its proc/mount view and any
`PrivateNetwork`, `NetworkNamespacePath`, `JoinsNamespaceOf`, container or
user-namespace configuration. The service must retain the actual trusted
namespace descriptor, not reconstruct its authority from serialized IDs.
The current session/listener candidates do not establish this launch boundary.
Support for externally invoked, inherited or sandboxed contexts must refuse
unless their anchor is independently established; this document introduces
no generic namespace-FD input or new privileged broker.

## Internal authenticator after both prerequisites

1. Obtain the trusted launch anchor and pin it. Require kernel-reported network
   namespace type and nonzero namespace ID. Create and exclusively retain the
   fixed NETLINK_NETFILTER socket inside that authenticated context.
2. Require the socket's namespace cookie to match the anchor and the current
   thread's verified namespace ID. Retain the FDs, not only their numbers.
   No arbitrary socket, path, namespace or cookie is accepted through IPC.
3. Check before and after each bounded exchange. Any unavailable primitive,
   changed identity, closed socket, unexpected protocol or observation error
   permanently refuses that instance. No retry, setns repair, reopen, or
   fallback can revive its authority.
4. Expose an internal borrowed session guard only; avoid a public `Copy`,
   `Deserialize` or integer-field constructor for authenticated facts. Do not
   manufacture `EffectIdentity` or table ownership from this session alone.

Repeated before/after checks cannot detect a thread's switch-and-return by
themselves. The production design must also prohibit namespace transitions on
the session-owning thread and control socket/FD lifetime. Pinned netns IDs do
not prove nft subsystem continuity or persistent application ownership. The
live exclusive creator, complete policy verification and conditional-effects
gates remain separate even after successful namespace authentication.

## Validation needed when implementing

| Case | Required outcome |
| --- | --- |
| regular file, wrong namespace kind, substituted bind mount | no canonical authority |
| nested PID/user namespace with its own PID 1 | not authenticated by PID number or root UID |
| socket created in namespace B, anchor in A | refusal before nft exchange |
| current thread changes namespace after socket creation | permanently refused session |
| switch-and-return | structurally prevented; sampled checks alone are insufficient |
| unavailable ioctl/socket option, malformed length or zero result | unavailable, never fallback |
| closed/replaced FD or caller-supplied matching numbers | cannot reconstruct an authenticated session |
| valid isolated fixture anchor/socket | local binding only, never canonical-host PASS |
| installed system service with reviewed launch | separate exact-head host gate |

This research performed no new VM test, namespace transition, socket operation,
host firewall/VPN change or package installation. #487's prior isolated result
remains evidence for its exact source and test mechanism; it cannot be relabelled
as acceptance of the proposed Rust authenticator.
