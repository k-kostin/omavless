# T3 encoder/libm live-mapping generation

This new developer-only generation combines the exact source proposal #635
`d39a6e2d8220d00842d71fcbe28083d0c21285d9` with the retained, zero-only
mapping protocol from #622 `b3a44c8f90d733e3975f06236b3d5e4e789afbab`.
It does not retry or clean any predecessor's unresolved namespace.

## Immutable scope

The manifest is exactly
`b914dece6cb3c58f74bb4cdea8b19ade7b3b032e1b12d112d7524a2c47ca6c87`:
20 logical paths / 19 original objects, including only the previously measured
canonical encoder and libm additions. The admission module retains every
original FD and ancestor before any copy and keeps all identity/hash/mode/owner
checks. Observed loader/SONAME aliases are not new targets or fallback paths.

The fresh fixed stage is
`/home/kdk_vm/.cache/t3-encoder-libm-mapping-review-1`.
Probe, strict validator, wrapper and trusted-stdin create-only transport bind
all nine fixed inputs transitively. Containment, original guest inventory and
owned daemon lifecycle bytes are unchanged from their reviewed predecessors.

The copy bridge changes only the exact source/copy count (17 to 19). It retains
original sources, read-only copied objects and store/proc/namespace anchors.
The 128-label cap is unchanged: a successful fixed path uses at most 107 labels
(101 predecessor labels plus two copies and four bind operations). Initial map
candidate records remain pre-decision public text, not admitted identity.

## Actual observation required

Both private dbus-daemon and systemd-resolved must stay bound to their unreaped
direct-child identities. Every original maps file is parsed strictly; every
named object must be an exact read-only copy with the matching mapped
device/inode before opening or hashing its target. A complete reread must match
each pass, and initial/final inventories must match. No dynamically observed
unknown object is admitted, copied or opened.

Only positive copy/maps/credential/live proofs allow one SIGTERM per daemon,
resolved first and then bus. WNOWAIT must report a genuine normal zero exit
before one exact reap. Unknown/nonzero/deadline/refusal permanently seals the
session; PID1 parks without a later query, signal, write or cleanup. The outer
wrapper stops immediately without after-state queries on such a failure.

A known successful invocation then requires strict typed receipt validation
and exact canonical/private/service/core/TUN/resolver/network preservation.
Even success is a bus/resolved inventory, not broker/core compatibility, DNS
mutation, installed package approval, arbitrary-dlopen safety or complete T3.

## Review and controls

All predecessor bridge, lifecycle, strict receipt, transport and pre-decision
controls are instantiated against the new graph. The 19-copy ordering check
retains all-source admission and all-original rechecks before overlays. Focused
controls use synthetic bytes only and do not launch a daemon, mount or guest.

Full parent and independent review, complete source/native gates, a fresh
baseline preflight and exclusive ROOT VM lease are required before the one
actual invocation. Old failures remain NONPASS and their evidence is unchanged.
Production runtime, QML, main, RC, installed packages and primary-PC networking
are not changed by this fixture.
