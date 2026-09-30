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
  Integration and installed acceptance are pending, not inherited from source CI.
- #375 at `b012c98be82f800525975d3e2aa079a6696f724c` is an audit composition
  based on older RC source. Reuse owning feature branches, not its whole tree;
  replacing the accepted RC tree would lose later DNS/CLI fixes.

## Not activated by this selection

T3 connection closing still lacks effect-time core identity guarantees. T4
refresh/recovery/quota/backup foundations are inactive; backup framing is not
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
