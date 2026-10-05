# K1 exact dependency-set fixture

This separate immutable generation follows the retained #601 pre-start NONPASS
at `b1dd6dc3a046faf2dbf5b7d567166cf9b9f378d2`. That failure and its archive are
documented in [the preceding contract](K1_TYPED_MANAGER_FILTER.md); neither the
old root stage nor its measured source is reused for another invocation.

The new literal unit/stage are `omavless-k1-typed-order-filter-fixture.service`
and `/run/omavless-k1-typed-order-filter-fixture`. The frozen probe, immutable
original raw-wait query support, complete baseline, namespace restrictions,
privilege bounds, source integrity and failure-retention rules are unchanged.
This is not installed or production firewall/namespace authority.

## Exact typed set, not ordered presentation

The fixed helper still requires all eight byte-exact empty Service property
receipts. Only after that succeeds does a second fixed read-only query request
`Requires` from the same literal object's Unit interface. Its complete byte
output must be one of exactly two `as 2` receipts containing `sysinit.target`
and `system.slice` once each, in either order. Missing, duplicate, extra,
unknown, malformed or differently framed entries refuse. No caller selects
unit, property, argv or environment. Unknown process state prevents the second
query; uncertainty during it refuses and cannot be repaired by a later value.

The runner no longer compares the manager's presentation order. All other
pre-start conditions remain. A failed typed query still exits before start,
next case or unlink; the outer guard still refuses a nonzero nested exit before
after-snapshot. This does not relax empty-property detection or cgroup/process
quiescence, and does not adopt dependencies as lifecycle authority.

Pure tests cover both fixed positive permutations, exact argv for both calls,
bad counts/signatures/duplicates/extras/missing framing, first-query refusal
before the second call and second-query uncertainty. Full source gates and an
independent exact-head review precede any new create-only staging/invocation.
## Exact-head development-VM evidence

Measured code is `b6ab5b647f2d8f598969073c6ad5c384c7c4e509`.
The source gate passed 568 tests with two pre-existing skips and frontend
checks; 25 focused pure/runner/guard cases passed. Root and independent
architecture reviews inspected the complete scoped source before invocation.

One create-only, explicitly authorized KVM invocation passed both control and
filtered cases and the outer guard on 2026-10-04. The frozen probe hash is
`b7dc81b89045c591efd375765ddf4fc4792afedf86cbc73c18fd94d1227d7332`;
runner hash is
`d573b198ea685495399475732dd84cd1cbd687c12404966cb9eccb8ed69d8637`.
The probe distinguishes EINVAL for the fixed invalid `/dev/null` setns control
from EPERM with the namespace filter. It performs no valid namespace
transition, firewall write, socket delegation or canonical authorization.

The outer receipt confirms exact three-line case output, known exit zero,
unchanged probe/runner/query identities, absent owned unit link and cgroup,
and complete before/after preservation. That includes canonical runtime epoch,
core/TUN/private-file inventory, resolver, installed packages/executable
capabilities, service/activation/environment graph and full IPv4/IPv6 state.
Only the explicitly bounded address-lifetime countdown exception is allowed.
The root stage and all evidence remain retained; no failure cleanup occurred.

The private nine-member archive was copied and independently checked on the
host. Guest and host SHA-256 agree:
`594662c42c6ccc98baefdfd8f6f73d05d5eefb7041c9e0057dea65c3fba6f348`.
All seven source/executable member hashes match the sealed generation, all
typed result booleans are true, and the exact case output was independently
read. Raw private observations are not published.

This new-generation PASS does not change the preceding #593/#599/#601
NONPASS results. It does not prove manager-created private-network isolation,
namespace type/cookie authority, complete nft ownership, installed kill-switch
integration, reboot/power-loss behavior or product readiness. Those retain
their separate contracts and acceptance gates. Main, RC and installed product
packages were unchanged; primary-PC networking was not used.
