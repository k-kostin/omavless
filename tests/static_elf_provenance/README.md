# Public static ELF candidate provenance — source-only proposal

This separate branch follows retained tmpfs inventory refusal `eef9151`, reported
at `36cd3ed`. It does not extend any executable manifest or allowlist. The seeds
are the unchanged 16 logical original ELF records and the explicitly observed
public candidate `/usr/lib/libbrotlicommon.so.1.2.0`. Only static DT_NEEDED and
fixed interpreter edges are followed. This is not proof of every runtime dlopen.

The sole executed ELF tool is `/usr/bin/readelf` from binutils 2.47-4, independently
captured without execution and manually reviewed before this proposal. Its pin is
`a72f12f3dd8c560554a3ba818bcecbda9178db5befbcc64d91bbb6a241fc21bc`,
device 31/inode 29149, size 810072, root:root regular 0755, nlink 1, no file caps.
Private tool-metadata receipt SHA-256:
`c0f0c5c0428f6726e8c767441066658f69bfd92b818d563f44162be4b9c92c68`.
This pin authorizes no production privileges or candidate ELF admission.

The tool is opened no-follow and executed through its retained `/proc/self/fd`
path, with only `--wide --dynamic --program-headers` and a retained candidate FD.
Candidate ELFs are never executed; no dynamic loader/`ldd` invocation occurs.
The original frozen supervisor supplies bounded known-child capture, exact raw
reaping, and terminal unknown-state quarantine without wait/poll fallbacks.
Environment is fixed C locale and PATH, with no inherited loader environment.

Each candidate must have stable original-FD/root-owned/public regular metadata,
bounded ELF bytes/hash and stable canonical path/link chain. Original manifest
hashes and alias targets remain mandatory. DT_NEEDED values must be basenames;
only fixed `/usr/lib` and `/usr/lib/systemd` are searched. Missing or multiple
matches refuse, not guess loader precedence. Only fixed public interpreter paths
and known public search tokens are accepted. Private paths, custom search paths,
filter/audit tags, deleted/racy/symlink-escape states and malformed output refuse.

Public package ownership/version comes from bounded no-follow root-owned pacman
file lists and description records, not arbitrary package scripts or command
execution. Only selected package name/version and metadata hashes are retained;
unrelated file lists/description fields are never output. Selected package data,
candidate FDs/paths/aliases and the tool are rechecked at the end. The bound is
64 canonical objects, depth 8, 512 traversed edges, 32 MiB per ELF, 128 MiB total,
and 60 seconds plus at most one bounded in-flight command. Package scanning has
separate count/byte limits. First uncertainty stops all remaining tool calls.

The output is `OBSERVED_STATIC_CANDIDATE_CLOSURE`, not loaded-object proof,
allowlist admission, installed or broker/core compatibility acceptance. It is a
finite candidate table for manual root review of every path, package and hash.
A later immutable copy manifest needs separate authorization; no source here
edits an existing manifest or runs broker/core/resolved/dbus or DNS operations.

The separate whole-invocation guard pins source/containment/original inventory/
tool, preserves canonical epoch, eight baseline categories and IPv4/IPv6 state,
and checks the fixed launcher/readelf process forms are absent. Source/tests and
the complete guard require review before any exclusive VM lease or invocation.
No closure capture has yet executed. The earlier narrow tool-metadata lease was
returned; its receipt contains only public fields and confirms canonical epoch.

