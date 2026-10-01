# K1 packaged-core socket-mark probe

This is an opt-in, development-only check of bounded Mihomo socket-mark paths. It
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

### UDP and TCP resolver extension

The harness additionally starts fixed UDP and TCP DNS responders on the same
child-only documentation address. They answer only `mark-probe.invalid` A/AAAA
questions, never recurse, and return the synthetic HTTP server's address for A.
Each fresh core resolves that name for an HTTP proxy request, with hosts/system
hosts disabled, `redir-host`, and exactly one numeric upstream using either
`udp://` or `tcp://`. No resolver name bootstrap or outside network is needed.
Counter predicates now select the configured DNS transport and upstream port,
so the successful HTTP connection cannot substitute for resolver evidence.

For each resolver transport, the candidate phase requires every counted upstream
packet to carry the K1 mark, with nonzero total traffic. The wrong-mark phase
requires zero K1-marked packets and nonzero successful resolver traffic. Both
still require the fixed HTTP token and post-core-stop proxy refusal. Separate
core processes avoid an earlier phase's in-memory DNS cache. The original direct
TCP pair remains in the same run. That checkpoint used a 100-second parent
deadline for all six bounded phases; namespace and command guards were unchanged.

On October 1, 2026 all six phases passed in the same x86_64 KVM development VM,
kernel `7.2.5-3-omarchy`, with `omavless-dns 0.9.5beta1-1` and core hash below.
The extended script SHA-256 is
`5fd2f463a021e3b387720bfe9af5e38cab2a345340a8f1259cd4eb4624fbcb0d`.
This proves only numeric-upstream IPv4 UDP/TCP resolver sockets under the fixed
synthetic configuration. It does not establish direct-DNS blocking, local-stub
behavior, encrypted DNS, hostname bootstrap, proxy endpoint resolution, IPv6,
TUN capture, physical egress or complete production resolver coverage.
The test is based on the upstream [DNS configuration contract](https://wiki.metacubex.one/en/config/dns/);
the measured binary identity, rather than current documentation, owns the result.

### IPv6 socket extension

The next checkpoint adds `2001:db8::2/128` to child loopback with `nodad`,
alongside the existing IPv4 fixture. It creates no route to another namespace,
physical link or internet destination. IPv6 TCP HTTP and UDP/TCP DNS responders
bind only this documentation address. Six additional candidate/wrong-mark phases
test direct IPv6 TCP and both numeric IPv6 DNS upstream transports. No IPv6
sysctl, host interface, default route or firewall is changed. Missing IPv6 support
fails the probe instead of silently skipping these phases.

The direct counters now also select their server port, excluding the fixture
server's replies, and require every counted request packet to be marked in both
families. DNS counters select the IPv6 upstream address, transport and port;
the DNS answer remains a synthetic IPv4 HTTP address, so those phases establish
the resolver socket's family separately from the application destination. The
direct IPv6 phase independently checks an IPv6 application destination. All
twelve phases retain successful-token and stopped-core controls. The bounded
parent deadline is 180 seconds for the expanded matrix.

An initial IPv6 resolver attempt retained `dns.ipv6: false` and failed at the
UDP request; it is not a mark failure or passing resolver result. With both
global and DNS IPv6 enabled in the synthetic IPv6 configuration, all twelve
phases passed on October 1 in the same guest/kernel/package/core identified
above. No production configuration was altered. Exact extended script SHA-256:
`d7bc48a4b779698f5514e2fe950d858ed42f7f537fae9dc8aab1466a0a580d7e`.

This is isolated IPv6 socket-mark evidence. It does not prove routed IPv6,
physical egress, IPv6 confidentiality on core failure, neighbor discovery,
router advertisements, actual provider transports, encrypted DNS, or K1's host
failure matrix. The lack of an external IPv6 network is deliberate and cannot
be used to claim those acceptance rows passed.

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
other outbound transports, general UDP proxying, resolver bootstrap, DNS broker, IPv6, retries,
all provider-controlled proxy options, mark persistence across reload, TUN
auto-routing, physical egress, or that today's active OmaVLESS configuration
contains the candidate mark. The normal `Meta` TUN name also differs from K1's
reserved `omavless0`. K1 must continue refusing production activation until
the runtime config/owned interface and every required core/resolver socket path
are checked under an exact installed candidate, plus the root helper and host
failure matrix. Do not infer readiness from this single positive probe.
