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
