# 0.8.1 native artifact checkpoint — September 21, 2026

The owner selected 0.8.1 rather than replacing the immutable public 0.8.0
prerelease. #261 integrates #249 setup and #257 PC evidence with #258–260,
whose runtime corrections are already on main. Marketplace submission remains
explicitly withheld until another owner instruction.

## Exact builds

Both architectures were built from
`98e0b275ae43d6e7d7901b58fab31457b542f878`, not a synthetic PR merge commit.
[Native package CI](https://github.com/k-kostin/omavless/actions/runs/35597702333)
and [Test CI](https://github.com/k-kostin/omavless/actions/runs/35597702207)
passed. Locked Rust 1.98.0 builds ran on native x86_64 and ARM64 runners in
isolated Arch environments. The normal strict packager and native archive
inspector passed, and each resulting executable ran its fixed `--help` command.
Build logs, compiler/library versions, package payload lists, ELF headers and
checksums are retained with the CI artifacts.

| Artifact | SHA-256 |
| --- | --- |
| `omavless-0.8.1-1-x86_64.pkg.tar.zst` | `d4bc14db7f5bd415c14b31664f3125c8cc6f52b7bf660aa189528dae7172e15f` |
| `omavless-0.8.1-1-aarch64.pkg.tar.zst` | `e0ea1ba20636cb802511ecbe5fa36beeb5bcac00d9ed8f4f0dfdcdbeabc6ef7b` |
| x86_64 executable | `ffaa5ee7e49e3cfbd4a4ced4417a556832805b421096080c093862e1039d395a` |
| ARM64 executable | `27540e0acd11bbb508bbb0bd5a08bd0040e9edd46318964f79f7a05aa3cb18cb` |

The x86_64 archive additionally passed the local strict inspector and extracted
CLI loader smoke on the physical Omarchy PC, without package installation,
runtime restart, authorization or VPN transition. Both downloaded CI artifact
sets passed their checksums. Frontend setup pins are added in a descendant
commit; the release's `frontend-pair.json` records that frontend identity and
unchanged protected runtime/build/package inputs. The common QML archive serves
both architectures; runtime build provenance stays architecture-specific.

## Public prerelease verification

[#261](https://github.com/k-kostin/omavless/pull/261) merged at
`20b5e6c4f3ef466207d37306d4a70fe5876162f6`; its tree is identical to the
fully green final PR head `26ea8c6d4ad0c61dbec7d429c70a585fb671de49`.
Main CI passed. The [v0.8.1 prerelease](https://github.com/k-kostin/omavless/releases/tag/v0.8.1)
points to that merge and is explicitly **not latest/stable**. The old v0.8.0
tag still points to `b7fd0a99b8b169f0933e5f43ea4389642015193a`.

The final common frontend SHA-256 is
`4d47554fbc257801f69834f7c64fcdc0ad1bd70ff5d29df07a16165ea63de447`.
Its offline pairing reports `runtimeInputsEquivalent: true` and
`bootstrapPins: matched`. All six public assets were fetched anonymously using
bounded HTTPS-only redirects. The downloaded `SHA256SUMS` matched the retained
local file, and all five covered artifacts passed verification. The pairing
JSON preserves its earlier assembly-time `publishedDownloadVerified: false`;
this subsequent download check does not rewrite that provenance record.

The public packages were not installed on the active PC by this release pass.
Its read-only observation still reported Connected/Rule without manual recovery.

## Evidence boundaries

- Local deterministic gates: Rust 978 passed / 11 ignored, 72 suites;
  format, Clippy and parity passed. Isolated shell/Python/JS/QML checks,
  manifest validation, QML lint and documentation navigation passed.
- Installed PC behavior is the accepted development candidate recorded in the
  [PC continuation](PC_080_PREINSTALL_2026-09-21.md#corrected-installed-candidate--september-21).
  A version bump and new compiler output are not a new installed-package result.
- Native ARM64 CI is package/loader evidence, not a Try Omarchy GUI/TUN session.
- Fresh guided provisioning and applicable final installed-artifact checks
  remain the stable-promotion gate. A GitHub prerelease may distribute these
  candidates for that purpose, without claiming a stable marketplace update.
- AUTO-1 enabled-login, DNS/provider findings, V0 fixtures and #135 remain
  separately scoped; none becomes PASS from this packaging work.

The running PC installation and private data were not modified by this release
preparation. Public assets contain only committed frontend files, inspected
native packages and credential-free build/provenance material.
