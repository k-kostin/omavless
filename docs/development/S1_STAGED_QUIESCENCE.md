# S1 desktop-only staged quiescence candidate

Inactive successor to [mode-last ordering](S1_MODE_LAST_ORDERING.md), 2026-10-03.
This closes a journal representation gap, not installed activation. App proxy
remains unavailable. No ordinary executor accepts the staged journal, and no
production host constructor, helper, IPC/CLI/UI capability or package is added.
Shared backend/runtime contracts, existing v2/v3 interpretation and the frozen
Python archive are unchanged.
The [exact-source private evidence](../testing/S1_STAGED_PRIVATE_2026-10-03.md)
records the completed declared gates separately from unrun installed admission.
The separate [Hyprland default-consumer probe](../testing/S1_GIO_HYPRLAND_2026-10-03.md)
records a demonstrated applicability limitation, not a passing transaction gate.

## Fixed staged plan

The model permits saved effective `none`, `manual` or `auto` (PAC), including
absent overrides whose default supplies that mode. Intended mode must be an
explicit manual override. All defaults, locks and layered consistency checks
remain mandatory. All ten manager fields must remain exactly unchanged: desktop
mode cannot quiesce inherited environment consumers.

Apply: explicit user `none` → settled readback → 15 fixed non-mode desktop
controls → intended manual mode last. Compensation: explicit user `none` →
settled readback → saved controls in reverse → saved layered mode last. An
already equal explicit-none value needs no setter, but the durable stage still
advances only against the complete expected observation. An absent override is
not equal to explicit none and is restored exactly at the final mode operation.

The repeated mode effects have distinct fixed identities: `QuiesceApply`,
`Enable`, `QuiesceRestore`, `RestoreMode`. Saved and intended manual values may
be equal; this does not collapse the intervening quiescence or erase the two
activation steps. No control effect is prepared outside a quiescent mode stage.
No final saved mode is prepared before every non-mode control matches its saved
original. Full readbacks include the unchanged manager fields.

## Separate strict record and recovery

`app_proxy::staged` is effect-free. `journal::staged::StagedJournal` uses the
fixed private `app-proxy-staged` child and a separate strict version-1 format.
It cannot open or reinterpret records from the v1 two-surface or v2/v3 field
directories. No automatic migration, initialization from current state,
staging cleanup or tombstone deletion occurs.

The record stores complete private original/intended snapshots, canonical
control-prefix sides, an explicit original/quiescent/intended mode role, one
stage and one optional fixed pending step. Quiescence is a derived explicit
user-none value with unchanged default/lock, never a generic accepted third
value. The staged validator checks phase/role, prefix, no-op and pending-step
reachability. Wrong versions, unknown/duplicate fields, invalid control keys,
inconsistent layers/defaults/locks and redirected storage refuse.

Pending reconciliation accepts only the exact recorded complete pre-effect
state or that one pending effect's complete post-state. Confirmed control resets
and unrelated changes are foreign; there is no blanket original/intended/none
mixture allowance. Intent publication, confirmation and compensation use the
existing create-only staging/file-fsync/rename/directory-fsync storage boundary;
any storage error poisons the in-memory handle before another effect can escape.

Reopened handles refuse applying/confirmation. Pure `begin_restore` still
requires the trusted caller to establish retained-origin drain first: its state
comparison is not that proof. There is deliberately no installed staged executor
or fresh-port admission constructor. Read-only review checks exact record bytes
and interrupted staging and emits only stage, fixed pending ID, bounded count
and before/after/foreign relationship. Before and after a dead writer's delayed
commit it retains unsettled evidence, even when the result equals the saved
original. A released original retains its tombstone; visible foreign edits retain
both external values and journal evidence.

## Real private fixture guarantee and limits

Only the opt-in GIO crate's test module contains an executing staged driver. It
uses the same fixture-created bus/service/profile/database and internal dconf
fixed-key protocol as the [earlier writer experiment](S1_FIXED_TRANSACTION_WRITER.md).
Durable intent precedes a repeated full pre-effect check. The driver fences before
every operation, drains the same retained connection/unique owner, independently
reads persisted state and durably confirms before advancing. Same-port reentry
drains before compensation; an unknown request cannot admit a later field.

Within that declared fixture, the owned writer's control writes follow settled
explicit-none mode, and saved Manual/PAC reactivation follows complete saved
controls. This is an ordering/representation guarantee, not compare-and-swap
between desktop keys or protection from concurrent identical-value edits/ABA.
An external writer can race the final check; matching values cannot identify
their author. Detected mode/control/default/lock drift preserves foreign values.
Applications may cache older settings, and manager/environment consumption is
not covered. These limitations remain production admission/conflict gates.

Pure tests cover six layered baselines, every apply/restore prefix, both outcomes
of every pending field, all four mode identities and exact original restoration.
Journal tests cover every recorded intent, strict decoding, separate directories,
foreign records, fsync poison and actual child exit at all five storage checkpoints
for all four mode intents. Private dconf tests cover all 54 confirmed apply
prefixes across None/Manual/PAC, initial and every nonterminal confirmed restore
prefix plus reopened Released tombstones, timed-out mode operations and dead-writer
delayed mode operations, including absent reset. Independent persisted baseline
checks precede coverage; seed setters and sync do not establish that proof.
The crash child receives only a fixed private synthetic target file; that file is
not installed authority. Unattempted external none is explicitly foreign.

No actual desktop proxy, manager environment, host bus/session/services, private
profiles/credentials, VPN/TUN or route state is read or changed. PAC URLs are
synthetic and never fetched. Only fixture-owned children are stopped/resumed/
reaped. No VM control, main/RC merge, release or marketplace action.

Installed AUTH-writer/session/lifetime provenance, target continuity, a reviewed
installed write/drain API, owner/revision-bound loopback/TUN-disabled listener,
new-app UWSM consumption and conflict escape remain unclosed. Cross-owner crash
takeover, filesystem power-loss, ARM64/VM and NixOS acceptance are unrun, not PASS.

The installed default GIO resolver under the observed Hyprland desktop selectors
was `GLibproxyResolver`. Despite fresh persisted Manual readback with a synthetic
loopback HTTP endpoint and empty bypass, it selected only `direct://`; the private
probe refused before any application request and restored the absent overrides.
Installed GNOME schemas/modules alone therefore cannot admit this desktop surface
for that default consumer. A later applicability gate must establish the actual
supported consumer/session semantics and real listener traffic without resolver
forcing or desktop spoofing. A separately reviewed environment/application strategy
would need its own provenance and restoration gates; the staged desktop journal
does not authorize it or acquire new consumer acceptance from this probe.
