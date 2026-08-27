# Try Omarchy D1 acceptance and lifecycle findings — 2026-08-27

This report is the credential-safe continuity record for the 2026-08-27 Try Omarchy acceptance session that completed D1 / PR #27 and found/fixed an intermittent Full VPN startup race. It is intended for future cloud, desktop and guest agents so the reasoning is not dependent on chat history.

No profile names, provider names, endpoints, share URIs, UUIDs, subscription URLs, passwords, private keys or reusable credentials are recorded here.

## Canonical checkpoint

- Repository: `k-kostin/omavless`
- D1 PR: `#27` / `codex/advanced-mihomo-diagnostics`
- D1 exact accepted head: `2c2615247d32e7ac4b2f7ada055d1028586cc639`
- D1 merge commit in `main`: `5a50d59ae9fada9c61ee3ed9037f72e35bf09853`
- Published marketplace snapshot remains OmaVLESS `0.7.0` at `69fe05b03129a23664fff3f8289821a7b7f80095`; advancing `main` does not change that published snapshot.
- Next active runtime step after this checkpoint: rebase/retarget PR `#30` (`codex/experimental-protocol-live-gates`) onto the new `main`, rerun cloud checks, then execute V0 live protocol/provider/TUN validation.

The generic bare-metal wording at the end of `docs/testing/TRY_OMARCHY_ARM64_2026-08-26.md` predates the current acceptance policy. `AGENTS.md` and `docs/roadmap/ACCEPTANCE_ENVIRONMENTS.md` are canonical: Try Omarchy ARM64 is an accepted integration/acceptance environment for ordinary plugin/runtime work; bare-metal is an optional confidence pass unless a concrete hardware-sensitive reason makes it mandatory.

## D1 architecture accepted in #27

D1 adds advanced Mihomo diagnostics and adaptive polling while preserving the private-controller boundary.

Key accepted properties:

- diagnostics use only OmaVLESS's private Unix Mihomo controller;
- no TCP `external-controller` listener is introduced;
- loaded rules/providers are bounded before entering public state;
- public rule targets are reduced to `VPN`, `DIRECT` and `REJECT` instead of exposing internal proxy/group/profile names;
- diagnostics errors remain separate from normal fatal connection state;
- rule search is local over already loaded data;
- stale page-generation responses are rejected;
- provider refresh-all keeps all-or-nothing timestamp semantics;
- adaptive polling keeps lightweight background status polling without continuously fetching rules/providers;
- bar-throughput sampling may continue independently without activating diagnostics fetches.

This controller/privacy boundary is a prerequisite for later active-connections and TUI/operator views and should not be bypassed by opening a TCP controller or exposing raw Mihomo internals to public UI state.

## Full VPN lifecycle knowledge carried forward

The earlier 2026-08-26 report documents two prerequisite lifecycle bugs and their fixes:

1. `run-core` self-deadlocked behind the user mutation operation lock during synchronous service restart;
2. Full VPN must select the nested topology `PROXY -> active profile`, then `GLOBAL -> PROXY`, rather than attempting `GLOBAL -> active profile`.

Both were already merged through PR #35 before the D1 acceptance session.

The 2026-08-27 session discovered a third, independent lifecycle edge case: **Mihomo controller `/version` readiness does not guarantee that selector endpoints are ready on the same instant**.

### Confirmed selector-readiness race

On D1 head `3e7866161eb8b2dd7c51e31a7b825f2f5836bff4`, Routing -> Full VPN was intermittent despite the earlier lifecycle fixes.

The decisive instrumented reproduction captured:

- `/version` returned ready;
- the first `PUT PROXY -> active private profile` followed with effectively `0.0 ms` readiness gap;
- that original PUT returned HTTP `404`;
- the identical request after `+50 ms` returned `204`;
- readback immediately matched the intended target;
- later `+150 ms` and `+350 ms` probes also remained accepted/matching.

