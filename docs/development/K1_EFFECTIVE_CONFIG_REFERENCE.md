# K1 effective configuration reference capture

This is a developer-only successor investigation based on
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
manual starts with `RefuseManualStart=yes`, and must never be started. It
explicitly configures WatchdogSec=0 and append-mode paths
under the new private `/run/omavless-k1-effective-config-reference` stage.
No parser for the manager's debug-dump text is assumed or admitted here.
The typed identity must be the exact loaded, inactive/dead, no-drop-in unit,
with all three process IDs and start timestamp zero, typed Job `(uo)` equal to
`(0, "/")`, and an empty ControlGroup. Selected typed values and
the nonempty bounded string dump are emitted privately with
`OBSERVED_CONFIG_DATA_NOT_ADMISSION`. Missing, wrongly typed, empty or oversized
evidence refuses. In particular an empty array's element signature is checked
explicitly, not inferred from successful conversion to an empty Rust vector.
Both `a{sv}` dictionaries reject duplicate names before insertion and permit at
most 512 entries with bounded ASCII property names. Unknown properties are not
reported or treated as configuration admission.

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
Only its acknowledged empty reply permits one further fixed `GetAll("")` on
the same connection and original unique manager owner. That one reply must
contain the complete current loaded/inactive/dead, exact unit/fragment,
zero-PID/start, no-job and empty-ControlGroup facts. A separate create-only
`reference-post-unref-state.json` is synchronized, followed by the acknowledged
Unref record; only then may the helper exit successfully. The combined
dictionary rejects duplicates, permits at most 1024 entries and retains the
same 1 MiB whole-message bound. Earlier interface-specific bounds remain 512.
No extra Ref, retry, reconnection or absent/missing-property success is added.
A failed/uncertain post-Unref query parks with the connection retained, but
does not claim the already-released manager reference is still held. A data
file or even the earlier acknowledged Unref alone is not a success receipt.

