# Inactive System historical-Off startup bridge

Base: #578, `7a5f1288e81cbfd89ac2892c792872720c204a01`.

This is a normal-compiled, **unregistered** developer integration, not adoption
of historical restore policy. The private no-argument `current()` in
`system_historical_off_candidate.rs` resolves current runtime, desired, cutover
and native host paths internally. It acquires only an existing migration lock,
retains the original current-Off snapshot before package/manager queries, and
uses `RetainedCurrentOff::system_off` to capture the genuine existing System
epoch proof. No caller path, Boolean, callback or supplied proof can select
that constructor. Missing, stale or old-manager receipts cannot be refreshed.

The constructor brackets actual native fresh observations with
original source/proof checks across resynchronization. Off and zero owned and
visible core/auxiliary/TUN observations are required; this is a conservative
empty-host subset, not permission to stop a foreign process. The same witness
and uninterrupted lease then span the actual production initializer,
coordinator, connection transaction, lifecycle reconciliation and pointer
plan/readback. Only SettledDisconnected and NoChange are admitted. No orphan
cleanup, core launch, pointer repair, receipt consumption or fence removal is
introduced. Both this bridge and the older fixed-path research adapter use a
sealed `ObservationOnlyNativeHost`. It delegates only actual native observations;
all lifecycle effects refuse and no inner host/resource slot can escape. Its
private destructor policy preserves unowned controller/staging names. Ordinary
`NativeLifecycleHost::new` retains its existing cleanup policy unchanged.

This distinction was necessary: the ordinary host destructor unconditionally
removes same-user controller and staged-config paths even if a fresh host never
prepared a core. Merely dropping an ordinary host would therefore violate this
bridge's no-cleanup scope. Empty observations or an absent-path check are not a
substitute for the sealed non-cleaning host.

The internal shared body returns only `ReviewedOffStillFenced`. It repeats
fresh empty observation and exact evidence checks, destroys the actual owner,
then rechecks evidence before returning. Owner, coordinator, host, witness and
registration authority cannot escape through that result. The generic body
permits deterministic test hosts, but its only non-test caller supplies the
concrete native host and System witness. Profile, connection and detached batch
historical conversions and Boolean research constructors remain test-only.
Ordinary `ProductionNativeOwner::current` and all generic pending guards remain
unchanged. No CLI, IPC, service or normal startup calls the private entry.

## Validation boundary

The bounded shared-body fixture covers Commit and Abort, full source identity
and private-byte preservation, owner destruction under the retained lease,
competing-lock Busy, final-observation refusal, same-byte receipt replacement,
a transient appearing during owner destruction, owned-stop refusal and pointer
repair refusal. Effect methods panic if reached. Existing real startup research
also covers stale manager, redirected paths and missing original directories.
Concrete native-host tests additionally preserve staged-file bytes/inode and a
bound, still-connectable controller socket across successful shared review,
stale-proof refusal, actual package-check refusal and observation failure.
An ordinary-host countercheck retains its existing cleanup behavior; all wrapper
effect methods refuse. The package-negative fixture invokes the actual package
check, not a synthetic positive, and asserts the test ELF differs from the fixed
installed executable before it can reach system-manager queries.
Deliberately restoring unconditional path cleanup makes the actual wrapper
preservation test fail with the staged file missing; the ordinary cleanup
countercheck still passes. The fixed wrapper passes both. This is a regression
counterexample for the new adapter's no-cleanup boundary, not a claim that the
ordinary owner's existing cleanup contract is itself a defect.
Source-retention checks are guards on wiring, not installed execution evidence.

Actual **System-positive execution is unrun** at this adapter's original cut.
An ordinarily uploaded developer/test ELF cannot satisfy the existing package
self-inode check against `/usr/bin/omavless`. The separate
[System-provider mechanism gate](T4_SYSTEM_PROVIDER_VM_GATE.md) specifies a
root-issued private read-only image mount without changing the global package;
this is not installed-release acceptance. A separately authorized disposable
account fixture must first create a genuine consumed receipt in the same
user-manager epoch, then establish
synthetic complete historical evidence and exercise this inactive bridge without
altering the ordinary startup refusal. No missing/new-manager recovery, normal
mutation adoption, private archive UX, physical power-loss, installed restore or
release acceptance follows from these tests. A normal admission caller still
requires explicit product policy and its complete effect/cancellation matrix.

Exact source, build and test outcomes belong to the development PR checkpoint;
the original research VM evidence remains attributed only to its own heads.

The later [late-epoch bridge regressions](T4_LATE_EPOCH_BRIDGE_TESTS.md) cover
Commit/Abort source drift at final observation and actual owner destruction.
They add no normal behavior or actual-System acceptance claim.
