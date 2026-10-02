# S1 mode-last ordering checkpoint

Development successor to [the fixed writer](S1_FIXED_TRANSACTION_WRITER.md),
2026-10-03. This is an explicit per-field journal/model and private installed-dconf
experiment, not installed activation. App proxy remains unavailable. No live host
constructor, readiness Boolean, helper, IPC/CLI verb, package or UI control is added.
The [exact-source private evidence](../testing/S1_MODE_LAST_PRIVATE_2026-10-03.md)
records the completed developer and private-dconf gates separately from those
unrun installed admission requirements.

## Bounded order and refusal

`ModeLastOriginalNone` accepts only a layered original whose effective desktop
mode is `none`, and an intended explicit user override of `manual`. All defaults,
locks and presence still retain the existing complete snapshot checks. An
original explicit `none` override is preserved exactly, not reset to absence.

Apply visits the 15 non-mode desktop fields and then the 10 fixed manager fields,
skipping unchanged fields; desktop mode is last. Every changed field must have
its own durable intent, same-origin settlement and complete independent readback
before the next one. Compensation reverses that order: the original inactive
mode is first if changed, before any endpoint/auth/PAC/bypass/manager restoration.
An unknown mode write never licenses another field while the port is undrained.
Mode ordering gates desktop consumers only; it cannot make inherited environment
changes atomic or stop applications that already hold a proxy configuration.

`Transaction::new` explicitly returns `ActivationNotAdmitted` for this order.
Neither caller-constructed snapshots, this journal, nor a `FixedHost` trait value
can provide missing installed listener/revision/ownership/lifetime admission.
The ordered real writer is a private test-only driver with its fixture-created
bus, service and database; it is not a second production executor.

## Prior Manual/PAC comparison

The original v2 planner applies enum order (mode first) and restores reverse enum
order (mode last). That is retained solely for interpreting existing v2 records.
It is not recommended installed activation: mode-first enable can expose stale
endpoints. Changing restore to mode-first unconditionally would also be unsafe.

| Saved mode | Bounded successor | Full safe sequence still required |
| --- | --- | --- |
| `none` | Controls first, `manual` last; restore saved `none` first, controls in reverse | Proven installed authority/readiness and drain |
| `manual` | Refuse before storage/effects | Explicitly quiesce to `none`, settle; change controls; enable last. Restore by quiescing/settling, restoring saved controls, saved `manual` last |
| `auto` (PAC) | Refuse before storage/effects | Same quiescence stages; restore saved PAC/bypass/auth/other layered fields before saved `auto` last |

The two-side journal records only original/intended values. For a saved Manual
or PAC mode, interim `none` is a third value, so neither blindly accepting it as
owned nor generic reset is correct. The new order refuses those baselines with
`UnsupportedOrder`, even if some controls already match. A future reviewed
three-value/staged journal must record every temporary mode transition, distinguish
external edits equal to a stage, and define crash recovery without weakening
the [takeover fence](S1_PROXY_FOUNDATION.md). Tests seed synthetic saved manual
hosts/PAC URLs privately and verify that refusal leaves every layered value
unchanged and creates no record. No PAC URL is fetched.

## Durable v3 semantics

v2 bytes remain version 2 without an order field and retain legacy order. The
new model writes version 3 with required `order: mode_last_original_none`.
There is no automatic v2 migration/reinterpretation or fresh baseline on restart.
Version/order mismatch, missing/null/unknown/duplicate order and non-prefix
mode intent refuse. Field indices/expected-side and attempted bitmaps retain
their fixed key identity; order rank, not index, validates apply prefixes and
restore pending suffixes. A pending restore of mode has stable index zero but
last apply rank and is legal; an endpoint restore while mode still remains
intended is invalid.

The durable-before-effect and storage-poison rules are unchanged. Reopening
cannot resume apply or confirm an unknown write. Recovery review is read-only:
original/intended/recorded mixtures retain unsettled evidence, foreign edits
preserve external values, and released original retains its tombstone. The
runtime executor still refuses all reopened predecessor records. Exact same-port
fixture reentry drains before compensation; a fresh connection or process exit
is never a predecessor-drain receipt.

## Verification and remaining gates

Pure tests cover every apply pending prefix, both possible unknown outcomes,
every restore-intent decode/reentry, exact presence, explicit order validation,
legacy v2 interpretation, Manual/PAC refusal, premature activation/restore and
external mode edits. The real private-dconf suite adds all 17 apply prefixes,
every confirmed restore prefix, delayed final enable and first disable, external
mode/host edits, Manual/PAC refusal, and a writer child whose final queued enable
commits after exit. Before and after that last-mode commit its journal remains
pending; neither disabled nor fully intended readback authorizes restoration.

These tests use the already documented internal dconf fixture protocol and
retained same-connection/unique-owner barrier, not a selected installed API.
They never read/change the desktop's actual proxy, real manager environment,
host bus/session/services, private profiles, routes or VPN/TUN. Manager values
are synthetic and unchanged in dconf tests. Only owned fixture children are
stopped/resumed/reaped. Process-crash takeover and power-loss guarantees remain
unclosed.

Before installed activation, the [admission chain](S1_ADMISSION_CHAIN.md) still
requires actual system/user-manager and broker AUTH-writer lifetime/session
provenance, target/schema/profile continuity, fixed installed writes/drain,
exclusive loopback/TUN-disabled listener readiness tied to the current owner
and revision, new-app UWSM consumption and conflict escape. Full prior Manual/PAC
staged recovery, ARM64/VM and NixOS acceptance remain separate unrun gates.

The [desktop-only staged successor](S1_STAGED_QUIESCENCE.md) separately models
the missing explicit-none transitions and exercises them in private dconf.
It does not relax this v3 refusal, create installed admission or solve
identical-value external edits/cross-owner crash takeover.
