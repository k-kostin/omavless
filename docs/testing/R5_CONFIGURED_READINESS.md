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

The native observation remains read-only. The startup-only continuation below
repairs a wrong Full VPN selector choice before admission. Python's corrective
selection remains intact; packaged Full VPN acceptance remains a follow-up
gate before cutover. Likewise,
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

## Startup-only Full VPN selection continuation (2026-09-08)

`OwnedCore::wait_configured` can restore the generated nested selection during
startup. Both selector types/membership and actual Global mode validate before
effects. Only fixed PUT `/proxies/PROXY` and `/proxies/GLOBAL` are available;
the profile from native preparation is JSON-encoded in the body, never a path,
argv, diagnostic or log. Retained selection is read back; a 204 alone is not
readiness. Complete mode/rule/provider/selector admission still follows.

The single startup deadline, 250-ms attempt bound, child-exit checks and existing
compensation remain. Observation/status/adoption never repairs. Rule/Direct
behavior is unchanged. No generic HTTP API, mode PATCH, service or cutover added.

Every repair connection checks the same-user non-symlink `0700` parent, socket
type/owner, device/inode stability, and peer UID plus exact owned child PID before
sending bytes. Mihomo creates `0666` sockets: both `0600` and `0666` are accepted
only inside that mandatory private parent. No filesystem permissions change.
This does not change the semantic OmaVLESS `control.sock` requirement of `0600`.

The effect-isolated oracle runs actual Python `select_global_proxy`; three
synthetic ASCII/Unicode/quoted labels compare only action digests. Intentional
differences: native checks topology before effects, skips already-correct
selections, and shares one startup deadline rather than per-selector deadlines.
Repeated PUTs are idempotent and cannot bypass verified configured admission.

Five focused tests cover nested order/no-op repeat, unretained/rejected updates,
wrong mode/member/type/target/peer, unsafe parent, bounded timeout and read-only
observation. The installed-Mihomo test deliberately starts both selectors at
DIRECT and requires corrected selection plus owned-child cleanup. This is
synthetic no-TUN integration, not live Full VPN or installed frontend acceptance.
#183 remains open for production preflight/adoption consolidation and packaged
host gates; #178 is separate. Python remains installed owner and cannot retire.

## Preflight version-liveness correction (2026-09-08)

Production ownership observation previously accepted any successfully parsed
HTTP/JSON response from `/version`, including errors or `{}`. It now shares
`ControllerResponse::has_live_version` with `OwnedCore`: HTTP 200 plus a nonempty
string version are mandatory. The supervisor predicate is unchanged; only the
incorrectly permissive preflight tightens. Its fixed 300-ms read deadline and
private path policy remain. No writes, new queries or public fields are added.

Twelve synthetic actual Unix-response cases cover valid version, HTTP errors,
204, missing/empty/null/non-string version, non-object payload and malformed
JSON. Rejected responses cannot yield `ready_to_adopt` even with otherwise
matching synthetic process/TUN/config facts. Existing positive fixtures now
serve a real version shape instead of relying on the bug. Accepted native
supervisor semantics are the reference; Python's installed owner is unchanged.
This is liveness only: it does not complete configured preflight/adoption proof
or close #183/#178, activate cutover or allow Python removal.
