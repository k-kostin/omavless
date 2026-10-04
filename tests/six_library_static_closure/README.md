# Six recorded public candidates: bounded static closure

## Separate catalog diagnostic generation

This narrow successor starts at #640
`2906f8f1fb19fdb3815c7cd023b4696196cf61f0`. The parent's single #640
invocation returned NONPASS and its stage is stopped, not queried or reused.
The parent separately authorized a reviewed file-only observer; its filtered
public report SHA-256 is
`2863b938776163bfacdd1fd8fb25aa16262c575db01650595caa600ea37d15a0`.
That report recorded `catalog_names` as the last BEFORE/FAILED_AT phase and
an empty result. It does not prove the cause, baseline preservation or a fix.

The only new diagnostic behavior is a finite private catalog sub-boundary latch:
iterator open, next entry, entry-name access, type, count cap, component shape,
duplicate, iterator close and sorting. It performs no extra filesystem query.
On a catalog exception, the one existing terminal write additionally records
the literal sub-boundary and one exact-type whitelisted exception category.
Unknown exception types become `OtherBaseException`, never their class name,
message, repr, filename, entry names or raw catalog snapshots. These labels are
recorded text, not proof that an operation completed or that it caused failure.

No per-entry output multiplies the retained 4096-record/130816-byte BEFORE
budget. The terminal record remains at most 256 bytes and uses a single typed
full write, checked original source deadline before/after, and permanent
reported/sealed latch. An expired/unknown/short/throwing final write receives
no retry or further output. Deadline checks after the private latch precede
iterator creation, next-entry operation and entry-name access. The original
catalog grammar, 4096-name cap, package selection/membership, whole original-FD
catalog rechecks and six-plus-nineteen ELF scope are unchanged. In particular,
this proposal does not seek/reset the catalog FD or claim to fix its offset.

The fresh diagnostic stage below and all transitive source pins are new;
successful static receipt schema and negative adoption/compatibility flags are
unchanged. No old NONPASS becomes PASS. Full source gates and parent/independent
review precede any separately authorized fresh invocation; this author has no
VM lease. The retained parent proposal description follows.

Developer-only successor to #637
`d00dff5a506bf6df349c6e106b795b308b6e79ca`. That invocation remains NONPASS;
its scope is stopped and is neither queried nor reused by this source.
The separate file-only observer completed known zero according to the parent.
Its filtered public report SHA-256 is
`2e9ace2ffae52b7f3e3861a7adb9102a12d5c03d356e8673919816591f1dc846`.
It recorded 24 initial candidate rows, nine bus and fifteen resolved. Six
original-path candidates were outside the unchanged copied nineteen objects:

- `/usr/lib/libcrypto.so.3`
- `/usr/lib/libidn2.so.0.4.0`
- `/usr/lib/libssl.so.3`
- `/usr/lib/libunistring.so.5.2.1`
- `/usr/lib/libz.so.1.3.2`
- `/usr/lib/libzstd.so.1.5.7`

Those recorded map tuples are candidates, not admitted original identities.
They do not prove the cause of the failure, a loaded-object match or recovery.
The new static probe does not inspect a process, old namespace or old stage.

## Package grounding before candidate opens

The retained nineteen-object manifest is byte-for-byte unchanged, SHA-256
`b914dece6cb3c58f74bb4cdea8b19ade7b3b032e1b12d112d7524a2c47ca6c87`.
It has no package record pins for the six new candidates. None are invented.
With explicit parent approval, this generation therefore reads the names in
one original-FD root-owned `/var/lib/pacman/local` directory, capped at 4,096
entries. Every name must fit a finite ASCII component grammar. No unselected
package content, signature, ownership index or private path is opened.

Exactly one version directory must be selected for each of the five literal
names `openssl`, `libidn2`, `libunistring`, `zlib` and `zstd`. Versions
use a bounded pkgver-pkgrel grammar with an optional numeric epoch colon.
Unknown, ambiguous, missing or noncanonical names refuse. Only the selected
ten `desc`/`files` original records are then opened. Their hashes are
measured, not pre-accepted. Strict sections must bind exact NAME and VERSION
to the selected directory spelling and list the six exact canonical filenames.
OpenSSL must list both crypto and ssl. Duplicate/noncanonical file entries
or missing memberships refuse.

