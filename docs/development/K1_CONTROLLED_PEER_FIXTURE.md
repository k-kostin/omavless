# K1 controlled peer fixture — source candidate

Test-only Python standard-library tooling; not a runtime fallback, package
component, launcher or admission issuer. Based on #679 `1526e075`. Production
coverage issuance remains closed. Neither these tests nor sending a response
proves actual Mihomo TLS verification, SO_MARK, DNS isolation or native K1.

## Fixed topology and scope

The peer listens only at `10.77.0.2:24443`, accepts source `10.77.0.1`, and
implements VLESS version zero, TCP, zero addons, numeric IPv4. Only two inner
destinations exist: HTTP `192.0.2.80:80` and canonical DoH `1.1.1.1:443`.
Both are answered inside the process. There is **no outbound connect, DNS
lookup, forwarding, subprocess or routing/firewall configuration** in the peer.
DoH is a second TLS layer inside VLESS, with ALPN restricted to HTTP/1.1.
It intentionally does not claim HTTP/2 connection-reuse coverage.

The exact canonical generated config remains unchanged: its fixed numeric
DoH URL is handled by the peer, not rewritten to loopback. A separate reviewed
VM setup supplies routed RFC1918 links and prohibits Internet forwarding.
The core's outer numeric endpoint is `10.77.0.2:24443`; this is global-unicast
under Go's address classification, unlike loopback (which skips SO_MARK).

Inputs are fixed private `/run/omavless-k1-peer/{identity.bin,peer.pem,peer.key}`.
The directory is owned by the unprivileged executing user, mode 0700; files
must be single-link, same-owner regular files mode 0400. Identity is exactly
16 fresh random bytes; PEM inputs are at most 16 KiB each. Symlinks are refused.
Files are opened relative to a held directory and certificate/key loading uses
held `/proc/self/fd` paths. This is not protection from concurrent malicious
same-UID writes; fixture custody remains a trusted reviewed VM prerequisite.
ROOT generates ephemeral fixture secrets only in the disposable VM. None are
committed, printed or passed in argv. One leaf certificate can contain both
the chosen outer SNI `peer.k1.invalid` and inner IP SAN `1.1.1.1`. The clone's
controlled system trust must admit the fixture CA: core spawn has only LANG=C,
not SSL_CERT_* overrides. Wrong-certificate cases use separately reviewed
fixture provisioning, never disabled verification.

Only invocation mode is accepted: `success`, `doh-malformed`, `doh-close`,
`redirect-https`, or `redirect-http`. No path, address, port, command, config
or forwarding target can be supplied. Numeric HTTP is GET `/k1` and fixed body.
DoH accepts only a single RD A/IN question for `probe.k1.invalid`, without
EDNS/compression; returns the fixed numeric HTTP address with TTL zero. This
is a deliberately narrow query fixture, not an implementation of general DNS.
The later client must create this exact query; resolver-generated AAAA or
EDNS queries are refusals, not evidence of a Mihomo failure.

Both TLS layers use MemoryBIO, with underlying total wire-byte budgets of
64 KiB read and 64 KiB written per accepted connection, eight seconds total
per connection, three seconds per blocking I/O, sixteen accepted connections
and sixty seconds per process. Header limit is 4096 bytes. No automatic restart.
Only fixed event literals are printed; a SENT event is peer behavior, not a
claim the client accepted it. Idle listener timeout yields STOPPED/exit 1.
Budgets ending at sixteen connections yields BUDGET_END/exit 0, **not PASS**.

## Offline gate

`PYTHONDONTWRITEBYTECODE=1 python -m unittest discover -s tools/k1_peer`

In-memory tests cover fragmentation/truncation, VLESS UUID/version/addons/
command/target refusal, HTTP bounds/framing/duplicate fields, strict DNS wire
shape/base64, fixed failure responses and wire budgets. No sockets, TLS
handshakes, keys, system changes or core execution are used. Real nested TLS
interoperability, CA behavior and installed file custody remain pending.
The MemoryBIO WantRead path feeds one byte at a time to avoid overread; two
handshakes can require roughly twenty thousand iterations within the shared
eight-second connection deadline. Actual interoperability may require a
separately reviewed bounded-chunk optimization. No timing success is claimed.

## Exact production-render helper — ignored, separate action

