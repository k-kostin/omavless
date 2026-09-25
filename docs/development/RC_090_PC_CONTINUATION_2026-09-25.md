# Continue 0.9.0 on the Omarchy PC's disposable VM

This is the current cross-machine handoff, not release approval. It supersedes
older handoff instructions about uninstalled DNS experiments or pending
post-reboot Routing restoration. Read the retained evidence, not a chat replay.

## Ownership and starting refs

The Try Omarchy writer stops after publishing this document and the linked #270
handoff comment. The PC agent takes ownership of `dev/dns-transaction-foundation`
at the **exact handoff commit linked in that comment**. Its predecessor was
`ae30bbe3879248d6a45fe2b90da098dfd49d2c9e`; this document's checkpoint only updates
documentation. Fetch before starting; reconcile any unexpected later writer.

| Ref / issue | Verified departure state |
| --- | --- |
| `main` | `d620c300020d3acfa9c00418da7f6cded485ffdb`, frozen |
| `rc/0.9.0` | `a49ec92598a3f7c195177cb3f3ca2c1b44319749`, unchanged |
| #295 | Draft, DNS dev branch → RC; implementation/evidence pending completion |
| #278 | Draft, RC → main; NOT release-ready |
| #270 / #132 | OPEN: scoped DNS ownership / truthful authorization failure |
| #271 / #272 | Accepted in RC, closed; do not restart completed work |
| #135 | Closed Python proposal; native successor #289 is in RC |
| #30 | Original Draft retained; Rust successor #290 is in RC, partial XHTTP evidence |

Do not push/merge main, publish packages/tags or update the marketplace without
new explicit owner authorization. Prepare independently reviewable work on dev;
only integrate green declared checkpoints into RC under applicable owner scope.
This handoff itself does not merge #295 or declare the candidate ready.

## Read before effects

Read `AGENTS.md` and its required documents, then:

- [RC ledger and owner-required gates](RC_090.md).
- [DNS authorization contract](../roadmap/DNS_AUTHORIZATION.md),
  [TUN authority](DNS_TUN_AUTHORITY.md), [retention](DNS_FDSTORE.md).
- [Installed evidence and exact ARM64 composition](../testing/DNS_BROKER_TRY_OMARCHY_2026-09-25.md).
- [Host candidate/enrollment contract](../../tests/dns_broker_host/README.md),
  [experimental package](../../tests/dns_broker_host/package/README.md),
  [pinned core patches](../../tests/core_dns_adapter/README.md).
