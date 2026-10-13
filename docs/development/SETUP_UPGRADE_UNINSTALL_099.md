# Existing-install upgrade and accessible removal

Status: owner-requested 0.9.9 SOURCE design, 2026-10-13. Not implemented,
installed acceptance, an updater release or permission to mutate the host.
Source baseline: `b4ef783e14676099b9d6429304c57542ff948804`.
The current setup failure-stage correction is separate work; this contract
does not replace its writer or change its runtime behavior.

## Observed gap and owning boundaries

Updating the Omarchy checkout does not update installed Arch packages. A
committed Rust 0.8.2 application with the 0.9.8 frontend and no DNS companion
currently reaches `needs_attention`, with no guided upgrade action. This is a
real existing-user dead end, not proof that package replacement is unsafe in
every such installation.

`setup_status` requires the exact current app package before inspecting its
owner. `install_package` and `install` intentionally require the fresh boundary.
`SetupState.missingAction` also refuses a missing companion. Preserve those
fresh-install protections; implement a distinct, explicit update action.

The [managed DNS distribution contract](DNS_RELEASE_DISTRIBUTION.md) already
requires existing-0.8.x migration and guarded removal. Its current fresh-use
sequence is not the sequence for an already committed Rust owner. The
[native guide](../user/NATIVE_INSTALL.md) documents the manual update boundary,
but its historical one-app command is not a complete managed-pair updater.
[Execution policy](EXECUTION_POLICY.md), [host authorization](../testing/HOST_AUTHORIZATION_ACCEPTANCE.md)
and the [UI contract](../roadmap/UI_UX_CONTRACT.md) continue to apply.

## Exact first supported predecessor

Select public `v0.8.2`, application package `omavless 0.8.2-1`, native producer
`22e23e64c49b8110088b6be3f063b5e641ba0853`. These archive hashes are recorded
in that tag's `plugin/runtime-release.json`:

| Architecture | Application archive SHA-256 |
| --- | --- |
| x86_64 | `381a1427b2728f35bfd49247e4373238deb4ef53c5850eec2ad68e91de382b6c` |
| aarch64 | `ef53c795a62825b5bd1b928663c751b2d66f5c2da711fd5dc4542e367b7bbf57` |

This predecessor used separately installed Mihomo and no managed DNS package.
Its stable build identity has schema 3 and a recorded binary digest. A pacman
version label or `plugin target = rust` alone is not installed-byte provenance.
Bounded installed-file/receipt and package ownership checks must establish the
selected predecessor, compatible private state and committed Rust ownership.
Do not infer that the owner's currently installed binary has passed these
checks from the source package pins above.

The current proposed destination is the reviewed `v0.9.8-fix.3` app/DNS pair,
producer `8d02f9d7dd1a9063a82e9dcd93cb19cf4c5d8524`; use the frontend's
actual architecture-specific `runtime-release.json` and `dns-release.json` pins.
A later 0.9.9 destination must select its own inspected, immutable pair.
Do not copy these destination pins into a future installer by assumption.
Other versions, development builds, experimental enrollments and modified
receipts require a separately defined compatibility route, not this offer.

## Explicit upgrade, not another activation

The user goal is to retain existing profiles and ownership while installing
the components required by the updated frontend. Show **Update required
components**, its installed/required versions and its exact scope. Opening this
screen is navigation. Only separate `UPGRADE` consent admits a transaction.
Unknown discovery shows a concrete bounded explanation and a recheck/help path,
not invented missing packages or an enabled installer.

1. Discover installed app/core/companion identities and predecessor capabilities
   without sudo or effects. Do not probe an old binary with unsupported
   `dns-pair` commands and reinterpret its refusal as missing enrollment.
   Never advertise Ready from presence/version alone; same-version corrected
   deliveries also require source/digest inspection.
