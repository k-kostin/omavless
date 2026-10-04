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
