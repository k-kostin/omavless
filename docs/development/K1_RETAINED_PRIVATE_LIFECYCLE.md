# K1 retained-reference private lifecycle successor

Source-only successor to #625 `76f2c73a7cff7eafbb280eda0e4bf769d55d02aa`.
It reuses the existing #604 creator composition, not a second nft writer.
The first checkpoint `bc1a642395b4fbc418ceb183d498eba84b543951` contained only
a cfg(test) coordinator and deterministic ordering controls. The successor
implementation adds its fixed bus/file adapter and outer guard. **It remains
source-only until sealed full gates, fresh executable pins and complete review;
no VM invocation is authorized by this document.**

## Evidence and remaining boundary

The #625 fixed configured-facts capture completed with known-zero helper and
outer exit, nine valid RPC receipts, acknowledged Unref, exact own-link
retirement and full before/after preservation. Its private 44-member evidence
archive SHA-256 is
`46d98a44148eb03680f72075256dfc0881ab4bd86a59105d365e98017abc62e5`.
Raw dumps, baselines and logs remain private. These observations are not a
lifecycle, host-network or production kill-switch PASS. All older NONPASS
invocations and retained stages remain unchanged.

The existing `kernel_response_diagnostic_fixture` already composes the real
`FixtureCreator`, retained `LocalReadSession`, complete inventory and
`LockedState`. Before its first netlink socket and at transaction boundaries it
checks actual namespace separation, inherited negative-witness FD, credentials,
CAP_NET_ADMIN-only/NNP/filter and loopback-only isolation. Its one sequence is
absent → durable Pending and exclusive atomic FullVpn create → complete
inventory → independent second-socket ownership refusal → conditional
same-session delete → absent and Closed/Retired. Unknown outcomes retain the
actual creator, state and namespace/socket owners in a parked process.
The synthetic epoch and PID1 namespace negative witness remain non-authorizing.

### Material private-witness provenance change

