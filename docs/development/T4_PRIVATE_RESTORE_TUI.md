# Experimental preview-bound Restore through TUI

Status: source-ready for the next explicit installed slot, not installed
acceptance or default activation. This
bounded cycle depends on Backup assembly #707 at
`8de29a164e885f398dbb926c69fd377af4764869` and its exact opt-in API/Backup UI.
It composes the [authenticated preview](T4_PRIVATE_RESTORE_PREVIEW.md),
[normal private-pair API](T4_PRIVATE_PAIR_NORMAL_API.md) and
[private backup contract](../roadmap/PRIVATE_BACKUP_RESTORE.md). It does not close
whole T4 or add connected Restore, OS recovery, K1, App proxy or a GUI.

## User decision and real entry

The user wants to replace current profiles/subscriptions and the supported
routing template with an authenticated archive. The primary action first asks
the runtime for Preview; a separate explicit confirmation requests Restore.
The target is the complete displayed absolute archive path. Navigation/Cancel
before Restore is not a private-pair mutation. Preview counts never reveal
profile names, URLs or keys, and are comparison DATA, not execution permission.

The ordinary package and `tui` entry remain unchanged. The explicit feature
build is defaults plus `t4-manager-actor-service`; the new client entry is
`tui --developer-private-restore`. It retains Backup and its separate original
`--developer-private-backup` entry. Settings offers the Restore workspace only
with the explicit adapter, fresh parsed owned Off context and both exact
preview/preview-bound methods. Settings uses `R`; Backup keeps `b`.

## Private input and confirmation

The editor accepts an absolute normalized archive path up to 160 UTF-8 bytes and
a masked 12–1024-byte passphrase. Controls/bidirectional override input refuse.
There is no shell, clipboard, file browser, public raw method selector or secret
argument/environment/log. Owned secret buffers preallocate their ceilings and
zeroize removed suffixes; requests/completions move, never derive Clone/Debug.
JSON/terminal-library temporary allocations are not all guaranteed wiped.

The original parsed instance/revision is captured on entry and never rebased.
One asynchronous Preview returns authenticated bounded profile/subscription
counts, exact privatePair scope and the SAME ciphertext digest. The confirmation
warns that profiles/subscriptions and the finite supported routing template are
replaced, **not all Settings/OS/runtime state**. Changed metadata invalidates
the editor/confirmation. Cancel wipes client credentials and cannot submit.

Preview has no mutation ID, reservation or history effect. Closing its screen
cannot cancel a running authentication on the server; an abandoned/late preview
reply never enables Restore. A new editor has a distinct nonescaping local
correlation nonce; completion from an old editor cannot target it.

Confirmation creates one operation ID and moves one immutable request including
the expected ciphertext digest and original instance/revision. The backend
re-earns current-origin/Off/idle/keeper/history/capacity guards. It authenticates
and hashes the same bounded guarded read, comparing ciphertext before held-slot,
lease, staging or live/history effects. Preview does not grant entry or relax
the existing original-lease, source-drift or sticky UNKNOWN guarantees.

## Pending and uncertainty

A capacity-one Restore worker uses only the two closed typed request variants.
Each request has its original absolute 120-second deadline; credentials, peer,
frame and request ID checks reuse the existing Backup transport. There is one
nonblocking socket connect and one framed exchange, no retries, hello refresh,
new operation ID or cancellation request after a possible write.

Restore success requires the original reply to report completed/privatePair,
not replayed, and exactly one revision advance. Allowlisted known refusals show
Denied. Missing, malformed, lost, late, replayed or indeterminate result remains
Unknown. Result delivery cannot outrun the original deadline. Submitted/Unknown
survive screen navigation/reentry and block Backup, ordinary mutations, jobs and
Quit; closing the TUI itself does not disconnect or cancel backend work. A lost
read-only observation during a pending mutation is not proof of another daemon.

## Source checks and evidence limits

The initial integration `c49eb531bd8da2417e9d62914a94c70bdd1de35a` preserves the
preview source `6164e7f564ae42422eb98d91949f77e4cb08297a`. Independent Astra
implementation review of that changed backend boundary found no reachable
blocker (SOURCE CLEAR only). Eleven no-default opt-in preview controls passed.
The new client/transport received separate final checks/review below; the
initial preview-only results are not substituted for that changed boundary.

### Final source freeze

