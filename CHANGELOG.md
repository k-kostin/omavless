# Release notes

## 0.9.9 — unreleased development preview

These notes describe the proposed next snapshot. Public installation and
downloads remain **0.9.8 fix.8** until a separately qualified release.

- Add direct private link/key and local-bundle import using the existing
  preview, validation and confirmation flow.
- Add selected encrypted-backup Restore and explicit recovery after supported
  interruptions, preserving the existing strict data and ownership checks.
- Extend experimental one-peer IPv4 WireGuard/AmneziaWG support within its
  documented subset; unsupported configurations remain refused.
- Add subscription refresh progress/cancellation and the selected Daily CLI
  schedule. Preserve explicit Disconnect across ordinary runtime restarts.
- Add Preset default, Google and Cloudflare DNS choices, saved while disconnected
  and applied on the next connection.
- Add opt-in recorded TUN traffic for one period across restarts, with explicit
  controls and a partial-data marker. This is not provider quota or a complete
  history of internet traffic.
- Restore the main-panel HTTPS Test and Ping sections with clear unavailable
  results, and improve existing import, Connections and removal guidance.
- Add explicit saving of the whole original profile store before normal startup
  when strict validation refuses it. The copy is private and **unencrypted**;
  saving does not repair profiles or enable connection.
- Update the next-version guides and Help. The Russian guide is not active yet;
  Help retains English fallback.

Physical sleep/network and Tailscale acceptance remain open. New connection
closing, App Proxy, kill switch, persistent Lockdown, YAML editors, extra
TUIC/Hysteria2 options and manual core choice are deferred to 1.0.0.

## 0.9.8 — maintenance update

- Add complete terminal removal, including profiles and settings. The helper
  closes OmaVLESS, asks before restarting the computer and resumes afterward,
  even if the widget has already been removed. Other VPN clients and a
  separately installed Mihomo are kept.
- Simplify setup and onboarding. When the core is ready in onboarding, show its version,
  the next step and only **Close** / **Continue**.
- Make partial setup and supported component updates easier to follow. Explain
  the DNS helper before asking for permission, keep confirmation steps clear,
  and preserve settings during updates from earlier public releases.
- Keep component updates working after choosing a routing preset in onboarding;
  retain the original setup backup without rewriting your routing settings.
- Bring installation, updates, removal, troubleshooting and FAQ together in
  one [user guide](docs/user/NATIVE_INSTALL.md). Add a separate
  [guide for your assistant](docs/user/USER_AGENT_GUIDE.md) and short installed help.
- Add **Help** with a ready-to-copy request for your assistant, the user guide
  and direct problem reporting. Help is available before setup is complete.

The product version remains **0.9.8**; this delivery is **fix.8**. Update the
widget first, then follow the offered component update or remaining setup step.
Updating the widget alone does not replace the application packages.

0.9.8 includes the terminal application, managed VPN core and DNS helper,
English/Russian interface and encrypted **Backup**. **Restore is not included.**
Closing the terminal leaves the shared VPN connection running; use
**Disconnect** or Settings → **Shut down OmaVLESS** as appropriate.

Earlier 0.9.8 maintenance updates corrected clipboard handling, subscription
scanning and first-run/update setup. Earlier tags and downloads remain unchanged.

## 0.8.2 — September 22, 2026

- Fix shutdown checks without weakening ownership and cleanup protections.
- Restore keyboard navigation after opening a subscription's server list.
- Clarify name-based profile search in English and Russian.

## Earlier releases

See the [release archive](https://github.com/k-kostin/omavless/releases) for
version-specific downloads and notes. For current installation and update
instructions, use the [user guide](docs/user/NATIVE_INSTALL.md).