2. Before replacement, qualify compatible private files, template and committed
   Rust ownership through reviewed native readers. Preserve private backups and
   record startup preferences/unit enablement privately. Require known
   disconnected state, settled authorizations, a stopped runtime and no pending
   operation. Explain an active connection and offer the separately confirmed
   existing shutdown path; opening Update must not disconnect automatically.
   Prevent concurrent startup through the reviewed lifecycle boundary, without
   silently rewriting saved preferences or enabling units on completion.
3. Download both fixed, same-source packages and validate their hashes,
   architecture, version, app identity and DNS receipt before effect. Use normal
   `sudo pacman -U` for both packages in one transaction. Recheck admission under
   the existing per-user setup exclusion immediately before submission; a shell
   readback is not a transaction-wide ownership lock. Package guards remain the
   authority for privileged replacement. No override flags, fallback core,
   background sudo or automatic rollback to an incompatible old app.
4. For a previously absent companion, install its dormant unit first. First
   enrollment requires its actual root empty-state admission and separate `DNS`
   consent for this desktop UID. This is first broker enrollment, **not** repeat
   application activation or recovery of an unknown old enrollment.
   If a compatible release enrollment/selector already exists, preserve it and
   use the existing guarded start/selection route without re-enrollment. If an
   existing companion must be replaced, require its guarded clean stop/empty
   boundary; a live broker, retained socket, journal or FD store blocks pacman.
   Never unlink such state merely to make the update pass.
5. With the runtime stopped, use the reviewed template/selection consumers.
   `prepare-template` currently accepts only an already managed policy or the
   exact bundled default, creates a private original-template backup and refuses
   custom/partial YAML. The 0.8.2 and current bundled defaults match in source,
   but this proves nothing about a user's actual routing template. Detect an
   unsupported template before replacement where possible and provide a bounded
   review path; never reset it to default or claim a universal upgrade.
   `select` is create-only: an existing selector must be validated and preserved,
   not submitted to `select` again.
6. Verify the installed pair, store preservation, unchanged committed owner and
   actual broker readiness. Do **not** run `setup initialize` or `cutover activate`
   for `target = rust`. Offer explicit **Start OmaVLESS** only after native
   readiness admission; no Connect and no automatic startup enablement.

If a later phase fails, report that phase and the known completed effects.
Cancel-before-submit, refusal, completed and unknown are distinct outcomes.
Never claim “no changes” after a successful package phase or resend a submitted
enrollment/selection after a lost reply. A current refusal still needs a useful
next step; safe refusal alone does not close the user workflow.

Implementation gaps remain: runtime-independent upgrade inventory/action,
bounded predecessor qualification and pre-replacement compatibility checks,
single-writer admission, partial-result handling, and installed upgrade evidence.
This document grants none of the effect authority those implementations need.

### Minimal native upgrade-readiness boundary (design only)

Propose a fixed zero-argument semantic `app upgrade-readiness` command in the
qualified candidate, backed by one native inspector. No raw JSON method,
caller-supplied path/UID/unit/command or daemon snapshot is required. The old
0.8.2 binary and current public 0.9.8 do **not** implement this proposed command;
do not invoke it on the predecessor or assume its availability from destination
version. The reviewed delivery must supply the inspector before advertising an
executable updater. Source design is not permission to download/run an arbitrary
helper or treat a staged candidate as installed package ownership.

The smallest proposed internal API has two distinct outputs:

```rust
// Proposed signatures, not existing implementations.
pub fn inspect_upgrade_readiness() -> UpgradeReadiness;
fn inspect_existing_scope(exclusion: &ExistingUpgradeExclusion)
    -> Result<UpgradeSnapshot, UpgradeBlocker>;
```

`UpgradeSnapshot` keeps private captured inputs and original file identities
inside Rust; no Debug/Clone/Serialize or caller-constructed authority.
`ExistingUpgradeExclusion` borrows only actual validated existing locks for this
invocation. The CLI emits a bounded fixed projection, for example:

```json
{"schemaVersion":1,"scope":"stopped_native_upgrade_data","status":"compatible","reason":"none"}
```