The private runtime unit-test module `protected_preparation/tests/peer_renderer.rs`
adds one explicitly ignored `render_fixed_peer_config_once` test. It reads the
same fixed 16-byte identity under held input-directory/file checks and calls
`parse_canonical` then the actual `protected_preparation::render`. It does not
recreate policy in Python, expose a public renderer or open the coverage issuer.
No test execution with `--ignored` has been performed in source gates.

ROOT must separately create a fresh empty `/run/omavless-k1-rendered` directory,
uid 1000 and mode 0700. The helper requires real/effective uid 1000 and fixed
mode-0400 identity input. It exclusively creates `generated.json` relative to
the original held directory, mode 0600, writes/syncs, and verifies original
file identity/digest and directory identity. No overwrite, unlink or retry is
provided; a partial publication or failed final check leaves the file and is
refusal/outcome-unknown, not safe-to-retry. The fixed generated controller is
`/run/omavless-k1-rendered/controller.sock`. Only a fixed success literal is
emitted by the helper; libtest adds its normal fixed test-name/result framing.
Private URI/config/UUID bytes are never logged or included in argv.

This is a file-only action, separately selected from peer startup, `-t`, actual
core startup, observation and privileged VM provisioning. ROOT reviews the exact
test-binary SHA before selecting the one fully qualified ignored test. Merely
building or running the nonignored pure renderer test is not file-publication
or installed-core evidence. The helper uses the existing private renderer;
ordinary runtime/API/package sources and admission issuance are unchanged.

## Six separately selected VM cases

Runtime cases remain pending. The separately observed original `-t` subgate in
case 6 passed as recorded below; this does not accept its later lifecycle gate.

1. Numeric success: exact managed core SHA
   `1da6469cd2d122ddc9073835ba5fcee083509e1c76845fb67b8efacb8f448619`,
   exact package receipt/source/filecaps, canonical config, valid outer TLS;
   fixed numeric HTTP response and original core ownership.
2. Resolver success: exact A question, verified inner DoH TLS, fixed answer,
   then HTTP using that result. Observe all egress and TCP/UDP53 sinks.
3. Outer refusal: refused port, wrong certificate/SNI, wrong fresh identity;
   no fallback or direct traffic. Each is a separately selected invocation.
4. Resolver failure/retry: close and malformed modes, cold and repeated queries;
   no direct/system DNS, all fresh outer sockets retain the mark. This fixture
   closes successful HTTP responses; warm HTTP/2 retry coverage needs a later
   explicitly reviewed extension, not an invented PASS here.
5. Redirects: HTTPS to fixed `/refused`, and downgrade to HTTP port 80. The
   former route reaches a refused path; the latter a refused VLESS destination.
   Observe whether the core attempts either; no success claim from refusal.
   Current Go DoH http.Client has no CheckRedirect restriction. Admission
   cannot promise redirect refusal until measured and/or policy is hardened.
6. Original `-t` and lifecycle boundaries: original successful validation reap,
   bounded filesystem trace/no network/no external children, later same-owner
   prepare/Arm/start/stop/Disarm only after independent coverage approval.
   Parse temporarily applies routing-mark defaults even during `-t`; never
   assume potential validation sockets would be harmless or unmarked.

ROOT alone provisions/selects/runs VM phases. No automatic cleanup, compensating
network changes, recovery, retry or software installation is provided here.

## BPF observation boundary

Do not launch the file-capability core under ptrace: it may change exec privilege
semantics. ROOT reported kernel BTF available and subsequently installed
bpftrace/BCC in the VM. ROOT owns the
separately reviewed observer packet. Capture original exec identity, pid/start epoch and
credentials; socket creation, connect destination, SO_MARK setsockopt value and
return, and effective socket `sk_mark` at connect/send. Correlate socket identity
and original process/task lineage, including retries; PID alone is insufficient.
Include IPv4/IPv6 TCP/UDP and explicit event-loss counters. Bounded ring buffer,
fixed event schema, finite duration, no packet payloads, no paths/keys/config
capture. Overflow, unsupported hook, missing attach, lost correlation or observer
exit invalidates evidence, never becomes zero events. Readiness is all hooks
attached before original execution. Do not add marks to fixture/probe processes.
Independent nft counters/capture are useful corroboration but cannot replace
effective per-socket mark attribution. No production observer or permissive
bypass rule is added by this fixture.

## Exact validation and collector-load results — 2026-10-06

