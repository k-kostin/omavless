# 0.8.0 marketplace preparation

Owner-approved content direction, 2026-09-15. **Preparation only:** no release,
marketplace submission, installation classification or publication is implied.

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
cards. At this checkpoint its real package pins are empty. Current README
therefore retains the truthful manual package-first instructions and marks the
guided route as in validation, not generally available.

After immutable packages/pins and clean end-to-end acceptance, replace that
candidate warning with:

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
a fabricated working VPN. Source plugin: `4e2f17f88b230e4d43e05b738c6cf6f4c6caea43`.
The fixture transport replaces only process responses; no profile or subscription
is persisted. The actual installed service is not accessed by the test instance.
See [reproduction and limits](../../tests/marketplace-visual/README.md).

Owner-requested naming revision: visible labels use ordinary country/city names
(Netherlands · Amsterdam, Germany · Frankfurt, Finland · Helsinki,
Sweden · Stockholm, France · Paris) and “My servers”, without a “Demo” prefix.
These are invented display labels, not actual provider locations or a claim of
live connectivity. Internal fixture IDs and the disconnected state are unchanged.
The affected main/expanded views are recaptured from QML, not image-retouched.
Both revised captures were inspected at native size: country/city labels fit
without clipping or shifting Connect controls. Fixture/parser/refusal tests and
QML contracts pass. Settings is unchanged because it contains no profile names.

- [Main panel](images/main-en.png): mode choices, row Connect actions,
  subscription refresh and separate management dock.
- [Expanded subscription](images/subscription-en.png): demo child profiles and
  the server-list refresh action. No server/URL/credential fields.
- [Settings](images/settings-en.png): language, routing and subscription entry
  points. This is one scroll position, not a claim the complete Settings fits.

These are panel-only crops, **not a full-desktop hero composition**. At native
460-pixel width the real monospace text stays readable without scaling or fake
desktop chrome. Preserve masters; do not upscale or squeeze into 16:9. A wider
desktop/bar hero remains an owner-choice follow-up, ideally captured at native
HiDPI on the PC. Compare actual responsive card cropping before selecting the
single root preview; the current root preview is deliberately unchanged here.

There is no installation hero: simulated ready/missing-component facts are not
evidence of the still-unrun published package setup. Capture that README image
only after the real path is accepted. Do not expose or retouch real credentials.

## Publication checklist

- [x] Agree benefit-first product description and own-access disclaimer.
- [x] Remove unsupported autoconnect promotion from widget metadata.
- [x] Prepare native, credential-free disconnected UI screenshots.
- [ ] Publish reviewed architecture packages only with owner authorization.
- [ ] Pin and verify the real guided first-run install path on ARM64/x86_64.
- [ ] Confirm final release package/frontend pairing and exact source records.
- [ ] Replace candidate installation prose only after those gates pass.
- [ ] Choose root preview and verify the actual Marketplace card/detail crop.
- [ ] Confirm existing listing identity and installation classification.
- [ ] Owner approves release and, separately, marketplace submission/update.

Do not close R6 again, change its accepted scope, or relabel AUTO-1/DNS/V0 gaps.
