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

No K1 enforcement, S1 host-manager cutover, T4 encrypted-backup integration,
P4 protocol activation, G1 desktop app or T3 connection-closing mutation is
selected. #30 remains Draft with its existing exact-head XHTTP evidence and
unavailable protocol fixtures. Beta assembly is not release acceptance.