This removes a predictable race from the unexecuted `013908c` observer: its
separate post-Unref systemctl/property clients could lose the inactive unit to
GC between calls. v261
[GetAll dispatch](https://github.com/systemd/systemd/blob/v261/src/libsystemd/sd-bus/bus-objects.c#L1475)
treats an empty interface as all interfaces, collecting the vtables in one
reply without returning to the event loop between them. The
[exact-path fallback](https://github.com/systemd/systemd/blob/v261/src/core/dbus.c#L250)
loads the requested unit if GC occurred before dispatch; the response must
still explicitly prove loaded, never-started state, not merely absence.
This is a point-in-time manager observation, not exclusion of concurrent
trusted-root actors; the exclusive VM/unchanged pinned-unit assumptions remain.

The [registered Service tables](https://github.com/systemd/systemd/blob/v261/src/core/dbus.c#L488)
plus Unit have 471 visible expanded property names in the inspected v261
source and no duplicates. Obsolete cross-interface aliases are marked hidden
in [dbus-service.c](https://github.com/systemd/systemd/blob/v261/src/core/dbus-service.c#L438);
[GetAll skips hidden/explicit properties](https://github.com/systemd/systemd/blob/v261/src/libsystemd/sd-bus/bus-objects.c#L884).
This source check justifies strict duplicate refusal, not broad duplicate
normalization or a claim that private VM output was already observed.

Each reply wait has a five-second library timeout. The library timeout does
not cover connection setup or the initial send; the outer observer's direct
owned-child raw-wait deadline is therefore mandatory. The library receives a
message before the helper's 1 MiB check, with a 128 MiB upstream message limit.
The smaller bound limits decoding/output, not initial allocation. Queue size
is eight; output JSON is bounded to 2 MiB. No ordinary test opens a system bus.

**No repeat execution is authorized:** the exact reviewed capture below is
complete; a successor needs complete root review and a new explicit VM lease.
`config_reference_guard.py` and `config_reference_stage.py` implement
the separate fixed outer observer and create-only loader. The probe is the
ordinary test build of `6be4d75e9f0d4b99e1533446ff6269d5e392cda1`, SHA-256
`0585efaa8116b242654e20d58ebb34e861d85e4c911c749b00de3760105f8551`.
The loader admits all four artifacts into memory from stable original FDs
before root publication; only its ELF bound is 128 MiB. The loader itself must
arrive through the trusted host/root delivery channel (`python -I -B`), never
be sudo-executed from the user-writable staging directory. The pin graph is
acyclic: loader → outer guard → original read-side definitions/unit/ELF.
All older guards are loaded as definitions only; their main is never called.

The outer must stop without subsequent queries/effects on helper nonzero, uncertainty,
malformed evidence or timeout. It must not kill/reap a live or unknown child,
release its reference, or use Popen's destructor as a hidden wait path.
Any eventual whole-host preservation comparison must use a before and after
snapshot inside the same invocation. Cross-PAM-session evidence, session-scope
masking or a new allowance for IPv6 RA lifetime renewals is not acceptable.

The observer launches the helper directly from the retained original ELF FD,
with fixed argv/environment, root credentials and no supplementary groups.
No claim is made that this root helper has a zero capability set. Its only
manager mutations are the fixed RefUnit/UnrefUnit pair; it cannot run a unit.
No initial daemon-reload occurs: the RefUnit call loads the freshly published
own unit. Any unfamiliar cache/load result refuses rather than retrying.
Only known helper exit zero, the separate acknowledged-Unref and post-Unref
state records, strict bounded typed metadata, never-started
zero-PID/no-job/no-cgroup state, unchanged
original unit link/parent FD identity and point-in-time original ELF inode
absence permit removing that exact own link through the retained parent FD.
The outer makes no split manager state/Job queries between helper success and
unlink; all those facts come from the one typed post-Unref reply.
Only then may the outer issue its fixed daemon-reload, verify not-found and
compare the same-invocation full after-baseline. No stop, signal, reset, source
cleanup or failure compensation exists. All private output/artifacts remain.
The narrow proc check reads UID/stat/executable metadata, not command lines;
twice-stable zombies and all-root PF_KTHREAD processes are the only accepted
no-executable exceptions. It is not global descendant quiescence.

Pure controls exercise the actual capture sequence against synthetic typed
replies, including every call failing/malformed, output collision, wrong unit,
wrong empty-array signature and prior-start facts. Their dump is deliberately
opaque synthetic text, not evidence of actual systemd output grammar.

## Exact capture checkpoint — October 4

After full parent and independent source review, exactly one dedicated x86_64
Omarchy KVM capture at source
`b0af7fa7c43aae4e4ea0a15d1fcbf14aa14bbeb3` returned exit zero and
`K1_CONFIG_REFERENCE_CAPTURE_PRESERVED_NOT_ADMISSION`. The native ELF is the
`6be4d75` build pinned above; guard SHA-256 is
`acd2a92ab12c63cf948bc3c64339fa8364b14b243a976a90168ab3e75342ba6f`,
and trusted-host-stdin loader SHA-256 is
`1ce68472453bf56cb5c11b3109ff260459cf53801be16ac7c9ea6026cd137434`.
The inert unit was never started. The fixed Ref/acknowledged Unref sequence,
post-Unref single-reply never-started state, exact own-link cleanup and full
same-invocation preservation comparison succeeded. No lifecycle, configured
admission, canonical namespace authority or complete K1 acceptance follows.

The exact source gate passed 589 Python tests with two skips and frontend/QML
checks. The complete Rust gate exited zero: 129 suite summaries, 2040 passed,
77 ignored, with formatting, strict Clippy, terminal tests and parity passing.
These counts and VM results belong to that immutable head, not later changes.

A separately reviewed success-only export returned known exit zero. The private
archive SHA-256 is
`1996d7799850ea9d091afa844b8ca1c53119decaaaaf59e4131a4b0f84173c2b`
(67,594,240 bytes). Both reviewers independently checked all 13 exact members,
root ownership/modes, artifact pins and seven strict JSON records, and reran
the pinned preservation comparator successfully across all 15 categories.
The dump and raw baselines remain private and are not committed. The first
local transport decoder refused terminal CR framing; the corrected bounded
offline decoder did not repeat any guest command. All earlier #604 NONPASS
evidence and stages remain unchanged. No separate post-export current-process
quiescence observation is claimed.

## Narrow configured-text proposal, not admission

The actual capture supplies one nonempty LF-terminated unit dump with a single
exact unit header, tab-indented scalar fields and one nested ExecStart command.
Its selected fixed fields report configured WatchdogSec zero, append stdout and
stderr at the declared private paths, RefuseManualStart yes, oneshot, root/root
and `/usr/bin/false`. Runtime WatchdogUSec remains u64 maximum both before and
after Unref; it is not configured zero. These are scoped observations only.

The frozen helper did **not** read the manager's Version property on its pinned
connection. Therefore passing an assumed version string to an offline parser
cannot upgrade this capture into version-authenticated admission. A future
immutable capture generation must obtain the exact typed Manager.Version from
the same unique owner on the same connection, retain unchanged raw unit bytes
and original file identity, and bind the dump to the exact read-side unit and
fragment observations. Unknown versions refuse, without broad major-version
or prefix matching. A later lifecycle still needs separate phase-specific
runtime checks, complete effective-unit admission and all ownership gates.

The cfg(test)-only `manager_config_dump_proposal` is deliberately separate from
the executed helper, observer and loader. Its input version `261.2` and unit
digest are explicitly **assumptions**, not authenticated evidence. It bounds
total text to 64 KiB, 256 lines and 4096 bytes per line; requires one exact unit
header, tab indentation, ten unique selected fields and the one adjacent
inert ExecStart command; and rejects unknown field labels, alternate stdio-file
forms, lookalikes, hidden control characters and substring/path-prefix matches.
Other allowlisted field values are opaque and confer no fact. Only the six
observed dependency-label classes may repeat, at most 16 times each. The five
pure control groups use synthetic public input, not the private raw dump. The
parser performs no I/O and is not called by the live capture or production code.

The primary v261 [unit dump](https://github.com/systemd/systemd/blob/v261/src/core/unit-serialize.c#L466)
prints the unit ID, fragment and manual-start setting, then invokes the service
dump with an additional tab. The
[service dump](https://github.com/systemd/systemd/blob/v261/src/core/service.c#L1211)
uses the configured watchdog member and nested command lists. The
[execution-context dump](https://github.com/systemd/systemd/blob/v261/src/core/execute.c#L1166)
selects distinct file/append/truncate labels from the configured stdio mode.
Its [command dump](https://github.com/systemd/systemd/blob/v261/src/core/execute.c#L2277)
quotes argv; it does not establish every execution flag. Consequently the
exact raw unit pin and complete typed effective-unit checks remain mandatory
before any future execution claim. No generic arbitrary-unit text trust is
proposed. The [manager implementation](https://github.com/systemd/systemd/blob/v261/src/core/manager-dump.c#L68)
explicitly treats this format as unstable developer output; matched-unit dumps
omit its version header. Unknown package/version/grammar must refuse, never
be normalized into a known format.
