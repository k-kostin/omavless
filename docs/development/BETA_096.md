# 0.9.6 development assembly

Owner-selected scope, 2026-10-01. Base: accepted `rc/0.9.5` at
`4bb142b964eec9e747a1f41ad2c92e0bb9b03cf6`. This temporary `beta/0.9.6`
does not replace that RC, stable main, immutable assets or Marketplace.

## Selected changes

| Source PR | Exact source | Scope |
| --- | --- | --- |
| #403 | `4457c9b6a2f6f66f1169eac7e9ba9241d8a863f9` | Volatile five-minute TUI traffic trend |
| #408 | `8cad5d0ded1a65eb04f89f02fe687a3eee1bac35` | Explicit partial session-history notice |
| #413 | `4ec5ec5e01cd46e7c6f73788c38f6df0b2673423` | Bounded log-collection end state, preserving hint visibility |
| #429 | `5ec930c643d6f8d7fc6d0bef853eb5db04a6ca99` | Age of an explicit route-check result, not status-poll freshness |
| #409 | `ff17a594a8a708ad47c50e45b50bade03be481ed` | Distinct unavailable/empty/missing/search-empty profile states |
| #411 | `c70e64550cda6ec0b917f86100b53f14269dd5a7` | Explain search-forced subscription expansion; preserve saved choice |
| #420 | `643a477160fe3208c30f03b099a43da1d2dade2f` | Row-bounded wrapping tooltip, pointer and keyboard focus |

Eight owning commits are replayed rather than merging obsolete stacks. The
route-age conflict retains both provider-usage invalidation from 0.9.5 and
route-result timestamp invalidation. The QML test conflict retains both
independent test groups. The accepted strict snapshot/log-hint parser fix and
managed-DNS/lifecycle boundaries remain untouched.

## Gates and limits

Combined deterministic suites, EN/RU actual QML/TUI rendering, narrow/small
views, focus and search/expansion behavior are required. Installed ARM64
read-only checks must distinguish real runtime observations from isolated
synthetic UI states. Captures and private data stay outside Git. A read-only
selection does not justify replaying unrelated network/crash acceptance.

The honest source version is `0.9.6-beta.1` (`0.9.6beta1-1` in Arch). Package
pins are empty: no older RC package is relabeled, and public guided provisioning
is deliberately unavailable until separately authorized immutable assets exist.
Local app/DNS/frontend packages must have matching exact version and provenance.

### Completed assembly checks

The selected integration is [PR #430](https://github.com/k-kostin/omavless/pull/430),
targeting **beta/0.9.6 only**. Range-diff preserves the selected patches, with
only the conflict resolutions described above. Runtime, DNS-broker and strict
native-snapshot parser sources are unchanged from accepted RC 0.9.5.

- Local developer suite: 499 Python tests, two expected skips, all Node/QML
  contracts; native-main-panel contracts include 43 tests. Full Rust recheck:
  1,385 successful test executions, 12 ignored, plus format/Clippy and parity.
  The independent PTY suite passes 11 tests. An initial existing helper-drain
  timing failure passed on an unchanged full rerun; no assertion was relaxed.
- Shell/JSON/diff checks, plugin validation and Qt6 qmllint pass. The installed
  Mihomo opt-in validation test passes; core identity is Meta 1.10.0/linux arm64.
  CI at `34d31bee1e26bb1c5135d37db910cab2c2ba25b0` passes tests and both
  architecture app/DNS package jobs.
- Actual isolated production QML and TUI rendering inspected in EN/RU, with
  narrow panels/terminals and scrolling. Empty/unavailable/missing/search states,
  forced expansion without persisting it, keyboard/pointer tooltip bounds,
  partial activity, collection-ended wording and explicit route-result age pass.
  These are synthetic presentation checks, not live connection evidence.
- Installed ARM64 developer pair: app/DNS package source is the CI head above;
  frontend source is `dd3545115485ef1127992166be83392c33be2447` (subsequent
  harness-only corrections; protected package inputs are equivalent). Both
  packages are `0.9.6beta1-1`; installed binaries and 30 frontend files match.
  Real plugin search/keyboard tooltip and real Russian TUI navigation/close
  pass. Fourteen bounded read methods pass, private store stays byte-identical,
  and TUI close does not stop the sole runtime owner.

Final VM state is disconnected/Routing, plugin enabled, one user runtime and
one system DNS broker, zero core/TUN. No network transition was needed for this
read-side selection. Captures and local packages remain outside Git; private
installed screenshots are not publication material. x86_64 package CI passes,
but installed x86_64 beta acceptance and public provisioning are **not claimed**.

No K1 enforcement, S1 host-manager cutover, T4 encrypted-backup integration,
P4 protocol activation, G1 desktop app or T3 connection-closing mutation is
selected. #30 remains Draft with its existing exact-head XHTTP evidence and
unavailable protocol fixtures. Beta assembly is not release acceptance.
