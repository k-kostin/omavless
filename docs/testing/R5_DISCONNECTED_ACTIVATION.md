# Explicit disconnected native activation

This R5 checkpoint exposes exactly `omavless cutover activate`. It admits a
disconnected legacy installation and invokes the existing cutover transaction,
fixed production host and generation-fenced frontend selector. It adds no IPC
method, caller paths, service names, shell/JSON input, force flag or rollback
shortcut. Python remains the installed owner until successful execution and
the retained rollback/oracle afterward. This checkpoint does not prove R5/R6,
login activation, connected adoption or provider interoperability.

## Admission

The running executable must be the root-owned, non-writable-by-group/other
`/usr/bin/omavless`, with the same device/inode as `/proc/self/exe`. The fixed
`/usr/lib/systemd/user/omavless-runtime.service` must be a root-owned ordinary
file matching the exact checked-in unit embedded when building that binary.
Integration with a changed package unit requires rebuilding the binary.
`OMAVLESS_HOME` overrides refuse. Before constructing the migration host or
creating its operational lock, the CLI makes exactly one bounded read-only
`/usr/bin/systemctl --user show-environment` query. It projects only HOME,
XDG_CONFIG_HOME, XDG_STATE_HOME, XDG_CACHE_HOME and XDG_RUNTIME_DIR, rejects a
manager OMAVLESS_HOME override, and requires agreement with the CLI's effective
roots. Missing manager HOME uses the current account home; absent XDG values use
their actual HOME-based defaults and `/run/user/UID`. Explicit empty/relative
paths, parent traversal, duplicate keys, malformed relevant assignments and
unsupported shell quoting/escaping fail closed. Unrelated environment values
are never interpreted or logged. The query shares the fixed service reader's
deadline and 64-KiB output limit; a descendant retaining stdout cannot extend it.

Under the shared migration lease, activation requires legacy ownership,
repeated strict process/TUN inventories, both fixed services inactive with zero
MainPID, and absent legacy/native controllers and native control socket. Any
unavailable inventory refuses. Both units must report disabled startup, no
drop-ins and no pending daemon reload; the runtime fragment must be the fixed
packaged path. This operation never enables or disables a unit.

The complete private store must validate, have explicitly configured disabled
startup, and require no compatibility-pointer repair or missing-profile pruning
when the candidate reconciles disconnected state. Existing desired state must
be valid and disconnected. A login receipt or interrupted-preset barrier
refuses. Template mode and desired-generation capacity are checked before
preparing ownership. Private data is neither printed nor repaired to force
admission. Use the existing legacy operations and `store-compatibility` guidance
to resolve ordinary prerequisites before retrying.

## Failure and recovery contract

| Boundary | Result and recovery |
| --- | --- |
| Invalid command, package mismatch, unsafe/incompatible store, connected host, startup enabled/unconfigured, unit mismatch, receipt/barrier, incomplete observation | Refusal before ownership/service/bridge effects. Existing helpers may prepare the fixed operational lock/state directory. |
| CLI/user-manager HOME or XDG roots differ, environment query fails, or relevant encoding is ambiguous | Refusal before migration-host construction, operational lock/state preparation or service/ownership effects. |
| Desired staging or candidate start/bootstrap/hello/status/bridge failure with unchanged preparing marker | Existing reverse compensation returns the bridge to legacy, stops native service, verifies strict emptiness and acquires any existing native owner lock, restores exact original desired bytes or original absence, then commits verified legacy ownership. |
| Desired publication reports an error after replacing the file | The host retains the exact candidate before publication; compensation can identify and restore that candidate. Unknown current bytes are never overwritten. |
| Native stop, socket/controller/process/TUN absence or owner-lock proof fails | Manual recovery; no legacy restart or ownership restoration. |
| Lock handoff cannot be reacquired, marker diverges, or compensation cannot be verified | Manual recovery; never guess a replacement owner. |
| Marker write reports an error | Existing coordinator rereads durable state: exact target is committed, unchanged source permits the corresponding compensation, ambiguous state requires manual recovery. |
| CLI dies while preparing | Durable preparing ownership keeps both mutation paths closed. No automatic resume/rollback exists and no claim of crash-safe automatic recovery is made. |
| Commit succeeds but client loses output | Ownership is already Rust. Read the committed launcher target and runtime status; do not repeat activation or infer rollback from missing output. |

The desired snapshot is private, bounded and **in memory**, not a durable backup
or recovery journal. The existing preparing-marker contract supplies crash
refusal, not reconstruction of a lost snapshot. Operator recovery must verify
the exact private state, native service/process/controller/TUN absence, and
frontend generation before choosing a separately reviewed recovery action.
There is deliberately no command that deletes a marker or blindly restores
legacy ownership. Connected adoption remains unexposed: its existing internal
config-regeneration and legacy-restart compensation requires separate review
and exact host evidence.

## Validation and remaining host gate

