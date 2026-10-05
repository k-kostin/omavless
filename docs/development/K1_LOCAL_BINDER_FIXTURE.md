# Fixed local binder fixture: source only

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
yet a frozen binary, delivery graph, actual invocation or VM acceptance.

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
