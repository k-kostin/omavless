# K1 response-boundary diagnostic

Developer-only successor to the first `fe944166` capture NONPASS. The old
unit, retained reference, stage, executable and evidence remain unchanged.
This generation is not a retry of the old invocation, a lifecycle gate or
permission to execute a service. Source review and an explicit VM lease precede
any invocation.

## Fixed scope

The new literal unit is `omavless-k1-admission-response-diagnostic.service`;
its exclusive root stage is `/run/omavless-k1-admission-response-diagnostic`.
There is no caller-selected unit, path, property list, command or environment.
The cfg(test) helper retains one manager connection before Ref, pins its unique
owner and exact version, and preserves the predecessor's finite sequence:
Owner, Version-before, Ref, Unit, Service, Dump, Version-after, Unref, post-Unref.
No Start, Stop, activation, old-unit Ref or lifecycle writer is invoked.

Before each RPC, publish and synchronize an exclusive finite phase receipt
through the original stage FD. After a successful reply, decode under the
existing limits, validate that response before the next RPC, and write a bounded
private response receipt. Selected Unit/Service variants retain their signatures
and values even when they mismatch; missing/type/value mismatches have fixed
field identifiers. Unrelated properties are not emitted. Dump data stays private
and bounded. Diagnostic schema explicitly states `admission: false`.

The old permission and Dump predicates are not relaxed. Only the new fixed
unit/stage/writer identities differ. A response mismatch, transport/decoding
failure, receipt collision/write failure or panic is terminal: preserve the
connection/reference and all original descriptors; no next RPC, compensating
Unref, signal, cleanup or retry. A failed receipt write is not retried. The
outer raw-owned observer admits only an exact zero WNOWAIT outcome before its
single matching reap; all other outcomes preserve the stage and stop queries.

## Evidence and tests

The prior #612 capture contains only selected old identity/service facts, not
the new complete permission selection. Its 167-line Dump has no unknown labels;
Environment/Open File and manual-start differences follow intentional unit
changes. The first fe944 native receipt was written only after several response
validations. Its absence therefore does not establish a failing property or
manager defect. No source guess is relabelled an observed cause.

Pure fake-bus controls must cover every response/decode/validation boundary and
every receipt write boundary, asserting no next call after the first failure.
Wrong signatures, typed empty containers, duplicate keys, missing selected
fields, bounded values, fixed mismatch identifiers and private-only output have
independent controls. Source gates, full Rust/fmt/clippy, original ELF freezing,
acyclic artifact pins, full parent and independent review remain required.
Neither a successful diagnostic nor an independently observed unchanged host
would repair the old NONPASS or establish full K1 acceptance.

The first source checkpoint copies the predecessor's cfg(test) validator/helper
and literal-only writer modules into distinct diagnostic modules. Older modules
remain byte-identical; the new unit's ignored writer selector points only at its
new stage and is never executed by capture. Per-response evidence uses schema 1
and explicit diagnostic/non-admission flags; the retained final configured-fact
receipts preserve schema 2. The outer checks all eighteen response-boundary
receipts before any known-success teardown. Native artifact pins remain
deliberately impossible until a built original ELF has been frozen and bound.

Initial focused source controls passed 23 native tests (one guest-only entry
ignored) and 17 Python controls. The first copied Python literal-comparison
control retained the predecessor selector in its expected transformation and
failed; correcting that NEW test expectation preserved the literal-only writer
comparison and all old tests. No guest invocation occurred.