ROOT selected an independent, flattened full disk copy of the Omarchy dev VM;
the original VM/disk/NVRAM remained off and unchanged. The installed user
runtime and netguard were inactive, with only loopback and the VM Ethernet
interface. No physical-host VPN, service, network or package was changed.
Synthetic inputs were generated privately, and the exact renderer image from
`b4430ef01e40714ccfc2870654986ba7df5fa4e2` returned original0. This is real
private file publication, not runtime activation. A separately reviewed fixed
ephemeral CA was installed only in this disposable copy for upcoming TLS tests.

The final validator source SHA256 is
`ebba5a704c43f7673f5a1a2fb9c1b990c6d6985f235cbeab585d7a65441e5607`;
its file-only staging source SHA256 is
`7eb91c6e6c250eecfd7761a248eb5092ef751b7b2044a07fcd3293076f056ea2`.
The separately frozen passive observer SHA256 is
`f4f9d506d3befa255dc0f3ec7e97ce0153d12ee7e636ad25c7ba90feeb578240`.
Twenty-one offline controls passed, including real temporary-file staging,
exact embedded bytes and unchanged existing config/data. Original child
execution was held until all-hook, original-PID/credentials-authenticated
notification; no ptrace or extra mark was introduced.

Actual selection `fb097e` returned originalSSH0 and the fixed result
`K1_VALIDATOR_ORIGINAL_ZERO_NO_SOCKET_EVENTS 10`. It positively observed/reaped
the original observer and original core exit0, confirmed all original thread
exits, held/named input postchecks, empty unchanged data directory and zero core
stderr. A separate fixed read `81b8dc` returned original0: one exec, four thread
births, five exits, zero socket/network/mark events, zero observer stderr and
the expected finalizer. The 376-byte private trace SHA256 is
`343118467513d39d4dcdcbe79fece933f1608a9be595c81f93472a47b41107bc`.
These are bounded kernel socket/task and filesystem-inventory facts for this
exact canonical `-t` invocation, not a general filesystem syscall trace or
proof for other configs, builds or runtime paths.

Earlier selections remained NONPASS. Their original input/process graphs were
retained rather than retried/adopted: installed bpftrace adds exactly two final
LF bytes, and distinct thread exits can share a nanosecond timestamp. The
successor accepts only that exact finalizer and independent, already-known,
distinct exit-only ties; unknown events, duplicate threads and all other ties
still refuse. An unselected staging source with descriptor-variable shadowing
was rejected before VM execution; the corrected real-file test covers it.

The runtime collector's exact-broker successor SHA256 is
`b1a123643f5e0cbcfe1a0710751040fc9c0b71f0c5e7afe4c3a2922da47b3540`.
Its separate load verifier SHA256 is
`c4ddbdbfa4a652aad338ad2ad21b0d4d9d567c87a70a4b9c9f8dd463a0f634ae`.
It returned original0 (`f7d2fb`) after actual passive `--dry-run` attachment
and original status/reap on kernel `7.2.5-3-omarchy`, installed bpftrace binary
SHA256 `b38f3edca7ae17f78274c3eb0d569af417da25eb735467423223008de019bfbd`.
Its verbose private stderr is 1,119 bytes, not an empty-diagnostics claim.
This closes only compilation/load of the specified hooks, including the real
DNS broker's fixed UNIX SEQPACKET connect. No runtime core, TLS peer, packet
matrix, coverage issuance, Arm or native lifecycle acceptance follows from it.
All raw captures, fixture identity/keys/config and observation tools remain
private outside Git. Production coverage issuance is still closed.

## Numeric runtime attempt and managed-device prerequisite

The later Numeric10 collector
`a6709b18591f984b717c8c583d9d16aab9e835555188f3809d4db6dbeac82c2b`
and its separate load verifier
`62467e8526f48cb7603211133ec45740e3a7e1b0741f38651ffdccd7feeece49`
passed only the original passive load gate (`19a15a`, original0). The real
Numeric10 runtime selection remained **NONPASS**: it stopped at
`TUN_CONTROLLER_AVAILABILITY`, before the route query and numeric request.
No successful TLS exchange, socket-mark census, original core/peer reaps or
cleanup is claimed for that selection. Its uncertain original graph and private
captures were retained; a later observer is not authority to retry it.

