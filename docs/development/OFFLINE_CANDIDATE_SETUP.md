# Offline development-candidate setup

Owner-selected October19 for bounded fresh-install stabilization. This is a
nonpublic developer path, not a release, Marketplace installation or anonymous
download test. The original October19 selection kept public main/fix.8 and
`setup-runtime.sh`'s direct public entry unchanged. Their immutable published
asset URLs must not be retargeted.

The proposed 0.9.9 install/guide source successor removes the committed marker
to select its ordinary fixed-version public entry. That source change is not a
published release or an anonymous-download result. The historical offline lane
below remains explicit fixture data in its tests; adding its marker back to a
successor is not an allowed fallback after an update refusal.

## Selection and inputs

The reviewed frontend explicitly carries byte-exact `plugin/setup-channel.json`
for `unpublished-offline-candidate` version `0.9.9`. `setup-entry.sh` selects
only this fixed candidate script or the public script when the marker is absent.
A present invalid marker refuses, including unknown fields/versions and unsafe
files. The candidate independently checks the marker and matching manifest.
There is no environment flag, supplied executable, URL or replacement checksum.
Do not edit installed metadata to enable another version or package source.

Both archives must already be independently reviewed against their build/CI
provenance and the committed frontend pins:

- `omavless-0.9.9-1-ARCH.pkg.tar.zst`;
- `omavless-dns-0.9.9-1-ARCH.pkg.tar.zst`.

Use the selected architecture (`x86_64` or `aarch64`), one common pinned producer
and exact archive bytes. Put the files in a private, current-user-owned directory
outside Git, with mode `700` and mode `600` regular, single-link archives.
Ancestors must be root/current-user-owned, non-writable by other users and not
symlinks. These are trusted local inputs, not authenticity from caller-supplied
checksums. The installer takes expected hashes only from its reviewed frontend.
No package rebuild or fresh download is implied by this procedure.

## Actual user journey

1. Install/enable the exact reviewed candidate frontend through ordinary Omarchy
   plugin installation. Selecting an unpublished frontend is fixture selection,
   not proof of installing public main. Start with a genuinely absent native
   app/companion/owner; do not preinstall packages, seed enrollment or reset a
   used private store to make this case appear fresh.
2. Open OmaVLESS and choose **Install required components**. The terminal names
   the unpublished offline candidate before the ordinary `INSTALL` confirmation.
   Enter without `INSTALL` cancels without package or native effects.
   Starting the fixed terminal launcher closes only the panel so it cannot
   retain the keyboard over the prompt. This is a handoff, not launch or setup
   success. Reopening retains the terminal-closure barrier; while the original
   launcher is running, acknowledgement cannot clear it. A launcher failure also
   requires explicit closure acknowledgement before a new setup action.
3. Enter the absolute archive-directory path when requested; Enter cancels.
   Both complete pinned hashes are verified before parsing or staging. Bounded
   private copies are rehashed and published without replacing existing cache
   entries. An unsafe/poisoned entry refuses; nothing is fetched from the network.
   A complete already-verified cache needs no new directory prompt.
4. Follow ordinary pacman dependency/transaction confirmation and sudo's own
   password prompt. The installer adds no `--nodeps`, `--overwrite`,
   `--noconfirm`, sudoers or polkit exception. Then follow the **separate DNS**
   consent, normal initialization, template selection and native activation.
   Installation/setup must leave VPN startup and the VPN connection Off.
5. Finish/cancel system prompts, close the terminal and use **Terminal and system
   prompts are closed** / **Check again**. Verify the refreshed installed state,
   ordinary panel and close/reopen. Package presence is not core/TUN/DNS/Internet
   health, nor proof that an earlier authorization succeeded.

The candidate supports only fresh install and the existing guarded continuation
steps `resume-dns`, `finish-selection` and `start-broker`. It does not offer public
predecessor updates, package repair, arbitrary reinstall, or helper-access
recovery. Unknown/mixed/modified installed payloads remain blockers.
Candidate inventory explicitly marks this channel so the shared UI hides the
unsupported helper-access recovery disclosure, while retaining ordinary helper
Start and the direct native application Start continuation. Public recovery
visibility and its existing guards are unchanged. Actual
installed identity includes exact schema-3 version/source, app/core/broker bytes
and DNS receipt, not just `pacman -Q`.

Cancellation after a completed package or DNS phase does not undo it. Preserve
its original outcome and use only the next supported action after independently
observing the state. Unknown results never authorize automatic resend or cleanup.
An empty account needs `setup initialize` and `store-compatibility`, not a
renderable last-profile `store-preflight` or administrator-created JSON.

## Evidence and limits

Source tests use inert package/native leaves and synthetic archive bytes; they
do not install anything. Before effects, independently review the exact artifact
selection/privileged caller delta and exercise the continuous journey in a NEW
owner-authorized guest, including cancellation, terminal closure and truthful
Off. Apply [VM operations](../../skills/omavless-dev-vm/SKILL.md) and the
[execution policy](EXECUTION_POLICY.md). Never reopen sealed old fixtures.
Review whole affected Setup/Help EN/RU, normal/constrained and actual keyboard
states; synthetic rendering or a helper exit cannot replace installed evidence.
Record native architecture, exact frontend/pair and pending gates in the owning
PR. ARM installation and literal human physical keyboard are separate claims.
Full exact PR and independent merged-PUSH/export/bootstrap gates, merge authority
and public release authorization remain separate.
