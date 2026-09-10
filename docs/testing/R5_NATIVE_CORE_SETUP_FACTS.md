# Native desktop core setup facts

The fixed read-only `omavless desktop core-readiness` command supplies version-1
`desktop_setup_facts` JSON for future Settings/onboarding composition. It is not
a daemon method and does not open the store or a controller, create a TUN,
change service state, install anything or invoke privilege escalation.

It reuses accepted core discovery precedence (explicit override, user-local,
absolute PATH candidates) in the calling desktop environment. This environment
may differ from the systemd user runtime. No claim is made that the executable
discovered here is the runtime's current child.

The public projection contains only:

- `installed` and an optional canonical numeric release `version`;
- `tunDevice`: `present`, `unavailable` or `unknown`, from metadata for the
  fixed `/dev/net/tun` character device (major 10, minor 200), never an open/ioctl;
- `fileNetworkCapabilities`: `present`, `missing`, `unknown` or `not_applicable`;
- `servicePermissionReadiness: not_verified` and explicit false coverage flags
  for service context, TUN creation and controller access;
- a fixed host-setup remediation identifier, never a command from helper output.

There is deliberately no `tunReady` assertion. The Arch service contract uses
separately provisioned Mihomo file capabilities and a compatible runtime
service context; it does not give ambient network capabilities to the runtime.
File capability presence alone cannot establish namespace, NoNewPrivileges,
bounding-set, mount-policy, D-Bus authorization or TUN readiness. The accepted
owned runtime observation/connection gates remain authoritative. This command
does not advertise Arch-specific remediation as a NixOS setup contract.

The `-v` subprocess is capped at 4096 bytes and two seconds; `getcap` at 8192
bytes and two seconds. Both use fixed argv and discarded stderr through the
existing bounded desktop executor. Only a three-component numeric version is
retained; build labels, paths and arbitrary output are omitted. Unknown facts
remain unknown rather than fabricated success.

The actual Python `core_setup_status` oracle covers all eight combinations of
the three expected file capabilities. Native inventory matches these canonical
combinations, but intentionally does not reuse Python's over-broad `tunReady`
label or private executable path. Effective/permitted capability flags and exact
tokens are required, rather than substring matches. Deterministic gates cover
missing/failing discovery helpers, malformed/private output, non-device paths,
response scope, fixed CLI arity, byte/time bounds and no persistent writes.

The Settings binding reuses `SettingsActionRow` with an enabled Refresh action,
strict bounded parser and disposable watchdog-protected process. It refreshes
on Settings entry and refuses late replies after page closure/ownership change;
fast close/reopen schedules a fresh sample rather than accepting the old one.
English/Russian text explicitly separates desktop inventory from runtime
permission/TUN readiness. It does not write legacy `coreSetup.tunReady`, expose a
nonfunctional Configure action, or execute/copy guessed installation commands.
Installed visual review remains separate, and Python retirement is not claimed.

## Local candidate validation

Try Omarchy ARM64, 2026-09-10: a dedicated Cargo target directory was used to
avoid cross-branch executable collisions. `cargo test -p omavless-runtime`
passed 542 tests, including the actual Python eight-case inventory oracle;
strict all-targets clippy, formatting and diff checks passed. The candidate's
read-only command reported Mihomo `1.19.30`, TUN device and file capabilities
present, with service permission readiness still explicitly `not_verified`.
This was a command probe, not installation or a TUN/connect test.
