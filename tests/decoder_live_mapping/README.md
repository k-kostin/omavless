# Fixed 17-object live mapping — unexecuted source candidate

This distinct generation is based on sealed #618 `ce973d0`, not a retry of the
retained #611 review-2 invocation. It uses exactly the immutable decoder copy
manifest at SHA `d6ca9cd599adb573f4419f49147677f681db08fcc54a06cb0f34ed950ce19c36`:
18 logical targets, 17 canonical objects, no new aliases. The old manifest,
sources, failures and retained stages remain unchanged. Static #616 provenance
does not establish loaded-object identity or runtime compatibility.

Only a private bus and actual resolved are proposed. There is no broker, core,
DNS setter, canonical service operation, package installation or production API.
The new stage is `/home/kdk_vm/.cache/t3-decoder-tmpfs-review-1`. Nine fixed
members are staged exclusively: probe, lifecycle, bridge, admission, containment,
validator, two manifests and wrapper. The trusted-stdin transport is separate
host code, never a caller-selected loader or fallback. The acyclic pins cover
the entire dependency graph. No guest invocation is authorized by this source.

## Copy and live proof

The #618 admission bytes remain unchanged. The fd-safe #611 bridge changes its
exact source/copy count from 16 to 17 and removes failure-path owner closure.
The Sources object remains held without a context-manager exit across the entire
live lifecycle. Pending copy-write/target-read FDs remain retained on uncertainty;
there is no explicit cleanup API. All originals and ancestor FDs are
admitted together before copying; the retained original FDs supply bounded
exclusive copies. Source identities are rechecked after copying and before
binding. Whole tmpfs superblock and each file bind must be read-only, nosuid and
nodev, with no writable copy FD. Loaded map device/inode must equal an exact
retained copy before opening/hash; unknown or deleted paths never extend the
table. Every daemon gets two complete matching map passes, plus re-admission
immediately before its authorized shutdown. This is inventory, not DNS behavior.

## New ownership protocol, not old broad cleanup

`lifecycle.py` replaces every reached old command/wait/stop/supervise path.
The fixed probe explicitly binds containment's `command`, `child_status` and
`no_directory_fds` to the new session. Remaining reached containment helpers are
namespace/input/subordinate checks, isolation/config construction, fixed argv,
resource limits and credential checks. Old main/exercise, broad group scans,
stop, reap helpers and cancellation cleanup are never called. The source
boundary controls check this dynamic composition rather than assuming that
historical review approves new child behavior.

Short bootstrap utilities and the outer unshare require WNOWAIT exact typed
zero, followed by one raw exact-PID genuine-zero waitpid. Nonzero, signal death,
boolean/invalid metadata, unknown, deadline or cancellation seals permanently
before any retry/reap/signal or later query. Output is read only after zero.
No Popen destructor/poll/wait fallback queries process state. The bootstrap
does not claim group absence through the pre-chroot parent proc mount.

Each daemon follows spawned → ready → mapped → shutdown-authorized → term-sent
→ zero-reaped. The unreaped direct child is the PID reuse anchor. Original proc
directory and PID/network namespace FDs stay retained, with exact PID, parent,
starttime, proc topology and namespace binding checked around live observations.
Readiness sockets alone do not authorize shutdown: complete copy/map admission,
resolved credentials and a final known-live anchor check must all pass first.
Only then is one SIGTERM sent to that exact unreaped child. ESRCH is uncertainty,
not absence. There is no SIGKILL, escalation, group signal or retry.

After TERM only the owned WNOWAIT wait and exact zero reap are permitted: no
post-exit proc reread. Even ordinary signal death (-15) is NONPASS, not accepted
cleanup. Resolved shuts down first; only its positive typed zero completion
allows bus/copy re-admission and the bus's one TERM. An unexpectedly early zero
exit is also a failure, because live mapping/authorized shutdown was not proved.

## Retention on failure and finite scope

The namespace-init Python parks on any isolated error, retaining raw original
namespace/copy anchors instead of exiting and implicitly killing the namespace.
The park performs no observation, signal, diagnostic read/write or cleanup.
Unshare deliberately has no `--kill-child` option. After its 65-second deadline
the outer observer refuses without signalling/reaping or reading results. This
leaves an explicitly unresolved namespace, not a cleanup success. No further
guest action, automatic retry, export or recovery is authorized on that path.
Any subsequent investigation or manual recovery needs its own reviewed scope.

The one-invocation bounds retain RLIMIT_NOFILE 128, core dumps disabled, file
size 32 MiB, private root tmpfs 32 MiB and copy tmpfs 128 MiB. Copies retain
32-MiB/file and 128-MiB total bounds. Session spawning is capped at 96 children;
after the first failure no additional child can be launched. Readiness is eight
seconds, utility completion five, daemon shutdown six, map/hash passes five,
and outer completion 65. These are checked deadlines, not syscall cancellation;
the intentionally parked failed namespace is not time-bounded reclamation.

## Eligibility and evidence

The strict validator rejects duplicate/type/schema/pin/count changes, unequal
map passes, nonzero shutdown, extra signals and wrong namespace identity shapes.
Only known complete probe plus valid receipt permits the wrapper's canonical
epoch/eight-category/full five IPv4/IPv6 comparisons and fixed absence checks.
Only independently confirmed address lifetime countdowns may differ. Neither
native inventory nor a later independent observation repairs a failed wrapper.

Ordinary tests are inert synthetic source controls only. Full root and peer
review of this entire graph, source gates, exact frozen pins and an explicit
exclusive VM lease are required before one invocation. Successful inventory
would still not authorize broker/core composition or claim installed acceptance.
