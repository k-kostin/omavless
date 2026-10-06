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
