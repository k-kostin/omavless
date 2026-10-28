# Daily subscription refresh

This supporting T4 slice adds one opt-in daily schedule per existing subscription.
The accepted selected-refresh implementation remains its only executor. This is
not AUTO1, a network-return trigger, durable job recovery or physical suspend
acceptance. The initial supported settings interface is the ordinary CLI; a TUI
or QML schedule editor and configurable intervals are separate work.

## Ordinary interface

```text
omavless subscription list
omavless subscription schedule SUBSCRIPTION_ID
omavless subscription schedule SUBSCRIPTION_ID daily OPERATION_ID REVISION
omavless subscription schedule SUBSCRIPTION_ID off OPERATION_ID REVISION
omavless operation get INSTANCE_ID OPERATION_ID
omavless operation cancel INSTANCE_ID OPERATION_ID
```

Read the current revision through hello/status/settings before a settings change.
Use a new settings operation ID and retain it if the reply is lost. An exact
settings replay returns its original result without resetting the deadline.
Lost or malformed settings replies produce CLI exit73; the CLI never resends.
The settings read exposes the original scheduled instance/operation handle and
its ordinary operation projection while retained. It accepts only an opaque
subscription ID; configuration additionally requires enabled, operation ID and
expected revision. URL, clock, path, deadline, interval and Connect inputs are
not accepted. These are private same-user settings, not support diagnostics.

Default is Off. Explicit Off-to-Daily makes the first slot due immediately,
except that a backward clock cannot reuse an already admitted timestamp. The
next eligible slot is at least24hours after admission and terminal completion.
Settings changes return Busy while the existing worker is active; Off does not
implicitly cancel a job. Use the existing Cancel, wait for its terminal receipt,
then turn the schedule off. Future scheduler passes remain disabled until an
explicit opt-in. Closing a client does not cancel the runtime operation.

## Admission and time

The runtime's existing serve-until loop already wakes at bounded intervals. It
now re-evaluates schedules at most once per second, without a new thread,
service, timer dependency or privileges. `Instant` only limits this inexpensive
recheck; persisted UTC wall deadlines determine eligibility. After resume or a
clock jump, the next recheck reads the current wall time and canonical store.
It does not wake a sleeping machine. This consciously reuses an existing wake
source instead of adding the research report's proposed timerfd adapter.

Under the same owner mutex and migration lease, admission checks ownership,
recovery barriers and the shared worker's availability, then atomically writes
`refreshSchedule.lastAttempt=admitted` and a strictly future `nextDueAt`. Only a
confirmed write advances the shared revision and captures the selected feed
for the existing worker under that same lease. No network request precedes the
durable slot consumption. Manual and scheduled work share one registry, thread,
fetch pool and unchanged deadlines. Busy does not consume a slot.

Missed days collapse to one new admission, rather than replaying every missed
day. Forward/backward clock changes cannot reissue a consumed slot. If a clock
jump occurs during work, terminal publication moves the next deadline into the
future too. No fast retry, backoff, jitter or network watcher is introduced.
Refresh and schedule completion have no connection authority.

## Results and restart

Confirmed feed publication and `committed` schedule metadata use one existing
atomic store replacement. Cancellation or a known failure before commit keeps
the feed unchanged and records its terminal disposition when the current owner
can safely persist that bookkeeping. Store I/O ambiguity retains the existing
manual-recovery barrier; it does not become success or a retry instruction.

The private receipt is not a durable operation registry. Restart keeps the
consumed slot and future due time, while get/cancel for the old runtime instance
returns not_found. A remaining foreign-instance `admitted` record projects as
Unknown. Eviction also cannot turn admission into success. A crash between the
admission write and the GET may skip that day's update: this is deliberately
at-most-one admission, not exactly-once execution. The next normal daily slot
may create a new operation; the old slot is never automatically resent.
Graceful shutdown can instead retain a known failure before commit when its
original worker drains and the owner safely saves that terminal result.

Existing revision, exact selected URL/timestamp, cancellation and ownership
checks still fence publication. Other subscriptions, profiles, startup policy,
current desired VPN state and private extension data are preserved.

## Completion boundary

Required source evidence covers default Off, due math and clock changes,
reconstruction/no replay, admission before fetch, manual-vs-scheduled ownership,
cancel/fetch/URL conflicts, post-rename ambiguity using the existing store seam,
exact private protocol/CLI shapes and real HTTP/private Unix-socket composition.
Installed acceptance uses the packaged Rust runtime in the existing Dev VM:
explicit daily admission, ordinary handle/cancel, same-session runtime restart,
old-instance not_found, no duplicate GET and unchanged disconnected intent.
Reviews, affected checks, exact source CI and normal development/final
distribution admission remain separate required gates. Source tests and VM
evidence do not establish physical suspend or whole T4/AUTO1 completion.
