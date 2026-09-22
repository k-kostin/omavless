# Native marketplace update preparation (0.8.2)

Updated September 22. The established filename is retained for external links.
The current stable release is **0.8.2**, with published, inspected ARM64/x86_64
packages and matching frontend pins. The
[clean x86_64 guided setup](../testing/NATIVE_082_FRESH_VM_2026-09-21.md) passed;
the [artifact record](../testing/NATIVE_082_ARTIFACTS_2026-09-21.md) identifies
the immutable bytes. Do not reuse 0.8.0/0.8.1 hashes or reopen unchanged R6 gates.
September 22: the owner authorized finishing release and marketplace submission.
GitHub now marks 0.8.2 stable/latest with unchanged tag/assets. Submit the final
reviewed main after the documentation cleanup and exact-SHA checks; marketplace
maintainer approval and deployment are not implied by the stable release.

The September 15–16 preparation notes below preserve content decisions and
their historical limits; current publication status above supersedes old holds.

## Product story

The root manifest carries the agreed 364-character description: VPN app for
Omarchy, everyday bar controls, your own compatible access, three modes,
subscription refresh, free/open source, no included VPN service. The README
continues with everyday use and precise protocol limitations, not migration
history or test counts as selling points.

September 16 editorial pass keeps README product-facing: one short truthful
unpublished-candidate notice and the installation guide replace the engineering
release narrative. SHA identities, activation commands, architecture/pairing
details and historical marketplace facts remain in the user/release/status
documents; they are not deleted or promoted to release claims. No screenshot,
runtime, protocol maturity or installation implementation changes in this pass.

Do not advertise TUI, WireGuard/AmneziaWG/`vpn://`, kill switch or unaccepted
login autoconnect. Advanced VLESS Encryption/REALITY PQ and advanced XHTTP
combinations retain their experimental evidence limits alongside
Trojan/Hysteria2/TUIC. Do not imply all provider combinations were live-tested.

## Installation copy — release gate

PR #249 adds guided first-run provisioning and persistent missing-component
cards. September 16: real ARM64/x86_64 pins are committed and the public
`v0.8.0` prerelease assets pass anonymous-download checksum verification.
README links the testing release without claiming stable or accepted guided
installation. See the current first-run evidence and PC gate; the older
empty-pin state is superseded, not an ongoing asset-publication blocker.

The earlier empty-pin/clean-setup blockers are now superseded by the 0.8.2
evidence above. Stable promotion now replaces the old candidate notice with
the supported guided-install explanation:

> Install the plugin through Omarchy, then open its panel. Required components
> shows any missing OmaVLESS application or Mihomo core. Start setup from the
> panel and confirm the steps in the terminal; OmaVLESS selects the matching
> application package automatically. Already installed but not activated? Use
> Complete setup instead. You can return later. Setup does not connect a VPN.
> Once component installation and activation finish, continue onboarding to
> check TUN permissions and add your own compatible profile or subscription.

Presence/activation is **not TUN readiness**. Do not promise that disappearance
of component cards alone proves the host can connect. Keep optional desktop
helpers and ordinary OS authorization clear in the linked installation guide.

Target standard installation after proof, but do not self-certify the
Marketplace classification. If maintainers require Manual setup, use the same
actual guided flow with an explicit opening sentence that initial setup is
required, linking the guide. Do not restore obsolete instructions that make
users choose architecture or type internal activation commands when the guided
path is accepted. No new marketplace ID or second repository is needed.

## Screenshots

Selected PNGs are real renders of unmodified product QML, native 1x screen
scale on the 1920x1080 ARM64 Try Omarchy guest, using a network-isolated test
instance and credential-free demo metadata. They show **Disconnected**, never
a fabricated working VPN. September 22 recapture source:
`6b1baa13aa8a9d3f32dfa50fdb91bdce84f7fb65` (0.8.2 product code).
The fixture transport replaces only process responses; no profile or subscription
is persisted. The actual installed service is not accessed by the test instance.
See [reproduction and limits](../../tests/marketplace-visual/README.md).

Owner-requested naming revision: visible labels use ordinary country/city names
(Netherlands · Amsterdam, Germany · Frankfurt, Finland · Helsinki,
Sweden · Stockholm, France · Paris) and “My servers”, without a “Demo” prefix.
These are invented display labels, not actual provider locations or a claim of
live connectivity. Internal fixture IDs and the disconnected state are unchanged.
The affected main/expanded views are recaptured from QML, not image-retouched.
The current captures include #263's corrected name-only search hint; they do
not retain the older hint advertising country/host search.
Both revised captures were inspected at native size: country/city labels fit
without clipping or shifting Connect controls. Fixture/parser/refusal tests and
QML contracts pass. Settings is unchanged because it contains no profile names.

