# Fixed local binder fixture: scoped local-pair evidence

## Fresh v3 actual result

ROOT's sole fresh v3 invocation completed with authoritative `KNOWN_ZERO`,
exit 0. The retained host capture (`6872ec`) is a 51-byte, mode-0600,
single-link file containing exactly
`K1_BINDER_LOCAL_PAIR_OUTER_PRESERVED_NOT_CANONICAL` plus newline; SHA-256
`301aae4061148e7ce980654c060305224a8588b6a309f899abdfa17ef292416f`.
Create and all five fixed transfers completed zero before the sole root
invocation. The strict controller accepted both the actual matching binder
and the separate PrivateNetwork mismatch result, including their original
completion checks. No subsequent current-state query or cleanup is evidence
for this result.

The exact native source is `e6488ed5c39486a5f967859c20a0b84859ae4b5d`.
The separately frozen 1,368,848-byte probe has SHA-256
`96c128af90c0c11c25b9f87d8d5c41b412ac5bacef67456eeb6e04ae4`;
its 1,292-byte receipt has SHA-256
`f502a2f947b732ed6eee640cd1cbd0473ba8b246d07986de4ad9aca74c9c1488`.
ROOT performed the actual freeze (`eeb2e7`); the original build was not chmodded.
The v3 controller SHA-256 is
`bf85f27247a9aaa0aef26f43eeb8113d1aa9fd86a947505269e1f1d10a9b3d1f`;
the outer loader SHA-256 is
`d97b479e67e96e1a61847e87c0985a4dcf4e22902f26c7f80dee816d83776f18`.
Both complete private source graphs received ROOT and independent review:
34 controller controls (`3fbba4`, independent `3dd5d5`) and 28 outer controls
(`a686cc`, independent `bf9594`) passed before invocation.

The separate v1 and v2 invocations remain **NONPASS**, permanently stopped;
v3 does not retrospectively accept them. A separately reviewed, fixed-file
observation of v2's original cached configuration recorded WatchdogUSec as
`18446744073709551615` (report SHA-256
`ec44051b6e4ab6a009c3866988be5c54c099644a5db547ea19e690b05102c0b9`).
Pinned systemd v261 `de9dbc37ad4aa637e200ac02a0545095997055df`
initializes the original watchdog interval to infinity; its getter exposes
that interval before start, while service_start copies the configured interval.
V3 therefore requires exactly MAX before start and exactly zero after completion,
with explicit `WatchdogSec=0`. It accepts neither value interchangeably and
does not infer any other failure cause or that v2 started a service.

This closes the fixed local namespace/socket agreement and mismatch fixture,
not canonical manager origin, installed-package authority, actual nft creator
ownership/readback/effects, production acquisition or broad network acceptance.
No dependency adoption, merge, release or production availability follows.
The next isolated creator-owned launcher work is tracked separately in #654.

## Source design and historical prerequisites

The external review-only package now includes a fixed opt-in executable and
two noninstalled unit templates in `tests/k1_namespace_binder/fixture`.
The historical b0a35e7 build remains retained. Its library had its own two-second
budget starting after the outer inherited opener, so the outer budget could
expire while the library continued within its newer budget. It is not accepted
as a strict shared-deadline fixture. The corrected `FixedAttempt` starts before
configuration and supplies one private, non-resettable deadline to the fixed
opener, all constructor/recheck leaves and output gates. No caller can supply a
deadline, path or provider. An executed synthetic counterexample spends 1.9s in
the opener and 0.11s in the next leaf: it retains that returned FD and refuses
before any subsequent namespace-type query. Attempt reuse is permanently refused
after success, failure or panic. This is not
yet a frozen binary, delivery graph, actual invocation or VM acceptance at that
historical checkpoint; the exact later v3 evidence is recorded above.

The positive invocation requires local namespace/socket agreement; the
negative invocation accepts only `Refused::Mismatch`. Each expected result
exits zero with its distinct fixed `NOT_PRODUCTION` marker. Unsupported APIs,
unavailable descriptors, expiration, unexpected agreement/refusal, short output
or output errors are nonzero, never accepted as the negative control.

The first file opener reopens only `/proc/self/fd/3`, retaining the namespace
object before late classification. It does not adopt the same open file
description. Exactly one inherited FD and its fixed name plus own-process
LISTEN_PID consistency are configuration checks, not authenticated provenance.
No caller path, PID query, raw identity or descriptor export is introduced.

Both templates request exactly one read-only OpenFile for `/proc/self/ns/net`;
the mismatch template additionally requests PrivateNetwork. They run without
capabilities as the ordinary nobody account. They are not installed, enabled
or started by source tests. A future isolated delivery must prove exact effective
unit contents and absence of other passed descriptors/drop-ins; neither the
account nor the unit filename is canonical authority.

The executable uses an original two-second elapsed budget around handoff,
binding and its single output write/flush. It never retries output or emits a
fallback after an attempt. A blocking kernel syscall cannot be cancelled by
these checks. Unit start timeout is deliberately infinite: an unknown/stalled
run is not silently killed into a supposed acceptance result. A future bounded
parent controller must retain an unknown original unit/process and stop its
scope without retry, query-based adoption or cleanup.

The original synthetic gates passed nine library tests, five binary configuration/sequencing/output
tests and three compile-fail doctests. Corrected shared-deadline gates are separate.
These tests do not execute `main`, the
actual binder, namespace ioctls or netlink construction. Exact final source,
MSRV/full-source gates, build provenance, immutable
freeze and delivery/negative-witness review remain prerequisites. ROOT alone
may perform a separately approved actual invocation. No product dependency,
normal acquisition provider, nftables effect or canonical claim is added.
