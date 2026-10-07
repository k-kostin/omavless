# Private first-restore Abort boundary: implementation review

Base: #591, `840da93acb63bda73c0d8abba9ac17ecfade87b6`.

This slice is under source review. A private recovery-only composition is present;
no normal constructor, dispatcher, CLI or product operation is connected. Existing independently
blocked coordinators remain blocked; ordinary startup still rejects evidence.

## Lower retained-journal prerequisite

The exclusive journal writer now retains its actual created descriptor and
parent, checks exact identity and record bytes before return, and provides the
old unit-returning API through `map(drop)`. A distinct private `CreatedAbort`
can be minted only from an Abort record. It is neither cloneable nor a reopened
terminal classification.

The new inactive Intent-only lower Abort primitive shares existing replacement,
pair verification and journal synchronization. It refuses both terminal phases
before its host callback. After publishing Abort it retains the original created
record through every subsequent callback and final read, then moves that value
to its caller. A same-byte terminal substituted after publication is refused,
not adopted. Its private recovery caller supplies authentication and source pins.
Existing Abort re-entry uses
separate read-only retained evidence, never construct `CreatedAbort`.

The lower primitive does not itself authenticate an archive or synchronize all
staged source members before rollback; the recovery-only composition does so.
New private strict helper variants
gate individual replacement and live/journal synchronization effects; existing
callers retain their original grouped wrappers. Terminal publication repeats the
common owner/stage gate while the original exclusively created inode is empty,
and advances expected metadata only after its own write, before callbacks.
Original identity is rechecked around each create/write/file-sync/parent-sync
boundary; a failed callback stops before the next effect and preserves the prefix.

## Inactive recovery-only composition

The private entry derives UID and current runtime/state/desired/host
paths internally. It uses `ObservationOnlyNativeHost`, including its preserved
drop paths, without ordinary owner construction, startup reconciliation, probe
orphan cleanup, login consume or historical admission. Only a test-only entry
may supply a synthetic host/path bundle. Private transfer/passphrase and user
confirmation remain separate product decisions.

Freshly authenticate the archive before effects, acquire one existing migration
lease, and hold both through final return. Match every staged NEW byte to the
authenticated startup-disabled store and template. Retain exact staged OLD,
ready record and Intent bindings. Equivalent authenticated payloads are
equivalent; existing records do not identify the original ciphertext file.
The archive authenticates NEW, not OLD. OLD has private transaction/checksum
provenance, not cryptographic protection against a hostile same-user writer.

Recovery cannot retain the earlier process's staging descriptors. Pin a separate
recovered-evidence type before the first external callback: original-to-this-
invocation stage members, ready/Intent, existing Abort if any, owner/desired/
login members, fixed private directories and lease. Never call a reopened stage
`CreatedStage`, or rebaseline a replaced source after a callback.

Repeat exact same-lease owner-generation, explicit Off desired, login identity,
fresh empty-owned-host and source checks before/after effects. Missing locks,
connected/uncertain host, unsafe/torn/crossed evidence, unrelated routing/
successor/closure/disposition/finalization state and foreign replacement slots
refuse. Existing receipt checks are not current-manager epoch proof; no new
receipt or historical exception is added. Foreign visibility grants no stop
authority.

Synchronize pinned staged OLD/NEW/ready/Intent and applicable directories before
rollback, with immutable source and host rechecks. Commit terminals refuse.
An admitted existing Abort may only be reverified/resynchronized through its
separate retained read-only identity. Intent may replace only exact classified
OLD/mixed/NEW members toward OLD, then publish a newly owned Abort. Final success
requires exact OLD, matching Abort and retained boundary readback; its only
result is `AbortedStillFenced`. Preserve all artifacts on success and ambiguity.
No automatic retry, rollback reversal, cleanup or restored ordinary authority.

A distinct non-Clone `RetainedPair` pins both live files and every optional OLD/
NEW slot before the first host callback. The owner keeps it through final return;
the strict lower entry requires a loan of that same proof. An owned link binds
the original O_TMPFILE descriptor before callbacks; an existing slot is never
recaptured. An owned rename transfers that descriptor from slot to live with an
explicit ctime transition before callbacks. Every callback rechecks all remaining
live/slot identities; final success cannot adopt a same-byte live replacement.

## Source verification and remaining acceptance

Source guards pin fixed private constructor provenance and ordinary fences.
Nine owner fixtures cover OLD/mixed/NEW Intent, existing Abort, Commit refusal,
wrong archive, missing lease, foreign slots, invalid login, connected desired,
original stage/owner/desired swaps, every original-source sync boundary, late
fences/host loss, and live/slot swaps at every reachable host callback including
final readback. Lower fixtures additionally inject swaps at link/rename/terminal
hooks and empty/full terminal write/sync boundaries. These are synthetic local
checks, not positive installed execution of the fixed-current constructor.
Process death/re-entry and power-loss acceptance remain separate.

The next [fixed-current process-loss fixture](T4_FIRST_ABORT_PROCESS_REENTRY.md)
is explicitly ignored pending review and separate execution authorization;
its presence does not turn process-death or installed acceptance green.

The first strict-helper test run preserved an earlier refusal: terminal swap was
detected before its old fixture expected the Terminal hook. The corrected fixture
deliberately waits for full terminal bytes and requires hook reach only for the
hook-injection case. Initial local compiler diagnostics are retained privately.
The original #591 `8e3e74c` static-source failure remains NONPASS; its test-only
chain correction and successful exact-head gates belong to `840da93`, not here.

Final exact-head gates and root review of the complete composition remain required.
Product transfer/confirmation, format and
template compatibility, installed restore/recovery/upgrade/downgrade and normal
registration remain open. No VM, main/RC merge or release is performed here.
