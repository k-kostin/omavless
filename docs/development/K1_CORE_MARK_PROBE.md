# K1 packaged-core socket-mark probe

This is an opt-in, development-only check of one Mihomo socket-mark path. It
does not enable NetGuard, alter an installed OmaVLESS connection, install a
package, create a physical route or establish K1 protection.

## Fixed isolated test

Run **only in the delegated disposable Omarchy Dev VM**, with its packaged
`/usr/lib/omavless-dns/mihomo`, `/usr/bin/unshare`, `/usr/bin/ip`, `/usr/bin/nft`,
`/usr/bin/curl` and Python 3:

```sh
OMAVLESS_K1_CORE_MARK_VM=1 python3 tests/k1-core-mark-vm.py
```

The parent pins its network namespace descriptor. A child maps only the current
user into a fresh user+network namespace, requires loopback as its sole link,
and rechecks the inherited parent descriptor and its own namespace before and
after fixed commands. It brings up only child loopback and assigns
`192.0.2.2/32` to it, verifying that neither IPv4 nor IPv6 has a default
route. That documentation-prefix destination is locally delivered but has the
global-unicast shape needed to exercise Mihomo's mark hook. A fixed synthetic
HTTP server and a TUN-disabled Mihomo process live only in the child. A proxy
request must return a fixed token and must fail after that exact process stops;
this rejects a curl bypass false positive.

The child-only nft table has two counters for that synthetic destination: one
for the K1 candidate mark `0x4f4d4101`, one for all packets. With the candidate
`routing-mark`, the marked counter must advance. With a different existing
probe mark (`524288`), the same request must work but the K1-mark counter must
not advance. Both phases must have actual egress at the output hook. The test
deletes only its own child table, stops its own core, and lets namespace exit
reclaim the address. It captures no private packets or logs and prints only a
fixed pass or bounded stage/reason. Tool output and core stderr are suppressed.
No credentials, provider URL, real endpoint, host firewall or OmaVLESS daemon
state enters the test.

## Exact evidence and source limits

On September 30, 2026 the test passed inside the x86_64 Omarchy Dev VM, kernel
`7.2.5-3-omarchy`, against packaged `omavless-dns 0.9.5beta1-1` core binary
SHA-256 `1da6469cd2d122ddc9073835ba5fcee083509e1c76845fb67b8efacb8f448619`.
The tested script SHA-256 was
`7fb00e385dad4e19b1994e3fd216559e1607401e9570f7f8db13ddb9e9501fa5`.
The package's corresponding-source receipt identifies Mihomo v1.19.31 at
`ab405bad5beeeac8b003bb01f60f134f6df54471`. The transferred script was
removed from the VM after the check. No physical-PC network action occurred.

That exact source's `component/dialer/mark_linux.go` deliberately skips
`SO_MARK` for destinations that are not global-unicast-shaped. A first test
against `127.0.0.2` therefore correctly found no mark; it was a fixture error,
not a reason to change K1's policy. The final test uses only the locally owned
documentation address. The source also has paths requiring separate review:
`component/dialer/dialer.go` returns early for a custom `NetDialer` and bypasses
ordinary mark setup when `DefaultSocketHook` is set;
`adapter/outbound/base.go` can supply a per-proxy routing mark; and
`listener/sing_tun/server.go` has an auto-redirect mark mode that refuses a
simultaneous global routing mark. None is covered by this direct TCP control.

This result proves only that the **tested packaged core**, with the synthetic
global-unicast-shaped direct TCP request and canonical mark setting, emits
packets bearing K1's candidate mark. It does not prove VLESS/REALITY/gRPC or
other outbound transports, UDP, resolver bootstrap, DNS broker, IPv6, retries,
all provider-controlled proxy options, mark persistence across reload, TUN
auto-routing, physical egress, or that today's active OmaVLESS configuration
contains the candidate mark. The normal `Meta` TUN name also differs from K1's
reserved `omavless0`. K1 must continue refusing production activation until
the runtime config/owned interface and every required core/resolver socket path
are checked under an exact installed candidate, plus the root helper and host
failure matrix. Do not infer readiness from this single positive probe.
