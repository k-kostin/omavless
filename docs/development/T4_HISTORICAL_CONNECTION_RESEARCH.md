# T4 actual connection-lifecycle research

The next detached batch slice has separate
[research boundaries and evidence](T4_HISTORICAL_BATCH_RESEARCH.md); it does not
extend this checkpoint's evidence or authorize normal historical admission.

Dev-only integration on current `rc/0.9.7` owner/backend. Draft #561 first
composes the inherited #557 chain at `78ca31bd3fea126877594d301fd42014d68657b0`:
77 files, 33,476 inserted lines and 122 deleted lines relative to RC
`c4e800425243c1b02165f82153e4bf418fe465e6`. This substantial source dependency
is not a small standalone patch. Current ManagedPair/Selection and ordinary
production admission remain authoritative. The composition baseline passed
the frozen runtime suite (1035 passed, 30 ignored) and source/frontend suite
(499 tests, two skipped); these results do not establish later code heads.

## Actual caller and retained evidence

The ordinary `execute_connection` caller delegates through its Ordinary typed
alternative to the existing scheduler, guard, lease, transaction, lifecycle,
prepared compatibility-pointer writer, compensation and finish. No generic C1
or pending predicate is relaxed, no IPC path is added, and the historical
alternative exists only under `cfg(test)`. It allows Connect/Disconnect, not
SetMode, startup registration or detached/background publication.

The initial same-UID/exact-ownership-generation current-Off/current-manager
witness is consumed into independently validated evolving current bundled
evidence. It is not carried as a fixed Off fact through a connected effect.
The same continuously retained migration lease spans the existing bounded
fixed lifecycle transaction, exactly as ordinary Connect/Disconnect requires.
No subscription/provider fetch is performed under this context or lease.

Only actual successful desired writes and prepared compatibility-pointer
commit/restore outputs advance the corresponding desired/store snapshot. The
desired output must equal the actual writer's canonical bytes; the store
output must equal the actual prepared candidate/original bytes. Each output
is independently read twice, pinned immediately before any After callback,
and rechecked against all immutable boundaries. A no-op retains complete
original identity, including inode. Missing live members are not default
authority. Failed writes must leave the retained full snapshot unchanged.

History, template, ownership UID/generation, login receipt, manager proof,
directory identities, transient absences and the exact coordinator identity
remain fixed. Replay revalidates them before returning cached data and invokes
no lifecycle/store effect. Same operation ID with a different request remains
a scheduler collision. A fresh coordinator cannot reuse retained authority.

Desired generation is deliberately different from ownership generation:
verified failed-Connect compensation may restore Off at a newer monotonic
desired generation while original ownership/history remain unchanged. A
recoverable failure is accepted only after actual owned-empty observation and
verified output/cleanup. Uncertain host acknowledgements, failed cleanup or
late proof loss poison the context and independently latch the owner; later
matching bytes do not cure it or fabricate success/Replay.

These are cooperative-lease and point-in-time descriptor guarantees, not an
atomic defense against hostile same-UID writes between checks and syscalls.
Checksums are not authentication and manager proof never mints a new receipt.

## Fixtures and acceptance boundary

Fixed invented private fixtures exercise real coordinator/transaction/
lifecycle/desired and private-pointer writers with a faulting synthetic host.
Coverage includes Commit/Abort history, Connect/Disconnect, exact Replay,
no-op identity, absent members, operation collision, new-owner refusal,
current DNS preflight refusal, real temporary-slot writer failure and verified
monotonic Off compensation/retry, uncertain cleanup, late host/writer fences,
same-byte inode substitution, receipt/manager revocation and permanent poison.
Fault callbacks may withdraw real success or change actual fixture evidence;
they cannot manufacture prepared outputs or grant new Boolean authority.

An ignored opt-in fixture additionally invokes production `OwnedCore`
spawn/readiness/group stop inside the same actual connection caller. Its fixed
DIRECT configuration has no provider, DNS, external listener, TUN or auto-route.
It must fail healthy-tunnel verification and prove actual parent-owned cleanup
and Off rollback. This is not healthy TUN/managed-DNS or installed-service
acceptance. It requires separate explicit actual-core invocation authorization.

