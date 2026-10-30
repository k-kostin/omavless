# Original-store preservation — selected 0.9.9 development contract

This is an unreleased native command, not a capability of public 0.9.8.
Issue [#169](https://github.com/k-kostin/omavless/issues/169) requires a reachable
way to retain incompatible original data. Strict profile validation remains
unchanged: ordinary editor, profile export and encrypted Backup may be unavailable
when any stored record is incompatible. Restore is not a universal cold-start
escape. The historical four XHTTP differences remain unsupported.

## Explicit entry and return

`omavless store-compatibility` gives a bounded credential-free report without
starting the application. A report exit of zero does not mean compatible.

The matching development application provides `omavless store-preserve-original`.
It reads one JSON **object** from non-terminal stdin, at most 8192 UTF-8 bytes,
with exactly:

- `destination`: an absolute new file name, at most 4096 bytes, in an existing
  private same-user directory outside managed config/state/runtime directories;
- `confirmUnencrypted`: the boolean `true`, explicitly acknowledging that the
  copy is **unencrypted and contains VPN access credentials**.

For example, the stdin object has the form below. Choose a real private directory
yourself; this synthetic example is not a command to run on the owner's data.

```json
{"destination":"/home/example/private/original-profiles.json","confirmUnencrypted":true}
```

Do not put the object or its destination into command arguments, shared reports,
clipboard automation or logs. No source path can be supplied. The source is only
the established current-user `~/.config/omavless/profiles.json` location (including
the existing test/developer `OMAVLESS_HOME` convention).
Interactive terminal stdin is refused before input is consumed or files are
touched. This avoids echoing the private destination at the native input entry.

The command saves the whole nonempty file as exact bytes, at most 5 MiB. It does
not parse, normalize, filter or repair JSON, UTF-8, URIs or metadata. On success
the new file is 0600; stdout contains only schema 1, `outcome: preserved` and
`unencrypted: true`. The original process exits zero back to the shell. The
source and application settings are not changed, and strict compatibility can
still refuse. Saving does not start the application, connect, change services,
restore the copy or provide an in-app repair workflow.

Exit 2 means input/admission/publication refusal. An existing destination is
never replaced, including a symlink. Exit 73 means the outcome is unknown: a
copy may already exist. Inspect the explicit destination privately; do not
automatically repeat, delete, overwrite or compensate. A killed process or
lost output is also not proof that no copy exists.

## Cold admission and file boundaries

No working runtime/committed native owner is needed. Stable Legacy (including
an absent ownership marker) and stable Rust ownership are allowed only under the
existing migration lock. The runtime base and state base must already exist and
be safe. A missing private `omavless` state child is permitted for preactivation
Legacy; no state directory or marker is created. Preparing ownership phases,
unsafe markers, an occupied lock and any pending private transaction refuse.
The command never reads or repairs pending transaction contents.

The ordinary shared-lock acquisition can create its fixed operational lock or
tighten the already admitted legacy lock mode. This is not a zero-filesystem-
interaction claim. All source, ownership, desired, template and startup files
remain untouched. No second store writer or ownership activation is introduced.

Existing no-follow ancestor/file traversal, held descriptors, same-user private
modes, single-link source and size/identity checks protect the snapshot. A
changing/unsafe/unreadable/oversized source refuses. The source and state fences
are checked again before and after destination publication. As with the shared
migration lock, this excludes cooperating writers; arbitrary hostile same-UID
replacement-and-restoration is not claimed to be prevented.

The source bytes stay in zeroizing memory while the descriptors are held.
A distinct raw-original type cannot become `SealedBackup`. Both typed callers
reuse a private unnamed-inode/no-clobber publisher; there is no named temporary
fallback. Data is synced/read back, linked exclusively and checked at the final
name before parent-directory sync. A failure after possible link is Unknown.
Encrypted Backup keeps its strict semantic validation and Rust-owner admission.

## Acceptance and stop boundary

Real CLI subprocess tests cover cold mixed/valid input, all four public XHTTP
cases, corrupt JSON/UTF-8, original bytes/modes, consent, limits, strict admission
remaining refused, no-clobber, unsafe files/ancestors, lock and pending ownership.
Filesystem publication controls distinguish changed-source refusal before link
from ambiguous post-link retention. These are source tests, not installed proof.

Before integration: affected source checks and independent Spec/Standards review,
new exact native artifacts/source receipts, coherent delivery pins/remover, and
one installed **synthetic** cold entry → refusal/guidance → preservation → shell
return with source/destination/settings checks. Never substitute the owner's
private store or replay closed Restore actors. Physical NIC/VPN/AUTO1/full R5/R6
acceptance is not needed for this local claim. All ordinary RC CI, merged-push
and fresh source/distribution admission remain mandatory.

Stop before a costly successor if this requires tolerant profiles, repairing the
working store, a new recovery journal/service, root API, Python fallback or an
automatic connection. Integrated repair and return to Connect would be a
different claim and require a separate scope decision.
