# First recovery permit after an original completed Connect

Status: test-only first-use candidate on the dormant maintenance owner, based
on `eb42ab9a45f027143ae48aa32092d547e7bb633c`. No new scenario has executed yet.
The new tests are ignored pending primary and independent exact-head review.
Normal factories and production behavior are unchanged; no production
`ResumeBinding`, host-source activation or restart provisioner is added.

## Executable milestone

One original RuntimeServer binds one original ProductionNativeOwner and its
coordinator. A fresh setup begins with durable desired Off and no Ready file.
The actual private control socket accepts a changed Connect, verifies and
commits its ordinary lifecycle/store operation, advances revision and caches
the original response. Only that completed operation may produce the first
Ready permit. A later source hint still needs the existing quiet/current
safety/reservation checks before one guarded recovery.

The positive fixture emits its own NetworkChanged during Connect. Enrollment
therefore handles ordinary connection bookkeeping without treating an event
as connection authority. It is synthetic host evidence and real private
socket/file/source execution, not installed VPN, host-bus or sleep/NIC evidence.

## Fresh provenance and startup

FreshSetupAuthority has no Clone, Debug, serialization, Default or from-path
constructor. Its test-only fixed setup constructor itself creates an exclusive
0700 directory and retains the original directory File/inode. It creates a
bounded 0600 Fresh enrollment anchor and the fixed owned fixture layout. A
caller-selected empty directory, a new process, counter, epoch, missing file
or successful Connect cannot construct this authority. Counter/path names
isolate fixtures; they are not cross-restart identity or rollback proof.

The private fresh constructor holds the original MigrationLock continuously
through marker/login checks, permanent lifecycle-guard installation, complete
Off/store capture, Fresh-to-Awaiting anchor publication/readback and narrow
settled-Off observation. It never calls general startup, repairs pointers,
stops a residual owner or seeds Ready. The anchor/receipt paths and original
directory identity must match the fixed setup. Existing/missing/corrupt or
replaced established state refuses; old seeded-Ready fixtures remain separate.

AwaitingConnect cannot recover. NetworkChanged frames are bounded bookkeeping
only. Suspend, Resume, malformed/gapped source data, loss, replacement,
shutdown or time regression permanently close its background eligibility.
Changed Off/owner/revision/store context also invalidates it. Accepted explicit
Disconnect cancels it even when NoChange leaves revision unchanged. Rejected,
invalid and replayed requests exit admission before consuming/resetting it.

## Original completion and lease

Normal execute_connection calls a shared private implementation with no
enrollment borrow. The privately registered developer owner manufactures only
fixed monomorphized connection hooks for H:ResumeBinding. The optional borrow
contains the original mutable Driver.source, its private Clock and that fixed
hook; no caller-supplied callback, lease, descriptor, clock or authority is
accepted by IPC or exported in NativeOwnerExecution.

After fresh admission and under the original transaction lease, Awaiting is
extracted to InFlight. A replay, NoChange, rejected result, failed/committed-error
operation, non-Connect action or metadata-only pointer repair cannot mint a
CompletedConnectAuthority. It requires the captured Off context, ordinary
changed Connect success, exact revision advancement, actual Connected and
complete committed desired/member/store/ownership context. The private
non-clone authority is consumed under that same still-held lease.

Source continuity additionally uses a private non-clone Weak-backed origin
binding captured from the original Source's unique Arc allocation. A new
Source with identical logical boot/instance bytes cannot substitute for it;
the lost flag and live/drained original source are checked too. This equality
binding is continuity bookkeeping, not a naked PID/FD/pointer lifetime claim
or recovery authority. Source itself has no Clone/reset method.

The fixed completion hook drains at most four NetworkChanged-only frames,
256 bytes each, under one sampled 400-ms aggregate deadline (and the existing
100-ms per-frame bound). Any Suspend or Resume, including duplicates, refuses.
It then takes a fresh complete healthy-owned observation plus the separate
binding proof under the same lease and exact context. A final strict quiescence
check follows; nothing drains after that fresh observation. Newly pending data
refuses instead of publishing against earlier facts.

The durable anchor is consumed, Ready is created atomically without replacing
any existing entry, and bounded exact readback/context/source checks precede
Installed. Any publication/readback error or panic leaves background InFlight/
Blocked, including persisted-but-unacknowledged Ready. It preserves the original
completed Connect result, desired/actual state, revision and cached response.
It never recovers a poisoned host mutex or claims that a lost source is live.
No retry, rearm, later event epoch or anchor replacement exists.

## Review and limits

The reached graph includes the registered mutation bridge, native dispatch,
production-owner private constructor, coordinator finish, lifecycle startup
guard/observation, source parser and both private writers. New socket/fault
tests stay ignored until exact-head primary and independent review. Compile
and lint results are not execution or source/safety acceptance.

The test-only PreparedConnectFixture exposes fresh setup/context before binding
the ONE original server, so the integration writer can inject the reviewed
HostEventSource backend without constructing another coordinator or seeding
Ready. No product API accepts a source factory. Host-source integration remains
separate until that actual backend and private-bus scenario are composed.

Private temporary storage remains the existing cooperating original-lease
fixture contract, not pinned-dirfd production storage, power-loss durability
or rollback-resistant bootstrap. Existing installation, restart, backup restore,
cross-boot provisioning and real suspend/NIC safety remain unavailable.
See [owner continuation](T4_NETWORK_RESUME_CONTINUATION.md),
[receipt protocol](T4_RECOVERY_ATTEMPT_RECEIPT.md) and
[execution policy](EXECUTION_POLICY.md).