Exact frozen-head local/CI/Dev-VM evidence belongs to Draft #561. Product
historical-policy adoption, ordinary availability, async publication and
re-admission, new-manager receipt transition, successor invalidation,
ownership rollover, private restore UX/API and installed host/package
acceptance remain gated. C1 and both historical records are never retired.
No merge, release, pin update, production activation or whole-T4 closure is
authorized by this research.

## Frozen code-head evidence

Code head `9f96b1e071e1e13b5613bebd7183089171c006ff`:

- Full frozen runtime library: 1044 passed, zero failed, 31 ignored, 427.15
  seconds at eight threads. The extra ignored test is explicit actual-core
  acceptance, not hidden acceptance of an unavailable effect.
- Source/frontend: 499 tests, two skipped, plus JS/QML/navigation gates PASS.
- Strict workspace/all-target clippy, formatting and diff checks PASS.
- All five exact-code-head CI jobs PASS: Test, both native DNS architectures
  and both package architectures. Final documentation-head CI is recorded on
  the Draft PR separately.
- Frozen binary SHA256:
  `818a865e32671402d795a37168eeff6e1694c6e9e46f35dd327d2d42a716fbe3`.

The earlier inherited 316-test source result belongs to #557, not this
current-RC composition; its baseline log also reports 499 tests/two skips.

## Dev-VM actual-core receipt and negative attempts

The same immutable binary was invoked under an exclusive isolated Dev-VM
lease. In the final guarded retry, the nine aggregate connection tests passed
twice (25.20/25.21 seconds), Ordinary separate-Desired/Cutover pending refusal
passed (0.02 seconds), and the opt-in actual parent-owned no-TUN core
failed-Connect/verified Off rollback fixture passed (5.75 seconds).
Actual core: installed stable `/usr/bin/mihomo`, no capabilities, SHA256
`ba7a74ed6bbc3098930e6e172fa9b7d4ef7e42a77f7a516046d61fbeb3bfcfe6`.
This is production `OwnedCore` inside a synthetic `NoTunCoreHost`, not full
`NativeLifecycleHost`, managed-DNS, healthy TUN or private-profile acceptance.

Negative evidence is retained as classifications, not relabeled success:

- Initial guarded invocation: all actual tests passed, but network verdict
  was **non-PASS** because one address lifetime pair increased. Renewal is an
  unproven hypothesis; available journal categories did not establish cause.
- Retry wrapper: all tests passed; wrapper exited 1 from an added unsupported
  maximum-two-path/one-address restriction. All four changed lifetime paths
  independently decreased, spanning two dynamic address entries and only the
  two approved field names `valid_life_time`/`preferred_life_time`.
- Without new effects, those exact retry snapshots passed the original K1
  numeric, individually non-increasing field-name comparator. IPv6 routes and
  rules were additionally compared exactly. This validated receipt does not
  retrospectively change either earlier wrapper exit status.

Every non-timer address/interface/route/rule field, resolver, installed private
profile/template/desired/ownership fingerprints, canonical user-service
state/executable/MainPID 86349 and parent network namespace were preserved.
Retry preflight also matched the original network non-timer baseline. Owned
core/TUN, temporary and HOME fixture inventories were empty after cleanup.
Only validated task-owned guest scratch/binaries/private snapshots/logs were
removed, and the exclusive VM lease was explicitly returned. No installed
service/package action, host DNS/route authorization, or private-profile
Connect/Disconnect was performed.

Later typed research admission refuses an already foreign-bound context without
poisoning its original owner or independently latching the receiving coordinator.
Direct binding/proof-loss poison and receiving-owner new-intent close revocation
remain intact. See
[the bounded foreign-owner correction and exact evidence](../testing/T4_FOREIGN_OWNER_CONTEXT_ACCEPTANCE_2026-10-03.md).
