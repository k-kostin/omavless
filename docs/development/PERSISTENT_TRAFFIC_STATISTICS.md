# Recorded TUN traffic — selected 0.9.9 scope

Status: local implementation, October30 (Moscow). No source, package or installed
acceptance follows from this contract. The owning research reconciliation is
`default/omavless/usage-statistics-research-review-2026-10-29`. The initial
maintenance hold was released before canonical import.

One opt-in period records observed RX/TX deltas from the existing authenticated
managed-TUN counter path. It is disabled by default. It is not provider quota,
Internet delivery, VPN-egress or proof that all bytes were observed. There is no
daily/monthly history, packet collector, dependency or privileged API.

The Rust runtime collects independently of QML/TUI windows. Enable, disable and
reset require independently settled Off and explicit semantic mutation. They
never connect, reload or change startup. This is Off at admission: another
client's later Connect/Disconnect is never blocked by the metadata write.
Retained operation receipts are checked before admission for a new command,
including when the owner revision or connection changed after a lost reply.
Disable retains the recorded period;
re-enabling resumes it with a partial marker. Reset starts a new empty period.
An absent default is not created by startup or reads. Disabled tracking makes
no periodic counter reads or writes in the persistent worker. The existing
visible live chart keeps its separate session-only behavior.

The fixed private statistics file is separate from `profiles.json`, bounded to
4KiB and excluded from profile Backup/Restore/export and support output. Unknown
schema, malformed data and unsafe paths remain unavailable and untouched. One
writer atomically stores both aggregate and raw baseline. Aggregate u64 values
cross the client boundary as decimal strings. Both clients display exact grouped
integers and the same unambiguous UTC date/time (1970..9999), with translated
surrounding labels; dates never participate in delta arithmetic. Sampling/checkpoints do not change
the profile revision or statistics control revision.

Only ordered observations within the same runtime incarnation and attributed
counter epoch may contribute a nonnegative delta. Restart discards the old raw
baseline; reset, missing samples, identity changes, overflow and unproven prefix
or tail intervals cannot become invented traffic. Partial is sticky until reset.
A normal shutdown does not establish completeness. Repeated samples add zero;
late samples cannot move the baseline backward. An uncertain atomic commit is
resolved from actual disk state without reissuing the old delta.

Statistics must not hold the general dispatcher/lifecycle lock during controller,
sysfs or disk I/O. A single subordinate runtime worker may use the existing
capture / unlocked collect / revalidate pattern, with an owner-issued context
for the current retained child, generation, runtime incarnation and control
epoch. It receives no caller PID/device/path and owns no lifecycle command.
There is one work slot, no catch-up queue and no second service or lifecycle
owner. One monotonic 10-second cadence coalesces missed ticks; it does not run
catch-up work. A stale checkpoint cannot overwrite a newer reset or disable.
A saturated controller or slow storage never holds the dispatcher through I/O;
a terminal worker is unavailable and never automatically restarted. The private
parent inode is pinned and exclusively locked until every possible write settles.

The existing durable Disconnect intent before `stop_owned` is unchanged. Neither
Disconnect nor runtime shutdown waits indefinitely for statistics I/O. Failure
to observe or save statistics is not a VPN/provider failure and does not prevent
ordinary shutdown. No mandatory final sample is inserted before durable Off.
Known recorded bytes survive; unobserved boundary traffic is marked partial.

Before UI or native production, qualify the actual accumulator algebra, fixed
private atomic store and separation of blocked statistics I/O from Disconnect.
Then require semantic IPC/replay/revision tests, affected EN/RU QML/TUI source
and installed review, collection with both clients closed, restart/normal return,
new native archives and coherent pins, exact PR and independent merged-push CI,
and a fresh distribution/source-CI/pre-FIRST inspection/FIRST-bootstrap gate.
Existing closed VPN/AUTO1/Restore/KDF tests are not replayed for this scope.
