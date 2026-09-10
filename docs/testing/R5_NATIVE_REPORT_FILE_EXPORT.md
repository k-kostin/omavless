# Native configuration report: explicit file export

This frontend checkpoint extends the native profile-export UI's confirmed path
and atomic desktop writer to the existing bounded configuration report. It does
not add a full live support bundle or change VPN lifecycle ownership.

Settings offers copy and file export separately. The file confirmation states
that an existing regular file will be replaced and explains the report's limited
scope. Only an absolute bounded path without control characters is accepted.
Destination and report bytes travel through writer stdin, not shell interpolation
or command arguments. The existing Rust writer retains mode 0600, atomic replace,
same-user ownership and symlink refusal.

The read result passes the same strict allowlisted `configurationReport` projector
as clipboard export; raw responses/errors cannot become file contents. Daemon
instance/revision and native admission are checked before reading and again before
handing any bytes to the writer. Changing state before handoff cancels export;
after an explicitly admitted atomic write starts it is allowed to complete.

Python's diagnostic-export chooser is the reference flow. Intentional differences:
this native checkpoint reuses the explicit path confirmation already used for
profile export, and exports configuration facts only (coverage flags remain false
for live host/controller/login observations). Python remains an oracle/rollback;
this does not complete R5/R6 or full support-diagnostic parity.

Validation includes the existing configuration-report privacy corpus plus nine
file-export callback tests: profile behavior unchanged, correct report parser and
fixed helper selection, stale/malformed result refusal, writer admission, bounded
paths and no path/content in argv. Installed atomic file export and EN/RU rendering
are separate gates, recorded against the combined candidate after installation.

Local VM validation note: the full reference run completed 343 tests with four
skips and one failure in the unchanged one-second status-cache sharing test.
Two interpreter launches exceeded that cache window under VM load (six host
polls instead of at most three); an isolated retry reproduced it. Earlier
concurrent runs also hit existing timing-dependent tests. These are recorded,
not relabelled as green or hidden by relaxing the production cache. The focused
export/report and QML contract checks passed; the combined serial gate remains
required before merge.
