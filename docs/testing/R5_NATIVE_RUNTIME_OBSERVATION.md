# R5 fresh local runtime observation

## Boundary

`runtime.observation` takes empty params and is exposed by exactly
`omavless runtime observation`. It is available only to the committed native
owner, under the same serialized dispatcher and migration lease as other native
reads. Exact ownership and desired state are checked again after observation.
No private-store parse is required: a damaged profile store must not make a
runtime observation repair, rewrite or disclose that store.

The response is schema version 1, scope `local_runtime_observation`, with the
normal envelope revision and daemon `instanceId`. It contains desired
connected/mode/generation (no profile ID), explicitly `lastKnownActual`, the
manual-recovery flag, transition tag, and `availability`:

- `observed`: `facts` contains owned child liveness, exact-name visible Mihomo
  count, visible TUN count, PID-authenticated desired controller configuration
  verification, and whether the owned profile matches desired intent.
- `unavailable`: `facts` is null, not false/zero. Incomplete or changed process
  inventories, unreadable/malformed network inventory, or changed child
  liveness are not evidence of an empty host.

`ownedControllerConfigVerified: false` means **not verified**, including a
  timeout, wrong PID, missing socket or configuration mismatch. It does not
  assert controller absence. `desiredProfileMatchesOwned` is an internal
  identity match, not server interoperability evidence.

Both inventories are sampled before and after controller reads. Exact-name
processes are bounded by the existing strict procfs reader; TUN enumeration uses
the existing strict sysfs reader. The four authenticated controller GETs share
one 250 ms deadline. No selector repair/PUT, process start/stop, service operation,
external request or desired-state write occurs. These are sequential local
observations, not an atomic kernel snapshot or a guarantee of continued health.

## Explicitly unverified

The `verification` object always reports false for service ownership, TUN
ownership, routes, DNS and internet. A visible TUN might belong to another VPN;
a named process count might include an unrelated core or omit a renamed child.
An owned child is separately tracked by its `Child` handle. Neither successful
controller GETs nor one visible TUN prove traffic protection/connectivity.
Cached failure/manual-recovery is never replaced by a green observation.

## Migration and privacy

Python remains the installed production owner. This additive native read does
not activate cutover, change `backend.sh`, enable QML mutation controls or alter
the strict `ui.snapshot` v1 contract. The current read-only native panel remains
explicit about missing live health; subsequent frontend work can consume these
facts without inventing legacy status fields. R5/R6 are not complete.

The Python reference supplies the migration distinction between active process,
desired mode and controller readiness; its permissive incomplete-inventory and
cached-state behavior is deliberately not copied. Existing differential suites
remain required. Synthetic tests and public response fields contain no real
profile IDs/names, endpoints, URI, credentials, controller path or raw errors.

## Acceptance

- Deterministic host tests: empty host, unrelated duplicate cores/TUNs,
  incomplete inventory, invalid intent and wrong-PID private controller.
- Socket/owner tests: fixed empty request, authorization and generation fences,
  unavailable facts, desired/ownership withdrawal during reads, unchanged
  revision and persistent data, and independence from corrupt store input.
- CLI test: exact command mapping; extra private arguments refused without echo.
- Installed Mihomo opt-in: synthetic owned core with private Unix controller,
  TUN/route/DNS disabled, no external traffic; matching/mismatched observations
  and cleanup. This is local adapter evidence, **not live VPN interoperability**.
- Full Rust/Python/QML/i18n and repository static gates remain required; results
  and exact accepted head are recorded in the PR.

No GUI strings or installed frontend behavior changes in this checkpoint; a
visual reacceptance of the unchanged QML pane is not required. Actual native
Full VPN activation and the independent service capability policy remain gates.
