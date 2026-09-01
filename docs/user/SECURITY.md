# Security and privacy

## Privilege boundary

OmaVLESS does not invoke `sudo` or `pkexec`, create administrator policy,
change firewall rules, edit crontab or download executable code. The user must
explicitly grant the separately installed Mihomo binary the Linux capabilities
required for TUN networking.

The plugin manages only user-level systemd units. Future privileged networking
features require their own reviewed fixed-purpose design.

## Private storage

Profiles, subscription URLs, the routing template and generated Mihomo config
live under `~/.config/omavless/`. Private files are regular, same-user files
written with mode `0600` and atomic replacement where state changes.

Profile links and subscription URLs travel over stdin or private files instead
of process arguments. The ordinary status API does not return subscription
URLs. They are revealed only in their explicit editor.

## Import boundary

Supported links are parsed into explicit bounded fields. Duplicate, ambiguous,
unknown or unrepresentable connection options fail before storage. OmaVLESS
does not merge arbitrary URI data or remote YAML into runtime config.

Import review receives a redacted fact model rather than the original link.
Complete UUIDs, passwords, Encryption values, REALITY keys and subscription
URLs are excluded.

## Mihomo controller

The active core exposes a plugin-owned Unix socket inside the private user
runtime directory. Explicitly adopted templates have inherited controller
listeners and secrets stripped before OmaVLESS adds its socket. No attributable
Mihomo TCP external controller should be present.

Controller responses, rows and fields are bounded before display. Internal
profile/group names are reduced to safe public outcomes where possible.

## Services and lifecycle

Mutations are serialized by a user-runtime operation lock. Status polls share
a bounded short-lived cache and back off after repeated failures. The service
supervises its Mihomo child, and lifecycle operations are designed to avoid a
second owned core or TUN.

OmaVLESS detects common competing TUN/Mihomo/Xray clients and reports a neutral
hint. It never kills or reconfigures another application automatically.

## Subscriptions and probes

Remote subscription URLs require HTTPS except for explicit loopback
development. Downloads have a fixed size limit and normal certificate
verification. A complete candidate is validated before atomic commit.

Manual server tests use an isolated temporary no-TUN Mihomo core with a private
Unix controller, bounded DNS/address attempts and public connectivity targets.
Credential-bearing temporary config is private and removed after the child is
reaped. Tests run only after an explicit user action.

## Diagnostics

The shareable support snapshot omits:

- profile and subscription record IDs;
- profile, provider and server names;
- profile links and subscription URLs;
- UUIDs, passwords, keys and tokens;
- private endpoint/provider metadata;
- controller secrets and local executable paths.

Raw backend/core errors are not copied into public UI when they may contain
private material. Stable bounded public messages are used instead.

## Removal

Disabling or removing the plugin stops its TUN, disables login autoconnect and
removes generated user units. Private profiles are retained by default for
reinstallation. `uninstall.sh --purge` is the explicit destructive path for
removing private OmaVLESS data.

Neither removal path deletes Mihomo, changes Mihoro or touches another VPN's
configuration.
