# 0.9.8 development selection

Owner-selected on 2026-10-02. Base: `rc/0.9.7` at
`c4e800425243c1b02165f82153e4bf418fe465e6`, retaining the selected 0.9.6
read-side changes, honest Connections loading and manifest-derived Settings
version/credit. The historical [0.9.7 ledger](RC_097.md) keeps its own evidence;
unpublished source integration is not public release acceptance.

## Scope and ownership

The intended new product slice is authenticated private backup/restore, not all
of T4. Its active owning agent keeps its `dev/t4-*` branches; this assembly must
fetch and inspect the latest checkpoint **last**, record exact selected commits
and prove preservation, rather than merge an obsolete stack or concurrently
rewrite that work. Until reviewed integration is present, backup/restore is not
available and no UI, release prose or readiness label may suggest otherwise.

Admission requires more than internal encrypted-file primitives: a fixed
semantic owner/client path, safe passphrase/destination handling, explicit
authenticated preview and replacement confirmation, disconnected/drained owner
checks, durable multi-file outcome/restart handling and imported autoconnect Off.
Actual installed synthetic restore and interruption checks must pass before
claiming this user flow ready. Private real stores are not destructive fixtures.
K1, S1, P4, G1, connection closing and unrelated T4 scheduling/network-recovery
work are excluded. #30 implementation, Draft status and XHTTP evidence stay intact.

## Checkpoint and release boundary

Source version `0.9.8-beta.1` / Arch `0.9.8beta1-1`; both package pin maps are
empty. No old archive is relabeled. Baseline checks cover version/packaging
coherence, full developer/Rust suites and affected EN/RU actual QML/TUI review.
Late T4 selection requires the additional security/filesystem/runtime gates
above; baseline green is not sufficient for that selection.

Internal RC scope freeze may follow the declared risk-based checks without
publishing assets or repeating every predecessor's clean installation. Public
package/download/first-use and supported-host gates remain separate release
work. Main, accepted public 0.9.5 RC1 and Marketplace are unchanged.

## Baseline checkpoint evidence

Runtime/package source: `931809c48cf25956889963c0578437d074cee3fa`.
Reviewed frontend/test head: `8aef640312d3832465b18305d038e31d19387e29`.
The offline managed-DNS pair inspector proves identical protected runtime
inputs between them; both archives are newly labelled/built 0.9.8, not 0.9.6
archives renamed. Both pin maps stay empty and no public assets are published.

- Full developer suite: 501 Python tests, 2 expected skips; JS/QML contracts
  and documentation navigation pass. Focused package/setup tests: 50 pass.
- Full Rust workspace: 1385 pass, 12 ignored. Installed ARM64 managed Mihomo:
  2 renderer/Unix-controller integration tests and the loopback validation
  side-effect test pass. No real profile or host-network change is a fixture.
- Shell, JSON, compile/diff, plugin validate and Qt6 qmllint with installed
  Omarchy imports pass. All five initial CI jobs pass on the package source.
- Committed product QML in the isolated fixture: EN/RU Settings footer at
  widths 360/460 reviewed; correct version/credit and no Quit activation.
  The harness now stages its committed public manifest and has two regression
  tests. Synthetic rendering is not real connected/provider evidence.
- Agent-operated ARM64 upgrade from installed 0.9.6 beta to the inspected
  0.9.8 app/DNS/frontend pair passes the unchanged pretransaction guard and
  binary/27 frontend-file identity checks. Private store bytes unchanged;
  plugin enabled and IPC responds; Routing/Disconnected, zero core/TUN and
  no manual recovery. This is local developer integration, not a clean
  public first-use, physical-PC or enabled-login-autoconnect gate.

## Late T4 admission result

Latest reviewed owning checkpoint: [#493](https://github.com/k-kostin/omavless/pull/493)
at `2a75c2dfe1c5bcbbfa22dcf2107284fbae1393bb` (2026-10-02). Its exact-intent
successor executor is explicitly **inactive**. The stack still has no product
backup/restore caller, client/passphrase UX or approved ordinary-owner startup
admission; successor fences remain and installed whole-flow acceptance is not
claimed. Its security contract and PR expressly prohibit treating these green
internal primitives as product activation.

No T4 stack is admitted or rewritten by this assembly. The checked maintenance
base can live in `beta/0.9.8`; the intended backup/restore product scope and
promotion to a complete 0.9.8 RC remain pending that separate owning checkpoint.
Do not add fake working controls or call this beta a completed T4 release.
