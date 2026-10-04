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
connection and all original descriptors; before acknowledged Unref, the
reference is retained too, but after acknowledged Unref it is not claimed;
no next RPC, compensating
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
receipts before any known-success teardown. The initial checkpoint deliberately
had an impossible artifact pin pending the actual build and freeze below.

Initial focused source controls passed 23 native tests (one guest-only entry
ignored) and 17 Python controls. The first copied Python literal-comparison
control retained the predecessor selector in its expected transformation and
failed; correcting that NEW test expectation preserved the literal-only writer
comparison and all old tests. No guest invocation occurred.

Native source `725f6ef9b14084f58cd3589f9eee09472add9422` passed the full
Rust script (2,138 passing results across its test invocations, zero failures;
workspace/TUI strict clippy, formatting and two-case parity passed). The main
runtime suite passed 1,105 tests with 33 ignored; netguard passed 283 with 25
ignored. Source checks passed 657 tests with two expected skips plus JS/QML.
These are source-only results, not execution of the ignored guest fixture.

The actual frozen diagnostic ELF SHA-256 is
`a2b8dd3b7cc1658c536fd81bd25a74ae55255cb2159624cfe8f548c162ad0e9a`.
Its private receipt SHA-256 is
`4ff907db631850b03e37a5f94f369a04ee3a9f8345e8084f0b1eaf4dc48acaf7`.
The source original was a single-link mode-0755 regular ELF; the exclusive
single-link frozen copy is mode 0500. Original descriptors, ancestor/path
identity and complete hashes were checked before and after copying. Subsequent
outer artifact-pin changes do not relabel this native build's source head.
The old #615 helper/reference remains uncertain and retained; this new capture
does not query its unit/process state or claim its quiescence or recovery.

Independent review of `65c7713` found a predictable before-baseline refusal:
the old #615 retained link points outside the inherited target-root inventory.
The correction admits only that exact link to its exact fragment after binding
the four known old source/artifact hashes through original read-only FDs,
root-owned parents, unchanged stage metadata and exact symlink identity/target.
It neither adds a general `/run` prefix nor skips the old entry. The baseline
still records the exact link and fragment content; original source descriptors
remain held and rechecked. This is file metadata/content continuity only, not
an old-unit, process, reference, quiescence or recovery observation. No old
service query, Ref/Unref, Start/Stop or cleanup is added. The old NONPASS remains.
Real-FD synthetic controls cover same-byte link/fragment/source replacement,
metadata drift and permanent refusal; inventory controls reject another target
in the same directory and prove no external query or repeated inventory.
The native source and frozen ELF above are unchanged by this query-only fix.