- [Host authorization](../testing/HOST_AUTHORIZATION_ACCEPTANCE.md) and
  [owned-child cleanup](../testing/R5_OWNED_HELPER_CLEANUP.md#native-ownership-contract).

## Implemented and evidenced — do not rebuild the design from zero

Rust owns lifecycle. The optional broker composes fixed root admission, one
enrolled UID/socket ACL, kernel-validated transferred TUN descriptor, typed
resolved writes/readback, durable journal, systemd FD retention and fail-closed
cleanup. The patched core transfers its actual TUN FD and waits for broker Ready.
Legacy DNS remains the default. No broad polkit grant or arbitrary command API.

The experimental package is separate: it neither replaces stock Mihomo nor
auto-enrolls, starts or enables the broker. Root package installation still needs
authorization. Enrolled same-UID applications share the narrowly constrained
capability; do not claim executable-only identity authentication.

ARM64 installed evidence includes:

- Full VPN, one owned core/TUN, private controller, real resolved readback,
  TUN-bound HTTPS, normal release, and owner-confirmed **no separate DNS/route
  dialogs** on Connect/Disconnect after explicit installation.
- Cross-UID socket denial and rejection of a non-TUN descriptor.
- Root broker SIGKILL retains original TUN/journal/systemd FD. Actual restart
  and ALPM upgrade refuse retained state without erasing it. Coordinated reboot
  clears the old epoch. This is quarantine, not a kill switch.
- Original stock-runtime/profile/Routing restoration eventually passed real DNS
  and HTTPS on unchanged code. Two earlier Connect failures remain unexplained;
  do not blame password timing or claim a proven timeout defect.

Important correction: the core-crash runner counted a zombie as a live core.
`WNOWAIT` deliberately pins the dead leader's PID/PGID until explicit cleanup.
Fresh observation reported no running core and no TUN. Do **not** reap during
observation or weaken process ownership. The stopped test is not retroactively
PASS; repeat with the corrected evidence described below.

CI at `59a6610a30824d4183deec936e0cdbbc04d22662`: test, package and package-arm64
passed. Its non-Rust suite reported 451 tests (449 passed, two expected skips)
plus green QML/JS contracts. Affected broker/resolved suites: 47 + 35 passed;
separate real kernel/private-bus composition: ten scenarios passed. The final
docs-only handoff has its own checks; do not relabel earlier runtime results
as tests of new implementation. Run affected checks after any code change.

## PC execution plan

1. Fetch and inspect current branches, PRs, dirty work and installed identities.
   Work inside the disposable Omarchy VM, not the outer PC's network namespace.
   Record architecture, toolchain, systemd/resolved and installed package/core
   identity. Preserve private state and the exact usable rollback package. A VM
   snapshot is useful recovery insurance, not proof that DNS cleanup passed.
2. Adapt the installed-owner acceptance harness first. The old
   `native_service_acceptance.py` isolated service uses `OMAVLESS_HOME` and a
   noninstalled executable; current login admission correctly refuses it.
   Reuse its pure evidence helpers, not that invalid launch path. Do not weaken
   activation receipts/login checks. One-off ARM scripts are not a supported
   cross-machine runner; keep a reviewed portable scenario in Git with synthetic
   regression tests and private local case inputs.
3. Build native x86_64 runtime/broker and the exact reviewed core patches from
   their pinned upstreams. `mihomo-dns-broker.patch` already includes DNS-off:
   do not apply both alternative Mihomo patches. Patch the locked sing-tun copy
   and use a disposable local module replacement. Use the required production
   `with_gvisor` tags, locked dependencies and independently recorded digests.
   Stage with `tests/dns_broker_host/package/stage.py --arch x86_64` per its
   README; inspect archive, source receipt, unit, capabilities and ALPM hook.
   ARM binary hashes are evidence, not expected hashes for rebuilt x86 binaries.
4. Run `./tests/run.sh`, `./tests/run-rust.sh`, affected broker/package/unit
   tests and the documented isolated namespace/core probes. Run heavy suites
   sequentially on a small VM. Preserve assertions; do not call resource-loaded
   failures PASS. Then perform explicit installation/enrollment and verify
   normal positive Full VPN/Disconnect before destructive cases.
5. Finish the outstanding installed matrix below, one case at a time. A fully
   controllable VM allows coordinated reboot/crash testing, not forged human
   `ready`/`settled` or authorization bypass. Follow existing barriers; if a real
   prompt requires the owner, stop that effect and continue independent tests.
6. Finish reviewed distribution/update/enrollment/revocation/recovery integration.
   The opt-in package alone is not a normal-user shipped #270 solution. Resolve
   how the reviewed patched-core dependency is supplied and maintained; do not
   silently ship an unreviewed fork or make broker mode default prematurely.
7. Reconcile #270/#132, #295 and the RC ledger using exact evidence. Report
   blockers honestly, keep private outputs local, push useful dev checkpoints.
   Propose RC inclusion only after its declared gates; main stays frozen.

## Outstanding installed matrix

| Case | Required evidence / current status |
| --- | --- |
| Mode sequence | Full VPN → Routing → Direct → Full VPN, actual active mode and DNS ownership each step; NOT RUN on final installed pair |
| Core SIGKILL | Broker release, no live owned core/TUN, fresh UI/runtime truth; distinguish stat `Z` leader with exact parent/group from live residual children; separately authorized Disconnect must reap and clean; corrected full gate NOT RUN |
| Package removal while active | Actual ALPM refusal preserves binary/journal/retained object; upgrade refusal already passed but does not prove removal; NOT RUN |
| Clean removal | Verified release + empty journal/FD store + root service inactive before ordinary package removal; no force flags or cleanup hook deleting unknown state; NOT RUN |
| Failure/negative paths | Applicable partial DNS write, owner drift, channel loss and cancellation/recovery cases from the contract; fake/private-bus evidence does not replace actual host evidence |
| #132 | No false connected/mode success after rejected/failed authorization. Do not ask for wrong passwords or repeat failures into PAM lockout. Legacy path remains a separate issue while selectable |
| Root crash/reboot | ARM64 PASS under exact composition; run applicable x86 composition checks, with no blind deletion of retained FDs/journal |
| Final restoration | Exact original package/config/profile/mode and real DNS/HTTPS; fresh no-manual-recovery observation, no duplicate core/TUN or TCP controller |

Unknown old writes require quarantine and the coordinated epoch-boundary
recovery. Never use FD-store cleanup, unit unloading, journal deletion or
enrollment replacement to force a green result.

## Departure state belongs to the ARM VM only

After the successful restoration above, a later gate installed the experimental
runtime/configuration again, then stopped at `human_authorization_unsettled`
**before starting the runtime**. At departure: user runtime inactive/disabled,
MainPID zero; no TUN; root broker active with FDstore zero; experimental core
override remains. No automated restoration followed the blocked barrier.
Private backups and rollback packages are preserved locally outside Git.

Do not assume the PC VM has these packages, paths, private records or host state.
Do not import ARM host enrollment/templates or run an old restoration script
there. Obtain fixture availability through sanitized local inventory only.

## What to return

- Exact source/package/core hashes, branch and PR links, CI/local check counts.
- Installed gate matrix with PASS/FAIL/NOT RUN and safe classifications, including
  prompt behavior and actual DNS readback, not just empty process counts.
- #270/#132 disposition; remaining distribution and recovery decisions.
- Final runtime/mode/connection, core/TUN/FD counts, original-state restoration.
- Proposed RC integration and release blockers. Keep #30 partial: unavailable
  Trojan/Hysteria2/TUIC/PQ or restricted-network fixtures must not be invented.
- Confirm main/marketplace unchanged and secrets absent from Git/output.
