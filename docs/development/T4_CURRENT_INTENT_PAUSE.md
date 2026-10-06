# Developer current Intent pause and same-held Abort

Source successor to the exact installed `9b7f33d9` checkpoint. This is one
explicit controlled positive interruption, not recovery from a revoked or
unknown-error engine. Default Backup/Restore dispatch stays unavailable.
The exact-source installed VM gate subsequently completed; see the
[Current36 evidence](../testing/T4_CURRENT_INTENT_VM_2026-10-07.md). That result
is controlled positive pause/Abort, not arbitrary uncertain-error recovery.

The fixed private pause entry uses the existing genuine current constructor,
instance/revision, peer, owner mutex, authenticated archive, original boundary
and owned migration lease. It publishes and reads back the original durable
Intent, rechecks OLD/staged originals and the final current origin, then yields
an opaque `IntentPaused` state inside that SAME installed holder. Forward entry
is consumed; ordinary operations remain Busy/Unavailable. This positive pause
does not call the irreversible error-revoke path or set an independent block
that would require a general readiness waiver.

The separately selected fixed Abort entry consumes that pause before its first
fallible operation. It checks the same owner/instance/revision, lease, boundary,
prepared plaintext, Intent and OLD named/held bytes. Because no live rename has
occurred, it must not fabricate MIXED state, recapture live paths or rename an
unchanged OLD pair. It exclusively publishes the exact matching Aborted record,
uses terminal-specific OLD retirement/disposition and moves the ONE original
owned lease through the pre-reserved ordinary destination. Its scheduler proof
is Aborted/OLD, not a fake NEW/Committed proof; completion advances once only
after all final gates and preserves historical replay.

No default advertisement, arbitrary phase/fault knob, shell/service/PID action,
decoded-record grant, alternate current factory or FreshRecovery substitution.
Any throw, late/unknown I/O, failed publication, drift or reentry while Resuming
keeps the original graph occupied, unavailable and irrevocably revoked. There
is no late reactivation, automatic Abort/cleanup or fatal-FD-survival claim.

Source controls cover positive pause→OLD/Aborted→ordinary use, exact lease
contention, second pause/forward/ordinary mutation while paused, wrong instance
and revision, resume consumption/replay refusal, source/terminal substitution,
before/after publication and final-gate cuts. Actual scope needs a preserved
fresh whole VM baseline with history absence, not deleting the earlier accepted
Current34 audit record. Ordinary current wrong-passphrase refusal is a separate
before-effect case: authentication precedes holder/lease installation.

The fixed private client commands are `developer pause-current-intent
--confirm-private-intent-pause` with the existing bounded schema-1 archive and
passphrase stdin, and `developer abort-current-intent
--confirm-private-intent-abort` with only `{"schema":1}` stdin. The latter obtains
the SAME server instance and revision from normal `system.hello`; it does not
reopen the archive or require normal ownership availability while Intent is
pending. The server still requires its genuine current-origin generation, exact
instance/revision, normal peer credential, quit guard and SAME owner mutex.
The pause response is `intentPaused: true`, never a fake completion. Abort
returns `completed: true` only after the separate OLD/Aborted disposition.

The private pause handler keeps that owner mutex through original response
encoding/write. An encoding/write failure consumes and revokes the SAME pause,
blocks the owner and retains its original graph. Successful write is not proof
of client receipt; a client transport error is UNKNOWN and never auto-retries.
The source test for lost publication uses the production private response
encoder/unary writer and same holder failure callback on a real failed Unix
stream. It does not forge a current factory or claim an installed daemon result.

The later diagnostic-only source successor emits the single feature-only stderr
literal `T4_CURRENT_INTENT_PUBLICATION_SEALED` only when that existing callback
actually consumed the SAME positive pause, revoked its engine/made it unavailable and
independently blocked the transaction at the exact revision. No new state reads,
RPC or continuation permission is added; only the fixed stderr diagnostic write.
That write is best-effort/nonpanicking: stderr failure cannot replace the
original publication error or unwind/poison the SAME owner mutex after sealing.
A nonmatching revision,
missing/poisoned holder, nonpaused request or repeated callback cannot emit it.
This new-image publication-loss scenario is **not yet VM accepted**; Current36
at immutable6fe remains only its controlled positive publication/Abort proof.
Client loss, unavailable hello and Intent presence alone cannot distinguish an
ordinary resumable pause from this sealed state. Without the exact source-closed
consequence, the result remains UNKNOWN with no Abort/forward/retry.

No descriptor role or file-capacity envelope is added. The one bounded holder
adds an opaque Arc identity, one fixed-size original Intent record, and the
original scheduler revision/instance. It cannot hold a second pause or admit
another forward execution. Existing file slots, exclusive publication,
readback, catalogues, deadlines and namespace/resource predicates remain in
force. The synthetic Off-host filesystem controls do not themselves attest
genuine installed current-owner acceptance; the subsequent Current36 package/VM
gate supplies its separately recorded scope, not a broader default activation.
