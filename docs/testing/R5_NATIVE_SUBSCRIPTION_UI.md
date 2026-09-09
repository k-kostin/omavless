# R5 native subscription UI checkpoint

This Draft slice depends on the native main-panel branch (PR #215), not a
second implementation on main. It does not complete R5/R6 or authorize merging
the unfinished frontend. The exact installed candidate is recorded in its PR.

## Ownership and parity

The legacy Python frontend path is retained unchanged as reference/rollback.
In native mode QML now routes add, edit, delete and individual refresh through
fixed Rust `plugin.action` mappings to the existing canonical subscription
operations. No URL policy is reimplemented in QML. URLs/names travel on bounded
stdin, never argv. Clipboard/file subscription previews open confirmation;
they do not silently create a subscription. Existing duplicate detection stays
in the canonical preview boundary.

Remote work uses the existing detached subscription preflight/fetch/completion
path, not the general owner mutex. Socket tests prove status and disconnect
remain available while a fetch is blocked. The frontend still serializes its
own pending mutation; this is not a claim that every QML control stays enabled.

Instance/revision fences and operation IDs protect mutations. Delete captures
its fence when confirmation opens. Lost replies retain exact replay metadata;
acknowledging an unknown result does not enable a new save of that draft.
Exit 74 means locally rejected before socket dispatch for these fixed actions;
transport uncertainty remains exit 73. Private edit-read collectors are
per-request and destroyed on completion; cancelled/stale results cannot open
another draft. Public errors are fixed localized classifications.

## Validation matrix

Initial local checkpoint, Try Omarchy ARM64, 2026-09-09:

- Rust workspace: 758 passed, 4 ignored; clippy all targets with warnings denied.
- Python reference/launcher suite: 342 tests, 341 passed, 1 root-only skip.
- Native subscription QML-function contracts: 13 passed; native import: 8.
- Existing native action/main/editor/QR/presentation and QML/i18n contracts pass.
- Python compile, shell syntax, manifest, rustfmt, diff check and plugin validate.
- Installed Mihomo v1.19.30 linux arm64: supervisor, native host and private-store
  config opt-in tests pass (3 tests); no public fixture material is emitted.

Real Quickshell with production plugin files and the new Rust binary, isolated
HOME/config/state/runtime and a loopback-only synthetic HTTP feed:

| Operation | Evidence |
| --- | --- |
| Add / cancel | English and Russian masked-URL confirmation; cancel has no mutation |
| Add / confirm | One subscription and managed profile added after confirmation |
| Edit | Private read opens correct masked editor; rename commits and is observed |
| Individual refresh | Action admitted and revision advances after completion |
| Delete | Explicit confirmation fence; synthetic subscription/profile removed |
| Restoration | Original synthetic counts restored; no VPN was started |

The UI scenario waits for ready observations and revision advancement rather
than treating a stale success label as evidence. Visual review caught mid-word
hint wrapping; word-aware wrapping with long-token fallback fixes it.
Screenshots remain private (desktop background can contain unrelated private
material); only this matrix is shareable. These synthetic rendering checks are
not an installed-package/private-provider acceptance claim.

## Remaining gates / exclusions

Install and verify the exact committed package/plugin; exercise an existing
private subscription editor/cancel and refresh without disclosing its identity.
Retain the dependent profile-replacement live-save gate. Batch refresh, latency
tests, startup controls, routing tools, traffic and advanced diagnostic surfaces
are separate restoration slices. No new protocol or privileged path is added.
