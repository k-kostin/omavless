# Try Omarchy R5 native surface acceptance — 2026-09-08

Environment: Try Omarchy on Apple Silicon, Linux ARM64. Mihomo Meta v1.19.30
linux arm64, Go1.26.6, `with_gvisor`. All inputs were synthetic, credential-free
test fixtures. No real profile/subscription was read, changed or published.

## Ownership and evidence boundary

Python/QML remains the installed production owner. These changes extend the
native semantic implementation and its registered, ownership-gated dispatcher;
they do not execute a cutover, switch the installed plugin, or make Python
removable. Startup preferences are explicitly offline and unregistered.

Actual Mihomo checks use isolated owned processes and private Unix controllers
without TUN/TCP listeners or a real server. They validate the native adapter,
not Full-VPN interoperability or a packaged production cutover. No bare-metal,
physical-network or new-protocol evidence is claimed.

## Accepted candidates and local gates

Starting main was `5325617a75ef1687f486328a8a05111823b51dae` (desktop helper
PR #175 already merged; its prior GUI evidence remains separate).

| PR / surface | Exact candidate | Local gate |
| --- | --- | --- |
| #177 bounded live diagnostics | `a93904ea3a17a4277fd64cbafabca86fd492637e` | 517 Rust passed /4 ignored; 31 Python diagnostic comparisons; empty/inline-provider actual core cases |
| #176 offline startup preferences | `877d30132bdce4808dbaaa726e6cbdefb64b71be` | 525 Rust passed /4 ignored; 8 actual-Python complete-store comparisons |
| #179 compensated preset selection | `72f090a0fe2e953226334ec95b4957b90acaf3b9` | Full532/4 before context-only rebase; final322 runtime tests +5 startup domain tests, Clippy/fmt; 24 Python comparisons and all3 bundled configs validated by installed Mihomo |
| #174 custom-rule add/delete | `c0b54131e7ae38938f93c9dbe7479be7bb6958d5` | 548 Rust passed /4 ignored; 32 Python comparisons; actual core sees exact added/deleted rule state; 12 socket requests |
| #180 route-check fast paths | `3143add3f115e57172f674433324cfcbdb623a76` | Final combined555 Rust passed /4 ignored; 78 Python route comparisons |

The final #180 candidate contains all other listed implementations, so its
full555-test gate also validates their composition. Earlier exact-head tests
are preserved; range-diff showed context-only rebase adaptations, not discarded
semantics or replaced test groups. Final Python suite:270 run,0 skipped with
installed-Mihomo opt-ins. QML/i18n/search contracts, Python compile, shell syntax,
manifest JSON, diff check, plugin validate, Rust fmt/Clippy/parity all PASS.
CI is checked on each exact PR candidate before its authorized merge.

All five exact-head CIs passed and the PRs merged in this order:

| PR | Verified merge commit |
| --- | --- |
| #177 | `ccaab476a422dd5144198888323f140aa5ec1a01` |
| #176 | `4b43e251d263478eeb3842656b5a91f3794006ea` |
| #179 | `281a46025f8c03256df512376287c84e5c372f93` |
| #174 | `c14ff811095543ad66db9a9b65d162069be16b22` |
| #180 | `a6de2a78588bb34dd663d8894e1275b808335258` |

Subsequent documentation consolidation changes no runtime code. Merged source
branches are removed only after their trees are compared with merge commits;
historical worktrees remain available, and open evidence branches are preserved.

## Findings and fixes

- Mihomo `/version` can respond before configured rules/providers initialize.
  Tests now wait for exact synthetic loaded state under a deadline. They do
  not turn transient `providers:null` into fabricated empty-provider success.
  This supersedes the initial unproven artifact-only explanation in the
  historical shutdown handoff. Native production readiness follow-up: #183.
- Rule deletion cannot pass merely because an early core rule array is empty:
  the test also requires the exact fallback rule and configured row count.
- Preset recovery has a durable private pending barrier before member writes,
  retained through verification. Four child-process exit boundaries prove
  restart refusal; uncertain restoration keeps the barrier. This is not a
  cross-file atomicity or automatic recovery claim.
- One preset socket fixture initially wrote its marker outside the canonical
  desired-state directory. The fixture now derives the path from DesiredPaths;
  production logic was unchanged by this correction.
- Combined custom-rule tests prove an interrupted preset also rejects cached
  add/delete replies before host effects or private-store changes.
- Python versions differ in mapped-IPv6 display spelling. The differential
  oracle explicitly normalizes only that equivalent query representation;
  outcome/rule fields remain exact and no case is silently omitted.
- Shared Cargo artifacts are cleaned between worktrees and builds serialized
  with two jobs; no concurrent writers or worktree source mixing is assumed.

## Installed runtime and privacy

Sanitized host checks: plugin enabled; disconnected; previous `global` mode
preserved; statusFailures0; both user services inactive; supervisor/Mihomo/TUN
0/0/0. Isolated test children are reaped. No installed runtime restart, marker
fabrication, private fixture import, OS hardening relaxation, privileged helper,
or arbitrary command IPC was introduced.

V0/#30 remains Draft at accepted head
`a643db595ad5369b5fea200ebc601b4f0f70f18f`. Its implementation and XHTTP evidence
are unchanged. Missing protocol fixtures do not become new evidence here.

## Remaining work

See [the code-backed native surface audit](R5_NATIVE_SURFACE_AUDIT_2026-09-08.md).
Actual login integration, host capability contract #178 and configured-runtime
readiness #183 remain distinct from the finished foundations. Provider refresh,
live unmatched route observation, subscription probes, support/status views,
telemetry and small UI state/lifecycle operations still need owning changes.
R5/R6 are not complete; there is no percentage or Python-retirement claim.
