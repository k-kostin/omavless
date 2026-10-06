# K1 protected native preparation candidate

Historical contract for original source `8608857b` (#674). The sections below
describe that immutable preparation-only snapshot, not the composed successor.
Current private consuming seams, owned validation, typed readiness, three held
record descriptors and the still-closed coverage issuer are documented in
[owned native composition](K1_OWNED_NATIVE_COMPOSITION.md). No original acceptance
is retroactively expanded by the successor.

Inactive source successor of `9ed6b286fc94536670905cbee1c82c5a1f02fe85`.
This implements local preparation on the existing `NativeLifecycleHost`, behind
the existing non-default `netguard-runtime-candidate` feature. It does not wire
the [protected lifecycle seam](K1_RUNTIME_PROTECTION_SEAM.md), change its trait,
register a constructor/capability/IPC method, run Mihomo, or contact NetGuard.
The [K1 contract](../roadmap/KILL_SWITCH.md) and actual acceptance remain open.

## Implemented scope

The native-only entry requires the selected managed DNS pair and its package
identity checks. It holds and hashes the selected core file, validates the
complete private store, and selects exactly the requested non-missing canonical
profile. The first policy admits only VLESS TCP with verified TLS, a numeric
IPv4 endpoint, and no flow, packet-encoding, VLESS Encryption, REALITY, XHTTP,
fingerprint, ALPN or other emitted transport extensions. An exact structured
renderer-key allowlist refuses future canonical renderer extensions too.

The generated JSON (Mihomo YAML-compatible input, not yet actual-core validated)
has fixed profile/selector names, Full desired intent with core **Rule** mode,
`omavless0`, socket mark `0x4f4d4101`, managed-DNS flags and auto-route without
auto-redirect. Its exact ordered rules are `NETWORK,UDP,REJECT` then
`MATCH,PROXY`; `tun.disable-icmp-forwarding` is explicitly true. `udp: false`
alone is not a TCP-only enforcement mechanism in the pinned core's Global mode:
VLESS can still enter its packet path. The fixed rule rejects non-hijacked UDP;
port-53 DNS is intercepted earlier and uses the fixed resolver. ICMP forwarding
is disabled, but the system stack can synthesize local echo replies: these are
not evidence of remote reachability. IPv6 remains disabled. No user template, custom rule,
provider/geodata dependency, inherited listener, cached selector, host-file
resolver or caller-selected bypass mark is incorporated. Ordinary templates and
probe mark `524288` are unchanged. JSON string encoding prevents profile/controller
text from becoming additional configuration syntax.

Protected readiness separately requires Rule mode, the explicit ICMP-disable
flag, exactly those two ordered enabled rule rows, no rule providers, and a
single-member PROXY selector currently selecting the fixed VLESS profile.
Ordinary readiness and templates retain their existing behavior. This source
successor does not retroactively change the old Global-mode matrix's scope;
fresh exact-renderer DNS/TCP success, non-DNS UDP rejection and ICMP no-network
observations are required before a separate issuer decision. The issuer remains
`Unsupported`.

DNS selects numeric `https://1.1.1.1/dns-query#PROXY` and a numeric default
resolver. Numeric proxy and DoH endpoints eliminate required hostname bootstrap
in this selected shape. This is a concrete policy for a future controlled
fixture, not evidence that the core never uses the default resolver or that
encrypted resolver sockets are covered. Actual resolution paths and TLS/proxy
behavior must still be measured. No profile-family or IPv6 maturity is promoted.

One separate fixed private `.config.k1.candidate.json` is exclusively created,
synced and re-read. The record belongs to the same native host, has no public
constructor, Debug, Clone or serialization, and binds exact desired fields,
store digest, held core file and held staged file. Store binding is exact
bytes/digest, not a retained store inode. Core/staged-file rechecks require
unchanged named/held device/inode/mode/owner/link count/size/timestamps and hashes.
The reserved TUN must be absent before and after preparation. This does not
prove empty host inventory, kernel policy, core capability or socket coverage.

Publication consumes the host's attempt slot before create/write/sync. Failed
publication leaves that slot unusable; no overwrite, unlink, retry or rollback
is performed. A fresh host also cannot overwrite an existing candidate. The
private file is deliberately retained after host drop; this source has no
recovery/cleanup API. Known synthetic test files are reclaimed only with their
own test directory. The persistent record retains two file handles per host;
this is not a peak descriptor bound. Publication temporarily also holds the
write handle and opens the parent directory for synchronization before capturing
the staged file.

Ordinary `.config.candidate.yaml`, profile/readiness fields, desired state and
active config remain untouched. In particular, calling ordinary `start_prepared`
after this preparation refuses: this candidate is not a second start path.

## Exact future seam and remaining implementation

Proposed ordering inside the existing serialized owner:

1. Existing owner/migration-lock admission and strict empty-host check.
2. Explicit protected preparation of the selected target (not ordinary prepare).
3. Validate the exact prepared file with the admitted managed core; recheck its
   binding and consume real installed transport/resolver/mark evidence.
4. Reserve disconnected generation, then exact Arm, then connected intent.
5. Start the same prepared configuration; check fixed TUN identity, mark/config
   expectations, selectors, managed DNS and attributable protected probe.

`admit_prepared_protection` is the proposed post-prepare/pre-Arm local seam.
Its success type is intentionally uninhabited: unchanged source preparation
always returns Unsupported. There is no boolean or caller-created evidence
that can make it ready. No current K1 preflight behavior changes.

The existing line-oriented `ConfigReadiness` parser does not admit this JSON
spelling; future protected startup must derive typed expectations from this
generated policy and validate actual authenticated controller facts. It must
not silently fall back to ordinary readiness. Likewise the existing lifecycle
seam still calls ordinary preparation; this candidate does not claim composition.
Core execution must retain/revalidate executable selection rather than convert
these local file checks into an unqualified future execution promise.

Future integration must prevent every unsupported mutation/startup path from
using ordinary unprotected lifecycle operations while protection is intended.
No independent probe receives the K1 bypass mark. Installed managed-core
validation, complete admitted TCP/TLS/resolver paths, TUN routing and packet
denial, disconnect, failure/restart and the explicit physical gates remain
required. The fixed-client sibling is independently reviewed and not included
in this source base.

## Source checks

Ten synthetic unit cases cover policy shape and refusals, absent managed pair,
exact input binding, file replacement/mutation/symlink/mode drift, missing
profile, reserved TUN collision, create failure and no ordinary start/Arm.
They use inert file bytes, not an executable core or helper. The test-only
inner preparation injects a synthetic core identity without pretending to
validate a managed package. No ignored/native/kernel/VM tests are selected.

The first compiler pass caught a field inserted in a similarly named observer
struct; the initial fixture then omitted required store metadata/UUID spelling.
Those source/fixture failures were corrected, not reported as passing runs.
Exact successful source gate results are recorded with the final commit/PR.
