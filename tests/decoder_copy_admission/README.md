# Decoder copy admission — source proposal only

This separate successor preserves every measured #608/#611 source, stage and
failure. No launcher, mounts, process supervision, runtime effects or VM calls
are implemented by this slice. Python remains developer-fixture tooling only.

## Exact observed evidence

The one #616 static capture at `9876005ed109aef844b21d7c095dfccead487c1e`
completed with known zero. Its strict typed receipt, canonical epoch, eight
baseline categories and all five IPv4/IPv6 network comparisons passed. Only
the two independently checked address lifetime countdown fields differed.
The separately reviewed success-only export completed with known zero; archive
SHA-256 is `81fecc9b4338b908617560a26c780bd112e906974bd86fb450c0f66de2cfea3d`.
Root and independent local review checked all 21 regular members, eight source
pins, strict receipt, empty logs and network comparisons. The archive was read
as data; none of its source members was imported or executed.

The decoder's original public FD was device 31, inode 8049, size 55168, mode
0100755, root 0:0, nlink 1, SHA-256
`0883fb05fa42aa6a883722098530ab08551ed317e38a2311f5c96e94dd460979`.
Its ELF64 little-endian x86-64 DYN declarations need `libbrotlicommon.so.1`
and `libc.so.6`. The observed common SONAME link resolves to the already
reviewed common object; common → libc → loader close into the prior table.
All three dependency objects match the prior reviewed source identities/hashes.
The decoder's canonical target itself had no link chain.

The fixed local `brotli` version `1.2.0-1` desc/files hashes and retained
metadata prove checked local record membership, not package signatures or
global owner uniqueness. Static headers/declarations do not prove arbitrary
dlopen closure. Candidate execution, loaded-object identity and runtime
compatibility remain unproven. The previous #611 refusal is not repaired or
retrospectively relabelled by this capture. Export checks are per-file stable,
not an atomic multi-file snapshot or historical process-state proof.

## Immutable proposed table and admission

`copy-manifest.json` adds exactly the canonical decoder to the predecessor
`40a95c1e682f94ee379a8f1cf8c387e60cdbe08ac16516309ede5e0711a5f5fb`:
18 logical paths and 17 unique original objects. Every existing ELF and
provenance row remains unchanged. Historical top-level source fields continue
to describe those original rows; separate `decoder_evidence` binds the new
row to its exact source/archive/result, header, declarations and package facts.
There is no decoder/common SONAME or canonical loader alias expansion.
Observed dependency links are evidence only, not additional copy targets.

`admission.py` changes only the frozen manifest hash and exact counts from
the existing inert admission implementation. All canonical sources and every
ancestor stay retained simultaneously before any caller could copy. Original
installed ownership must appear as unmapped 65534 inside the already validated
private namespace, distinct from namespace-root-owned copies. Device/inode,
size/mode/nlink and bounded hash checks remain strict. Full original-FD/path/
ancestor checks retain mtime/ctime comparisons and the permanent refusal/closed
latch. No failure can be retried by changing the clock or restoring metadata.
Closing attempts only owned file descriptors once; it is not process cleanup.

Fifteen inert controls repeat original FD retention, all-source-before-copy,
metadata/parent/replacement/change-reversion/deadline/unknown failure tests and
permanent sealing against this exact 17-object generation. They also reject
old manifests and alias-expanded bytes before any source open. No actual ELF,
package record, daemon, namespace, mount or guest is accessed by those tests.

## Later integration gates

Reserved future stage: `/home/kdk_vm/.cache/t3-decoder-tmpfs-review-1`.
There is deliberately no staging helper or invocation in this proposal.
The next separately reviewed launcher must pin this entire new generation,
retain strict copied tmpfs map device/inode proof and two full map passes,
and preserve the live FD enumeration fix. It must not blindly reuse #611's
historical known-anchor kill/reap behavior: first nonzero, timeout or unknown
inside any owned supervision path must stop without signals, reap, retries,
after-queries, archive or cleanup. Known successful shutdown needs an explicit
reviewed lifecycle design; static direct-child zero completion is not that
design. No daemon invocation is eligible until full source and independent
review, exact gates/pins, and a new explicit exclusive VM lease.
