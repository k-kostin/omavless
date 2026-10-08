# Current delivery status

Updated 2026-10-08. This is the compact current-state entry point; the detailed
[delivery roadmap](../../DEVELOPMENT_ROADMAP.md) preserves the implementation
history. GitHub's actual main/PR state is authoritative for publication.

## Main and open work

- **Owner release decision, 2026-10-08:** public 0.9.8 is **Backup-only**.
  Trusted product scope restricts capabilities, RPC, CLI and developer actor
  entries; full Restore remains in development. Preserve old pending/history
  fences. **Usable Restore plus explicit interrupted/fatal recovery is mandatory
  for 0.9.9.** Older positive/negative evidence is retained, not relabeled PASS.
  The new restriction has its own exact-source package and installed gates.

- **0.9.8 internal RC scope freeze:** `rc/0.9.8`, source `0.9.8-rc.1`, selects
  normal-entry Backup (Restore excluded by the decision above), corrected
  explicit application relaunch and ARM64
  link-count portability on the beta.4 maintenance base. The
  [RC ledger](../development/RC_098.md) retains exact inputs, original failed
  installed readiness and remaining candidate/package/host gates. Exact
  `5e7d4258` has inspected offline pairs for both architectures and passing
  native package CI; its final full Test CI subsequently passed. #717's managed
  template correction is integrated into RC, with exact-source cloud/build and
  x86_64 manual lifecycle evidence. #718's stopped-page and sustained disable
  successor is installed-checked and carried to RC; #719's private-test scratch
  correction is integrated after full CI. Final combined artifacts/docs are
  being reconciled.
  Its
  [installed x86_64 report](../testing/RC_098_VM_2026-10-08.md) passes normal
  entry/navigation/reopen, known Quit → re-enable → actual Start and real cold
  configured-Off Start with login preparation, preserved data and disabled
  startup units. The separately recorded continuation adds normal Completed
  Restore, controlled same-intent Abort, connected modes and actual sustained
  disable → re-enable → Start; initial evidence is not rewritten. It is
  agent-operated, not ARM-installed, whole T4 or public provisioning acceptance.
  Lost-owner Restore recovery is mandatory for 0.9.9, not an ARM/operator
  checkbox. Pins are
  empty; main, immutable releases and Marketplace are unchanged. This is not
  accepted/public RC or whole T3/T4 completion.

