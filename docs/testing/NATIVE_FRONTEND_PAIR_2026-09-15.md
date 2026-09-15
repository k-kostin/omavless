# Native frontend/package pairing — September 15 VM preparation

This is offline release preparation on Try Omarchy ARM64, not a package
installation, published release, clean provisioning result or marketplace update.
The installed frontend/runtime and private store were not changed by this work.

## Exact sources

- Refreshed main: `56800b0a1f05eb83f8a761a20772153883c914f1`.
- Accepted runtime package source: `b7fd0a99b8b169f0933e5f43ea4389642015193a`.
- Pairing implementation: `789238f82869531a680fc5e5241b66eb7f8e97bc`.
- #249 guided setup: `4e2f17f88b230e4d43e05b738c6cf6f4c6caea43`.
- #250 copy/visual fixtures: `b3a3f0178c83475ee592d2b5da5d87bda47cad7f`.
- Local combined candidate: `67491a71765cfb88c715e3aa688dd226fc3139be`.

The scratch candidate was based on main and cherry-picked #249's `2aa5ba0`,
`4e2f17f`, then #250's `b3a3f01`, then pairing `789238f`, without conflicts.
It is not a permanent PR stack and was not pushed as a release branch. Reproduce
from the published constituent PR commits; commit timestamps may yield a
different scratch SHA. A later evidence-only commit does not alter these tested
implementation heads or claim its SHA was inside the accepted native ELF.

## Offline artifact results

The real previously accepted `omavless-0.8.0-1-aarch64.pkg.tar.zst` was inspected
and copied unchanged. No synthetic ELF was used for this gate.

| Identity | SHA-256 |
| --- | --- |
| Original and paired ARM package | `454662a76f106af5b2f4ee8ab3ef4626a1641981fd9e6baa0a2ca8d91dc7983d` |
| Embedded accepted ARM ELF | `12afa0a6ae279d23f1b89426d47fdd478e0ef1987f171a92924b9c925f6f0458` |
| Pairing-only frontend at `789238f` | `d7d8cae2da49687e8a0d18a658c74afe66a40e21fee4069ca330e31908ea685c` |
| Combined frontend at `67491a7` | `5ed018ee6fad7d56537db54c189fc482a38d4384c2ea81f000e6bbea577287e1` |
| Combined `frontend-pair.json` | `a23f37a52879edd631eebb4514b277d55302ac10319181de4dcef6b37ddbafb3` |

Both runs passed strict archive inspection, clean-head checks, ancestor/source
input-tree equality, original-package checksum preservation and output
`SHA256SUMS` verification. The frontend allowlist excludes Python, tests, skills
and crate sources. The combined archive contains the guided-setup frontend,
but its real release pins remain **empty**. The pairing-only branch records pins
as **absent**. Neither is public guided-install readiness.

Identity records retain both source commits and explicitly report
`publication: unpublished-candidate` and `publishedDownloadVerified: false`.
Do not distribute the two different frontend archives under the same final
release asset name: choose the final approved candidate before publication.
The preserved original build/installed ARM evidence remains in
[the final candidate report](NATIVE_080_FINAL_CANDIDATE_2026-09-14.md).

## Deterministic checks

- Pairing branch: **249 Python tests, 247 PASS / 2 existing SKIP**.
- Combined scratch: **275 Python tests, 273 PASS / 2 existing SKIP**.
- New pairing suite: **14 tests**, including unchanged inputs, runtime/template/
  license/unit/build changes, new files, deletions and mode changes; ancestry;
  absent/empty/malformed/duplicate/mismatched pins; bounded copying; preserved
  package bytes and dual source identity; dirty/occupied/symlink output refusal.
- Existing JS suites and QML contracts: PASS in both checkouts.
- Screenshot fixtures' production parsers, disconnected-only state and nine
  refused mutation/export commands: PASS in the combined checkout.
- Python compile, affected shell syntax, manifest JSON, plugin validation and
  `git diff --check`: PASS.

The two Python skips remain installed-Mihomo opt-in and root-specific execution;
no new test was skipped. Rust/package payload inputs did not change, so this
developer-only checkpoint did not repeat the full live R6/VPN matrix.

## Visual and publication boundary

#250 contains three actual QML captures using synthetic, credential-free data:
main, expanded subscription and Settings. The state is honestly disconnected;
the renderer was isolated from the user's home, runtime sockets and network.
See that PR's `docs/marketing/MARKETPLACE_080.md` for exact screenshot provenance
and reproduction. These are native panel crops, not a fabricated connected
hero or a claim of clean guided installation. Root `preview.png` is unchanged.

Next owner/PC work:

1. Fetch and reconcile these independent PRs before choosing the final frontend.
2. Complete actual x86_64 package/build/installed gates without relabelling ARM
   evidence or rebuilding the accepted ARM package for docs-only changes.
3. Publish only owner-approved immutable architecture assets, commit matching
   source/hash pins, then pair the selected frontend against those exact bytes.
4. Run clean real download → install → activation → usable frontend acceptance;
   separately cover postponed setup and already-installed/unactivated recovery.
5. Classify standard installation with marketplace maintainers using that
   observed flow; choose the final safe hero and obtain separate owner-present
   marketplace approval.

No tag, asset upload, main merge, marketplace change, package installation,
service restart, VPN transition or privileged runtime path was introduced here.
V0/#30, AUTO-1 and recorded DNS/provider limitations remain separate and open.

## Consolidated PC handoff and instruction audit

Use [the September 15 PC prompt](PC_080_ACCEPTANCE_PROMPT_2026-09-15.md), not the
older shareable September 14 prompt alone. It identifies the independent PRs,
latest screenshot revision, retained A/B source evidence, actual x86_64 work,
unpublished-asset blocker and separate per-architecture guided-install matrix.
It grants no release/marketplace/merge authority and explicitly prevents another
ceremonial R6 acceptance cycle.

The instruction audit checked #249's `setup-runtime.sh`, `SetupPage.qml`,
`RequiredComponents.qml`, onboarding handlers and the release assemblers:

| User path | Documentation conclusion |
| --- | --- |
| First installation | Distinguish manual candidate path from gated future public provisioning; component presence is not TUN/Internet readiness. |
| App installed, not activated | Complete setup, not blind reinstall; existing data and migration preconditions preserved. |
| Native update / frontend-only update | No repeated initialize/activate; package changes require disconnected update; compatible frontend-only changes do not replace the runtime. |
| Set up later / Finish later | Bootstrap closes without dismissing component reminders; final wizard action records completion without requiring a profile. Neither connects a VPN. |
| Cancel / retry | Initial consent is before effects; later failures may leave completed steps. No automatic rollback, auto-ack, stale-lock removal or unresolved-auth retry. |
| Reopen after Quit | First-run helper deliberately does not re-enable an already activated owner; use explicit documented reopen. |
| Artifact identity | Single-source candidate record or explicit dual-source pairing; never version-only compatibility or a new SHA attributed to an old ELF. |
| Stable metadata | Schema 3 is stable 0.8.0; schema 2 wording is explicitly historical RC. |

The user-guide corrections are docs-only in #249 at
`329b77a660a46ef142eee5a907f8824cfb63fc17`; its tested production tree is identical
to `4e2f17f`. This follow-up likewise changes no pairing implementation or
installed code. Local relative-link/diff checks are sufficient for these
documentation changes; no new package/GUI/authorization/live acceptance is claimed.
