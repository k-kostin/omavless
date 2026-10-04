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
