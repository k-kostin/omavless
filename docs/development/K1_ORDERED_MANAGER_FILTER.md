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
No VM invocation has occurred for this generation.
