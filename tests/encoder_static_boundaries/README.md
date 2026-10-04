# Static encoder boundary observation — source only

Successor to #626 `1d4299b21e6ca41028680f8656d59e7f9583723f`.
The fixed encoder, 17-object dependency manifest, package/readelf pins, retained
original-FD admission and zero-only supervision are unchanged. No encoder
object or alias has been admitted; this is developer-only Python tooling.

## Historical outcome retained

The one #626 invocation returned NONPASS before after-baseline queries.
A separately authorized original-FD file observation matched all eight source
pins and recorded an empty result, a 30-byte capture NONPASS marker and a
34-byte supervision NONPASS marker. No public frames or reasons were recorded:
the capture exception handler intentionally withheld its exception text.
This establishes neither failure cause nor current process state, preservation,
cleanup or compatibility. Old source/stages/results remain immutable. This
generation does not query or replay them.

## Finite new observations

The new probe emits `T3_ENCODER_STATIC_BEFORE_V1 <phase>` to private stderr
before each relevant input, package, original-FD open/shape/read/hash/path/pin,
queued canonical identity, readelf, dependency and final receipt boundary.
Each phase belongs to the literal `PHASES` enum. Labels contain no path,
observed metadata, package content, exception text or profile values. They are
before-check labels, not completion or identity proof. Earlier labels identify
the surrounding package/tool/candidate step; repeated source checks may share
the same finite label.

The observer is bounded to 2,048 before-events and 130,816 bytes, reserving
space within 128 KiB for the single failure label. It is inert on import.
The fixed main installs it; unknown enums, exhausted bounds and short/unknown
writes permanently seal it before propagation and prohibit later output.
Capture errors seal it before the next step. The outer capture exception
handler may emit at most one explicitly authorized
`T3_ENCODER_STATIC_FAILED_AT_V1 <retained-phase>` label. This is a retained
enum, not a new observation or exception-derived reason. If that write fails,
there is no retry or traceback/context echo.

There are no post-failure process queries, signals, reaps, source reads,
network observations, export or cleanup. Owned temporary/read FDs unwind as
in the predecessor; this is not process cleanup. The byte-identical owned
supervisor continues to allow one exact raw reap only after exact WNOWAIT zero.
No broad frozen supervise/command/reap helper is reached. The new markers do
not change the whole-invocation first-failure terminal policy.

## New immutable invocation graph

Literal stage:
`/home/kdk_vm/.cache/t3-encoder-static-boundaries-review-1`.
Transport creates it and its eight fixed files exclusively, with no fallback,
overwrite or continuation of any old generation. The same strict typed
receipt and complete canonical/network baseline remain prerequisites for whole
known-zero success; new receipt schema is `fixed-encoder-static-boundaries-v1`.
Diagnostic stderr is intentionally nonempty even on success, so the old
success exporter is **ineligible**. Any future exporter must separately review
and validate bounded finite transcripts without importing staged source.

| Staged file | SHA-256 |
| --- | --- |
| probe.py | 2cfa873da2da5fec9f10447553903d603a9489e0ea8d47367c94b7c48c831ee4 |
| supervisor.py | 9290f047a255e4f5111d8e5126749da72b1a49c0805b6a6f58f57da8300e1445 |
| validator.py | ff46ab49fc02d2c4ec93446427058057beded45989b79b2d9e2232b8ae2beba8 |
| owned.py | 8d9504cff609d703870b4b7f8bec4e4b601d8763b5f69b7df62e3b5da9106258 |
| helpers.py | cccc171213f4631f54d906652aeaf7954230949a7de40b2093c8ab587f86aa00 |
| containment.py | 2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592 |
| copy-manifest.json | d6ca9cd599adb573f4419f49147677f681db08fcc54a06cb0f34ed950ce19c36 |
| vm-guard.sh | 18aec8e9496cd548a6339ea271596c68106ccc177da78fd18fa348dc5c5516a9 |

Transport:
`2f4ffd2d45738ce035b523653f3ce3ec1338f95fcfa2b4fdea57879b71891e7a`.
The reached frozen helpers and all retained source limits are documented in
the [predecessor contract](../brotli_encoder_provenance/README.md). Only the
fixed pinned readelf may execute; the candidate ELF is data. No broker/core,
daemon, DNS setter, arbitrary search, alias expansion or installed runtime
mutation is introduced.

## Controls and eligibility

The predecessor's 24 controls are rerun against this exact new graph. Eight new
inert controls cover finite successful labels, package/queued/readelf failure
prefixes, permanent no-next-step sealing, one failure label, event count/byte
bounds, short/unknown/cancelled writes and no private exception-context echo.
None invokes a real inspection tool, candidate, namespace or VM.

Sealed local source checkpoint: 32 focused controls PASS; full source suite
676 tests with two declared skips and JavaScript/QML checks PASS; shell syntax
and diff checks PASS. Rust/production sources are unchanged. No actual static
capture, runtime acceptance or new host observation is claimed.

Full parent and independent reachable-graph review, sealed source gates and
an explicit exclusive lease are required before one new invocation. Source
tests and static metadata cannot claim actual mapped identity, installed
compatibility, historical recovery or copy-manifest admission.