The restricted writer no longer dereferences `/proc/1/ns/net`. This is a
material change, not just a literal tuple rebind: Linux
[proc namespace links](https://github.com/torvalds/linux/blob/v6.17/fs/proc/namespaces.c#L39)
require `PTRACE_MODE_READ_FSCREDS`, and
[commoncap](https://github.com/torvalds/linux/blob/v6.17/security/commoncap.c#L154)
requires the caller's effective capabilities to cover the target's permitted
set or CAP_SYS_PTRACE. CAP_NET_ADMIN alone therefore cannot generally read PID1's
namespace link. No CAP_SYS_PTRACE or CAP_SYS_ADMIN is added.

The test-only root adapter retains original PID1 and own-current network
namespace FDs, checks nsfs plus the exact network namespace label and equality,
then exclusively publishes `host-negative-witness.json` through its retained
root-private stage FD before Ref. This bounded schema-1 root0600 receipt binds
the fixed unit, original stage device/inode and namespace device/inode, with
`negative_witness_only=true` and `canonical_authority=false`. Both processes
retain original receipt FDs and stage ancestry; metadata, no-xattrs, digest,
duplicate/unknown/type rejection and exact identities are rechecked. Root also
rechecks its original/current namespace anchors immediately before Ref and at
each subsequent lifecycle-adapter RPC/effect boundary (the read-only admission
prefix remains its own bounded sequence). Its retained FD prevents namespace
inode reuse.

The restricted child retains inherited FD3 before any other opener, reads only
this fixed receipt, and requires its own network namespace to differ. Receipt
checks precede the first netlink socket and existing transaction checks. This
is an explicitly trusted-root/exclusive-VM **negative fixture bridge**, never
canonical namespace authority, adoption or a production IPC surface. Tests
cover malformed/short/missing/duplicate/wrong/stale records, original-FD
replacement/mutation, modes, xattrs and failure before simulated socket access.

What #625 does not provide is a retained-reference launch/completion bridge.
It Unrefs before its inert-only cleanup. The old #604 Python observer is not an
eligible substitute: its property representation and owned-wait composition
predate the corrected admission path. The next adapter must keep the same
unique-owner connection and reference through the actual lifecycle.

## Fixed composition

1. Create the fresh literal `omavless-k1-retained-private-lifecycle.service`,
   root stage `/run/omavless-k1-retained-private-lifecycle` and delivery source
   `/home/kdk_vm/.cache/k1-retained-private-stage-v1`. Retain original root-owned unit,
   ELF, link and evidence-directory FDs; reject reuse or any unknown ancestry.
   Preserve the full before baseline before publishing the own link.
2. Pin the manager's unique owner and exact `261.2-1-arch` version. Ref once;
   repeat the complete supported typed security/configured-Dump admission,
   including exact names/dependencies, empty hooks/drop-ins, permissions,
   namespace/FD/environment restrictions and never-started/no-job state.
3. Recheck original artifacts immediately before exactly one fixed
   `StartUnit(UNIT, "fail")`. Decode one canonical nonzero u32 job path.
   Never use replace/restart/transient units or caller-selected arguments.
4. Bounded same-owner typed observations distinguish exact known pending states
   from failure/uncertainty. Completion requires durable RemainAfterExit
   active/exited state, no current job, fresh invocation/timestamps, normal
   exit zero, zero manager live PIDs and an empty own cgroup. Runtime fields
   cannot be compared blindly to the never-started snapshot: stable security
   configuration is checked separately without weakening its predicates.
5. Verify the original native receipt and Closed/Retired/absence evidence before
   the sole fixed Stop. Observe known stopped/no-job/zero-PID state, recheck
   original provenance and evidence, then and only then acknowledge Unref.
6. Only known-zero coordinator completion with strict terminal evidence permits
   the outer observer's exact own-link retirement, fixed reload and full
   after-baseline comparison. The inert #625 cleanup proof must not be carried
   into this active lifecycle. No teardown is authorized by partial receipts.

No signal subscription is needed for this proposed completion mechanism:
RemainAfterExit preserves the successful oneshot state, and the exact returned
job plus typed current state and native evidence are checked. There is no
GetJob-after-disappearance or missing-object-as-success fallback.

Any failed/malformed/unknown RPC, unexpected state, evidence failure or deadline
permanently seals the coordinator before any later call. In particular, an
uncertain Start reply may already have caused effects; it permits no follow-up
query, Stop, Unref, signal, retry or cleanup. The real entry must leak/retain its
connection and original owners before Ref and park on error/panic. A timeout
does not cancel the manager job. Known pending observations alone permit another
bounded observation. Phase evidence is exclusive and finite; receipt failure
is itself terminal, not permission to recover.

## Trust limitation and tests

RefUnit prevents collection; it is **not a configuration lock or atomic CAS**.
Upstream v261 [Ref implementation](https://github.com/systemd/systemd/blob/v261/src/core/dbus-unit.c#L642)
tracks the bus sender, while
[StartUnit](https://github.com/systemd/systemd/blob/v261/src/core/dbus-manager.c#L697)
is a separate job-enqueue operation. The bounded fixture requires trusted root,
an exclusive development VM and no concurrent privileged manager/configuration
mutation. It does not resist a root actor changing configuration between reads
and Start, or prove absence of future activation from unloaded hostile units.
The actual child checks before sockets/effects remain mandatory regardless of
typed manager observations. No host namespace adoption or product authority is
created by this assumption.

The coordinator controls cover exact successful ordering, every callback
error and panic, permanent non-reusability, canonical job paths, pending polling
and both count/elapsed bounds. They use no system bus, process, namespace, nft,
unit or guest operation. Adapter controls add strict typed phase/unchanged-field
counterexamples and real original-FD replacement, reversion, contents, mode,
bound, symlink and hard-link rejection. Full source/Rust gates, a fresh frozen
ELF and acyclic pins, complete parent and independent review, and a separate
exclusive VM lease are required before any invocation.

## Retention, timing and evidence details

The adapter uses the existing supported admission prefix on its original
connection, including the duplicate-rejecting dictionary decoder, exact distro
version and strict configured Dump parser. The original metadata-only ignored
entry and its controls remain separate; the new guard selects only
`manager_retained_lifecycle::adapter::run_private_lifecycle`. It does not run
that capture first and then start through another client.

The new adapter retains the raw unit bytes (exact compiled public unit), original
ELF identity/content digest, root directory ancestors and exact own symlink.
The outer guard independently pins the frozen ELF hash before execution through
its original read-only FD. The native adapter compares its actual executable
identity with the retained stage ELF; its self-content digest is continuity,
not a self-attesting replacement for the outer frozen hash. Initial outer xattr
checks remain mandatory; native metadata includes ctime and every effect boundary
also rehashes the original admitted files. Existing root-only ancestry and the
exclusive trusted-root assumption remain part of this composition.

Each start/stop observation phase has both a 450-query count bound and a
45-second elapsed bound checked before and after replies. Known pending replies
alone allow a 100-ms delay and another observation. The direct helper has a
180-second outer owned-wait deadline, with exact typed zero WNOWAIT required
before the sole reap. No deadline sends a signal or cancels a job.
The bus library's upstream receive allocation ceiling is unchanged; the 1-MiB
message/FD-free bound is a decode bound, not a preallocation guarantee.

Stable security fields are never patched into received dictionaries. Only the
explicit runtime fields are separately phase-checked. InvocationID, ExecStart
and main-exec history, exact job path/ID, control/main live PIDs, result and restart
count are typed. Never-started runtime Watchdog infinity and post-start zero are
distinct from configured Dump WatchdogSec zero. A seen running invocation cannot
be replaced by another one before completion. Stop observes the same historical
completed invocation; missing or unfamiliar states are failures, not absence.
Never-started InvocationID is exactly an empty typed byte array, matching v261's
[ID getter](https://github.com/systemd/systemd/blob/v261/src/shared/bus-get-properties.c#L59).
Sixteen zero bytes, missing data and alternate variant types are not aliases;
post-start identity instead requires exactly sixteen bytes with a nonzero value.

Private finite before-RPC and selected-runtime observation records identify
boundaries without publishing raw values or claiming validation. Native receipt,
Closed marker and Retired receipt are read from stable original FDs with strict
duplicate/unknown/type rejection. Before Stop the native helper checks the exact
own cgroup twice empty (or stable absent), including its populated flag; it does
not scan unrelated processes or use recorded PIDs as signal authority. This
proof is for the fixed no-fork native unit, not general descendant quiescence.
The outer independently validates terminal/native files after known helper zero.
Any error retains the admitted connection, reference, original unit/ELF/state
owners and parked native owners; temporary failed read/decoded message objects
are not creator or connection owners and make no cleanup effects when dropped.
Retaining a connection object cannot guarantee that a server-side reference
survives a transport or manager failure; uncertainty is retained, not represented
as proof that the manager reference remains live.

The first unsealed Python transition run had three stale budget/source-string/
pin assertions; after correction all 26 controls passed. An expanded Rust
selection caught the old encoded unit object path in the admission prefix
(119 passed, one failed, ten ignored); it was corrected to the new exact literal
before any execution. These intermediate failures remain NONPASS and are not
evidence for a guest invocation.

Full Rust at intermediate `0e79d67` finished with 129 suites, 2,157 passed,
zero failed and 85 ignored, plus fmt/Clippy/TUI/parity. Its full source gate
retained one failure (666 tests, two skips): a new test-only module name collided
with the unchanged old `manager_private` substring boundary assertion. The
module was renamed rather than weakening that assertion. Primary-source review
also corrected the initial synthetic sixteen-zero-byte InvocationID hypothesis
to the exact empty-array wire representation before any freeze or guest run.
These intermediate results do not replace corrected-head verification.

## Reviewed native freeze checkpoint

Corrected native source `4c8e0c9f3b7aab2b7387f58c711220d9dd4a20f6`
passed the full source gate (667 tests, two skips, frontend/QML) and full Rust
gate (129 suites, 2,158 passed, zero failed, 85 ignored, formatting, strict
workspace/TUI Clippy, terminal and parity). Parent and independent source
review found no blocker in the retained lifecycle and reached graph.

A fresh exclusive host build used locked offline Cargo, the library test
target and `--no-run`; the actual build completed with exit zero. The native
ELF is 78,128,704 bytes, SHA-256
`b9b07d98dfcfdbe13b9bbedc033769d5590327b4ea51770f46c165eb4385e5f5`.
The separately reviewed fixed host freeze completed with exit zero. It retained
the original single-link mode-0700 build ELF without modification and published
one exclusive single-link mode-0500 frozen ELF through original descriptors.
Strict build JSON, known-zero terminal receipt, original/frozen metadata,
ancestor continuity, hashes, no-xattrs and complete two-member readback were
checked. The private frozen receipt SHA-256 is
`3a7292ebfb58c63076e9fec220a0a6dddf55ee406303a4bce3980f73b9297172`.
Nine inert freeze controls passed; checked elapsed budgets are not a promise
to cancel blocked filesystem syscalls. The original host artifacts and receipts
remain outside Git.

The guard now pins that exact native source and frozen ELF; the staging guard
hash is recomputed in acyclic order. Native source bytes and fixture behavior
are unchanged. This is host build provenance, not guest execution, lifecycle
acceptance or a production kill-switch claim. Private delivery and any VM
invocation still require their separate review and exclusive authorization.
