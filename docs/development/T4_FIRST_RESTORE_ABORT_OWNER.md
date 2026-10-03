# Private first-restore Abort boundary: implementation review

Base: #591, `840da93acb63bda73c0d8abba9ac17ecfade87b6`.

This slice is under source review. No new recovery owner, normal constructor,
dispatcher, CLI or product operation is connected. Existing independently
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
not adopted. It has no owner-level caller yet. Existing Abort re-entry must use
separate read-only retained evidence, never construct `CreatedAbort`.

The lower primitive does not itself authenticate an archive or synchronize all
staged source members before rollback. Its existing helper checkpoints group
some effects. These obligations must be resolved explicitly before claiming
the proposed recovery-boundary contract below.

## Required recovery-only composition

The proposed private entry derives UID and current runtime/state/desired/host
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

## Verification still required

Source guards must pin fixed constructor provenance, authentication/lifetime,
existing lease and conservative ordinary fences. Actual synthetic fixtures must
cover OLD/mixed/NEW Intent, existing Abort, Commit refusal, wrong archive,
unsafe/torn stage/journal/slots, lease/owner/desired/login drift, late fences,
same-byte substitutions before/after synchronization and host loss surrounding
the first real rollback replacement. Process death/re-entry is separate from
power-loss and installed acceptance.

Root review of the complete lower diff and constructor is required before new
owner wiring is treated as checked. Product transfer/confirmation, format and
template compatibility, installed restore/recovery/upgrade/downgrade and normal
registration remain open. No VM, main/RC merge or release is performed here.
