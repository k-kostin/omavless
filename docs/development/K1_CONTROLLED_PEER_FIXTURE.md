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

## Six separately selected VM cases — all pending

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

## BPF observer design, not implementation or permission

Do not launch the file-capability core under ptrace: it may change exec privilege
semantics. ROOT reported kernel BTF available and subsequently installed
bpftrace/BCC in the VM; no observer attach has been claimed. ROOT owns the
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
