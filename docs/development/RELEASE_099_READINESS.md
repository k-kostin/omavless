# 0.9.9 development readiness and release remainder

Reconciled November 2, 2026 against accepted development source
`ce0d3597f741780e1c6821e5e78ed540eeef755b` ([#843](https://github.com/k-kostin/omavless/pull/843)).
This is the release preparation index, not a public RC freeze or a release.
Stable main remains `bcd744ecb3c1c61bbfe4a8ab090358b225ce1e76`, publishing
0.9.8 fix.8 from full source `5edf4eb203926d8bd39a3ff80f611a19435c3787`.
The proposed 0.9.9 guide and local packages are not public download acceptance.

## What is already earned

The selected development composition has passed its PR checks and independent
merged-development push checks, ordinary x86_64 0.9.8-to-0.9.9 update with data
preservation, and affected installed Help/Settings review. A new exact merged
source export, offline validation, committed distribution, source-CI admission,
full Git byte/mode inspection before its first generated bootstrap, and
post-clean checks also completed. Their original receipts belong to #843;
this editorial reconciliation does not repeat or broaden those results.

The native producer selected by the frontend is
`6159aa66b9a6e9c9a8cfa7c558a6006c5b08070f` for both architectures. Actual app/DNS
archives, matching pins and corresponding-source inputs were inspected. The
installed ordinary-update result is x86_64 evidence; archive inspection is not
an installed ARM64 result. Any later source composition must pass the existing
[input-equivalence and delivery procedure](../../packaging/release/README.md).

## Completion matrix

Row numbers preserve the owner's full 0.9.9 scope inventory. **Accepted** means
the stated development scope earned its owning gates. **Retained** identifies
existing behavior and inherited evidence, not new installed acceptance for every
protocol/platform. **Open**, **Hold** and **External** retain their concrete next
proof. **Deferred** is a version decision, never a passing test.

The separately excluded full GUI/G1, V0 protocol campaign, standalone Arch/NixOS
delivery and Xray backend/routing are not additional 0.9.9 blockers. Retained
research and historical results do not activate those tracks.

| Row | Selected user result | State and evidence | Remaining boundary |
| --- | --- | --- | --- |
| 1 | Explicit Restore recovery preserving the selected old history and returning to ordinary Off | Accepted selected recovery scope through [#823](https://github.com/k-kostin/omavless/pull/823)–[#826](https://github.com/k-kostin/omavless/pull/826); [Restore contract](../roadmap/PRIVATE_BACKUP_RESTORE.md) | Preserve failed aggregates and individual earned outcomes; no universal power-cut, all-platform or public-install claim. |
| 2 | Read-only Connections | Retained accepted read-only scope; [TUI contract](../roadmap/TUI_APP.md) | New Single/Set Close entirely deferred to 1.0.0; unfinished [#833](https://github.com/k-kostin/omavless/pull/833) is not 0.9.9 readiness work. |
| 3 | Import and use the supported experimental WG/AWG subset | Accepted selected code and installed scope through [#835](https://github.com/k-kostin/omavless/pull/835), with later combined CI/source admission; [protocol contract](../../PROTOCOL_ROADMAP.md) | Experimental IPv4 stock WG/AWG3-order12 evidence does not cover every AWG format, IPv6, ARM installation or kill switch. |
| 4 | Subscription refresh progress/Cancel and selected Daily scheduling | Accepted selected refresh/replay/restart-no-resend and Daily CLI slices, [#836](https://github.com/k-kostin/omavless/pull/836)/[#837](https://github.com/k-kostin/omavless/pull/837) | No new scheduler, graphical schedule editor or automatic retry authority; physical resume/network remains row 5. |
| 5 | Last/pinned login behavior and recovery after sleep/network changes | Accepted selected x86_64 VM Last/pinned and same-login Disconnect/restart-Off evidence; [AUTO-1 contract](../roadmap/LOGIN_AUTOCONNECT_FOLLOWUP.md) | **Hold:** physical suspend/resume and real network transitions require the separately owner-ready session. Selected VM login evidence does not close them. |
| 6 | App Proxy without TUN | Deferred entirely to 1.0.0 by owner choice | Preserve [S1 research](https://github.com/k-kostin/omavless/pull/693); no curl-only replacement or activation for 0.9.9. |
| 7 | Full VPN kill switch | Deferred entirely to 1.0.0 by owner choice | No default-off readiness batch for 0.9.9; retain future [security/physical gates](../roadmap/KILL_SWITCH.md). |
| 8 | Persistent Lockdown | Deferred entirely to 1.0.0 by its separate owner choice | No implicit permanent internet-blocking or startup policy. |
| 9 | Useful Connections details, activity/history and bounded diagnostics | Retained read-only implementation and owning T2/T3 evidence; [TUI contract](../roadmap/TUI_APP.md) | No durable raw-log framework or new Close writer. Preserve exact accepted scopes; do not turn code presence into new all-platform installed proof. |
| 10 | Opt-in recorded TUN traffic across runtime restarts | Accepted selected statistics development delivery, [#841](https://github.com/k-kostin/omavless/pull/841)/[#842](https://github.com/k-kostin/omavless/pull/842); [statistics contract](PERSISTENT_TRAFFIC_STATISTICS.md) | Partial observed RX/TX for one period, not provider quota or complete internet traffic. Original C5 CI failure remains failed; the combined successor does not rewrite it. |
| 11 | Three DNS presets saved while Off for the next connection | Accepted [#840](https://github.com/k-kostin/omavless/pull/840); [DNS contract](USER_DNS_PRESETS.md) | No arbitrary DNS editor, hot reload, root API or general leak/health claim; custom-template and WG DNS boundaries remain. |
| 12 | Supported routing presets and custom rules | Retained semantic preset selection and rule list/add/delete/check; [control-plane contract](../roadmap/CONTROL_PLANE.md) | General template editing belongs to deferred row 15. Existing controls need no second writer or reimplementation. |
| 13 | Explicit bounded batch profile checks with progress and cancellation | Retained selected/all probe jobs and TUI Jobs; [probe semantics](PROBE_SEMANTICS.md) and [T2 evidence](../testing/T2_MVP_2026-09-24.md) | Preserve supported-target and unavailable-result limits. Historical negative connected measurements are not positive coverage of every protocol or WG/AWG target. |
| 14 | Main-panel HTTPS Test and Ping with truthful results | Accepted [#839](https://github.com/k-kostin/omavless/pull/839), including affected EN/RU rendering, keyboard and Off preservation | No new prober or broad network/ARM/physical acceptance claim. |
| 15 | Own-format YAML import/export and text/template editors | Deferred entirely to 1.0.0 by owner choice | Existing URI/WG import/export, text editing, presets and custom rules remain; no partial YAML plumbing for 0.9.9. |
| 16 | Complete matching EN/RU guidance, accessible Help and locale routing | Technical guide/Help composition accepted in [#843](https://github.com/k-kostin/omavless/pull/843) | **Open:** owner meaning review of the exact guide pair, then the same authorized stable EN/RU/Help snapshot. Russian Help currently falls back to English. |
| 17 | Ordinary Tailscale coexistence without manual repairs | Historical conditional [#808](https://github.com/k-kostin/omavless/issues/808) evidence retained | **Hold:** explicit owner-ready physical session. Historical VM configuration and separate GBrain DNS work do not establish this outcome. |
| 18 | Guided installation/update and reachable all-state removal | Accepted selected Remove UI [#820](https://github.com/k-kostin/omavless/pull/820) and ordinary installed update [#843](https://github.com/k-kostin/omavless/pull/843) | Public download and clean guided installation from the selected public delivery remain distinct. Official repositories/Marketplace are separate channels. |
| 19 | Existing Backup/Restore reliability and selected KDF hardening | Accepted selected [#838](https://github.com/k-kostin/omavless/pull/838) and inherited Backup/Restore gates | Do not recreate completed workspace/openat work. Retained catalog/recovery research is not automatically a new blocker or a new accepted capability. |
| 20 | Required builders, fixtures and concurrency controls | Maintained source fixture/export/validation route is available and used; [delivery tooling](../../packaging/release/README.md) | Distinct retained [#550](https://github.com/k-kostin/omavless/pull/550)/[#572](https://github.com/k-kostin/omavless/pull/572) work requires an actual selected dependency; no new framework or speculative test-failure claim. |
| 21 | Save exact original legacy-store bytes before normal startup | Accepted [#842](https://github.com/k-kostin/omavless/pull/842), closing [#169](https://github.com/k-kostin/omavless/issues/169); [preservation contract](ORIGINAL_STORE_PRESERVATION.md) | Explicit private **unencrypted whole-file copy**, not repair, supported-profile export or permission to Connect. Strict XHTTP rejection remains. |
| 22 | Accurate support and reporter follow-up | External wait in [#773](https://github.com/k-kostin/omavless/issues/773) after the existing response | No duplicate support reply or inferred reporter success; missing confirmation is not missing Remove implementation. |
| 23 | Complete CI with useful early failures and timings | Required PR/push recipes retained; accepted workflow corrections remain integrated | No measured full-CI speedup or Blaze adoption. Performance experiments cannot waive exact-head checks. |
| 24 | Bounded storage and justified retention | Operational admission gate before allocation and at closure; [retention policy](README.md#proactive-retention-gate) | Fresh whole accounting and retirement remain necessary. The finite whole allowance is not ordinary 20+5 GiB compliance or authority to delete unknown/private state. |
| 25 | Useful source carriers and qualified branch/worktree retirement | Operational classification under the [cleanup lifecycle](../roadmap/DEVELOPMENT_WORKFLOW.md#branch-cleanup-lifecycle) | Preserve actual open-PR/base, unique/evidence and active-writer dependencies. A remote ref, local ref and working folder are different cleanup outcomes. |
| 26 | One truthful final 0.9.9 delivery | Selected development technical composition admitted through [#843](https://github.com/k-kostin/omavless/pull/843) | **Open:** resolve rows 5/16/17 and declare final release/platform scope; then applicable public package/download/install, main, tag/assets and Marketplace gates. Prepared offline pairs are not public delivery. |
| 27 | Extra TUIC 0-RTT / Hysteria2 Realm and local handshake timeout | Deferred entirely to 1.0.0 by owner choice | Existing experimental families remain; no unsafe implicit 0-RTT or new provider-controlled tuning. |
| 28 | Manual core chooser | Deferred entirely to 1.0.0 by owner choice | Ordinary setup/update/version inventory remains. No arbitrary binary path, second unverified pair or decorative selector substituted for the feature. |

## Exact remaining decisions and proofs

1. **Owner meaning review:** the candidate English guide SHA256 is
   `20464ca1574e0e517fcc9c05a95fde6235574ea347ed504c2b028ed847aa1014`;
   Russian is `47824f4fe895466664ba65d3ccafcaf8fab1a355ad1b5440b0d505e9540bba1e`.
   [`GuideLinks.js`](../../plugin/GuideLinks.js) records `humanReview: null`.
   Technical/model review cannot replace the owner's reading or activate RU.
2. **Physical readiness:** obtain the separately selected ordinary sleep/network
   and Tailscale journey on the physical PC. Keep requested host network state
   and the existing authorization boundaries; do not substitute another VM run.
3. **Final composition and public delivery:** select exact source/frontend and
   matched packages, reconcile these retained limits, and earn applicable
   [release gates](../roadmap/DEVELOPMENT_WORKFLOW.md#release-reconciliation-checklist).
   Public asset availability and a real public guided install cannot be inherited
   from offline inspection or cache-seeded update. Main, tags/assets and
   Marketplace each retain their applicable owner authorization.

Rows with retained implementation are not a request to rebuild existing features.
Seven entire owner deferrals are excluded from 0.9.9 blockers, not marked PASS.
Supporter's external reply is independent of technical source admission.
The accepted development CI/installed operations stay closed unless a later
selected input materially changes their boundary. This page adds no new feature,
waives no failed/unknown gate and grants no publication or host authority.