Public-source inspection found a concrete mismatch: the canonical protected
renderer selects `omavless0`, while the installed managed Go DNS adapter and
Rust TUN verifier require `Meta`. Successful `-t` parsing does not exercise the
listener's device-policy check. The development-only source prerequisite in
#684 at `08194a275d315db7ca502e50f960c80af6dc163b` introduces a closed,
build-selected `omavless0` flavor and distinct enrollment policy; legacy `Meta`
selection and consent remain unchanged. Focused Rust and both Go device-flavor
tests passed, but this is **not** a qualified package, installed TUN check or
coverage-issuer acceptance. Its distinct package family and live integration
must be checked before another real runtime selection.

A separately scoped read of Numeric10's bounded private captures also anchored
three unrecognized observer lines to the exact public collector location,
source excerpt and underline. The warning is `Return value discarded.` at
the `delete(@birth, $sk)` statement, not evidence that deletion actually failed.
The frozen successor collector
`f2bab830ee7f786041ed3e9bdf73fce2b71ee0ef3cdc0f2af920797260c416ed`
consumes the deletion result: success emits the original birth's `free` event;
failure emits a bounded, refusing `bad` event. The parser is unchanged and
26 pure controls pass. Its separate load verifier
`e401d5786c0a6fbef3912fe65fb4176cc1b13638bc6cb7c7a28b237176a58053`
then returned originalSSH0 in the disposable VM at boot
`bd49e220-7200-4610-9f6e-d84d59672b71` (`4f4f68`). The original verifier
observed its successful exit and reap; stdout was empty, and its bounded private
stderr was 1,281 bytes with SHA256
`6a5f57a5fe32c595106928f45d84c93ac022cb82e9babc2052668b05b447b0b6`.
The discarded-return warning is gone. This closes only the successor's actual
compilation/passive-load gate: no runtime core, peer, TUN, packet request,
socket-mark census or cleanup followed from it. Numeric10's outcome is not
rewritten as success. The reviewed default/K1 Go device tests and a distinct
four-patch package build remain separate from the next real protected run.

## Four-family Numeric12 selection

The actual four-family package built from
`4d4747af47d60956b370e588469a3b9472ed42a1` was separately installed and
enrolled in the disposable VM; the owning package contract records its exact
pins. At fresh boot `c655aa71-9bc9-4c39-adf8-c2e7a73194ed`, collector11's
separate passive-load selection again returned originalSSH0 (`0e16f8`), with
the same 1,281-byte private diagnostic hash above. Source-only staging
(`861b05`) and additive fixture-network preparation (`91dd9d`) returned
originalSSH0. These are separate selections, not an automatic test chain.

Numeric12 uses the unchanged strict parser, original child ownership and one
numeric request. It progressed through authenticated observer/peer readiness,
the TUN/controller/listener prerequisites and the UID1000 route to `omavless0`.
It then remained **NONPASS** at `ONE_NUMERIC_TRIGGER` (`518e98`); the original
supervisor and uncertain graph were retained, without a resend, timeout kill,
compensating cleanup or success/reap claim. The route capture is 135 bytes,
SHA256 `157bea18df8e14c987ea48ac68345cd9f8955040c2296f8a2d2b9bdced6531b1`;
the trigger's sole refusal marker is 24 bytes,
SHA256 `060e864190e62e12ff13ce626a5a96608e219ae0bd98b648f3f73f06e2f09955`.
The separately bounded capture observation (`5932e0`) saw a positive kernel
listener and fixed DNS-broker SEQPACKET connection, but no accept, clone,
outbound connect, mark or send. Core stderr was empty; no core error/warning
marker was present. Raw logs and private fixture inputs remain outside Git.

Read-only network diagnostics (`4d3b1f`, `3ac48f`) found seven TUN TX and RX
packets without device drops, seven admitted numeric OUTPUT packets, and no
inner-return or managed-outer packets. The preserved UFW IPv4 INPUT chain has
DROP policy. The system TUN implementation rewrites the original SYN into an
incoming `198.18.0.2 -> 198.18.0.1:listener-port` SYN before local acceptance;
the fixture had admitted OUTPUT only. This is a concrete next hypothesis,
not proof that every missing packet is attributed to the core. A separately
reviewed fresh experiment may admit only that observed tuple in the actual
blocking VM chain. A fixture-only firewall admission is not product ownership,
foreign-firewall preservation, coverage issuance or permission to Arm.

## Numeric13 positive exchange, refusing census