The whole name catalog, its original FD and ancestry, plus every selected
record's original bytes/metadata/path/xattrs, are rechecked before any newly
scoped ELF or readelf is opened, around each tool call and at the final
boundary. Catalog receipt output is only a count, canonical newline-separated
names digest and directory metadata; unrelated names/content are not exported.
This is a local package-record observation. It proves neither signatures nor
global package-owner uniqueness, and supplies no automatic admission.

## Static graph and terminal behavior

All six canonical paths are roots of the same finite recursive DT_NEEDED /
declared-interpreter closure. The only eligible targets are those six plus
the existing nineteen manifest objects. A seventh unknown target refuses
before that target open or another tool call. All six roots must be canonical
without symlinks. Reached known objects must match their reviewed original
metadata/hash exactly. Every alias, dependency and reachable record must
agree at final validation; surplus/unreachable records are rejected.

ELF bytes remain data. Only the retained original pinned readelf FD executes,
with the original target FD and the fixed read-only argument list. The
canonical resolver / readelf decoder and OwnedProcess/quarantine/limits are
reused from frozen source, not its generic capture, exercise, supervisor or
main. The new owned adapter strictly types raw WNOWAIT pid/code/status before
accepting exact zero and permits one exact typed zero reap. Nonzero, signal,
timeout or unknown permanently blocks requery, reap, retry, cancellation,
signals and output reads for that child. No loader or candidate is executed.

Readelf output scratch files use only the fresh fixed stage's private scratch,
not a global /tmp fallback. The supervisor explicitly carries that same fixed
TMPDIR into the probe's environment. The source deadline is checked after the
last pre-readelf event and again after private scratch opens, immediately
before the owned tool spawn. Inert execution controls cover this environment
handoff and delayed event/scratch-open refusal, not just source substrings.
FD unwinding is not process cleanup or recovery.
Final report writes require exact integer full-length completion, successful
flush and a checked post-flush deadline; success and uncertainty seal the
original scope permanently.

Bounds are 36 original regular-file FDs (ten records, tool, at most 25 reached
ELFs), plus retained ancestors within the inherited FD ceiling of 128;
4 MiB per package record, 32 MiB per ELF and 128 MiB aggregate retained bytes;
128 queue steps/aliases, depth eight, at most 64 declarations per object;
at most 25 readelf calls with a five-second zero-only ownership deadline each.
The whole source deadline is 120 seconds, outer deadline 140 seconds.
Checked deadlines are not cancellation of a blocked syscall. The event budget
is 4,096 finite records / 130,816 bytes; exhaustion refuses, even if a theoretical
maximum graph would need more. Result bytes are at most 262,144.

## Fresh scope and acceptance boundary

Fixed new stage:
`/home/kdk_vm/.cache/t3-six-library-catalog-diagnostic-review-1`.
Create-only transfer refuses reuse. The same-invocation wrapper keeps the
canonical runtime/private-file/service/executable/namespace/core/TUN/resolver
and network comparisons; only the retained address lifetime countdown
allowance remains. Failure and invalid-receipt branches stop before all after
queries. No previous NONPASS is changed.

The acyclic source pins are owned → probe → supervisor → wrapper → transport;
validator is separately pinned by wrapper and transport. The reused manifest,
containment and helper hashes are exact. Inert controls cover catalog bounds,
ambiguity/version/membership, original-FD mutation/FIFO/link/xattr refusals,
package-before-ELF ordering, complete six-root closure, forbidden new edges,
strict receipt/type/flag checks, terminal ownership and actual shell failure
branches, final-write uncertainty and create-only transfer.

Full parent and independent source review and sealed gates are required before
one separately authorized fresh static VM invocation. A successful receipt
would prove only this bounded static observation and same-invocation baseline.
All four flags remain false: candidate execution, allowlist adoption, loaded
ELF identity proof and compatibility acceptance. Measured hashes may ground a
later separately reviewed proposal, not this source's copy or live admission.
