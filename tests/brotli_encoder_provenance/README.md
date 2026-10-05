# Fixed Brotli encoder static provenance proposal

Source-only successor to [the decoder capture](../brotli_decoder_provenance/README.md),
based on #618 `ce973d0c85922350fc6984a76ab33c23db055a39`. This is developer
fixture tooling, not a Python runtime fallback or product compatibility gate.
No invocation or encoder identity has been accepted by this source checkpoint.

## Why this fixed target

The immutable #622 `b3a44c8f90d733e3975f06236b3d5e4e789afbab` invocation
returned NONPASS. A separately authorized file-only observation read stable
original fixture FDs and recorded the pre-admission public map-text candidate
`/usr/lib/libbrotlienc.so.1.2.0`, mapped device 29 / inode 8052, during initial
resolved membership checking. Both identity/adoption flags remained false.
The current 17-object manifest omits that pathname. This establishes a recorded
membership boundary, not the identity of an opened ELF, a syscall failure cause,
current process state, preservation, or compatibility. Old stages and failed
measurements are unchanged; this capture neither queries nor cleans them.

## Deliberately narrow graph

The sole new candidate is the canonical encoder pathname above. The exact
17-object [decoder admission manifest](../decoder_copy_admission/copy-manifest.json)
is reused byte-for-byte as the known dependency boundary. No encoder row or
alias is added. Candidate symlinks refuse. Only the two exact public ALPM records
`/var/lib/pacman/local/brotli-1.2.0-1/{desc,files}` are read, with the same pinned
hashes as #616; they must name that package/version and list the canonical
encoder. This is not a broad package-database scan or proof of global ownership
uniqueness/signatures. A declared dependency outside the known 17 objects
refuses before opening its object or running another inspection command.

The pinned static helper is `tests/static_elf_provenance/probe_four_mib.py`:
only canonical public link resolution, fixed search, and bounded readelf decode
are reached. Its old main/capture/package index are not called. The frozen Rust
`host-fixture` is not used: it belongs to runtime composition, whereas this
graph executes only hash-pinned `/usr/bin/readelf` from its original FD with
the candidate's original FD passed for data inspection. There is no ldd,
candidate ELF execution, broker/core, daemon startup, DNS setter or copy mount.

The #616 retained source-FD and ancestor checks are unchanged: nofollow opens,
root ownership, regular single-link files, no writable mode or xattrs, full
device/inode/mode/size/mtime/ctime and path rechecks, bounded bytes/hashes before
and after tools. ELF64 little-endian x86-64 DYN headers and declared dynamic
metadata are observed, not promoted to actual loader closure. Queued dependency
target/link identity is retained and compared before use, then checked again.
Limits remain 20 retained files, 32 MiB per ELF, 4 MiB per fixed package record,
128 MiB aggregate, 128 queue steps, depth 8, 50-second checked source deadline,
5-second owned tool deadline and 65-second outer child deadline. Deadlines are
checked observations, not syscall cancellation guarantees.

`owned.py` is byte-identical to #616. It uses only the frozen containment
module's OwnedProcess, child_status, quarantine, require and resource limits.
It never calls old broad supervise/command/reap helpers. First nonzero,
timeout, invalid/bool status or unknown is permanent: no subsequent process
query, signal, reap or output read. Only exact WNOWAIT zero allows one exact
zero raw reap. This does not prove descendant cleanup. Wrapper failure is
terminal before receipt reads or after-baseline queries. Temporary local output
FDs close on unwind; no guest file cleanup or process termination is attempted.

## New invocation generation (not authorized by source alone)

Literal fresh stage: `/home/kdk_vm/.cache/t3-brotli-encoder-provenance-review-1`.
The independently pinned trusted-stdin transporter creates it exclusively and
publishes exactly eight O_EXCL files, code 0500 and manifest 0600. There is no
overwrite/resume/fallback. Baseline and strict receipt checks preserve the
prior canonical epoch, eight inventory categories and all five IPv4/v6 JSON
collections; only confirmed address countdowns may differ. First failure stops
all further guest actions. Whole known-zero success is a prerequisite for any
separately reviewed private export; it does not itself authorize export or
encoder/copy-manifest admission. Retained old unresolved daemons are not
enumerated or altered by this static capture.

Exact eight staged source pins:

| Name | SHA-256 |
| --- | --- |
| probe.py | f2b5f27cd626283f9655a204d5943b4c1c07f9d2f2b42297e07a4ce9533ee0ff |
| supervisor.py | 8a6d35f17efd7c661d4d52ef12772e34088e9dcd7d811c6d26314cdd0624a5f6 |
| validator.py | 7a33c3e35f1b6edda95c65e89870d0e06c89dfdb0b9b8304cbcc74aeadafeef0 |
| owned.py | 8d9504cff609d703870b4b7f8bec4e4b601d8763b5f69b7df62e3b5da9106258 |
| helpers.py | cccc171213f4631f54d906652aeaf7954230949a7de40b2093c8ab587f86aa00 |
| containment.py | 2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592 |
| copy-manifest.json | d6ca9cd599adb573f4419f49147677f681db08fcc54a06cb0f34ed950ce19c36 |
| vm-guard.sh | c00f63d2c87a6cede35fc0f2926428c28219122424a3601788e287acddc519ef |

Transport SHA-256:
`c6737d2843a1d201fb33ff61b8309bc1fe7754817a675a45768ee0b032ac0b11`.
Readelf remains
`a72f12f3dd8c560554a3ba818bcecbda9178db5befbcc64d91bbb6a241fc21bc`.

## Source controls

The 24 focused inert controls cover all source pins, unknown/dependency drift
before next object/tool, canonical package membership rather than aliases,
exact 17-object boundary, strict nested receipt/duplicate/type rejection,
original FD replacement and mutate/revert refusal, permanent deadline/unknown
seals, zero-only reap, no queries/signals/reap on first failure, actual shell
terminal branches, and exclusive synthetic staging. They use no real readelf,
VM, namespace or candidate execution. Full source gates are recorded with the
sealed checkpoint; these controls do not manufacture runtime acceptance.

Local source checkpoint: all 24 focused controls PASS; full `tests/run.sh`
PASS (644 Python tests, two declared skips, JavaScript/QML checks PASS), shell
syntax and `git diff --check` PASS. An intermediate pin-wiring run caught the
predecessor wrapper hash in the new transporter; only that stale literal was
corrected before the complete green run. No VM or inspection-tool execution
occurred. Rust is unchanged and no fresh Rust/runtime gate is claimed.