- **0.9.8 internal beta.4:** #711 selects the real Backup and authenticated
  Preview/Cancel/Restore path from #701/#705/#708/#710. Its fixed package build
  selector and ordinary **Open app / `omavless tui`** expose those selected
  controls; a per-invocation research flag is not the beta entry. Installed
  x86_64 source `09f238ff` / frontend `a2e5b647` passed normal entry/reopen,
  wrong-key refusal, unchanged Preview/Cancel, one original Completed Restore
  with independent pair/revision readback and known-Off ordinary restart.
  Agent-operated evidence is not owner manual, ARM-installed or fault acceptance.
  Runtime relaunch after plugin shutdown is included through #712/#714 in the
  exact RC checkpoint above, not borrowed from that earlier bundle.
  See [selection](../development/BETA_098.md) and
  [assisted test card](../testing/BETA_098_MANUAL_PLAN.md). Main, releases and
  Marketplace are unchanged; beta.4 is retained history for the selected RC.
  [The consolidation ledger](https://github.com/k-kostin/omavless/issues/706#issuecomment-6048668247)
  records 92 verified no-merge PR closures: the prior 82 plus ten historical
  predecessors with unresolved requirements retained in open owning records.
  Source/evidence branches and unique
  unfinished work are retained; administrative closure adds no acceptance.

- **0.9.8 beta.2 baseline:** [#702](https://github.com/k-kostin/omavless/pull/702)
  continues existing beta from `a04dfde8` with exact #690 retained restore and
  #694 opt-in selective-close checkpoints, version `0.9.8-beta.2`. Both pin maps
  remain empty. The [selection](../development/BETA_098.md) records complete
  source histories, merge safeguards, dormant dependencies and exclusions.
  Ordinary runtime does not gain default Backup/Restore UI or close authority.
  Both architecture app/DNS/frontend triples were assembled and inspected from
  exact executable-source checkpoint `4594a487`; combined test outcomes belong
  to #702, not to prior installed-head reports. Its later final source
  `06f9bc1c` and inspected package pairs remain recorded in #702. Background subscriptions/resume #700,
  product K1/S1/P4 and GUI are not selected. Main, frozen 0.9.7 RC and Marketplace
  stay unchanged; internal beta is not a release or whole T3/T4 closure.

- **0.9.7 maintenance RC source selected via #438:** selected 0.9.6 read-side
  scope plus #435's honest Connections loading state and a small Settings
  credit from the installed plugin manifest. #433's socket-fixture correction
  was already integrated into 0.9.6, so its changes are not duplicated. See
  [the 0.9.7 ledger](../development/RC_097.md) for exact selection, exclusions
  and pending package/installed gates. `0.9.7-rc.1` is not accepted or public.

- **0.9.6 RC source prepared:** the owner selected the reviewed beta for
  `rc/0.9.6`; [its ledger](../development/RC_096.md) freezes the seven
  read-side/presentation changes and records remaining exact-RC checks and
  distribution gates. Source version `0.9.6-rc.1` has empty package pins.
  This branch is RC-integrated, **not yet an accepted or published RC**.
  Public 0.9.5 RC1 remains the accepted historical candidate; main and
  Marketplace remain at stable 0.8.2.

- **0.9.6 beta assembly:** owner selected #403/#408/#413/#429 TUI read-side
  refinements and #409/#411/#420 QML state/tooltips, based on accepted
  `rc/0.9.5`. See the [bounded selection](../development/BETA_096.md).
  Combined deterministic and actual EN/RU UI checks pass. Installed ARM64 and
  [x86_64](../testing/BETA_096_PC_VM_2026-10-01.md) bounded read-side beta
  reviews pass. Version `0.9.6-beta.1` still has empty unpublished package
  pins; public/clean-install and later RC/release acceptance are not claimed.
  This is the recorded input to the 0.9.6 RC scope freeze.

- **0.9.5 RC accepted:** `rc/0.9.5` supersedes 0.9.0 as the selected candidate.
  Public `v0.9.5-rc.1` packages for both architectures have matching pins;
  anonymous downloads and clean guided ARM64 setup/onboarding passed. Use the
  corrected `frontend2` asset: the first pass found and fixed strict QML
  rejection of T3's bounded log-hint extension. No runtime/security policy
  changed. The [RC ledger](../development/RC_095.md#public-rc1-acceptance)
  records exact artifacts, retained host evidence and limits. Main/Marketplace
  stay on stable 0.8.2; 0.9.0 branch/assets are immutable historical acceptance.

- **0.9.5 scope freeze history:** `rc/0.9.5` selects T3 read-only operator views and
  explicit transient T4 provider information on the accepted managed-DNS/T2
  base. #392 is integrated into beta; installed ARM64 beta review now passes,
  complementing the x86_64 VM record. Source version is `0.9.5-rc.1`.
  The [RC ledger](../development/RC_095.md) owns final checks/artifacts,
  limitations and excluded foundations. At scope freeze pins were empty; stable
  main/Marketplace and accepted `rc/0.9.0` are unchanged.

- **0.9.5 beta history:** owner-approved `beta/0.9.5` starts from accepted
  `rc/0.9.0` at `a543a45c34dcc953ef2e9cd019499146f85ac2eb`, not from the older
  #375 composition branch. Scoped `dev/*` PRs target beta; `rc/0.9.5` is a later
  scope freeze. The [#388](https://github.com/k-kostin/omavless/pull/388)
  T3 checkpoint preserves the original read-only chain, passes combined local
  checks and fixes a revoked-terminal cleanup panic. EN/RU synthetic terminal
  rendering was inspected. [#389](https://github.com/k-kostin/omavless/pull/389)
  adds explicit transient provider usage/expiry reads in TUI; ordinary lists and
  diagnostics exclude those private assertions. Source version is now
  `0.9.5-beta.1`, with empty unpublished package pins rather than relabeled RC2
  artifacts. The exact x86_64 app/DNS/frontend beta triple passed bounded
  installed T3 stale-private-row, two-client revision fence, EN/RU and synthetic
  T4 provider-claim checks in an isolated Omarchy VM. ARM64 CI artifacts were
  strictly paired offline with the reviewed frontend; installed ARM64 beta
  review passed at selection. External-provider positive evidence remains
  unavailable; public provisioning passed at the RC checkpoint above. Other
  T4/S1/K1 inactive foundations are not advertised
  as working features. The
  [beta ledger](../development/BETA_095.md) records selection and remaining gates.
  RC 0.9.0 history, stable main and Marketplace stay unchanged; subsequent owner
  authorization and RC 0.9.5 publication are recorded above.

- **0.9.0 managed-DNS candidate:** [#331](https://github.com/k-kostin/omavless/pull/331)
  merged into RC at `67b5f21`, integrating #295 and its stacked delivery work
  through #316: the fixed DNS broker/core pair, exact package admission,
  explicit enrollment and guided two-package first use. The supported candidate refuses a new Connect
  when the managed pair is absent; the shipped legacy path's cancelled-DNS
  defect [#132](https://github.com/k-kostin/omavless/issues/132) remains open.
  See the [distribution contract](../development/DNS_RELEASE_DISTRIBUTION.md)
  for the security and upgrade decisions.

- **Installed evidence:** The experimental ARM64 pair passed an owner-attended
  Full VPN/DNS/HTTPS/release cycle without recurring DNS dialogs. The isolated
  x86_64 PC VM passed agent-run release-pair first use, live modes and server
  change, resolved restoration, core/broker crash containment, active and
  quarantined package-removal refusal, and proven-empty removal/re-enrollment.
  Exact identities, failures and limits remain in the [PC record](../testing/DNS_BROKER_PC_PREINSTALL_2026-09-27.md),
  [fresh-setup](../testing/DNS_RELEASE_VM_FRESH_SETUP_2026-09-28.md),
  [network](../testing/DNS_RELEASE_VM_NETWORK_2026-09-28.md) and
  [removal](../testing/DNS_RELEASE_VM_REMOVAL_2026-09-28.md) reports. Those tests
  used temporary VM-only firewall allowances. Default-deny UFW blocked TUN
  ingress; restricting the exception to the TUN peer source did not work because
  return packets have remote source addresses. The allowances were removed.

- **Managed-DNS RC acceptance:** The production release pair passed an owner-attended
  ARM64 Full VPN/DNS/HTTPS/mode cycle and core-crash containment at the exact
  locally installed package identity; the user observed no separate DNS/route
  password dialogs during runtime transitions. The immutable validation-only
  `v0.9.0-rc.1` packages/frontend were anonymously downloaded and hash-verified.
  An ARM64 account with an empty private home and both system packages removed
  completed the real Required components GUI path: pinned public downloads,
  package installation, DNS enrollment, onboarding and the usable
  disconnected/Rule panel, with no profiles or automatic VPN. The original
  private store was preserved and its broker enrollment restored. A preceding
  empty-account pass found and fixed false pre-activation VPN controls.
  A stopped-broker Connect refusal restored Disconnected/Rule without a false Full VPN claim.
  A connected broker SIGKILL negative also retained the TUN/FD and refused a
  mode change as `manual_recovery_required`, without a false Full VPN claim;
  coordinated reboot restored clean Disconnected/Rule and original private data.
  The [candidate disposition](../development/RC_090.md#managed-dns-and-mode-failure-disposition)
  supersedes the legacy prompt scenario only for the mandatory managed 0.9 path.
  RC2 includes the corrected first-use frontend, the CLI lifecycle-response fix
  from #384 and fixed-enum recovery diagnostics. Both final architecture pairs
  are inspected/pinned. The installed final ARM64 pair passed Rule/Full VPN,
  TUN-bound HTTPS/DNS, a 300-second health watch, direct CLI modes/Disconnect and
  clean restoration. An earlier preliminary cycle under concurrent test load
  entered quarantine; its cause remains unproven, not claimed fixed by logging.
  Exact artifacts/public-download and CI results are in the
  [final RC2 checkpoint](../development/RC_090.md#final-rc2-artifact-and-acceptance-checkpoint)
  and #386. Stable promotion remains a separate owner decision.
  #270/#132 are open; `main` remains at stable 0.8.2, while `rc/0.9.0` now
  includes #331. The separately accepted #271/#272,
  native #135 disposition and available XHTTP V0 evidence stay recorded in the
  [RC ledger](../development/RC_090.md). Missing protocol fixtures are not
  represented as PASS.

- **Historical #292 preparation before public validation assets:** #292 aligns the candidate
  version and fail-closed bootstrap metadata. Native ARM64/x86_64 package CI and
  combined tests passed; the common frontend matches both build records. See
  [artifact identities and remaining attended gate](../testing/RC_090_PACKAGE_PREPARATION_2026-09-24.md).
  The prerelease pins were prepared but not publicly downloaded at this
  earlier checkpoint; the later result is recorded above. The exact local ARM64 package and common
  frontend passed attended replacement, private-state preservation and original
  Routing/profile restoration. New [DNS authorization evidence](../testing/RC_090_DNS_AUTHORIZATION_2026-09-24.md)
  reproduces #132: cancelled DNS prompts leave an incorrect connected claim.
  Following a PAM lockout/backoff, final original-state recovery passed with
  matching DNS readback and TUN-bound HTTPS; Open app/focus/close also passed.
  #288 remains investigation, not issue #270/#132 closure or RC readiness.

- **Owner-required RC completion gates, September 24:** T2 acceptance alone
  does not make 0.9.0 ready. Work through #272, #271, #270, the native disposition
  of #135/#132 and Rust adaptation of #30 before proposing main promotion.
  See [mandatory gates](../development/RC_090.md#additional-mandatory-owner-gates--september-24).
  #272/#286 is accepted in RC with installed EN/RU
  [probe presentation](../development/PROBE_SEMANTICS.md).
  #271/#287 is accepted in RC with installed bounded
  [setup diagnostics](../development/SETUP_DIAGNOSTICS.md).
  Native [mode confirmation](../development/NATIVE_MODE_CONFIRMATION.md), #289,
  supersedes the now-closed Python PR #135; DNS cancellation issue #132 remains.
  The native #30 successor [#290](../testing/NATIVE_LIVE_PROTOCOL_VALIDATION.md)
  passed available XHTTP `stream-one` Full VPN/TUN/HTTPS, private controller and
  original-state restoration. The original Draft #30 and historical evidence stay
  unchanged; its body links the native successor. A preceding admission refusal
  remains unexplained; a delay before `ready` is not its cause.
  DNS contract #288 remains a gated proposal, not installed/prompt-free behavior.
  #270/#132 and exact versioned-package host gates are not closed by T2 or V0.
  Missing V0 fixtures remain gaps, not a protocol-maturity promotion.

- **Next TUI RC, not main:** `rc/0.9.0` integrates T2a–f (#269/#274/#275/#277/#279/#280),
  the read-only subscription overview (#281), local session activity (#282),
  session-local language/theme settings (#283), the accepted T2 MVP (#284),
  the release-snapshot workflow (#276), and #270–272 triage docs (#273).
  See the [exact constituent ledger and release checklist](../development/RC_090.md).
  The name is a planning label; stable release version/assets and
  stable main `d620c300020d3acfa9c00418da7f6cded485ffdb` are unchanged.
  Marketplace request [#8093](https://github.com/omacom/omarchy-plugin-marketplace/issues/8093)
  targets that stable SHA and was approved/published; it does not verify the RC.
  T2d/e passed [combined ARM64 inspection](../testing/T2_INSPECTION_THEME_2026-09-22.md):
  live traffic, details, diagnostics and theme presentation; closing the client
  preserved the tunnel. T2f passed [attended single-subscription refresh](../testing/T2_SUBSCRIPTION_REFRESH_2026-09-22.md)
  without changing the active profile/mode. The integrated read-only overview shows
  empty subscriptions, saved/missing profile counts and saved-list age through
  the existing snapshot. The original read-only scope did not implicitly accept
  mutations; empty-feed refresh, refresh-all, attempt history and probes now
  have separate combined evidence in #284.
  The integrated TUI-only checkpoint #282 adds a 32-event in-memory session history,
  without private targets/raw logs, persistence or new runtime methods.
  The integrated session-settings slice #283 adds immediate window-local language/theme
  choices, including offline use; installed package/plugin settings stay unchanged.

- **T2 MVP accepted for RC, #284:** `dev/t2-mvp-completion` targets RC and
  implements the remaining operations, selected/all profile HTTPS checks,
  count-only connections, allowlisted details, default package feature and
  main-panel Open app below Profile actions (not Settings).
  Source `02a5a13b807aab8d984f37cc49e20eab71374942`
  passed 1,098 Rust tests / 11 ignored, developer/QML gates and test/x86_64/ARM64
  CI. The exact ARM64 developer package and matching frontend are installed in
  Try Omarchy; private data, disabled service enablement and startup Off were
  preserved. Stable restoration artifacts are retained outside Git. Installed
  lifecycle, refresh, read-side and close checks passed; connected profile-check
  jobs completed but their observed measurements were negative. The frontend
  follow-up moves Open app to the main footer and fixes first-window launch;
  launch/focus/close passed without changing the tunnel. The final combined pass
  confirmed cross-client stale-command rejection, cancellation, same-client
  runtime restart, original connection restoration and a positive profile HTTPS
  measurement with the main tunnel disconnected. Private Unix-only controller
  and PID-attributed absence of a TCP controller passed. EN/RU rendering and
  both-architecture package CI passed on implementation head
  `4e9960f1badf13f4426a4f49a4a7447d604d48f0`.
  See [combined acceptance and limits](../testing/T2_MVP_2026-09-24.md).
  **The bounded T2 MVP is complete as a development checkpoint**, not published
  0.9.0; AUTO-1, DNS/provider follow-ups and other host/protocol gates stay separate.
  This replaces only the VM's test installation, not any public 0.8.2 artifact,
  stable-main snapshot or marketplace submission.

- **September 22 release-snapshot workflow:** main stays at the owner-approved
  release snapshot until another explicit main-update instruction, including
  for docs-only work. The former automatic documentation merge permission is
  revoked. Daily decisions/status remain visible in issues and `dev/*` PRs;
  completed checkpoints and their docs may join a named `rc/<version>`.
  Every proposed main update must reconcile roadmap/current status/contracts
  and pending documentation PRs through the
  [release checklist](DEVELOPMENT_WORKFLOW.md#release-reconciliation-checklist).
  This policy candidate does not itself update main or the marketplace request.

- **Historical constituent T2c:** the dependent `dev/t2-grouped-browsing` branch
  adds subscription grouping, local favorites filtering and subscription-name
  search. Local suites, EN/RU terminal review and no-effect installed-runtime
  checks passed; no default package or main update. See
  [scope and evidence](../development/T2_GROUPED_BROWSING.md).

- **Historical constituent T2b:** `dev/t2-connection-actions` adds confirmed
  Connect/Disconnect/mode requests through the existing runtime, retaining exact
  requests on unknown outcomes. It depends on the T2a branch; neither is a main
  update or packaged MVP. Local automated/EN-RU rendering and attended ARM64
  connection/mode gates passed on the [recorded candidate](../testing/T2_CONNECTION_ACTIONS_2026-09-22.md).
  See [scope and gates](../development/T2_CONNECTION_ACTIONS.md).

- **Historical constituent T2a:** `dev/t2-readonly-client` adds an opt-in
  read-only terminal client using the existing Rust runtime. Main/installed
  0.8.2 remain unchanged while marketplace review targets the submitted SHA.
  Status, profile/source browsing, name search and safe close are this slice;
  mutations, packaging and Open app are not. See the
  [development boundary](../development/T2_READONLY_CLIENT.md).

- **September 22 stable release:** the owner authorized completing publication
  after the preparation checkpoint. `v0.8.2` is now stable/latest on GitHub;
  its tag and all six asset identities/digests are unchanged. README and
  installation guidance describe the actual guided path rather than an
  unpublished candidate. Marketplace submission targets the final reviewed
  main SHA after this documentation update; approval/deployment remain external
  steps. Do not confuse a submitted request or stable release with a changed
  marketplace snapshot. Seven fully merged remote source branches (#250–253,
  #263–265) were removed after exact-head/reachability checks. Preserve the
  Python archive and open #30/#135 evidence branches.

The dated preparation checkpoints below retain their original outcomes and
publication boundaries; the current release decision above supersedes their
older "withheld"/prerelease status, not their acceptance limits.

- **September 22 ARM64 update and marketplace preparation:** the paused Try
  Omarchy guest's original private state was restored, then the actual public
  0.8.2 ARM package and matching frontend were installed with attended
  authorization. Running identity, startup Off, initial disconnected cleanup
  and subsequently observed connected Routing state are recorded in the
  [ARM update section](../testing/NATIVE_082_ARTIFACTS_2026-09-21.md#september-22-installed-arm64-update-after-the-paused-clean-test).
  Current product screenshots and publication-preparation statuses were refreshed.
  No runtime implementation, immutable release asset or marketplace listing was
  changed by this preparation. Stable promotion/submission remain owner-controlled.

- **Fresh 0.8.2 x86_64 provisioning passed:** a clean Omarchy 4.0.4 QEMU/KVM
  guest installed unmodified main, then both absent Mihomo and the pinned
  native package through the real plugin UI. Activation, disconnected/empty
  startup Off, onboarding completion and settled reopen passed without manual
  repair. See [exact identity and limits](../testing/NATIVE_082_FRESH_VM_2026-09-21.md).
  The local marketplace baseline has no findings and expected capability-only
  review requirements. Stable promotion and an exact-current-HEAD marketplace
  update request remain separate owner decisions; no submission was made.

- **0.8.2 published as a prerelease:** #263 and #264 are merged. The
  [public release](https://github.com/k-kostin/omavless/releases/tag/v0.8.2)
  identifies `f442714362620c18e1bbaa6415d9e0c2e08c0a8a`; both native packages
  identify `22e23e64c49b8110088b6be3f063b5e641ba0853`. Final PR CI passed,
  frontend pairing matched the actual pins and protected runtime inputs, and
  all six public assets passed anonymous download/checksum verification.
  0.8.0/0.8.1 remain immutable. This is not stable promotion or marketplace
  submission; the later fresh guided gate is recorded above. The
  installed active connection was not changed by this build/release work.
  See the [0.8.2 artifact checkpoint](../testing/NATIVE_082_ARTIFACTS_2026-09-21.md).

- **Post-0.8.1 fixes, #263 (merged):** correct Full Quit's installation preflight and
  the installed-review findings for subscription keyboard focus and EN/RU
  name-search copy. The runtime fix has attended physical-PC disconnected
  Quit evidence; UI-only follow-up was checked without interrupting the
  owner's active connection. See the [follow-up record](../testing/R5_NATIVE_FULL_QUIT.md#september-21-release-follow-up-263).
  These fixes are in the new `v0.8.2` candidate, not the immutable public
  `v0.8.1` assets. Do not overwrite old releases or claim fresh guided
  provisioning passed from this UI review.

- **Historical 0.8.1 prerelease:** retain the already published `v0.8.0`
  prerelease unchanged. #258–260 and #261 are merged; the latter integrates setup and the
  corrected release candidate. Both native packages now pass CI at runtime source
  `98e0b275ae43d6e7d7901b58fab31457b542f878`; setup pins contain their actual
  hashes rather than the old 0.8.0 runtime. See the
  [0.8.1 artifact checkpoint](../testing/NATIVE_081_ARTIFACTS_2026-09-21.md).
  The public tag/frontend is `20b5e6c4f3ef466207d37306d4a70fe5876162f6`;
  all six release assets were downloaded anonymously and verified against the
  retained hashes. #249 and #257 are integrated and their branches are removed.
  Fresh guided provisioning remains the stable-promotion gate. Host evidence
  is not inferred from publication. Marketplace submission remains withheld.

- **Release finalization, September 21:** physical x86-64 found and corrected
  foreign-TUN recovery, connected-profile replacement and misleading subscription
  feedback in #258–260. The combined development candidate is now installed;
  attended package/update/reconnect, preserved profiles/foreign VPNs, fresh
  Connected/Rule and user-confirmed operation are recorded in the
  [PC continuation](../testing/PC_080_PREINSTALL_2026-09-21.md#corrected-installed-candidate--september-21).
  The matching QML required a graphical-shell restart to replace cached JS;
  this did not restart the native connection. Release integration reconciles
  #249's setup and #257's historical PC evidence with these fixes. Existing
  `v0.8.0` prerelease/tag/assets still identify the earlier runtime, not this
  installed candidate. Final corrected artifacts/pins and guided-install gates
  remain distinct. The owner authorizes main/release finalization but explicitly
  withholds marketplace submission until a separate instruction.

- **Historical release progress, September 16:** #250–253 are merged (user-facing content,
  preserved developer docs, runtime/frontend pairing and x86_64 package build).
  [v0.8.0](https://github.com/k-kostin/omavless/releases/tag/v0.8.0) is now public
  as a **prerelease / not latest**, with actual ARM64/x86_64 archives and the
  reviewed common frontend. Anonymous asset downloads and hashes PASS. #249
  has real pins and remains Draft for guided installation acceptance, not for
  unavailable release files. Physical x86_64 acceptance is planned, not PASS.
  Use the [current first-run report](../testing/MARKETPLACE_FIRST_RUN_2026-09-15.md)
  and [ready-package PC gate](../testing/PC_080_RELEASE_GATE_2026-09-16.md).
  Owner authorizes advancing release/marketplace after those gates; the old
  marketplace snapshot remains unchanged until its separate update approval.

- Owner-authorized final `0.8.0` source/package preparation follows the merged
  #244–#247 fixes and offline release tooling. This version preparation is not a
  tag or publication by itself; the actual later prerelease is recorded above.
  Use the [final candidate report](../testing/NATIVE_080_FINAL_CANDIDATE_2026-09-14.md)
  for actual ARM64 build/update results and outstanding x86_64/owner gates;
  old RC artifacts keep their original identity.
  Final ARM64 source `b7fd0a99b8b169f0933e5f43ea4389642015193a` now has
  full static/Rust/CI, real archive inspection, installed final binary/frontend
  identity, safe report and guarded Full VPN HTTPS/disconnect/restoration PASS.
  The update script's mistyped final acknowledgement remains qualified in the
  report; read-only checks prove its installed outcome and unchanged private
  data. Do not repeat package installation just to replace that transcript.

- Owner-approved [Python reference retirement](LEGACY_RETIREMENT.md) preserves
  the complete pre-retirement tree in frozen `archive/python-legacy` at
  `aa5873783c019edc303a732e55ea8c85f1f0b090`. Native RC delivery (#239) and
  native-only defaults (#242) are merged. [#243](https://github.com/k-kostin/omavless/pull/243)
  integrates the accepted fixture-backed legacy removal, completing the four-step
  checkpoint. Final-source preparation is now authorized; publication remains
  a separate owner decision. Use GitHub for the
  final merge SHA, not historical preparation-only status below.

- Native integration [#238](https://github.com/k-kostin/omavless/pull/238)
  merged at `778647215deb1cb27e66e10e628fd0e78beee1af`. Its PR and main CI
  passed; #214–#237 are merged or explicitly superseded and their remote
  source branches were cleaned up. This integration is not still waiting to
  be published to main.
- Subscription-row refresh [#240](https://github.com/k-kostin/omavless/pull/240)
  merged at `3b82c4f66ca624d88a6d28b6bd68ddd96c11155a`, after exact-head CI
  and Try Omarchy ARM64 UI/live-refresh acceptance. Its compact main-page
  action reuses the existing Rust operation; VPN ownership is unchanged.
- Native **0.8.0-rc.1** assembly merged in
  [#239](https://github.com/k-kostin/omavless/pull/239) at
  `87844c1ffc24320285427a740ec43ffc6e68bee8`, following the
  [release assembly guide](../../packaging/release/README.md). Local artifacts and
  archive checks and the attended ARM64 installed update from source
  `4549f6921e908a951698617027b4971057278920` are recorded there. The new pair
  includes #240; earlier archived pairs retain their original identities.
  This is not a stable release or marketplace update.
- #30 and #135 remain separate Drafts. Do not merge or discard their evidence
  as part of repository cleanup. Marketplace text/screenshots and publication
  remain owner-controlled.
- Native onboarding [#244](https://github.com/k-kostin/omavless/pull/244)
  merged at `9e8587847295a7a9a2dee64d7ab0ab004013cb73`; RC support-report
  parsing [#245](https://github.com/k-kostin/omavless/pull/245) merged at
  `6b9aa75dbe4ad76882b18d65895105ea85fbab93`. Their combined runtime/frontend
  bytes match the already installed Try Omarchy candidate. Clean same-account
  first-use onboarding and completion persistence passed; original private
  profile bytes were restored and verified. The missing final restoration
  helper marker and qualified owner recollection remain documented, not a
  reason to repeat destructive setup. See the
  [release handoff](../testing/NATIVE_080_RELEASE_HANDOFF_2026-09-14.md).

## Accepted native checkpoint

| Track | Current state |
| --- | --- |
| R0–R4 | Rust protocol, profile/domain/store and core foundations accepted; do not restart their migration |
| R5 / T1 | Rust owns the activated native application's state, mutations, background work and Mihomo lifecycle; restored QML is a client of that owner |
| R6 | Native installed path closed under the owner-approved scope, with deliberate installed Python-absence evidence |
| Enabled login autoconnect | AUTO-1 open; Last/pinned fresh-login acceptance is deferred, not PASS; default is Off |
| Network/DNS | Separate unresolved investigation; failed probes remain failures and are not hidden by migration closure |
| Native distribution | Reviewed local Arch package and explicit native-only frontend install accepted; no automatic marketplace migration or release implied |
| V0 / #30 | Draft; accepted exact XHTTP evidence preserved; missing-family/UDP-restricted evidence remains unavailable |
| T2 | Migration prerequisite satisfied for the accepted native path; client implementation is a separate task, not part of this integration |
| P4, K1, NixOS and other host tracks | Own design/fixture/security/host gates still apply; not promoted by R6 |

The [R6 closure](../testing/R6_LOCAL_CLOSURE_2026-09-13.md) states the exact scope,
tested runtime/frontend/package identities and exceptions. The
[integration evidence](../testing/R6_PUBLICATION_CANDIDATE_2026-09-13.md) records
the final local suites and the mapping of intermediate Drafts #214–#237 to
this tree. It is a dated preparation report, not a permanent publication block.
Do not merge those obsolete Draft implementations over the integrated result.

## What users install

**Additional release gate, 2026-09-15:** installed-candidate R6/ARM64 acceptance
did not cover a fresh marketplace clone with no `/usr/bin/omavless`. The new
[first-run setup checkpoint](../testing/MARKETPLACE_FIRST_RUN_2026-09-15.md)
adds a runtime-independent setup page and explicit terminal provisioning.
Published architecture-specific package pins and download checks are now ready;
real clean package install → activation → onboarding acceptance subsequently
passed on x86_64 as recorded in the [fresh VM report](../testing/NATIVE_082_FRESH_VM_2026-09-21.md).
This closes that scoped provisioning gate; it does not reopen the unchanged R6
migration, claim new live-network evidence or publish the marketplace snapshot.

The current marketplace 0.8.2 snapshot is
`d620c300020d3acfa9c00418da7f6cded485ffdb`, approved and published through #8093.
The 0.7.0 `69fe05b03129a23664fff3f8289821a7b7f80095` snapshot is historical;
the newer internal RC is not covered by that approval.
Neither a main merge nor the presence of Rust sources installs a native binary,
runs Cargo, grants capabilities, changes an ownership marker or enables VPN
startup on a user's machine.

The [native guide](../user/NATIVE_INSTALL.md) documents the stable guided path:
add the plugin, explicitly install any missing components in its setup terminal,
complete activation once and follow onboarding. Manual package-first installation
and existing-user migration/update remain separate routes. `./install.sh` installs
an already activated application's matching frontend. It is native-only;
`--native-only` is an alias. The launcher refuses absent/legacy/unknown native
ownership without Python. Omarchy's clone-based installation does not install
the package or run this installer. The old Python implementation is archived;
remaining Python is independent developer/test/build tooling and frozen-reference
replay, not a supported fallback or second native lifecycle owner.

## Next work, in order

1. Retain the completed four-step reference retirement, native-only default and
   scoped R6 evidence. Do not reintroduce the Python runtime or rerun unchanged
   migration gates merely because test/docs cleanup merged.
2. Follow the submitted exact-main marketplace update #8093 using the published
   0.8.2 artifacts/pins and accepted clean x86_64 provisioning. Do not resubmit or rebuild
   them or repeat R6 merely because documentation/images change. Follow the
   [publication preparation](../marketing/MARKETPLACE_080.md), rerun official
   compatibility/security checks on the final exact commit, and retain the
   existing listing while review is pending. Stable promotion is complete;
   record the actual request and bot/reviewer outcome in the release PR rather
   than inventing approval. Preserve older tags/assets unchanged.
3. Address [AUTO-1](LOGIN_AUTOCONNECT_FOLLOWUP.md) and the
   [network investigation](../testing/R6_NETWORK_DIAGNOSIS_2026-09-13.md) with
   their actual reproducible host scenarios; no repeated password/dialog loops.
4. Prepare a separately owner-authorized stable/main proposal from accepted
   `rc/0.9.5`, reconciling release notes, constituent PRs and candidate docs.
   RC1 public assets/pins and clean provisioning are completed, not a fresh work
   queue. Do not silently bump/publish stable packages or main. Preserve accepted UI unless the task
   deliberately changes it under the [UI/UX contract](UI_UX_CONTRACT.md).

Retained mandatory RC disposition: [the three native follow-up issues](../../DEVELOPMENT_ROADMAP.md#native-follow-up-triage--review-and-scope-the-issues)
for scoped DNS authorization (#270), network-setup diagnostics/compatibility
(#271), and ICMP/HTTPS result semantics (#272). The September 24 owner direction
makes their disposition and applicable implementation/acceptance mandatory.
That candidate gate is retained in RC 0.9.5; #270/#132 remain open for the older
stable 0.8.2 path, not a reason to repeat unchanged accepted RC host checks.

Historical acceptance reports retain their original heads and outcomes. Their
old "Python still owns production", "R5 incomplete" or "publication withheld"
sentences describe those earlier checkpoints, not the current native state.
Do not erase negative evidence, advertise untested host behavior or convert
deferred work to PASS while synchronizing documentation.