Tested implementation is exact
`5130b18f783b025ccc09eaaa76408ff874db2d00`. Independent Astra SOURCE review
cleared that frozen head after fixing two actual delivery/custody findings:
cancelled/expired Preview now retains its original worker fence until exact
completion/disconnect; known accepted other-instance metadata is applied before
accepting the old response. An unavailable snapshot alone retains Pending.
Foreign completion cannot drain the fence; a disconnected adapter is disabled
without backend cleanup. The fixes have actual app ordering/reentry regressions.

- Full `./tests/run-rust.sh` completed with original exit0 on that exact head,
  including default/workspace/DNS/terminal/strict-lint gates and the new actual
  feature combinations. Final product+T4 runtime library: 1491 PASS, 75 ignored,
  one separately handled helper filtered. Ignored resource gates are not PASS.
- Opt-in TUI184 and default TUI148 PASS; module9/app10 are included, not extra
  counts. Fixed Restore transport3, unchanged Backup transport3, preview11 and
  matching-envelope client/engine readback controls PASS. Strict all-target
  TUI default/feature lint and runtime combinations, fmt/diff/shell syntax PASS.
- `./tests/run.sh` original exit0:684 cases/2 opt-in skipped, plus Node/QML/
  catalogs/navigation checks. The final run used existing test-only
  `OMAVLESS_TEST_ROOT` and `TMPDIR` in a private verified runtime temporary
  directory. Earlier HOME/tmp `.git` and protected-ancestry fixture failures
  remain failures; no marker deletion, guard weakening, HOME override or claim
  of source-sandbox/VM acceptance. Rust caches/TMP remained under HOME.
- Source packaging40 cases:39 PASS/one root-only skip. No new installation or
  compatible Restore bundle is inferred from those tests or old beta artifacts.
- Twenty final EN/RU TestBackend captures were reviewed: editing, confirmation,
  maximum160-byte path, Completed/Denied/Unknown, Settings top/end,
  cancelled-preview waiting and narrow50x14. Secrets masked, full target and
  primary/return controls visible70x24; captions distinguish archive counts,
  historic completion and local cancellation from current state/backend abort.
  Images remain outside Git. This is not installed rendering evidence.

The later documentation successor only records these original results; runtime
evidence remains attached to5130b18f. Draft #708 targets #707 as a real
dependency. Before beta integration preserve the subsequent #707 Backup-only
Settings footer correction rather than replacing it with the older base.

Focused source controls cover default/adapter/capability absence, bounded masked
input, exact digest/instance/revision, Cancel/stale/wrong-key/different-envelope,
delivery deadlines, hidden/reentry/no-resend and Backup/close/Quit coexistence.
Real local credential/framing negative tests reject false-current factory;
positive synthetic client-to-retained-engine composition independently reads
pair bytes/projection, revision, history and replay after completion. It is
**not** genuine-current installed-positive evidence or a restart acceptance.

Synthetic EN/RU TestBackend screens at normal/constrained sizes are separate
from installed rendering. No VM access is authorized in this source cycle:
the neighboring beta.3 agent remains sole guest operator. Default activation,
combined installed/owner/ARM acceptance and normal restart remain UNRUN.

## Short installed card after explicit VM handoff

1. Obtain sole VM custody. Inspect one exact-source compatible bundle; never
   reuse Product enrollment from another binary or mix default/experimental
   variants. Use a disposable beta VM and synthetic private pair, owned Off.
2. Snapshot private-pair bytes/projection, revision and incumbent audit. Open
   the explicit Restore TUI, Settings → R. Verify EN/RU masking/focus and narrow
   layout. Preview an authenticated test archive; review counts/scope/warning.
3. Cancel before Submit: pair/Desired/revision/audit unchanged. Try a wrong key:
   unavailable preview, no Restore. A changed authenticated envelope or metadata
   must refuse the original confirmation, not retarget it.
4. Confirm one synthetic Restore. Preserve its original outcome and perform
   independent file/projection/revision/history readback. Lost/uncertain outcome
   is a stop, never a reason to click again or clear markers.
5. After known completion, verify historical replay/fresh-ID refusal using the
   existing fixed typed control, no new grant. Separately normal stop/start and
   readiness/readback on a fresh instance; retain original outcomes. Do not
   infer restart safety from a positive TUI label or existing source test.

No main/RC merge, version change, release, marketplace submission or historical
branch cleanup is part of this deliverable.
