# External real namespace/creator binder — review-only source

This is an isolated review-only successor to [the external API patches](K1_NAMESPACE_API_VALIDATION.md)
and [inactive acquisition](K1_LAUNCH_ACQUISITION.md), not another authority
interface or a dependency adoption. OmaVLESS Cargo manifests/lock, production
constructors and runtime behavior remain unchanged. K1 remains unavailable.

The source artifact is [tests/k1_namespace_binder](../../tests/k1_namespace_binder/).
Its standalone Cargo workspace must be replayed outside the OmaVLESS tree,
beside the exact patched nix and libc checkouts in `upstream.json`. Its own
lock fixes registry dependencies; path sources additionally require the exact
Git commits and patch digests, not just a version or Cargo.lock. No upstream
submission, installation or package service is authorized by this artifact.

## Actual mechanism, not yet executed on a kernel

`LocalBinding::bind_untrusted_anchor` consumes one original safe File and retains
it before fallible work. It opens the current thread's network namespace,
requires the patched fixed namespace type/ID APIs to agree, creates exactly one
nonblocking CLOEXEC NETLINK_NETFILTER socket, and binds that same socket with
kernel-assigned port and no multicast groups. It queries **that actual held
socket's** NetnsCookie; it never substitutes a second socket or SO_COOKIE.
It opens no caller-selected path and sends no datagram or firewall operation.

`verify_local` rereads the original anchor and original thread descriptor,
opens the fixed current-thread path to detect a later switch, and queries the
retained creator again. The private initial ID fences later changes but is not
exported or serialized. Wrong kinds, zero, unavailable APIs and disagreements
refuse. The owner has no descriptor/creator extraction, replacement or public
callback API, and is not Send/Sync. All original resources remain held on
error, unwind and ordinary drop; only a successful temporary current-thread
read handle is closed. Retention ends at process exit, not crash persistence.

Every syscall has before/after checks under the original two-second lifetime
budget. Expiry or error permanently seals verification. This bounds continued
work after a returned call, not a hard cancellation of a stalled kernel call.
No timeout refresh, retry, setns repair or automatic cleanup exists.

## Still deliberately untrusted

A caller can supply a matching namespace object from a nested context.
Successful local binding is therefore **not canonical host origin**, a provider,
creator/table ownership, complete nft inventory, packet enforcement or launch
acceptance. !Send and before/after sampling do not forbid same-thread
switch-and-return; a trusted launch and complete owning thread code must prevent
it structurally. There is no operation callback, nft exchange or AuthoritySession
adapter yet. Future integration must use this same socket through actual fixed
exchange code; exporting an ID and constructing another creator is prohibited.

The next source audit follows actual systemd descriptor opening/transfer before
sandbox setup, with immutable v261 and current HEAD recorded in upstream.json.
The existing [OpenFile experiment](K1_OPENFILE_DESCRIPTOR_FIXTURE.md) already
proved its narrow descriptor-match fixture; it is not repeated or promoted.
LISTEN_PID, descriptor names, root UID, PID 1 and sampled configuration cannot
authenticate manager origin. Trusted installed package/effective-unit ownership
and the exclusive manager launch path are separate external trust obligations.
That audit and a fresh fixed noninstalled handoff prototype remain pending.

## First checks and next gates

Nine native controls cover pure kind/ID/cookie mismatch, full-width IDs, and
the real private seal/lifetime-budget path. The latter use ordinary `/dev/null`
Files only; no namespace syscall, netlink socket, service or network operation
is executed. The private fixed-leaf query seam also drives the actual constructor
and verification path through 45 constructor and 24 verification error, panic
and late-return variants. They check no next call, permanent refusal and
retention of the exact synthetic original descriptors through this process's
own FD metadata. They do not read other processes or namespace contents.
The real normal entry always uses the fixed Real implementation; there is no
public injected backend. Three compile-fail doctests reject Send, Sync and creator field
replacement. Inert artifact guards check external pins and product exclusion.
Compilation uses the actual patched external libraries, not reimplemented stubs.

The first `67b4c6d` constructor continued to query the anchor after an initial
current-namespace opener failure before noticing the missing descriptor. The
follow-up refuses immediately after the opener's post-return budget gate;
the executed first-leaf failure control requires zero subsequent queries.
Earlier successful tests are not retrospective evidence for that correction.

MSRV/cross-compilation, full source checks and independent graph review remain
required. No real binder invocation or VM gate has run. Any future executable,
freeze and VM launch requires its own exact full review and ROOT-only operation.
