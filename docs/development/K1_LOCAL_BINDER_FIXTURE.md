# Fixed local binder fixture: source only

The external review-only package now includes a fixed opt-in executable and
two noninstalled unit templates in `tests/k1_namespace_binder/fixture`.
The previously reviewed library and fault matrix are unchanged. This is not
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

Fresh synthetic gates passed nine library tests, four binary sequencing/output
tests and three compile-fail doctests. These tests do not execute `main`, the
actual binder, namespace ioctls or netlink construction. Exact final source,
additional handoff-admission controls, MSRV/gates, build provenance, immutable
freeze and delivery/negative-witness review remain prerequisites. ROOT alone
may perform a separately approved actual invocation. No product dependency,
normal acquisition provider, nftables effect or canonical claim is added.
