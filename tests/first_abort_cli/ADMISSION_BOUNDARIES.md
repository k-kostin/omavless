# Fresh read-only stopped-owner boundary observation

This source generation follows the narrow inode-padding correction. That
confirmed parser bug is not the proved cause of #630's normal-CLI NONPASS.
The separately reviewed fixed file observation found the synthetic setup PASS
transcript, a first normal CLI invocation without the OLD-restored message,
and the public `RuntimeNotStoppedOrUnsafe` error category. The latter precedes
the first-Abort bridge and intentionally collapses several admission failures.
No exact failed inner predicate, current child/account state, quiescence or
preservation is inferred from those files. UID48045 and all old artifacts are
retained, never reused or queried by this new generation.

## New fixed generation

- UID/GID48046, account `ov-t4-abort-v3`, HOME `/home/ov-t4-abort-v3`.
- Runtime `/run/user/48046`; fixed artifacts below that new HOME.
- Delivery `/home/kdk_vm/.cache/t4-first-abort-cli-delivery-v5`.
- Root stage `/run/ov-t4-cli-guard-v5`; delivery schema v5.

The native helper must be rebuilt and independently frozen along with the
normal CLI from this generation's exact source. No previous 69557/2bf ELF is
relabelled. New original-FD artifact and source receipts, all fresh account /
instance / path absences, capacities, original catalogs, baselines and reviews
remain required. There has been no invocation for this generation.

## Shared checks, test-only observation

One new ignored entry `diagnose_stopped_admission` first uses the same fixed
native credential, namespace, original FD198/199 and executable hash admission
as the other ignored entries. It then invokes the same existing runtime lock
acquisition and stopped-owner capture logic, without reading a backup archive,
calling recovery, changing a fence or constructing an owner permit.

Only a cfg(test) enum variant admits the exact fixed helper/harness argv as
the diagnostic self. Production capture retains its exact four-token recovery
invocation rule. The complete same-UID inventory, retained lock, manager/namespace
anchors, fixed user-unit checks, listener checks and conservative unknown
refusals remain shared, not copied or bypassed. Production builds contain no
trace state, environment flag, callback selector, diagnostic entry or override.

The test-only thread-local sink emits bounded fixed BEFORE labels for each
major admission operation: runtime/lock, original self/proc visibility,
namespaces, manager query/record/process/image, each user-unit query/record,
inventory, Unix-table read/parse and final identity/budget/lock checks. Values
are a finite enum: no PID, private path, argv, response bytes or error text.
Repeated checks remain distinct records, not inferred from an earlier pass.
At most 128 BEFORE writes are possible; each write is attempted once and its
full count checked. A short/error write seals before the next operation and
does not emit another failure line. An operation failure emits at most one
fixed FAILED_AT label. Failure exits the helper without the generic fixture
stderr/harness followup. It never signals/reaps another process or retries.
The production observer's pre-existing exactly-known failed-query reap policy
is unchanged; this does not claim the outer zero-only policy for that code.

After synthetic mixed-Intent setup the root guard captures original lineage,
runs this read-only entry, and accepts only its exact known-zero child plus
the complete fixed success transcript. It rechecks the original pre-first
lineage afterward. A failed/unknown diagnostic stops before the normal CLI.
If successful, the normal CLI still independently repeats every admission
predicate; no test-only proof is passed into it. The diagnostic observes its
own new invocation, not the historical #630 state or cause.

## Required gates

Pure controls cover exact self argv, sink failures at every phase, bounded
output, permanent refusal, no late sink/operation, strict success transcript
and no normal CLI call after diagnostic failure. Full source and native
workspace/Clippy/TUI/parity gates, original-FD frozen receipts, parent/peer full
review and a new explicit exclusive VM lease remain required before effects.
Old failures and source heads remain immutable. No product PASS is claimed.