Therefore the failure was a selector-initialization race after the controller itself had become responsive, not a wrong selector topology and not a service/controller-start failure.

### Implemented lifecycle hardening

Local fix commits made on the D1 branch:

- `d989202e59c6d03cde57eda10756b5395a755bef` — bounded selector-readiness retry;
- `2c2615247d32e7ac4b2f7ada055d1028586cc639` — narrow retry coverage for HTTP protocol transport exceptions; this became the accepted D1 head.

Accepted behavior per selector step:

- one monotonic retry budget of approximately 1 second;
- backoff starts at `50 ms`, then `100 ms`, then `200 ms` capped until the deadline;
- transient/retryable cases include observed `404`, temporary `5xx`, Unix-socket/timeout failures, HTTP protocol transport failures, temporary readback failures and temporary readback mismatch;
- clearly permanent HTTP failures such as `400`, `401` and `403` fail immediately;
- no unconditional startup sleep was added, so the healthy path remains fast;
- internal profile/selector data is not exposed by public errors;
- if the deadline expires, the existing transactional rollback remains authoritative.

Do not replace this with a blind fixed sleep or broad `BackendError` retry. `BackendError` also represents deliberate validation failures such as oversized/invalid controller data and is intentionally not globally classified as transient.

## Polkit / password prompts are separate from the selector race

TUN recreation can produce three Omarchy authentication prompts because Mihomo launches three distinct `resolvectl` operations protected by separate systemd-resolved/polkit actions. The detailed chain is preserved in the 2026-08-26 report.

During investigation it was initially plausible that slow password entry could contribute to Full VPN failures. This was ruled out as the primary cause:

- a fully authorized CLI repeat still reproduced `Mihomo refused the Full VPN outbound selection`;
- the real plugin UI reproduced the same error **before the first visible password prompt appeared**;
- instrumented evidence later captured the independent selector `404 -> 204 after 50 ms` race.

Password/authentication issues may still affect DNS/TUN setup independently and should be investigated on their own evidence. They must not be conflated with selector readiness, and OmaVLESS must not weaken polkit/OS security policy merely to remove this UX friction.

## Exact-head automated/local validation

On exact accepted head `2c2615247d32e7ac4b2f7ada055d1028586cc639`:

- GitHub Actions `Test`: SUCCESS;
- full Python suite in Try Omarchy: 176 run, 173 passed, 3 expected skips;
- QML contracts: PASS;
- Python compile: PASS;
- shell syntax: PASS;
- manifest JSON: PASS;
- `git diff --check`: PASS;
- installed Mihomo config/selector opt-in tests: 2/2 PASS;
- `omarchy plugin validate`: PASS;
- installed runtime files were hash-verified byte-identical to the exact candidate checkout.

`qmllint` remained NOT RUN because the command was not installed in the guest. This was recorded as a non-blocker because the repository QML contracts were green and the runtime/UI acceptance was exercised directly.

A live provider partial-failure injection was intentionally not forced because there was no safe fixture that avoided modifying real provider configuration. Exact-head deterministic coverage passed for failure timestamp preservation and redacted generic errors; live Update all success was exercised with the real provider set.

## D1 command-level acceptance evidence

Sanitized accepted observations included:

- disconnected diagnostics: neutral/unavailable behavior, no fatal connection-state pollution;
- connected Routing runtime: exactly one service, one supervisor, one Mihomo and one TUN;
- rules populated: 27;
- providers populated: 23;
- public target classes: exactly `VPN`, `DIRECT`, `REJECT`;
- internal group/profile leakage: none observed;
- credential/private metadata leakage: none observed;
- private Unix controller: PASS;
- TCP controller: absent;
- local search: PASS for rule type, `VPN`, `DIRECT`, `REJECT` and a harmless public payload fragment;
- refresh single-flight/no queued backlog: PASS through exact-head deterministic/source contract evidence plus runtime process observation;
- stale-generation rejection: PASS;
- fresh load on a new generation: PASS;
- provider Update all: live PASS with 23 providers before/after; timestamp advanced only after complete success and state reloaded;
- provider partial-failure deterministic coverage: PASS;
- adaptive polling A-E gates: PASS, including external disconnect reflected in 15.6 s and external connect in 29.2 s in the background polling window.