Other fixed reasons include `runtime_active`, `operation_busy`,
`owner_exclusion_unavailable`, `ownership_unverified`, `desired_unavailable`,
`desired_connected`, `startup_requires_off`, `private_state_incompatible`,
`template_requires_review`, `pending_recovery` and `state_changed`.
`compatible` means a coherent compatible stopped-owner snapshot was observed
while its local exclusions were held, **not** permission to run pacman, proof
of root broker emptiness, readiness to connect or persistent exclusion after
the command returns. Unknown and unsupported facts remain a refusal, not Off.

Reuse the read-existing part of `runtime_relaunch::Installed::admit`, not its
whole Start admission:

1. Bind real/effective non-root UID, canonical same-user HOME/state/runtime
   roots and the supported predecessor's installed identity. Reject test-only
   `OMAVLESS_HOME` and inconsistent manager roots; allow no roots in argv.
   Candidate qualification and predecessor qualification are distinct: do not
   require the staged candidate's inode to equal `/usr/bin/omavless`.
2. Inspect the fixed runtime and login units' actual loaded state, jobs, PIDs,
   fragment/drop-ins and enablement against the selected predecessor; no
   `daemon-reload`, enable/disable, login preparation or service start. Acquire
   the existing daemon owner lock with read-only no-follow/nonblocking open and
   flock, and `MigrationLock::acquire_existing` in a consistent bounded order.
   Recheck pinned directory/name/inode/UID/mode/link identities. A busy lease
   refuses immediately. Missing locks/runtime directories have a separate
   exclusion-unavailable outcome; never create them or start the old runtime
   to make this diagnostic pass. The updater's cold-absence admission is a
   separate implementation decision, not inferred from this command.
3. Read existing ownership marker and frontend selector coherently; require
   committed Rust with their matching generation. Refuse pending private
   transactions, ambiguous transitions and unknown existing entries. Never
   consume a login receipt, recover a journal or advance owner generation.
4. Capture bounded private store, Desired and routing-template inputs under
   the same exclusions using validated no-follow descriptors and before/after
   identity checks. Strictly require an existing valid Desired file and
   `connected = false`; the generic reader's missing-file default is not an
   upgrade proof. Parse startup preference and require the declared Off
   condition without changing it. Use canonical store/template parsing and
   pure supported-template classification, not shell profile JSON, generated
   configuration output, core execution or network validation.
5. Recheck host state, exclusions, marker/selector and captured named-file
   identities/bytes before producing the projection. A change refuses. Drop
   the diagnostic leases normally, retaining no serialized ticket or FD for
   another process to adopt. Cooperative same-UID exclusion is not protection
   against arbitrary hostile same-UID replacement/restoration.

Existing wrappers which must **not** be called as this read-only boundary:

- `cutover-preflight`: correctly refuses a Rust marker as not legacy; not an
  upgrade precondition and not a reason to activate again.
- `runtime_relaunch::can_start` wholesale: validates current installed self/unit
  integration and Start/login receipts, not a candidate reading an old package.
- `current_store_compatibility`: calls `MigrationLock::acquire`, which may create
  or repair a lock. Reuse its strict parser/check logic with an existing lease.
- `OwnerLock::acquire`, `read_desired`, `read_marker`: may create directories,
  files or repair permissions; use read-existing variants instead.
- Startup reconciliation/validation: may prepare state, execute core validation
  or stage configuration. No such effects belong in this inspector.

Before any future update mutation, the trusted coordinator must independently
re-earn and retain its original local exclusions through the admitted phases;
the CLI's successful exit/JSON is advisory DATA, never a lock handoff. Admission
of all-user package replacement and privileged broker state stays with the
existing package/broker guards. This narrow inspector neither implements that
coordinator nor makes arbitrary package spawning part of the runtime IPC.

