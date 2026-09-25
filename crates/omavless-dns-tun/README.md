# Uninstalled DNS TUN admission leaf

`HeldTun::admit(OwnedFd)` consumes a received descriptor and validates the fixed
`Meta` device used by all three tracked routing templates. The public API has no
caller-selected interface, path, namespace or ioctl. It rejects non-TUN character
devices before driver calls, TAP, multiqueue, detached and persistent devices,
wrong names and foreign network namespaces. It retains the original descriptor,
namespace descriptor and query socket; read-side `recheck` detects changed facts
before later caller-owned effects. Positive indices fit resolved's signed type.

This crate is not connected to production, packaged, privileged, a broker, a DNS
writer, a route owner or a complete lease protocol. In particular it does not
establish fresh/pristine resolved state, peer authentication, replay ownership or
an atomic resolved transaction. A held descriptor prevents ordinary last-close
removal/reuse, not interference by root or already-privileged link managers.
TUNGETIFF aliases NOFILTER and NO_PI, so packet framing is deliberately not
claimed from that bit. The core owns packet-format conformance separately.

## Explicit unsafe leaf exception — requires owning review

Locked rustix 1.1.5 supplies safe descriptor metadata, fd flags, socket creation
and network-device name lookup, but no safe wrappers for TUNGETIFF or
TUNGETDEVNETNS. Its generic ioctl API itself requires unsafe implementation. A
small private `sys.rs` therefore uses exactly two fixed Linux ioctls:

- native 64-bit TUNGETIFF receives a zero-initialized, 8-byte-aligned 40-byte
  ifreq; only name and flags are decoded;
- TUNGETDEVNETNS returns a fresh namespace FD, immediately adopted into
  `OwnedFd` and made close-on-exec. Every refusal/error drops it.

Both functions independently gate the actual held FD's character-device type
and major/minor `(10, 200)` before ioctl. No generic opcode/pointer function is
exported. Support is limited to native Linux aarch64 and x86_64; other targets
refuse admission. Device input is owned, preventing safe Rust caller closure or
descriptor-number reuse during the call.

Linux TUNGETDEVNETNS itself requires CAP_NET_ADMIN in the device's owning user
namespace. The isolated probes possess that namespaced capability. A future
root service with an empty CapabilityBoundingSet will refuse admission; it
cannot claim compatibility based on these tests. Provisioning must explicitly
review this inspection privilege (or a different proven namespace mechanism).
This requirement does not add a route/link mutation API or require moving route
ownership out of Mihomo.

The workspace's `unsafe_code = forbid` remains unchanged for every existing
crate. This leaf explicitly uses `deny`, with function-local `allow` on these
two typed wrappers only; the rest of the leaf remains denied. The Clippy rules
match the workspace. This exception is visible here and in its manifest, not
a silent weakening of global lint policy. No new dependency version is added.

The helper must be single-purpose: using this crate does not grant permission
to run arbitrary privileged commands or expose its descriptors over public IPC.
`interface_index()` is internal integration data, not a shareable diagnostic.

## Tests

Ordinary unit tests use socket descriptors and an injected kernel boundary;
the only real production-admission inputs are an ordinary source file/socket,
which are rejected before any device access. No normal test opens `/dev/net/tun`.

The explicit conformance example is not installed and cannot configure a link.
The driver below starts as an ordinary user and creates new user/network/PID
namespaces before accessing a TUN. It tests the actual Rust leaf with synthetic
devices, foreign namespace input and bounded fixed-boolean results:

```sh
cargo build --offline -p omavless-dns-tun --example dns-tun-conformance
python3 crates/omavless-dns-tun/tests/namespace_probe.py /absolute/target/debug/examples/dns-tun-conformance
```

No sudo, routes, DNS, bus, private store, provider or installed runtime is used.
Unsupported kernel operations fail the experiment rather than count as expected
refusals. Actual host acceptance and both-architecture kernel evidence remain
distinct from mocked tests and cross-architecture compilation.