## Final mode-transition regression after the readiness fix

Exact accepted head `2c2615247d32e7ac4b2f7ada055d1028586cc639` was installed and pushed before the final live matrix.

Five independent Routing -> Full VPN trials all passed:

- trial 1: 1.444 s
- trial 2: 1.476 s
- trial 3: 1.378 s
- trial 4: 1.448 s
- trial 5: 1.465 s

Aggregate:

- 5/5 successful;
- minimum 1.378 s;
- median 1.448 s;
- maximum 1.476 s;
- selector failures: 0;
- controller failures: 0;
- duplicate-runtime incidents: 0.

Remaining required transitions also passed:

- Full VPN -> Routing: 1.227 s;
- Direct -> Full VPN: 1.385 s;
- Full VPN -> Direct: 1.217 s.

Every resulting connected runtime remained exactly `1 service / 1 supervisor / 1 Mihomo / 1 TUN`, private controller responsive, no TCP controller and no duplicates.

Post-transition D1 diagnostics remained healthy with 27 rules, 23 providers, redacted targets and private-controller-only access.

Final restoration after acceptance: Routing, disconnected, plugin enabled, service/supervisor/Mihomo/TUN all zero.

## Manual UI acceptance

The owner manually exercised the real plugin surface on the accepted installed head.

PASS:

- disconnected diagnostics neutral state and no fatal/red shield;
- connected diagnostics rules/providers;
- search responsiveness;
- Refresh behavior;
- close/reopen lifecycle behavior;
- long-content/layout/hover stability;
- Escape/back behavior;
- real UI Routing -> Full VPN after the readiness fix.

One deferred UX issue was found: `Tab` moves focus between Omarchy plugins instead of cycling controls inside the active OmaVLESS panel. This may be interaction with Omarchy Shell's global focus handling. It did not block D1 and is tracked separately as GitHub issue `#37` (`UX: keep Tab navigation inside OmaVLESS panel`). Future work should preserve Shell-global navigation if intentional while providing sensible scoped keyboard traversal where feasible.

## Cross-agent execution lesson

Codex Desktop on macOS could not directly automate the running Try Omarchy guest window through Computer Use in this setup: the visible Cocoa/SDL window belonged to a nested raw QEMU process not exposed as an addressable app, while the helper was an `LSUIElement`; no usable host-forwarded SSH/guest-command channel existed in the installed image.

The successful fallback workflow was:

1. cloud/browser agent acts as acceptance orchestrator and keeps the exact-head ledger;
2. user manually copies bounded prompts into Codex CLI already running inside Try Omarchy;
3. guest Codex executes command-level checks and returns sanitized evidence;
4. user performs the genuinely UI-only observations;
5. cloud agent verifies evidence and performs GitHub state transitions.

Do not spend future sessions repeatedly trying to drive this same QEMU guest through Desktop Computer Use unless Try Omarchy gains an addressable guest window or explicit host/guest command channel. This is an environment/tooling limitation, not an OmaVLESS runtime defect.

## Handoff after D1 merge

At merge commit `5a50d59ae9fada9c61ee3ed9037f72e35bf09853`:

- D1 / #27 is complete and merged; do not repeat its full acceptance by default;
- issue #37 holds the deferred Tab-navigation UX work;
- bare-metal Omarchy is optional confidence verification for D1, not a blocker;
- PR #30 is the next active runtime chain item and must now be rebased/retargeted onto the new `main` before its own checks;
- #30 must not merge without its core purpose: privacy-safe live validation against representative real servers/providers/TUN behavior for the already experimental protocol families;
- P4a WireGuard follows V0; do not skip V0 merely to start new protocol implementation.
