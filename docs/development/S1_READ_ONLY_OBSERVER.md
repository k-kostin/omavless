# S1 read-only GIO observer candidate

Development checkpoint, 2026-09-29. `omavless-s1-observer` is an opt-in Rust
workspace package. Its GIO dependency and executable are built only with the
`gio-observation` feature. The normal OmaVLESS binary, IPC capabilities,
package, UI, connection modes and host settings do not call this crate. This is
not App proxy enablement or a write adapter.

The helper has one fixed `--private-observe-v1` operation. It validates the five
fixed GNOME proxy schemas, paths, all 16 keys, signatures and supported ranges
before constructing `GSettings`. It refuses an explicit backend or schema-dir
override, a backend other than dconf, missing defaults and invalid values. For
every key it captures typed effective, default, optional user override and
writability. An absent override is distinct from an empty override or an
override equal to the default. Two full reads must agree.

The helper queries the systemd user manager's typed `Environment` property over
the local session bus. It pins that query to the manager's unique bus owner,
verifies the owner's UID equals the process UID before and after the two passes,
bounds the complete reply, and projects only the ten fixed case-sensitive proxy
variable names. Duplicates and malformed selected assignments refuse. Unrelated
environment entries are discarded before serialization. The two full manager
reads must agree. No `systemctl`, shell, GSettings write or D-Bus mutation is
used.

Successful observations still carry `Unverified` provenance. Matching bus UID
and owner do not prove that this bus shares the user manager's activation
environment, establish a trusted login/session binding, or support exact absent
restoration under a separate dbus-daemon. `admit_writes` always refuses. An
installed controlled-VM provenance gate and a reviewed process runner must be
implemented before any caller may use this helper for effects. The runner must
pin the absolute executable identity, capture bounded private stdout/stderr,
enforce a total timeout and reap the child. The helper's private length-framed
stdout is never a diagnostic; do not invoke it from an interactive shell to
inspect real settings or copy its output into an issue.

The pure tests cover selected environment preservation/rejection, exact framed
decode and unconditional write refusal. Opt-in GIO tests use the installed
public schema with an in-memory backend for absent/equal-default override and
inspect only the backend type, without reading user proxy values. They are
ignored by default because CI images need not ship GNOME schemas. No current
host proxy values, VM settings or live connection are part of this checkpoint.

Remaining work: fixed parent runner and fake-process failure matrix; verified
session/activation provenance on an isolated Omarchy VM; complete installed
snapshot behavior; per-key journaled effects and restoration; new-app
consumption. The separately documented NixOS adapter remains unimplemented.
