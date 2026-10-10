# K1 developer managed-device prerequisite

Source-only successor to `1526e07523c65762d984075584929fb858f4e88d`.
No installed package, service, renderer, ordinary runtime, coverage issuer,
NetGuard activation or default device changes.

## Reached mismatch

Numeric10 remained NONPASS at TUN/controller availability. The exact installed
managed core's listener guard requires `Meta`, whereas the canonical protected
renderer supplies `omavless0`. Config `-t` parses but does not execute that
listener guard. The Rust broker's held-TUN verifier likewise requires `Meta`.
Changing only the fixture to `Meta` would evade, not satisfy, K1's fixed policy.

## Closed build-selected families

| Build | Core Go tag | Broker feature | Sole device | Enrollment policy |
| --- | --- | --- | --- | --- |
| Existing experimental | absent | default | Meta | meta-ipv4-v1 |
| Existing release | absent | release-package | Meta | meta-ipv4-release-v1 |
| Developer K1 | omavless_k1_device | k1-managed-device | omavless0 | omavless0-ipv4-development-v1 |

The developer broker feature implies `release-package` and the matching leaf
feature. It does not accept both names. There is no caller name, environment
switch, IPC selector or new wire field. The private enum is retained in each
HeldTun; initial descriptor-name validation, kernel index lookup and every
subsequent check use that same selection. Original descriptor, namespace and
query socket remain held. Device type, flags, namespace, signed-index bounds
and immutable rechecks are unchanged. Root/CAP_NET_ADMIN interference remains
outside the existing threat boundary; this is not an atomic resolved lease.

The development policy cannot consume legacy enrollment as consent. Exact
existing enrollment parsing refuses other family strings. No automatic rewrite,
enrollment, service start or journal adoption is added. Existing nonempty
journal/FD-store startup quarantine remains unchanged. The fixed admin absence
check follows the build's sole device. Cross-family installation still requires
the separately admitted clean package boundary, not just this absence check.

## Overlay and package qualification

`tests/core_dns_adapter/mihomo-k1-device.patch` is an isolated overlay after the
unchanged legacy DNS and sing-tun patches, compatible in scope with the separate
conditional-close overlay. It adds two mutually exclusive Go build-tag files
and fixed policy tests. The two historical patch identities are not replaced.
With no tag the listener still selects Meta; the new tag selects omavless0.
It adds no relaxed TLS, resolver, address, external-FD or DNS ownership policy.

A source feature/tag is **not installed provenance**. Existing two-patch and
conditional-close three-patch receipts remain Meta-only. A separately reviewed
four-patch capability family, exact source/patch/tag/feature identities, ELF and
artifact hashes, corresponding source and managed-pair admission are required
before any developer package can be selected. This work does not edit those
T3-owned package/receipt paths or add a qualified core constructor.

## Enrolled socket access: no supplementary group grant

The broker's existing access.rs creates an exact named-UID POSIX ACL: owner rw,
enrolled UID rw, owning group none, mask rw, other none. Its 0660 mode represents
the ACL mask, not a group permission. Thus the numeric launcher may retain
cleared supplementary groups; adding groups or relaxing broker permissions is
neither needed nor authorized.

A fixed read-only fixture preflight must bind original socket identity,
root ownership, single link, exact mode, protected ancestry and exact ACL bytes
for the independently enrolled UID. Recheck identity around the ACL read.
GID/account membership or 0660 alone does not prove access. Kernel peer and
per-packet credentials remain the broker's independent admission checks.

## Gates and limits

Source tests cover both fixed leaf selections, cross-flavor refusal, retained
device on recheck, existing namespace/index/flag failures and every other
enrollment family. Go tests exercise the selected positive policy and reject
the opposite name before device effects. Run both default and explicit feature
Rust tests, and both Go tag selections against exact patched corresponding
source; no live test follows automatically from these commands.

Existing Meta-only kernel/conformance fixtures are not K1 acceptance. Required
later ROOT-selected gates include exact qualified package/enrollment, actual
held omavless0 namespace/index admission, unchanged Meta negatives, config
validation, original collector load and fresh finite numeric runtime census.
Numeric10's retained uncertain graph is not reused or called PASS. A successful
numeric census would still not cover the full resolver/failure matrix or open
the production coverage issuer.

### Focused source checkpoint

Offline locked Rust tests, one Cargo job and a dedicated HOME target/TMP:
the TUN leaf passes 15 tests with defaults and 15 with `k1-managed-device`.
The broker passes 52 tests with one existing ignored host-composition test in
each of default, `release-package` and `k1-managed-device` builds. The ignored
case was not selected. Formatting, whitespace and read-only overlay application
checks pass. No Go compilation, kernel TUN test, broker service or VM action was
performed by this source checkpoint. Final lint/commit identity belongs to the
owning Draft PR.
