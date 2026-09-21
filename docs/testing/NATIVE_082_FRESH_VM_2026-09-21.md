# 0.8.2 fresh guided installation — x86_64 VM

## Identity and scope

Tested September 21, 2026 on a newly installed Omarchy 4.0.4 x86_64 guest
under QEMU/KVM: 8 GiB RAM, four logical CPUs (two cores, two threads each),
40 GiB growing qcow2 disk, UEFI and the default encrypted guest installation.
The clean powered-off OS baseline is retained separately from the working
overlay. No physical disk or host home is shared; only guest SSH is forwarded
to host loopback. Host VPNs, installed OmaVLESS and network state were unchanged.

Frontend: unmodified upstream main
`f0e54488cb3e7e558d52aa0f18d3a5c17d355860`, installed by
`omarchy plugin add https://github.com/k-kostin/omavless --enable --yes`.
The noninteractive SSH invocation needed the guest desktop's `OMARCHY_PATH`
for enablement; no checkout substitution or product patch was used.
The installed checkout was clean. Its product code/pins are unchanged from
release frontend `f442714362620c18e1bbaa6415d9e0c2e08c0a8a`;
the intervening changes are documentation only.

Native package: public `omavless-0.8.2-1-x86_64.pkg.tar.zst`, runtime source
`22e23e64c49b8110088b6be3f063b5e641ba0853`. Installed executable SHA-256:
`3ff795316aa331463aff4e26571546433bdfd9317f348712d135be2f0a8d28a1`.
This matches the [immutable artifact record](NATIVE_082_ARTIFACTS_2026-09-21.md).

## Actual first-run path: PASS

- Initial state had neither OmaVLESS nor Mihomo, no private profiles and no
  legacy/native ownership. The rendered panel accurately showed both missing
  components. No installer started merely from enabling/opening the plugin.
- **Set up later** closed the panel. Clicking the bar icon reopened the same
  truthful missing-component state.
- Clicking **Install required components** opened the actual graphical
  terminal and disabled the install action while it was running. The owner
  completed the explicit installer consent and OS authorization and confirmed
  that the terminal/prompts had finished. No password was handled by the agent.
- The normal missing-core path installed `mihomo-bin 1.19.31-1` through
  Omarchy's AUR/yay route, then the pinned public OmaVLESS archive through
  normal `pacman -U`. Package receipts confirm both installations.
- Without manual activation or marker editing, component discovery returned
  `ready` / `present`, launcher target returned `rust`, and
  `omavless-runtime.service` was active and enabled.
- Fresh runtime observation reported disconnected, recovery false and zero
  visible/owned Mihomo, auxiliary and TUN counts. Profiles/subscriptions were
  empty; startup was configured **Off**. Private config/state directories
  had mode 0700. No VPN autoconnection occurred.
- All three onboarding steps rendered and responded to clicks. Core readiness
  truthfully reported installed Mihomo and missing file network capabilities;
  missing optional file-picker guidance was also visible. Routing/profile
  import were skipped, and **Finish later** persisted completion.
- Settled reopen showed the normal empty, disconnected panel without reopening
  the wizard. Settings navigation worked. Shell IPC remained responsive and
  the inspected journal tail had no QML type/reference/binding errors.

Immediately after completion, a refresh briefly rendered **State unverified**;
the subsequent fresh observation and settled UI reported **Disconnected**.
This was not a persistent recovery state or a successful-connect claim.

## Independent checks

- `tests/run.sh`: 276 Python tests (274 passed, two opt-in skips), JS/QML
  contracts and documentation navigation passed.
- Manifest JSON, installed Omarchy plugin validation, all plugin QML files
  linted with installed Omarchy imports, tracked shell syntax and whitespace
  checks passed. No tracked symlinks were present.
- The first Rust suite failed the two-second owned-child graceful-stop test
  with `StopFailed`. The isolated test and a subsequent complete unchanged
  `tests/run-rust.sh` both passed (979 tests passed, 11 opt-in tests ignored
  across 72 suites), including formatting, Clippy and parity.
  Retain this intermittent test observation; no production timing was relaxed
  and the first failure is not represented as a pass.
- Official marketplace scanner at
  `e3c924fa7f83b51673fba5ee688ec97288f01891`, against the exact tested frontend:
  complete baseline, no findings, **review-required** for installer,
  package-manager, service-management and privilege capabilities. This local
  run is preparation, not a bot-authored marketplace verification or audit.

## Boundaries and publication disposition

This closes the outstanding **x86_64 fresh guided download → absent-core
provisioning → package install → activation → onboarding** gate for the 0.8.2
product inputs. No new application build or release-asset replacement is needed
for this documentation-only evidence update.

This pass did not grant TUN capabilities, import private profiles, connect a
VPN, test enabled login autoconnect or simulate interrupted privileged package
effects. It is not a new ARM64 0.8.2 fresh-install test. Historical ARM64 setup,
accepted runtime/PC checks and deferred AUTO-1/DNS/V0 evidence retain their own
exact identities and scope. Guest NAT still uses the host's upstream network;
it is not an independent ISP connectivity test.

The GitHub release remains a prerelease until separately promoted. Marketplace
submission remains withheld by the owner. Before an authorized update request,
resolve current upstream HEAD, rerun exact-commit baseline/compatibility checks
and use the existing-plugin **Verify and publish a newer upstream commit** form,
not a duplicate new-plugin submission. The maintainer must review the reported
capabilities before approving that exact snapshot.
