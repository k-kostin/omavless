# 0.8.2 native artifact checkpoint — September 21, 2026

Later evidence: [fresh x86_64 guided installation](NATIVE_082_FRESH_VM_2026-09-21.md)
passed against these immutable package bytes and unchanged product inputs.
Pending-gate statements below describe artifact-publication time. Stable
promotion and marketplace publication remain separate owner decisions.

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

## Public prerelease verification

[#264](https://github.com/k-kostin/omavless/pull/264) merged at
`f442714362620c18e1bbaa6415d9e0c2e08c0a8a`; its tree is identical to final
PR head `4f99aac42da80f39bfe6d059a9dec77060702acd`. The final
[Test CI](https://github.com/k-kostin/omavless/actions/runs/35620145769) and
[both package jobs](https://github.com/k-kostin/omavless/actions/runs/35620145781)
passed. The published packages retain the original build-source identity above;
the later CI archives are not substituted for the reviewed pinned bytes.

The [v0.8.2 prerelease](https://github.com/k-kostin/omavless/releases/tag/v0.8.2)
points to that merge and is explicitly not latest/stable. Offline pairing for
that exact frontend reports `runtimeInputsEquivalent: true` and
`bootstrapPins: matched`. Its common frontend SHA-256 is
`0d17c13cfa71d3e27ef95991d1ed017de5e7601d5b6b98c85deeab424d8caf53`.

All six public assets were fetched anonymously with bounded HTTPS-only redirects.
Downloaded `SHA256SUMS` matched the retained local record and all five covered
assets passed verification. The pairing JSON retains its assembly-time
`publishedDownloadVerified: false`; this subsequent verification does not
rewrite immutable provenance. Architecture-specific original build evidence
ships separately as `native-build-evidence.tar.xz`.

The earlier `v0.8.0` and `v0.8.1` tags still identify
`b7fd0a99b8b169f0933e5f43ea4389642015193a` and
`20b5e6c4f3ef466207d37306d4a70fe5876162f6`. Neither was moved or overwritten.
The running PC daemon was not restarted or replaced. This is a prerelease
candidate, not stable or marketplace acceptance.

## September 22: installed ARM64 update after the paused clean test

The owner requested updating the existing Try Omarchy ARM64 VM before preparing
the marketplace request. Its September 16 clean-install test had retained the
original private data but paused before restoration. Read-only inspection found
the same boot, stopped native/login units, no core/TUN and intact private backups.
This was an existing native installation, not a reason to initialize/activate
again or repeat a destructive fresh-install test.

- Downloaded the actual public ARM64 package, common frontend and pairing record
  anonymously over bounded HTTPS; all hashes matched the published records above.
  The strict package inspector verified architecture, exact payload, unit/ELF
  digests and runtime source `22e23e64c49b8110088b6be3f063b5e641ba0853`.
  The shared public pairing record describes its x86 assembly; ARM identity was
  independently checked against the inspected ARM package, not inferred from it.
- In one attended terminal, each effect used the established human
  `ready`/`settled` barrier. The complete original private state was restored and
  byte-compared while stopped, with both original and temporary test data
  retained outside Git. No marker, profile or login receipt was hand-edited.
- Normal `pacman -U` updated OmaVLESS 0.8.0-1 to **0.8.2-1**. Existing
  `mihomo-bin 1.19.31-1` was retained; the owner's pre-test network-capability
  policy was explicitly restored to that inspected packaged core with normal
  sudo confirmation. This was an attended administrator step, not a new
  automatic privilege feature of the plugin or installer.
- Runtime start verified the actual running ARM ELF
  `62874def77df9e4f099baf74c57420a7ab1dab7b83935676828f6d445a5f357f`,
  original profile-store byte equality, startup Off, observed disconnected,
  no recovery and one native owner / zero cores / zero TUNs.
- The native-only frontend installer used main
  `6b1baa13aa8a9d3f32dfa50fdb91bdce84f7fb65`. All 29 checked runtime-relevant
  frontend/template files matched. Plugin validation and enablement passed.
  Old loaded QML initially could not parse the fresh observation; a supported
  shell-only restart restored metadata/observation availability and Disconnected.
  The native daemon PID remained unchanged. A rescan/file match alone was not
  counted as loaded-frontend acceptance.
- Later read-only inspection observed Connected/Rule with matching desired
  profile/controller, one owned core, one managed TUN, zero auxiliary cores and
  no recovery. The updater did not initiate Connect; the now-active session was
  left untouched. A bounded public HTTPS request returned 200. This is ordinary
  Routing connectivity, not proof that every destination traverses proxy egress
  or that earlier DNS/provider findings are resolved.
- Native control and Mihomo Unix sockets were 0600 inside a 0700 user directory.
  The generated config had a Unix controller and no TCP-controller setting;
  no attributable Mihomo TCP listener was observed. No private profile names,
  addresses, URLs or raw logs are included here.

This adds actual **0.8.2 ARM64 package-update** evidence. It does not relabel
the earlier ARM clean-install test as a fresh 0.8.2 pass, replace the independent
x86_64 clean setup report, close AUTO-1/V0 or authorize marketplace publication.
