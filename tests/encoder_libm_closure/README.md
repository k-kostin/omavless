# Encoder and libm bounded static closure

This is a developer-only successor to #629
`8ca09eb71bd2505a6a661c82065d654d5e07d17d`. No production code, copy
manifest, loaded-object admission or old fixture is modified.

## Grounded scope

The parent agent's separately verified #629 invocation and fixed file collector
completed known zero. The first unlisted edge was `libm.so.6`, with logical and
resolved path `/usr/lib/libm.so.6` and no links. Encoder declarations were
`libm.so.6`, `libbrotlicommon.so.1`, `libc.so.6`. That edge observation did not
open or prove libm's original inode/hash and did not establish a full closure.
Older #627/#619 failures and their retained artifacts remain NONPASS.

The unchanged manifest already binds libc and the loader to glibc
`2.44+r24+g16be1518495f-1`. Its fixed `desc` and `files` hashes ground the two
literal package paths; no version guessing or broad package-directory scan is
used. Together with the existing Brotli `1.2.0-1` records, all four package
original FDs are hash-checked, strictly parsed and rechecked before either
new candidate ELF is opened. Membership must include the exact encoder,
libm and known libc paths. This is local package provenance, not a package
signature or global package-owner uniqueness proof.

The finite object scope is the unchanged 17 manifest objects plus exactly
`/usr/lib/libbrotlienc.so.1.2.0` and `/usr/lib/libm.so.6`. Both additional
paths must be canonical without symlink aliases. The selected closure starts
at the encoder and recursively inspects only reached `DT_NEEDED` and declared
interpreter targets. An edge to a third unlisted object refuses before that
object is opened or another tool runs. Every known object must match its
original manifest metadata/hash. Newly measured objects never become allowed
copy targets as a side effect of this observation.

Original root-owned single-link regular files and ancestor FDs remain held
through repeated byte/metadata/path checks around every readelf call and final
closure verification. Candidate ELF bytes are data only. The one admitted
readelf executes through its original pinned FD, receiving only the original
candidate FD. No loader, candidate executable, namespace, service or core is
started by this static probe.

## Bounds and terminal behavior

- At most 24 source file FDs: four package records, readelf and at most 19
  reachable ELFs. Ancestor FDs are additional and the inherited process FD
  ceiling remains 128.
- 4 MiB per package record; 32 MiB per ELF; 128 MiB aggregate retained bytes.
- At most 128 queued steps/aliases, depth eight, 64 declarations per object;
  at most 19 readelf calls, five seconds each.
- Source deadline 120 seconds; outer child deadline 140 seconds. These replace
  the earlier 50/65-second limits only in this new generation. Checked
  deadlines are not cancellation of a blocked kernel call.
- At most 2,048 fixed boundary records / 130,816 bytes. An exhausted event
  budget refuses; it does not justify a larger or incomplete receipt.

`owned.py` retains zero-only raw WNOWAIT ownership. Unknown, malformed status,
signal, nonzero or timeout permanently prevents further queries, reap,
signals, retry and output reads. Only exact observed zero permits one matching
raw reap. Shell failure/invalid-receipt branches exit before after-baseline
queries. Temporary/read-FD unwind is not process cleanup.

The strict receipt requires both new candidates, exact package/type/identity
schemas, complete edge/alias/reachability consistency and four literal false
flags: candidate execution, allowlist adoption, loaded identity proof and
compatibility acceptance. No prior collector is eligible for this schema.

## Fresh graph and review gate

New fixed stage:
`/home/kdk_vm/.cache/t3-encoder-libm-closure-review-1`.
The create-only transporter does not reuse or clean any old stage. The wrapper
retains canonical runtime/private files/resolver/core/TUN/network comparisons,
including only the already-declared address lifetime countdown allowance.
Known-zero wrapper completion would prove only this bounded static observation
and its same-invocation baseline, not T3 product completion.

Reached frozen modules remain the manifest, the canonical public resolver /
readelf decoder from `static_elf_provenance/probe_four_mib.py`, and original
OwnedProcess/child_status/quarantine/limits from `real_resolved_binary/probe.py`.
No historical generic capture, command, supervise or main path is entered.
The acyclic pins run owned → probe → supervisor → wrapper → transport;
validator is separately pinned by wrapper and transport.

Source controls cover four-package-before-ELF ordering, missing/duplicate
membership, known-manifest grounding, full synthetic closure, third-object and
alias refusal, original-FD swaps/in-place changes/FIFO/mode/xattr failures,
permanent uncertainty, strict nested JSON/types, actual shell terminal branches
and create-only transport counterexamples. They do not execute the actual
readelf, encoder, libm or VM.

Full parent and independent source review, sealed gates and an explicit new VM
lease are required before an invocation. A later manifest/admission proposal
must use separately reviewed measured evidence; this source grants neither.