- [Main panel](images/main-en.png): mode choices, row Connect actions,
  subscription refresh and separate management dock.
- [Expanded subscription](images/subscription-en.png): demo child profiles and
  the server-list refresh action. No server/URL/credential fields.
- [Settings](images/settings-en.png): language, routing and subscription entry
  points. This is one scroll position, not a claim the complete Settings fits.

September 16 release preparation selects `main-en.png` as the root `preview.png`,
replacing the historical UI screenshot with the already reviewed native panel.
It is an exact byte copy, not a retouch or upscale. The disconnected state and
invented country/city labels remain explicit. A full-desktop hero is optional,
not a reason to keep advertising the old interface.

These are panel-only crops, **not a full-desktop hero composition**. At native
460-pixel width the real monospace text stays readable without scaling or fake
desktop chrome. Preserve masters; do not upscale or squeeze into 16:9. A wider
desktop/bar hero remains an owner-choice follow-up, ideally captured at native
HiDPI on the PC. The selected native panel is also linked from README.

September 22 local browser review used the current marketplace stylesheet
(`8c806312b129ebac0849a24b16d08cddba7f82b1`), 280/360-pixel card containers
with the actual 175-pixel preview height, and the 860-pixel detail width.
Cards retain the recognizable main controls/profile-list portion while clipping
lower content; they are not full-panel reproductions. The detail view shows the
whole panel. The 460-pixel native master is deliberately not upscaled on disk;
the wider detail view is therefore softer than a future real HiDPI capture.
This is an explicit quality limit, not a reason to invent pixels or Connected.
The local CSS review is not evidence of an already deployed marketplace page.

There is no installation hero: simulated ready/missing-component facts are not
installation evidence. Actual clean setup is recorded separately; an additional
onboarding image is optional, not a publication blocker. Do not expose or
retouch real credentials.

## Existing-listing update procedure

The existing registry entry is `kdk.omavless`, repository
`https://github.com/k-kostin/omavless`, category System, tags bar/quickshell/security.
Its recorded snapshot remains `69fe05b03129a23664fff3f8289821a7b7f80095`.
There is no explicit manual-installation override in the inspected registry.
Use **Verify and publish a newer upstream commit**, not a duplicate Plugin
submission, old-snapshot verification, or standard-installation override removal.

Read the current upstream `SUBMISSION.md`, `SECURITY.md`, `VERIFICATION.md` and
`verify-plugin.yml` before submission. The September 22 official local validator
passed compatibility and the exact configured plugin set at main `6b1baa1…`.
The complete security baseline had no findings and required review of
`installer`, `package-manager`, `service-management`, `privilege`. This is local
preparation, not a marketplace bot report, maintainer approval or security audit.
After this documentation/image update merges, rerun those checks on the final
exact main SHA and bind the external issue draft to that SHA; do not promote
an untested new head mechanically.

Keep the issue title `[Verify]: OmaVLESS 0.8.2 native update` and the current
form's headings/acknowledgment unchanged. Submission is now owner-authorized.
Inspect bot reports on that one issue; do not
apply maintainer labels or equate issue creation with published registry/deployment.
Document the explicit package consent, AUR Mihomo route, separate TUN permission
step, native user service and private-data-preserving updates for reviewers.

## Publication checklist

- [x] Agree benefit-first product description and own-access disclaimer.
- [x] Remove unsupported autoconnect promotion from widget metadata.
- [x] Prepare native, credential-free disconnected UI screenshots.
- [x] Publish reviewed 0.8.2 architecture packages as an authorized prerelease.
- [x] Pin real packages; record clean x86_64 first-run setup and its limits.
- [x] Confirm final release package/frontend pairing and exact source records.
- [x] Align guided-install prose with accepted setup and actual stable release.
- [x] Recapture current product QML and inspect local marketplace CSS crops.
- [x] Confirm existing listing identity and absence of a manual override.
- [x] Post-preparation main `6bfc864…` passed official local checks; draft bound.
- [x] Owner authorizes stable promotion and marketplace submission; release promoted.
- [ ] Repeat exact-SHA checks after stable-docs merge and submit that one update.
- [ ] Marketplace maintainer approves exact reviewed snapshot; deployment verified.

The scoped x86_64 clean gate does not claim a fresh ARM64 0.8.2 installation,
live connectivity, enabled autoconnect or every interrupted privileged effect.
An existing ARM64 installation update is separate evidence, not a clean-install
substitute. Do not demand another destructive reset merely to change this label.

Do not close R6 again, change its accepted scope, or relabel AUTO-1/DNS/V0 gaps.
