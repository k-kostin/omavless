# Native configured-readiness admission

This checkpoint addresses the native-host part of issue #183. It does not
switch the installed QML/Python owner or complete R5/R6.

## Reference and intentional boundary

Python `wait_private_controller` tests `/version` liveness; D1
`select_global_proxy` then uses bounded PUT/GET retries for the generated
`GLOBAL -> PROXY -> selected profile` topology. The new native admission checks
the same selected-member facts through fixed read-only endpoints, plus the
requested mode and initialized rule/provider collection shapes. Private labels
stay in memory and never enter public errors or diagnostics.

`NativeLifecycleHost` obtains the expected mode/profile from its validated
private-store preparation and checks it at startup and subsequent observation.
`OwnedCore::wait_ready` remains explicitly the low-level liveness primitive for
isolated core/probe callers. Version liveness now requires HTTP 200 and a
nonempty version string. Native profile matching requires configured readiness,
not only possession of an owned child and matching stored ID.

The native check does **not** repair a cached wrong selector choice. It fails
closed instead of reporting a wrong outbound as ready. Python's corrective
selection remains intact; the equivalent native fixed-purpose selection and
packaged Full VPN acceptance remain follow-up gates before cutover. Likewise,
production preflight/adoption observation still reports controller liveness,
not this stronger native-host admission fact. Keep #183 open for consolidation
with those gates; #178 remains a separate host-capability blocker.

Rule/Direct custom templates may omit PROXY; if PROXY exists it must be the
expected selector and point at the selected profile. Global requires both
selectors. Alternative custom PROXY group types are not admitted by this
checkpoint. Empty collections are legal; null collections are not silently
normalized. No fixture-specific rule counts are used in production. These
checks do not attest remote provider downloads, connectivity, or complete
template-content equality, and do not replace exact rule/provider fixture
checks in the existing diagnostics/custom-rule acceptance.

## Bounds and transport

- One startup deadline (existing 10 seconds in the native host); each polling
  attempt is at most 250 ms and all four requests share that attempt budget.
- Owned-child exit checked before polling and again before accepting success.
  Failed admission retains the child for existing lifecycle compensation;
  stop/drop still reap it and remove owned runtime artifacts.
- Controller connect is nonblocking, including a full Unix accept queue. All
  writes/reads share one deadline; fragmented responses cannot renew it.
- Fixed read-only Unix endpoints, existing 512-KiB response, JSON depth/string
  and header limits. No TCP controller, generic path or command channel added.
- HTTP/1.0 with connection close avoids Go's chunked large JSON responses.
  The bounded parser continues rejecting chunked responses. Installed Mihomo's
  `/proxies` exposed this compatibility issue during the local acceptance.
- `nix` 0.30 was already locked and used by the runtime; the Mihomo adapter adds
  only that existing dependency's socket feature. No downloader/helper added.

## Acceptance coverage

Deterministic tests cover correct/incorrect mode, selected-member/type checks,
nested Global selection, absent custom selectors, null versus empty collections,
early successful `/version`, delayed convergence, never-ready timeout, rejected
controller responses, stale selection, child exit, cleanup, fragmented-response
deadline and saturated accept queue. Public errors remain fixed and bounded.

`native_host_mihomo` exercises the installed ARM64 Mihomo against an ephemeral
synthetic store, all three configured modes, staged/committed config, actual
selector observation, wrong desired mode, custom-rule add/delete and cleanup.
It opens no TUN or TCP listener and sends no outbound profile traffic. This is
local native-host/controller integration, **not** a live Full VPN or cutover
claim. Existing exact-rule assertions are retained.

Record the final exact candidate, complete local test counts and CI result in
the PR before merge. No real credentials or private store are used by this gate.
