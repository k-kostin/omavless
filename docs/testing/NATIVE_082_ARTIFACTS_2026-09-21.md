# 0.8.2 native artifact checkpoint — September 21, 2026

The owner authorized merging #263 and a new GitHub prerelease, preserving the
published 0.8.0 and 0.8.1 tags/assets. Marketplace submission remains separately
withheld. #263 merged at `2e8721a4c594ac2f0a1a9a2a0a4298fe1eef05a4`.
It fixes Full Quit's strict installation query and two reviewed UI findings:
subscription keyboard focus and accurate EN/RU name-search copy.

## Exact native builds

Both packages were built from
`22e23e64c49b8110088b6be3f063b5e641ba0853` by
[Native packages CI](https://github.com/k-kostin/omavless/actions/runs/35619601060).
Both jobs passed on native runners in isolated Arch build environments using
locked Rust 1.98.0. Package payload/identity inspection and executable `--help`
loader checks passed on each architecture. Original build logs, toolchain and
package identities remain in the CI artifacts.

| Artifact | SHA-256 |
| --- | --- |
| `omavless-0.8.2-1-x86_64.pkg.tar.zst` | `381a1427b2728f35bfd49247e4373238deb4ef53c5850eec2ad68e91de382b6c` |
| `omavless-0.8.2-1-aarch64.pkg.tar.zst` | `ef53c795a62825b5bd1b928663c751b2d66f5c2da711fd5dc4542e367b7bbf57` |
| x86_64 executable | `3ff795316aa331463aff4e26571546433bdfd9317f348712d135be2f0a8d28a1` |
| ARM64 executable | `62874def77df9e4f099baf74c57420a7ab1dab7b83935676828f6d445a5f357f` |

Downloaded CI artifact sets passed their checksums. The physical x86_64 PC
also passed the strict package inspector and extracted executable loader check,
without installing the package or changing its running daemon. Final frontend
pins belong to a descendant commit; `frontend-pair.json` must record that exact
identity and protected runtime-input equality. The generated same-source CI
frontend is not the release frontend because its bootstrap pins were empty.

## Verification and boundaries

- The setup helper's exact accepted version, download cleanup names, setup
  fixture and release-version assertions now expect 0.8.2. Historical-version
  rejection/ordering fixtures remain unchanged.
- Local `tests/run.sh`, `tests/run-rust.sh`, compile-only QML load, QML lint,
  manifest/Omarchy plugin validation and documentation navigation passed.
  Release tests exposed old version assertions and the setup helper's old
  version guard; the corrected run passed. Initial build-source Test CI is not
  the final PR validation.
- Full Quit's attended disconnected-PC result and connected UI-only review
  belong to their exact development heads in the
  [#263 follow-up](R5_NATIVE_FULL_QUIT.md#september-21-release-follow-up-263).
  Rebuilding those sources with a new version is not a new installed acceptance.
- Fresh guided provisioning remains the stable-promotion gate. AUTO-1,
  DNS/provider findings, V0 fixtures and #135 keep their existing scope.
- Release preparation does not install packages, stop/restart the runtime,
  change routing or touch private profiles/other VPNs. No private screenshots,
  profiles or logs are included in publication assets.

Public tag/frontend identity and anonymous download verification are recorded
after publication; use GitHub's actual release state in the meantime. This is
a prerelease candidate, not stable or marketplace acceptance.
