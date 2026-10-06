# Developer current Intent pause and same-held Abort

Source successor to the exact installed `9b7f33d9` checkpoint. This is one
explicit controlled positive interruption, not recovery from a revoked or
unknown-error engine. Default Backup/Restore dispatch stays unavailable.
Actual VM selection requires primary and independent affected-graph review.

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

Source controls must cover positive pause→OLD/Aborted→ordinary use, exact lease
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
The source test for lost publication uses a real failed Unix-stream write and
the same holder refusal consumer; it is not an actual installed daemon result.

No descriptor role or file-capacity envelope is added. The one bounded holder
adds an opaque Arc identity, one fixed-size original Intent record, and the
original scheduler revision/instance. It cannot hold a second pause or admit
another forward execution. Existing file slots, exclusive publication,
readback, catalogues, deadlines and namespace/resource predicates remain in
force. The synthetic Off-host filesystem controls do not themselves attest
genuine installed current-owner acceptance; that remains a separately selected
package/VM gate after affected-graph review.
