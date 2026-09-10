# Native Settings helper inventory

Restores the existing file-import, profile-editor and QR SettingsActionRow
presentation using the Rust `desktop capabilities` boundary. Python Settings
remains the presentation oracle and rollback path; no Python call is added to
the native route. The helper inventory is not Mihomo/TUN readiness, network
health or proof that a future dialog will open successfully.

Opening native Settings or clicking its explicit helper Refresh performs one
fixed read, with a 10-second watchdog, 2-KiB accepted response bound, exact flat
schema and duplicate-key rejection. Missing/failed reads show unverified, not
missing or ready. Closing the panel invalidates pending inventory. No returned
path, arbitrary provider name or raw error reaches UI. Copy-command controls
only copy fixed installation instructions after an explicit click; they never
install packages or execute the copied command.

Deterministic tests cover three supported pickers, missing helpers, malformed,
duplicate/unknown/oversized responses, stale/closed results, fixed launcher and
watchdog QML structure. Required installed acceptance: English/Russian Settings
ready and missing/unknown states, refresh after dependency installation, copy
instruction without execution, and unchanged VPN runtime ownership.

This checkpoint does not complete R5/R6, verify autoconnect, or permit removal
of the retained Python oracle. Exact-head visual acceptance is recorded by the
installing agent separately.

The follow-up startup row uses the already validated native UI snapshot only.
It distinguishes missing metadata, unconfigured preferences, configured Off and
configured last/specific profile plus Routing/Full VPN. It never displays private
profile identity and always states that login activation is unverified. There
is no Configure button or mutation. The existing SettingsActionRow gains an
explicit optional hidden action, defaulting to its unchanged legacy behavior.
Onboarding completion is not presented as setup/core readiness.
