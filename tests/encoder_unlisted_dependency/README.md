# First unlisted encoder dependency — static observation only

This source-only successor preserves #627
`b69f2885b9bba93d4e62d1567cce623c83823ca8` and every earlier measured
generation. Python is developer fixture tooling, not a runtime fallback.

## Evidence motivating the diagnostic

The one #627 invocation returned NONPASS. A separately authorized file-only
observation matched all eight frozen source pins and read an empty result.
Its bounded recorded enum stream ended at `dependency_membership`, after
`dependency_resolve` and the first candidate readelf/retained-FD/canonical
recheck prefix. This narrows the recorded boundary to the old known-manifest
membership check. It does not establish a dependency name/path, root cause,
current process state, cleanup, preservation or compatibility. No dependency
is guessed from this observation.

## Deliberately different, non-admitting outcome

The new diagnostic inspects only the fixed canonical
`/usr/lib/libbrotlienc.so.1.2.0`. It retains exactly four source originals:
the encoder, hash-pinned readelf, and the two hash-pinned
`/var/lib/pacman/local/brotli-1.2.0-1/{desc,files}` records, plus their original
ancestors. Installed root ownership, regular single-link shape, no xattrs or
writable mode, full metadata, bounded bytes/hash and original-FD/path checks
remain binding. The candidate must be listed by the fixed package record and
must not be a symlink. Package signatures/global ownership uniqueness are not
proved. Readelf executes from its original pinned FD; the encoder is data.

Only the candidate's declared `DT_NEEDED` names are resolved, in original
order, using the frozen helper's two fixed public search roots. No broad
package scan, dependency ELF open/hash, dependency inspection tool, recursive
closure traversal, interpreter execution or loader fallback is performed.
Even already-listed dependency objects cannot be opened through the narrowed
source selector. An unknown resolver result, ambiguous/missing edge or
malformed public metadata still refuses.

The first resolved target outside the **unchanged 17-object manifest** ends
resolution. After a full recheck of the four retained original FDs/ancestors,
the diagnostic returns:

- outcome `OBSERVED_UNLISTED_DEPENDENCY`;
- original encoder identity/hash/header/declarations and original tool/package
  metadata already admitted by the fixed static checks;
- the resolved dependency prefix, including name, logical path, resolved public
  path and bounded original link metadata;
- the first unlisted edge, duplicated under a separately type-checked key.

No later edge is resolved after that decision. If all declared edges are
already listed, or the candidate declares none, this diagnostic refuses
instead of manufacturing an unlisted target. An interpreter declaration is
recorded as data but not followed by this narrow diagnostic.

The edge schema has exact keys/types, a finite public ASCII basename/path
grammar, at most eight links and exact numeric metadata bounds; link chains
must resolve coherently. The canonical path's presence/type checks come from
the resolver, **not** an original ELF FD or mapped-object identity proof.
`dependency_elf_opened`, `unlisted_object_identity_proven`,
`candidate_elf_executed`, `allowlist_adoption`,
`loaded_elf_identity_proven` and `compatibility_acceptance` are all false.
Observed dependency names/links never become copy targets or runtime authority.
The original #627 outcome remains NONPASS.

## Invocation and reachable graph

New literal stage:
`/home/kdk_vm/.cache/t3-encoder-unlisted-dependency-review-1`.
The trusted-stdin O_EXCL transporter and fixed wrapper are a new generation,
not a retry or mutation of any old stage. Wrapper known-zero means this narrow
typed observation and its strict canonical/network baseline completed; it is
not feature compatibility or a successful full dependency closure.

The byte-identical owned helper retains first-nonzero/unknown/timeout STOP,
without later query, signal, reap, retry, after-baseline read, export or cleanup.
Only exact WNOWAIT zero permits one exact raw reap. Bounded fixed BEFORE and
at-most-one FAILED_AT labels are inherited; no arbitrary exception text is
emitted. Temporary/read FD unwind is not process cleanup.

Reached frozen source remains:
`static_elf_provenance/probe_four_mib.py` canonical public resolution,
fixed-search dependency resolution and readelf decoding only;
`real_resolved_binary/probe.py` OwnedProcess, child_status, quarantine,
require and limits only. No broad historical command/supervise/capture/main.
Source bounds remain 32 MiB per ELF, 4 MiB per fixed package record, 128 MiB
aggregate, 50-second checked source deadline, 5-second tool and 65-second
outer deadlines; the retained source-file cap is narrowed from 20 to four.
At most 64 declared edges are considered. Checked deadlines are not syscall
cancellation guarantees.

| Staged file | SHA-256 |
| --- | --- |
| probe.py | d2465f004fe7cd08f8dae27fdff5abc5d598106038a585287758f938e323d304 |
| supervisor.py | 00ce656fdacd43d3d7a7dc75e78d4f659ab431d508831e38a13f50eba4d8998d |
| validator.py | 1f8c9dc22fa4fcdaa60aad958886109a66ebb1f215f4912ab0b5ea5871ed90c0 |
| owned.py | 8d9504cff609d703870b4b7f8bec4e4b601d8763b5f69b7df62e3b5da9106258 |
| helpers.py | cccc171213f4631f54d906652aeaf7954230949a7de40b2093c8ab587f86aa00 |
| containment.py | 2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592 |
| copy-manifest.json | d6ca9cd599adb573f4419f49147677f681db08fcc54a06cb0f34ed950ce19c36 |
| vm-guard.sh | 289454d5043d1f6ef31ef19e694ce6c690b0f267c82961d60f889c9425452de3 |

Transport SHA-256:
`d3d637f66b873a53e62c680cb2db590b9be75985667bc97860e3a9fd06c453f5`.
Readelf remains
`a72f12f3dd8c560554a3ba818bcecbda9178db5befbcc64d91bbb6a241fc21bc`.
No prior exporter or observer is eligible for this schema/stage.

## Source controls and later authority

The 31 inert controls cover exact pins, one candidate tool/four original
sources, no later edge after first unlisted result, no known/unknown dependency
FD opens, no fabricated result for known-only declarations, source mutation,
unknown final recheck, public path/link/type bounds, duplicate and boolean
receipt counterexamples including the duplicated last edge, fixed producer/
validator edge parity, finite event sealing, zero-only ownership, terminal
shell branches and exclusive transport. No real candidate/tool/VM is used.

Local source checkpoint: 31 focused controls PASS; full source suite PASS
(707 tests, two declared skips, JavaScript/QML checks), shell syntax and diff
checks PASS. Rust and production code are unchanged; no new runtime gate is
claimed.

Full parent and independent review, sealed source gates and an explicit new
exclusive lease are required before one diagnostic invocation. Any later
original-FD provenance capture or manifest proposal needs separate review and
authority; observing an unlisted public path does not authorize opening it.
