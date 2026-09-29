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
the fixed local user bus described below. It pins that query to the manager's unique bus owner,
verifies same-user kernel peer and manager identity before and after the two passes,
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

This opt-in source checkpoint adds `observe_via_fixed_runner` and a
disposable-VM-only `omavless-s1-observer-check` executable. The runner opens the
fixed `/usr/lib/omavless/omavless-s1-observer` path through checked root-owned,
non-writable, non-symlink ancestors, verifies a regular single-link executable
ELF without setuid/setgid bits, then executes its pinned descriptor with only
`--private-observe-v1`. Client input cannot select the path or arguments. The
helper is not installed by this source checkpoint; normal package/runtime/IPC
still has no caller. The runner limits stdout, privately drains bounded stderr,
enforces a 15-second total observation deadline, and kills/reaps its own child
process group on failure. A descendant holding a pipe cannot extend the wait.
It rejects nonzero exit or malformed output without a partial observation.
The optional check prints only `observer=unverified`, `observer=unavailable`, or
`observer=refused`; all successful data remains private and write admission
still refuses. Loader override variables are removed before launch and the
helper refuses GIO module/profile overrides.

The pure tests cover selected environment preservation/rejection, exact framed
decode and unconditional write refusal. Opt-in GIO tests use the installed
public schema with an in-memory backend for absent/equal-default override and
inspect only the backend type, without reading user proxy values. They are
ignored by default because CI images need not ship GNOME schemas. No current
host proxy values, VM settings or live connection are part of this checkpoint.

The runner's fake-process matrix covers normal framed success, malformed frame,
nonzero exit, stdout/stderr overflow, timeout, an inherited-pipe descendant,
and unsafe executable path refusal. It does not establish installed identity,
session/activation provenance, complete installed snapshot behavior, per-key
journaled effects/restoration, or new-app consumption. The separately documented
NixOS adapter remains unimplemented.

## Fixed local bus continuity candidate

The next inactive slice opens only `/run/user/<effective-uid>/bus`; root or
mismatched real/effective UIDs refuse. Root-owned ancestors must be directories
without group/other write access, the runtime directory must be same-user 0700,
and the endpoint must be a same-user single-link socket. Every component is
opened without following symlinks. An `O_PATH` descriptor pins the socket inode;
GIO connects through its Linux `/proc/self/fd` path and constructs a dedicated
D-Bus connection over that socket, never ambient session-bus discovery.

Before authentication, kernel credentials must identify a same-user positive
PID with a readable nonzero `/proc` start time. Authentication accepts EXTERNAL
only, with a three-second cancellation deadline; socket connection has a
two-second timeout. The existing parent still bounds the entire helper to
15 seconds. Fixed read calls use `NO_AUTO_START` and 1.5-second reply deadlines.
The complete credentials reply is typed and size-bounded; missing, duplicate,
foreign or wrongly typed UID/PID fields refuse. Unknown credential fields are
discarded privately, not treated as additional authority.

Bus `GetId`, manager unique owner, manager UID/PID/start time and kernel peer
PID/start time must remain equal across both reads. The canonical directory and
socket inode are reopened and compared at each checkpoint. No partial values
survive a failed check. `XDG_RUNTIME_DIR` must match the fixed directory, and an
explicit `DBUS_SESSION_BUS_ADDRESS` must be the exact fixed `unix:path` address;
other spellings, fallback lists and transports refuse. This also prevents
ambient dconf bus redirection without changing the process environment.

These are **within-observation continuity checks**, not a transferable host
scope. PID/start-time comparisons are conservative observations, not pidfd
capabilities or protection against a malicious same-user bus/process. In
particular the socket-activation creator PID is not assumed to identify the
broker worker. The helper does not authenticate the manager executable,
broker-launch controller, activation scope, login session or settings profile.
It neither accepts nor manufactures shared-activation authority. Successful
private frames remain version 1 with `Unverified` provenance; they deliberately
do not serialize these temporary identities. `admit_writes` still refuses
unconditionally, including for a separate dbus-daemon.

Synthetic tests use temporary Unix sockets and a native GDBus fake driver:
pinned-socket replacement, kernel peer rejection, unsafe endpoint/directory,
ambient redirection, EXTERNAL handshake/Hello, silent-auth timeout, typed
credential faults, owner/bus changes, failed/disconnected replies and the
no-auto-start flag. The fake same-user manager is explicitly not trusted as
systemd. No real manager environment, desktop setting, private profile, VM or
host session is used by these tests. Installed exact-head endpoint/backend
acceptance, broker provenance and all writable recovery gates remain pending.

The separate opt-in [AUTH sender probe](S1_AUTH_SENDER_PROBE.md) investigates
socket-activation provenance using kernel credentials on an authentication
response. It does not replace this observer's GIO handshake or establish
shared-activation authority.
