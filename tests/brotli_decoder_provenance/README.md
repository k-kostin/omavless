# Fixed Brotli decoder static provenance — source only

The measured #611 review-2 wrapper at `0d0740c` returned NONPASS. A separately
reviewed read-only file diagnosis then recorded the pinned probe's exact typed
UnknownMapping refusal for `/usr/lib/libbrotlidec.so.1.2.0`. All eight source
pins were unchanged. That recorded text did not prove loaded-object identity,
current process state, after-preservation, package authority or admission.
The original failed stages, wrappers and diagnosis sources remain unchanged.

This distinct source generation investigates only that literal decoder and its
declared dependency closure. It does not repeat a daemon startup. The existing
16-object copy manifest remains byte-identical and cannot grow automatically.
Any declared edge resolving outside it refuses before opening that object or
starting another inspection command. No observed alias becomes a copy-manifest
entry; any later proposal requires explicit review of observed package evidence.

## Reconciliation with the prior static capture

The frozen #602 helper is `../static_elf_provenance/probe_four_mib.py`, SHA
`cccc171213f4631f54d906652aeaf7954230949a7de40b2093c8ab587f86aa00`.
Only its public canonical-link resolution, finite dependency decoding and
fixed-search resolution functions are called. Neither its broad package index,
capture loop nor entry point runs. The new probe imports its exact staged
bytes as an inert fixed-name module, with no repository fallback.

Only `/var/lib/pacman/local/brotli-1.2.0-1/{desc,files}` is read. Their hashes
are pinned to the manually reviewed prior common-object provenance: desc
`974d3bdc717e12ac7f08e3f15afe3a67168d07aa854237269cf6528a05e1e0cc`,
files `ff22c1aea2a86cca33a7b4028870948e6e76b292722bdcd49c9e0dac2465c3d4`.
The exact NAME/VERSION and decoder membership must match. This is fixed local
package-record membership, not a global owner-uniqueness scan or signed package
attestation; the receipt explicitly leaves global uniqueness unproven.

The only inspected executable tool is the previously reviewed `/usr/bin/readelf`
at SHA `a72f12f3dd8c560554a3ba818bcecbda9178db5befbcc64d91bbb6a241fc21bc`
and exact device/inode/size/mode. It is invoked through its retained original
FD with the candidate's retained read-only FD, fixed dynamic/program-header
flags and fixed environment. Candidates are never executed, linked, imported
or passed to ldd. ELF64 little-endian x86-64 DYN header fields and bounded
declared dynamic metadata are captured only.

## Identity, bounds and failure

Every selected source and all ancestors are opened no-follow and retained.
Public files require root ownership, nlink 1, no group/other write and no xattrs.
Metadata includes complete device/inode/mode/owner/link count/size/mtime/ctime.
Known dependencies additionally match every reviewed original identity field
and exact hash. The decoder's identity/hash is observed, never presumed.
Full retained-FD content/path/parent checks run before and after inspection and
at the end. Queued alias edges must match their previously resolved chain
before processing; final cross-checks cover every edge.

Bounds remain finite: 20 retained files, 32 MiB per ELF, 4 MiB per fixed package
record, 128 MiB total, 128 queued steps, depth 8, 50-second capture deadline,
and the new five-second owned tool observer. The outer owned capture
supervisor is called once with 65 seconds. Deadlines are checked boundaries,
not syscall cancellation guarantees. Original source descriptors remain held
until capture-process exit; no ambiguous FD/process recovery is attempted.
Source-set read/recheck uncertainty permanently seals it.

Reachable ownership support is the exact frozen
`../real_resolved_binary/probe.py` SHA
`2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592`.
Only its `OwnedProcess`, `child_status`, `quarantine` and pure input/limit
helpers are reachable. Its old `command`, `supervise` and `reap_child`
are NOT called. The new pinned `owned.py` observes WNOWAIT status and stops
permanently on first nonzero, invalid/boolean, timeout or unknown observation,
including cancellation. Only exact known zero permits one raw waitpid whose
exact PID and genuine zero exit must match. There is no group scan, signal,
reap-on-failure, Popen poll/wait fallback or destructor process query.
This establishes direct-child known-zero completion only, not a subtree
cleanup proof. First unknown or nonzero completion is terminal:
no result read, after-query, retry, process-control fallback or automatic export.

## New fixed invocation graph

The distinct stage is `/home/kdk_vm/.cache/t3-brotli-decoder-provenance-review-1`.
It contains this probe, supervisor, strict validator, owned helper and wrapper, plus the
frozen helper, containment and unchanged copy manifest. The separately pinned
O_EXCL trusted-stdin transport stages these exact files without overwriting.
The wrapper pins the entire graph and retains the canonical epoch, eight
baseline categories and all five IPv4/IPv6 network snapshots. Only actual
known completion plus strict nested duplicate/type/shape validation permits
after-state queries. No old stage is reused, read or cleaned.

The receipt retains original decoder/dependency/tool/package metadata, hashes,
ELF headers, declared edges and bounded alias chains. Loaded identity,
candidate execution, allowlist adoption and compatibility flags remain false.
Public source tests are synthetic only: no actual readelf, daemon, namespace,
package database or guest access. A complete root and independent review,
source gates, exact frozen hashes and a new exclusive lease are required before
one read-only capture. A 17-object copy manifest is a later proposal only.