The fixed decode flags follow the primary [GNU readelf documentation](https://sourceware.org/binutils/docs/binutils/readelf.html).

## Source-review refusal and queued-edge correction

Initial source `651f333bd4304f9df2c8e0695660c3140b5da8f8` was **NONPASS in
source review**, not executed in the VM. It recorded a discovered dependency's
resolved path/link chain but did not carry that expectation through its queue;
a changed A-to-B resolution could be adopted at the candidate's first processing.
The original `vm-guard.sh` remains frozen with that source's hash and is not
eligible for invocation.

The corrected source carries every discovered dependency's expected canonical
target and complete link metadata through the queue and checks them before
opening or decoding the candidate, including already-seen targets. Fixed
interpreter edges also retain their discovered expectation. Final receipt edges
must match the final alias table. Regression cases replace A with B or change
the chain while keeping A; both refuse before the queued target's FD/tool call.
An unchanged positive edge remains accepted as static evidence only.

`vm-guard-queued-edge.sh` is the separately pinned replacement proposal. No VM
capture has occurred and none is authorized by this source correction alone.

## First approved static capture: package-index refusal

One separately reviewed invocation measured source
`3d4b7d5f6b5f31b77f0c4e31d80f4750dcae7a4f`, probe SHA-256
`b5f9c9651c9469e70f2d290252c897de62f11410228701c47d270fefa33831ec`,
queued-edge wrapper SHA-256
`4726bf496706e0518265cd09618afc2bc2a64ba16e5175290a46d89fb884485b`.
Its source gate passed 434 Python tests (2 skips), JS/QML and 13 focused tests.

The invocation returned **NONPASS**, reason `package_file_list_shape`, during
the initial package-index phase. No tool record or candidate record was emitted;
the failure precedes readelf execution. No cleanup reason was reported. The
failing package/path was not retained, so no particular package or underlying
format condition is established. No manifest or allowlist changed.

The canonical epoch, all eight baseline categories and all IPv4/IPv6 fields were
preserved. Independent read-only checks found the exact diagnostic/readelf
process forms absent and canonical PID 938/boot unchanged. The exclusive lease
was returned without retry, signal, adaptation or cleanup.

The complete private stage remains archived as
`t3-static-elf-3d4b7d5-nonpass.tar.gz`, host directory
`/home/kk/.cache/t3-real-resolved-build.XVxwu8AF/` and guest `/home/kdk_vm/.cache/`,
matching SHA-256
`a93d533de69a13cc372317f49c80b22929eb090726721df26fad84aafa39dd21`.
No candidate closure, loaded-object proof, admission or compatibility acceptance
was obtained. Any source correction requires separate review and a new lease.

## Narrow metadata follow-up and primary-format proposal (not executed)

The primary [ALPM local files format](https://alpm.archlinux.page/specifications/alpm-db-files.5.html)
permits packages with no tracked files to have an empty `files` record. Empty
lines are ignored. Nonempty records use one FILES section and optionally a
BACKUP section whose entries refer to files in that FILES section. Thus the
unconditional old marker check rejects a valid empty form. This is a source
defect and a possible explanation, **not** identification of the earlier guest
file: that attempt retained no package or raw shape.

`alpm_files_diagnostic.py` is a separate no-subprocess/no-ELF-execution proposal.
It scans the same fixed local package database in sorted order, with bounded
count/bytes/time and stable root-owned original-FD metadata, stopping at the first
failure of the old exactly-one-FILES-marker condition. It retains only the
validated public package name/version/directory, original file numeric metadata
and hash, description hash, and bounded format counts/booleans. No file list,
backup pathname, raw marker text, description or private log is emitted. The
selected file is measured again for equality; a failed observation is not retried.

The isolated `proposed_file_entries` parser is exercised by pure tests but is
**not wired into the static closure capture**. Empty data/empty lines produce
zero ownership edges. Malformed nonempty data, unsupported/duplicate sections,
empty sections, traversal/absolute paths, duplicate file entries and invalid
backup relationships still refuse. Synthetic privacy tests ensure metadata
summaries contain no input filenames or unknown section text. Empty-format
acceptance does not identify an owner for any ELF and cannot admit a dependency.

The old measured probe and both existing wrappers remain unchanged. New
`vm-guard-alpm-shape.sh` pins only this metadata diagnostic, using fresh stage
`t3-alpm-files-shape-review-1`, all original canonical/baseline/network checks,
whole-command timeout and independent process quiescence. It requires separate
full review and a new exclusive lease. No follow-up has run; the actual first
package-index failure remains unclassified until safely retained observations.

### Retained-FD review correction (source only)

`acdc601a18dd3246fc95ccaeb77b9963ce7c15a7` was source-review NONPASS and
was **never executed**: its selected-file recheck reopened the pathname, and its
wrapper used an unreviewed automatic timeout signal path. That old wrapper is
retained unchanged; it is not eligible for execution.

The corrected diagnostic retains the selected `files` FD across the description
read and final reread/hash. Original FD, pathname and parent identity checks
include mtime/ctime; replacement, in-place change/reversion and rename/reversion
are refused. Real temporary-file tests exercise these mutations and an unchanged
positive control; the selected file is opened exactly once. No raw file list is
retained and the parser proposal remains unwired into closure capture.

`alpm_files_supervisor.py` pins the original reviewed containment helper and uses
its OwnedProcess plus single raw-wait/WNOWAIT supervisor call. Unknown child
ownership remains terminal without fallback poll/wait/signal/reap or synthesized
status; the frozen destructor prevents hidden process queries. Known anchored
timeout handling is inherited unchanged, not delegated to GNU timeout. Tests
cover known completion, unknown observation, incomplete completion and existing
output refusal before launch.

The new `vm-guard-alpm-retained-fd.sh` pins diagnostic, supervisor and containment,
requires the fresh non-symlink UID/GID 1000 mode-0700 stage
`t3-alpm-files-retained-fd-review-1`, and enables umask 077 plus shell noclobber.
The runner creates result, stderr and supervisor receipt exclusively. All original
canonical and baseline/network checks remain; quiescence checks both fixed child
and supervisor forms. Full source review and a separate exclusive lease are
required before **one** invocation. No VM diagnosis or ELF admission is claimed.

The `5efed20` source review also refused wrapper continuation after a failed
supervisor and its missing-stage helper fallback; that head was not executed.
The wrapper now exits immediately on any supervisor failure, preserving existing
before snapshots without further guest queries, result reads or effects. Thus
after-state preservation is **unproven** on that path, not implicitly claimed.
Only successful supervision reaches later baseline/quiescence checks. A shell
regression executes the exact failed-supervisor branch with a synthetic failure
and verifies no later action occurs. Supervisor import is inert; actual main
requires the exact fixed stage script and helper, while tests explicitly load
the hash-pinned repository helper. A missing staged helper has no fallback.

### Measured metadata-only result at `512ce77`

One reviewed invocation of sealed
`512ce77f228d7e6be9ec56be7962782d719d6357` completed with
`OBSERVED_ALPM_FILELIST_SHAPE` and supervisor `KNOWN_COMPLETED`, returncode 0.
The first current failure of the legacy marker predicate was package `base`
version `3-3`: its retained original `files` FD was a zero-byte regular file,
root UID/GID 0, mode 0644, one link, with empty-content SHA-256
`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.
The selected FD remained open through description reading and final identity/hash
verification. No file list, readelf execution, candidate execution or admission
was involved. This reproduces the old predicate failure in the current database;
the earlier `3d4b7d5` attempt did not record package identity and remains NONPASS,
not retrospectively diagnosed or repaired by this observation.

Canonical epoch and all eight preservation categories matched. Full IPv4/IPv6
network fields matched except confirmed decreasing address lifetimes. Independent
read-only quiescence found the exact probe/supervisor forms absent and canonical
PID/boot unchanged. The exclusive VM lease was returned with no retry or cleanup.
The complete private stage is retained as `t3-alpm-files-512ce77-observed.tar.gz`
in guest `/home/kdk_vm/.cache/` and host
`/home/kk/.cache/t3-real-resolved-build.XVxwu8AF/`, matching SHA-256
`c38c211826c04b4a9c3c05eeef99966f5ec02bfc61b7bf1162a15be696a3b62d`.
The host retains the separate `alpm-files-512ce77-invocation.log` preservation
receipt and `alpm-files-512ce77-quiescence.log`. These are metadata-only evidence,
not static closure, loaded ELF proof, compatibility or installed acceptance.

### Narrow empty-record source correction (not executed)

`package_index` now permits only UTF-8 empty/newline-only records to contribute
zero ownership edges, retaining the exact raw file-list hash. It does not strip
whitespace, CR or BOM; malformed UTF-8 and nonempty records follow the existing
refusal/section logic unchanged. The broader proposed parser is still unwired.
Tests cover empty-package owner refusal, mixed valid ownership, duplicate-owner
refusal, pre-tool malformed-record refusal, original count/total bounds and final
selected-package hash mutation. Manifest, candidate paths, fixed dependency
search, queued-edge identity and tool/ELF authority are unchanged.

All historical wrappers remain frozen, including `vm-guard-queued-edge.sh` with
its original measured `b5f9c965…` probe pin. **No wrapper currently authorizes the
corrected static probe.** A separate terminal-unknown-safe supervisor/wrapper and
exact-pin full review are required before any new capture; the historical GNU
timeout/continuation path must not be reused as new execution authority.

### New supervised static-capture proposal (source only)

`static_capture_supervisor.py` and `vm-guard-empty-record.sh` propose the fresh
fixed stage `t3-static-elf-empty-record-review-1`. They do not replace or modify
historical wrappers. The new wrapper pins the corrected probe, supervisor,
original containment, manifest and independently reviewed readelf bytes. Both
supervisor and probe imports are inert; actual mains require exact stage paths
and their exact staged helper, with no repository fallback. Tests explicitly
load their own frozen helper. Candidate ELF files remain data, never executed.

The supervisor uses only the original frozen OwnedProcess/raw-wait supervisor
for its one fixed Python capture child, with a 75-second bound. Result/stderr and
receipt files are exclusive creates; stage admission requires private ancestry,
UID/GID 1000 and mode 0700. The shell enables umask 077 and noclobber. Any failed,
timed-out or unknown supervision exits the wrapper immediately without result
reads, after-state queries, quiescence scans, retry or cleanup. Before evidence
is retained and after-state preservation is unproven on that branch. Only known
successful child completion proceeds to typed receipt and baseline checks.
The unchanged frozen supervisor's known anchored group cleanup is not a claim
that arbitrary descendants or separately sessioned children have been recovered.

Pure tests exercise actual fixed argv/input pin ordering, known success/nonzero,
unknown/timeout, no fallback, prelaunch pin/existing-output refusal and the exact
shell terminal branch. Historical evidence remains immutable. This proposal
still requires complete root/independent review and a separate exclusive VM
lease; source tests neither invoke readelf nor establish closure or admission.