At boot `d560d51c-9526-42a6-aedf-e0e122773c51`, a separately reviewed
Numeric13 fixture admitted exactly the observed rewritten SYN tuple at the
front of the blocking VM INPUT chain. Its original before/admitted/after
snapshots fence the remaining foreign chain structure, allowing only counter
advancement. This is an explicit disposable-VM firewall fixture modification,
not production firewall ownership or foreign-rule preservation acceptance.
Staging (`b5d8f4`) and network preparation (`3fbbbd`) returned originalSSH0.

The runtime (`9c82df`) observed the exact numeric HTTP response and the peer's
HTTP-sent marker, followed by normal original core stop and observer finalization.
The trigger is 22 bytes, SHA256
`677090916d138d4290ee6c2760eeaf3ebe0cd9c83f6249dea42704b0fc7eab47`;
the peer marker capture is 32 bytes, SHA256
`ffe467ecd11465e7dfab4bace5bea24c6c72cddffbc0c93d085a840caf58479b`.
The overall selection still remained **NONPASS** at `CENSUS_PARSE`. No packet
postchecks, original reaps or cleanup are claimed. A bounded separate
observation (`030b8e`) found a listener-rooted clone/accept, marked outer TCP
connect and sends, but also five refusing unknown-socket events. Its private
14,391-byte trace has SHA256
`de70e1f4d5947af163cf74e326f883438e94fe320d091418b36d0361b1ff9301`.
Core stderr was empty and its bounded startup projection had no warning/error.

Source review exposed two observation gaps, not permission to relax `bad`
events: `sk_free` may release a write-memory reference before final retirement,
and the TUN's checksum-query path creates one UDP control socket solely for a
fixed ethtool ioctl. The separate successor observes `__sk_free` and requires
that control socket's positive creation, exact GET/device/mark, paired ioctl
completion and final retirement. UDP data/connect/mark still refuse. Its
compile/load and new real runtime gates are pending; it does not retroactively
accept Numeric13 or confer coverage authority.

Available private inputs and captures were archived before an explicit
disposable-VM administrator reboot (`aa5379`, `f2ca15`). The parked original SSH
then ended255 (`62a703`) because of reboot, not successful product recovery or
cleanup. The subsequent boot is
`a58ecb8b-e8fe-42ff-96bf-eec831cfc342`; the native user unit and DNS broker were
inactive with zero manager FD-store count. Saved private application profiles
were not reset, exported or committed. Production coverage issuance remains
closed.

## Control-socket observation correction and Numeric18

The Numeric16 successor reached the controller/listener and route prerequisites,
but refused before spawning its numeric trigger: bpftrace reported `EFAULT`
reading the ioctl's interface name. Source inspection distinguishes a user
pointer from an already copied BPF-stack character array. The Numeric18
collector reads the original fixed `ifreq` address with `str(uptr(args.arg),16)`;
the command/data-pointer checks and strict UDP-control policy remain unchanged.
Collector SHA256 is
`6ec13a5d865911638af7151ff42e50337cebc017ea5f726e46d1641f9e0ffaf3`;
parser SHA256 is
`231b054847f375738ab8e8156935dc6bb66b274a08da1b79af6cb0575e2bb79f`.
Numeric16 is not retroactively accepted.

At fresh VM boot `6850a242-f30f-4ab0-b8a8-1ee1c5638cf4`, the separate
Numeric18 passive-load selection returned originalSSH0, as did public-source
staging and network preparation. Its real runtime reached the exact HTTP
response, peer HTTP-sent marker, normal core stop and observer finalization,
but remained **NONPASS** at `CENSUS_PARSE`. The original supervisor retains its
graph; no original reaps, packet postchecks or cleanup are claimed.

The separately bounded read-only observation found empty core/observer stderr,
zero `bad` events, and one paired successful control ioctl, marked outer TCP
connection and inner accept. Its private 15,268-byte trace SHA256 is
`995607f175c54574a56465b99467f377324bf229ba44b8cd6ce7343dad0ba890`.
The exact pinned pure parser stopped at its unsupported-family assertion
(public source line 96). A separate bounded projection found one classless
family-zero allocation, subsequently retired, with no correlated request,
mark or network event. Its positive allocation provenance must be established;
this is not permission to admit arbitrary family-zero sockets. Neither a
successful request nor zero `bad` events substitutes for
the complete census, DNS case, negative packet matrix or production issuer.
