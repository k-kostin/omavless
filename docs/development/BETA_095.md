# 0.9.5 development assembly

Owner decision, 2026-09-30: develop in temporary `beta/0.9.5`, then select a
scope-frozen `rc/0.9.5`. Follow the [workflow](../roadmap/DEVELOPMENT_WORKFLOW.md).
Main and Marketplace remain held; a beta branch is not a published prerelease.

## Base and first selection

- Accepted base: `rc/0.9.0` at
  `a543a45c34dcc953ef2e9cd019499146f85ac2eb`. Preserve its managed-DNS pair,
  CLI fixes, exact RC2 artifacts and acceptance; do not reopen unchanged gates.
- First integration source: read-only T3 operator/doctor chain through #372,
  `dev/t3-doctor-view` at `95bf743a56d5e19aaf2102bbce8a8d9af88d14f2`.
  [#388](https://github.com/k-kostin/omavless/pull/388) retains all 19 original
  commits in merge `29b37b65cd1df5d6cf32e6859ea8c94acb3cfb10` on the accepted
  base. All 35 files preserve the source added/deleted-line multisets; only diff
  context/placement changes. No DNS/frontend/package boundary is reverted.
  Installed acceptance is pending, not inherited from source CI.
- #375 at `b012c98be82f800525975d3e2aa079a6696f724c` is an audit composition
  based on older RC source. Reuse owning feature branches, not its whole tree;
  replacing the accepted RC tree would lose later DNS/CLI fixes.

## Not activated by this selection

T3 connection closing still lacks effect-time core identity guarantees. T4
automatic refresh/recovery/persistent quota/backup foundations are inactive; backup framing is not
encryption. S1 still lacks trusted manager/writer admission and K1 lacks complete
policy ownership/effect enforcement. Integrate those only through separately
bounded decisions and gates; module presence is not product readiness.

#270/#132 remain open for stable 0.8.2. Their scoped managed-pair solution is
accepted in RC 0.9.0, not another blocker waiting for 0.9.5. #30 keeps its Draft
status and exact available XHTTP evidence; unavailable fixtures remain unavailable.

## Integration gates

Record selected source heads, compatibility adjustments and combined regression
results in each owning PR. Review T3 privacy/stale-read fences, EN/RU rendering
and read-only installed behavior. Preserve one runtime/core/TUN and Unix-only
controller ownership. A local 0.9.5 package requires an honest version and exact
runtime/DNS/frontend pair; never relabel RC2 pins as beta artifacts. Public
assets, main promotion and Marketplace each require separate authorization.

## First T3 checkpoint

The workspace adds page-local, fenced read-only rules/providers, saved overrides,
private connections, explicit one-shot route checks, volatile traffic history,
fixed core-log hints and local doctor facts. It does not infer network health
from inventory. Normal QML layout remains unchanged.

Combined local checks at code head `e24f44f`: 494 developer tests (two expected
skips), QML/Node contracts, 287 bounded EN/RU keys, 1,356 successful Rust test
executions (12 ignored), 11 PTY scenarios, fmt/Clippy, parity and plugin validate
pass. Actual terminal rendering of synthetic Connections/Diagnostics was inspected
in EN/RU; small-view behavior has deterministic coverage, not installed acceptance.
No private fixtures or captures are committed. Final-head CI is recorded in #388.

One substantive integration finding is fixed: Ratatui 0.30's terminal destructor
uses `eprintln!` when cursor restoration fails. With a revoked PTY/stderr, cleanup
itself can panic (13 failures in 30 reproduced exits). `e24f44f` contains only
terminal teardown, retains normal dropping and sanitized error handling, and
does not catch rendering/IPC errors. A deterministic drop regression, all PTY
scenarios and 400 revoked-PTY exits then pass. The temporary diagnostic hook was
removed before committing. No runtime/network behavior changes for this fix.

## First working T4 checkpoint

[#389](https://github.com/k-kostin/omavless/pull/389), code head
`c5318f87b976a7073e6bd7001f67766502061011`, adds an explicit `u` read for the
selected TUI subscription. Strict optional provider counters and expiry are
private, transient assertions, not connection health. No automatic fetch,
persistent quota, subscription mutation or QML layout change is introduced.
See the [owning contract](T4_SUBSCRIPTION_METADATA.md) for the detached HTTP,
capacity, deadline, ownership/revision/URL fences and private projection.

Local checks: 1,378 successful Rust executions (12 ignored), fmt/Clippy,
11 PTY scenarios and parity; 494 developer tests (two expected skips), QML,
300 EN/RU keys and plugin validation pass. Actual synthetic terminal rendering
was inspected in EN/RU. GitHub test and both native package jobs pass. These are
not installed beta or live provider evidence; those remain explicit gates.

The source version is now **0.9.5-beta.1** (`0.9.5beta1-1` in Arch packages).
Runtime, companion, manifest and setup version must agree. Beta/RC assembly is
explicitly separate from stable; positive prerelease numbers and strict exact
package/source/dependency checks apply. Bootstrap pins are empty, not reused
RC2 hashes: guided public provisioning remains unavailable until independently
built immutable beta assets are authorized, uploaded and verified.
Version metadata is **not** a built/published package or acceptance. Next gates
are local exact managed runtime/DNS/frontend assembly, then installed T3 privacy,
stale-owner, concurrent-client and EN/RU review before release scope freeze.
