# S1 host observation admission checkpoint

The original design checkpoint, 2026-09-29, narrowed the next slice of the
[S1 foundation](S1_PROXY_FOUNDATION.md). The first opt-in read-only GIO candidate
is recorded in [S1 read-only observer](S1_READ_ONLY_OBSERVER.md). It is not a
production adapter, session-provenance proof or feature enablement. Do not infer
installed-host support from the existing pure codecs or that candidate.

## Why the command-line shortcut is insufficient

The upstream [gsettings command implementation](https://github.com/GNOME/glib/blob/main/gio/gsettings-tool.c)
prints effective values for `get` and `list-recursively`. It does not expose the
complete layered observation required by our codec. An override equal to the
default is still an override; an administrator lock can hide an earlier user
value. GIO explicitly exposes these differences through
[`get_user_value`](https://docs.gtk.org/gio/method.Settings.get_user_value.html)
and [`get_default_value`](https://docs.gtk.org/gio/method.Settings.get_default_value.html).
The latter is not necessarily the compiled schema default.

Neither parsing CLI text nor substituting `dconf read` has established equivalent
semantics for the selected GSettings backend, administrator defaults and locks.
Do not infer `user: absent` from equal values or an empty command result. Do not
switch the user's settings backend/profile/schema search path to make reads pass.

## Selected next implementation boundary

Use a separately built, unprivileged Rust/GIO observation helper, isolated from
the normal runtime's dependency graph. Packaging/CI must explicitly opt into its
GIO dependency. A missing helper is `unsupported`, not a fallback path. The
candidate helper is source-only and is not in an OmaVLESS package.

The helper has one fixed read operation, no configurable schema/key/address and
no write/reset/import verbs. A parent runner enforces an absolute executable
identity, bounded private stdout/stderr, a total deadline and child reap. It must
not invoke a shell. Client input cannot choose executable, arguments or bus.
Captured stderr, settings and environment are private data, never user-facing
error text. Timeout, malformed output, cancellation and nonzero exit produce no
partial snapshot. The injectable runner must let tests model every failure
without starting a host process.

Before constructing Settings objects, enumerate and verify the fixed five
schema IDs, paths, 16 keys, signatures and supported ranges. Reject missing,
relocatable, differently typed or incompatible schemas. For every key obtain
typed effective value, reset/default value, optional user value and writability.
Preserve a null user value separately from empty string/list. A null required
default, unsupported backend or inconsistent layered read refuses admission.
Use the existing codec's type/size limits; do not stringify and reparse variants.

Read the systemd user manager's complete typed `Environment` property through
its fixed [D-Bus interface](https://github.com/systemd/systemd/blob/main/man/org.freedesktop.systemd1.xml).
Project only the codec's ten exact case-sensitive names, splitting each assignment
at its first equals sign. Reject malformed or duplicate selected assignments.
Missing selected names mean absent only after a successful complete bounded
reply. Discard unrelated entries privately; never serialize the whole manager
environment into the journal or diagnostics. This does not capture per-unit
overrides or already-running applications.

## Provenance is an admission requirement, not a Boolean from the helper

The production runner must bind observations to the runtime's trusted UID,
boot/session identity and a validated local user bus. It must obtain the manager's
unique bus owner and credentials, and detect owner/session changes across the
observation. A helper-supplied `broker: true`, process name, installed package,
environment variable or a successful `systemctl --user` invocation is not proof.

The exact native mechanism for proving that this bus's activation environment
is the same systemd manager has **not yet been established**. Therefore unknown
provenance remains unsupported even if both snapshots decode successfully.
Implementation must first demonstrate peer/owner/session validation in a fake
bus and controlled VM, without accepting client-supplied provenance claims.

[UWSM documents](https://github.com/Vladimir-csp/uwsm#concepts-and-features)
that dbus-broker reuses the systemd environment, while the separate reference
dbus-daemon environment cannot unset variables. **Separate dbus-daemon is
unsupported**: replacing absent with empty is not exact restoration. Do not
restart D-Bus, terminate a user session, edit UWSM files or silently omit this
surface as a workaround.

Acquire two complete bounded observations under unchanged provenance and reject
any difference. This detects observed concurrent changes, not atomicity or ABA;
future effects still require fresh reads and per-field durable intent. Successful
observation does not admit writes, establish listener readiness or claim that an
application honors either surface. Keep these capabilities separate.

## Required fake-host and installed gates

These are **pending test cases**, not executed adapter tests:

| Fixture / fault | Required result |
| --- | --- |
| Helper missing, wrong identity, nonzero exit, timeout, oversized output | No snapshot; fixed non-sensitive error |
| Schema/key missing, wrong path/type/range, unknown backend | Unsupported; no guessed default |
| Override absent, empty, equal to default; locked hidden override | Exact distinct values; locked/inconsistent write admission refused |
| Manager query fails or truncates | No all-absent synthetic snapshot |
| Empty value, first-equals split, case variants, literal newline | Preserve exact selected data; no shell parsing |
| Duplicate selected environment key or malformed typed reply | Refuse; no last-value-wins |
| Foreign UID/bus, owner change, session change, unknown activation | Refuse; invalidate complete observation |
| Separate dbus-daemon with otherwise valid values | Unsupported, including all-empty/all-absent cases |
| Foreign edit between passes; layer/default change only | Refuse; no journal initialization from torn state |
| Secret-shaped synthetic settings/stderr/unrelated environment | No value in logs, errors, argv or shareable evidence |

Installed gates then verify actual Omarchy schema/backend and broker provenance
on a controlled VM. Record only sanitized capability/results, not snapshots.
New-app consumption and restoration/crash acceptance belong to later writable
adapter gates; observation-only evidence cannot close S1. NixOS remains separate.

Existing executable codec tests already cover absent/empty, equal-default and
locked overrides, completeness, bounds, malformed documents and unsupported
activation classes. They cannot verify helper timeout, installed GIO semantics
or D-Bus provenance. Do not label the table above PASS on their strength.
