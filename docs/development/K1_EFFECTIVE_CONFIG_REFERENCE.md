# K1 effective configuration reference capture

This is a source-only, developer-only successor investigation based on
`c54697a9a3ffb7af082cd33ea43b87a19da2acdd`. It does not repair or repeat the
original #604 NONPASS, and does not authorize native lifecycle execution.
The old source, unit, probe and retained VM evidence remain immutable.

## Why one connection and a reference

The systemd v261 manager's
[matching dump implementation](https://github.com/systemd/systemd/blob/v261/src/core/manager-dump.c)
iterates currently loaded units; it does not load a matching unit from disk.
Separate property and dump clients therefore cannot establish continuity for
an inactive, collectable unit. An empty dump is not configured proof. The
observed old empty dump is consistent with collection between clients, but
does not itself prove that a collection event occurred.

The new cfg(test)-only `manager_config_reference_fixture` opens one literal
system-bus connection, resolves the manager's unique owner once, and addresses
that owner throughout. One fixed `RefUnit` loads and references only
`omavless-k1-effective-config-reference.service`; two typed `GetAll` calls
capture selected Unit/Service facts; one matching dump asks for that exact
literal name. The helper never reconnects or follows a replacement manager.
The [Ref/Unref implementation](https://github.com/systemd/systemd/blob/v261/src/core/dbus-unit.c)
tracks the calling bus sender. A separate short-lived `busctl RefUnit` would
not preserve that reference for another client's dump.

RefUnit is an explicitly authorized manager-state mutation, not a read-only
operation. It has manage-units/ref authorization and SELinux start-access
checks, but does not start a job. The helper exposes no installed service,
caller-selected unit/path/method, shell, network or privileged IPC interface.
Its existing workspace-pinned zbus 5.19.0 dependency is dev-only. The old native
creator/ownership APIs and production dependencies are unchanged.

## Capture, not configured admission

The distinct unit is deliberately inert (`ExecStart=/usr/bin/false`), refuses
manual starts with `RefuseManualStart=yes`, and must never be started. It explicitly configures WatchdogSec=0 and append-mode paths
under the new private `/run/omavless-k1-effective-config-reference` stage.
No parser for the manager's debug-dump text is assumed or admitted here.
The typed identity must be the exact loaded, inactive/dead, no-drop-in unit,
with all three process IDs and start timestamp zero. Selected typed values and
the nonempty bounded string dump are emitted privately with
`OBSERVED_CONFIG_DATA_NOT_ADMISSION`. Missing, wrongly typed, empty or oversized
evidence refuses. In particular an empty array's element signature is checked
explicitly, not inferred from successful conversion to an empty Rust vector.

In v261 the Service WatchdogUSec getter reflects the runtime original/override
state, initialized to infinity before a start. The dump reports configured
WatchdogSec instead. This capture preserves the exact u64 getter value without
calling it configured zero. A later lifecycle generation must separately prove
configured zero and phase-specific runtime infinity-before/zero-after facts.
See [dbus-service.c](https://github.com/systemd/systemd/blob/v261/src/core/dbus-service.c),
[service.c](https://github.com/systemd/systemd/blob/v261/src/core/service.c) and
[execute.c](https://github.com/systemd/systemd/blob/v261/src/core/execute.c).
The latter's append-file dump fields are not fabricated D-Bus File getters.

Only a reviewed actual capture can inform a later strict, version-pinned dump
parser. It cannot automatically admit paths, permit an empty-dump fallback or
authorize the original/later FullVPN lifecycle. A later runnable generation
needs new exact source/artifact pins and complete admission review.

## Uncertainty and evidence

The actual connection and stage FD are leaked before RefUnit. After that point
any call error, wrong reply, write failure or caught panic stops the sequence
and parks indefinitely, retaining the connection and its executor. There is
no compensating Unref, reconnect, retry, signal or destructor-based release.
Retention does not assert that the manager or bus remains alive, or that an
uncertain Ref/Unref took or did not take effect. This is intentional quarantine
in an exclusive development VM, not production recovery.

After the complete capture is written create-only through the retained stage
directory FD and synchronized, one UnrefUnit is sent on the same connection.
Only its acknowledged empty reply permits a separate create-only acknowledgment
record and successful helper exit. A data file alone is not a success receipt.

Each reply wait has a five-second library timeout. The library timeout does
not cover connection setup or the initial send; the outer observer's direct
owned-child raw-wait deadline is therefore mandatory. The library receives a
message before the helper's 1 MiB check, with a 128 MiB upstream message limit.
The smaller bound limits decoding/output, not initial allocation. Queue size
is eight; output JSON is bounded to 2 MiB. No ordinary test opens a system bus.

**Not ready for execution:** root outer observer/create-only loader, sealed
probe pins, full source review and explicit VM lease are required. The outer
must stop without subsequent queries/effects on helper nonzero, uncertainty,
malformed evidence or timeout. It must not kill/reap a live or unknown child,
release its reference, or use Popen's destructor as a hidden wait path.
Any eventual whole-host preservation comparison must use a before and after
snapshot inside the same invocation. Cross-PAM-session evidence, session-scope
masking or a new allowance for IPv6 RA lifetime renewals is not acceptable.

Pure controls exercise the actual capture sequence against synthetic typed
replies, including every call failing/malformed, output collision, wrong unit,
wrong empty-array signature and prior-start facts. Their dump is deliberately
opaque synthetic text, not evidence of actual systemd output grammar.