The executable test plan is bounded: genuine committed-Rust 0.8.2 fixtures and
stopped units; held/busy/absent/replaced locks; missing versus disconnected
Desired; configured startup On/unconfigured; duplicate/oversized/unsafe store
and template; pending recovery; marker/selector mismatch; races at capture and
final recheck; deterministic fixed public output with synthetic secrets absent.
Use another process to demonstrate exclusion while the inspector holds the
original locks, then prove its report cannot exclude a later start after exit.
Assert zero package/core/service mutation calls and unchanged private bytes,
metadata and directory membership. These tests are proposed, **not run**.

## 0.9.9 removal entry from every state

Provide **Remove OmaVLESS…** on setup, postponed, normal, missing-component,
incompatible-version and error/recovery surfaces. It must not depend on a
working daemon or disappear when settings/profiles are unavailable. Keep its
destination stable and keyboard reachable. Opening the removal review is
always navigation; it is not permission to remove anything.

The review distinguishes frontend-only removal, full app/companion removal and
private data. Default to retaining profiles/settings; no purge is bundled with
ordinary removal. An optional future purge requires its own reviewed fixed-path
contract, backup warning and confirmation, not a recursive HOME cleanup.

| Current facts | Allowed user-facing continuation |
| --- | --- |
| No app, setup never submitted | Confirm frontend-only removal; leave unrelated packages/data untouched. |
| Normal running app | Separate confirmed native shutdown; after verified cleanup, review package removal. |
| Stopped native app | Prove stopped/empty ownership and guarded companion state before removal. |
| Setup/update terminal or authorization pending | Review remains accessible; show the owning operation and settle/cancel it there. Disable removal execution meanwhile. |
| Partial known-completed setup | Fresh inventory of installed components; remove only selected known-owned components under their guards. |
| Lost reply, unknown ownership or failed recovery | Explain the blocker and offer observation/recovery guidance. No force-delete, automatic retry or success claim. |
| Native executable absent but private state exists | Preserve private state; use a separately reviewed administrative removal route. Absence is not verified shutdown. |

For full removal, reuse verified native Quit/shutdown while the matching binary
and units exist. Guard privileged DNS state, explicitly revoke the selected
UID only at its proven empty boundary while the broker binary still exists,
and disable the system unit before package removal. Another user's enrollment
or runtime is not this user's cleanup target. A remaining enrollment or lease
can refuse global removal. Preserve private ownership/data, as required by the
existing removal contract; do not run legacy `uninstall.sh --purge`.

Remove only the explicitly selected `omavless`/`omavless-dns` package set with
ordinary pacman checks. Do not uninstall shared `mihomo-bin`, Tailscale, V2RayN,
unrelated dependencies or firewall rules. Preserve installer/help execution until
the final frontend checkout removal; do not delete the script underneath the
active workflow. Plugin removal may trigger the existing unload observer, so
package removal must not race its shutdown request. Verify each actual outcome
and report the remaining component/data inventory instead of a blanket success.

## Required user-path gates

Clean first-use does not stand in for an upgrade. The owning release's workflow
matrix must include these distinct gates with exact frontend/package identities:

- Actual public 0.8.2 committed Rust, retained synthetic profiles/subscription
  records, no DNS companion; update **only the frontend**, open its real panel,
  and complete the explicit pinned-pair upgrade through normal user controls.
- Already selected/enrolled managed pair; guarded package update preserves
  selector/enrollment and unit preference, with no repeat first activation.
- Same-version corrected delivery; source/digests select the required pair.
- Unsupported custom template, newer/modified package, active/unknown owner,
  cancellation and partial package/enrollment/selection results.
- Removal/re-add before setup, after partial setup, normal stopped/running and
  unknown/pending authorization; reachable review and no blind cleanup.

Programmatic tests exercise production inventory/action admission and negative
routes, not mocks which redefine a previous stable install as fresh. Installed
tests cover actual panel/TUI meaning, consent, settled state, retained private
data, no unsolicited VPN/startup and the declared next action after refusal.
Keep x86_64/ARM64 evidence scoped; historical clean-install PASS remains valid
only for its original path. All gates above are pending in this SOURCE design.