Synthetic tests cover exact command/unit facts, activation success using fixed
service/private-socket doubles, startup/receipt/preset/TUN refusal, the existing
transaction fault matrix, exact desired formatting/absence restoration,
unrecognized replacement refusal and an externally held native owner lock.
An early legacy-stop failure with a pre-existing idle owner lock verifies that
compensation reuses its retained admission lease rather than conflicting with
itself. Reuse requires the fixed path to retain the locked device/inode and
private file policy; disappearance, replacement, symlink or unsafe mode refuses.
Final disconnected-candidate admission uses a separate repeated strict proof:
native service exactly active with a stable nonzero MainPID and successful
service result, legacy service exactly inactive with zero MainPID, native
control socket present, both core controllers absent, and complete empty
process/TUN inventories. It never turns an unavailable final inventory into
zero. A regression makes proc inventory incomplete after candidate startup and
before commit; Rust ownership is refused and the preparing barrier remains when
cleanup cannot prove emptiness either.
Environment tests cover account/default versus explicit matching roots, every
root mismatch, malformed/duplicate/quoted values and rejected overrides. A
transaction-entry counter proves a failed agreement cannot enter host effects;
the synthetic query checks exact argv, output bounds and unchanged fixture state.
The pre-existing language-neutral ownership/transaction contract is the
reference for this composition; no new Python behavior is substituted.

The activation helper initially ran only static checks; the integrating agent
subsequently executed the installed combined-candidate acceptance below. Keep
private profile/subscription/config data and process environments out of public
logs. Neither synthetic tests nor this bounded installed pass authorize Python
removal or replace the remaining complete frontend, login and recovery gates.

## Try Omarchy ARM64 installed acceptance — 2026-09-09

This evidence belongs to the **combined installed candidate**, including package
host, native frontend actions and explicit activation. It is not a claim that
standalone activation PR #207, or a later source head, independently passed the
same host gate. The environment is Try Omarchy on ARM64 under virtualization;
no bare-metal or NixOS acceptance is inferred.

| Artifact | Exact identity |
| --- | --- |
| Combined tested source | `ffcf4d2654b74c0fb746664eb73545eb9028fbc6` |
| Installed binary SHA-256 | `3ef3ac70b063c910b90fa032d52b4edbe253291e63eee1cfe94cf94abbc0a223` |
| Pacman package version | `0.0.0.r350.gffcf4d2654b7-1`, ARM64 |
| Installed runtime unit and QML | Exact comparison against the combined candidate passed |

The existing private store and template passed native validation. Startup was
already explicitly configured and disabled. Read-only inspection found a stale
active pointer despite stopped legacy service and no core/TUN; ordinary legacy
`down` cleared it before activation. No private JSON or ownership marker was
manually edited to satisfy admission.

The installed `/usr/bin/omavless cutover activate` completed with
`rust_committed`, ownership generation 2. Legacy service remained inactive,
native service became active, and both startup units remained disabled. The
installed plugin reported `nativeControls=true`, `metadataUnavailable=false`
and `localFactsCurrent=true`; native buttons were visible. The owner subsequently
confirmed successful connection and disconnection through those buttons.

The integrating agent's bounded private VLESS Full VPN probe passed on the
installed normal runtime path: connect took 146 ms and disconnect 4,182 ms in
the recorded successful run. It verified one cgroup-owned Mihomo core, one TUN,
and the private Unix controller's peer PID. HTTPS to the public test destination
`example.com` explicitly bound to the actual TUN succeeded while that TUN's
receive and transmit counters increased. Final cleanup passed. These observations
support this one private fixture and mode; they do not establish every provider,
protocol, Routing/Direct connectivity or physical network behavior.

The listener check used read-only privileged socket PID/inode evidence to
attribute the configured loopback mixed proxy and the system TUN forwarder to
the owned core. No TCP controller was present. The retained template and
generated config agreed on the loopback mixed listener and `allow-lan: false`;
neither configured a DNS listener or extra explicit TCP listener. Mihomo
[v1.19.30 pins sing-tun v0.4.22](https://github.com/MetaCubeX/mihomo/blob/v1.19.30/go.mod),
whose [system stack creates an ephemeral TCP forwarder on the TUN address](https://github.com/MetaCubeX/sing-tun/blob/v0.4.22/stack_system.go#L112-L165).
Source explains the expected listener category; actual PID/inode attribution
supplied the installed ownership proof.

Earlier probe attempts incorrectly rejected these expected TCP listeners. A
separate synthetic-interface-name assumption also rejected the real TUN name.
Only acceptance harness checks were corrected; no runtime networking policy or
security boundary was relaxed to pass. The successful gate retained exact
owned-listener attribution and rejected unexpected listeners rather than
allowing arbitrary TCP sockets.

The disconnected restart gate also passed: a semantic mode change and restoration
succeeded, restarting produced a new daemon instance with the same committed
Rust ownership, legacy stayed inactive, and core/TUN counts remained zero. This
is disconnected restart evidence, not connected adoption or once-per-login
autoconnect proof.

Combined validation recorded 744 Rust tests passed with four ignored; the Python
suite completed successfully with 326 tests and one root-only skip. QML,
localization, formatting, strict clippy and differential/parity gates passed.
Two installed-core opt-in tests also passed. Private profile IDs, labels, URIs,
server endpoints and process environments are omitted from this report.

Outstanding acceptance includes
remaining frontend operations and lifecycle coverage, login activation,
connected restart/recovery cases beyond those exercised here, and R6's deliberate
Python-absence matrix. Python remains retained rollback/oracle; R5 and R6 are not
complete, and production TUI implementation remains gated behind R6.
