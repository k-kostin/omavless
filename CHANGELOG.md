# Release notes

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
