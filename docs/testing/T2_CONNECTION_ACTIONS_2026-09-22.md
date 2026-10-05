# T2b terminal connection actions — ARM64 acceptance

Candidate: `6b907e568f777140c9cc3d178efdedbc26f92c56`, branch
`dev/t2-connection-actions`, dependent on T2a
`0575df72304d3f740c75d151f66c546f61eef9e6` (#269).
T2b PR: [#274](https://github.com/k-kostin/omavless/pull/274).

Feature-enabled client binary SHA-256:
`c102b7d8e14d28dc08f422cc1514aa4864a25258e00da2dc94ca501b18d72516`.
It attached to the installed stable 0.8.2 runtime without replacing the package,
QML frontend, systemd units or release pins. The following evidence-only commit
does not change that executable. Main stays at
`d620c300020d3acfa9c00418da7f6cded485ffdb` under the owner's explicit hold.

## Automated local checks

- Complete Rust script: **1006 PASS, 11 existing opt-ins ignored** (1005 in the
  workspace suite plus the feature-enabled TUI-to-canonical-parser check).
- Focused TUI: **26 PASS**, including 13 new action/admission/response/UI cases.
- Synthetic real PTYs: **10 PASS**, including q/SIGTERM while an action callback
  intentionally sleeps and actual destruction of a terminal. No test timeout
  was relaxed. Callback entry is proven by a fixed synthetic receipt, not by
  inferring a network effect from screen text.
- Existing developer suite: **276 tests, 2 skips**; JavaScript/QML contracts,
  plugin validate, Rust formatting/Clippy/parity, shell syntax, manifest JSON,
  documentation navigation and diff whitespace passed.
- **64** bounded EN/RU keys; shared QML translations match. No runtime Python
  dependency or default TUI/package feature was introduced.

## Rendered acceptance

Real Foot terminal: English and Russian normal list, selected Connect target,
current-session Disconnect target, mode confirmation, unknown outcome, exact
retry, current-state acknowledgement and help. Sixteen synthetic screenshots
were captured outside Git; dynamic names are untranslated and plain text.
The current connected profile stays distinct from the selected alternative.

TestBackend additionally verifies minimum 70x24 confirmation layout, local
control/markup sanitization and visible confirmation/exit hints. Shrinking the
viewport blocks invisible confirmation. Previous read-only small-window tests
remain intact. No synthetic screenshot is live VPN or marketplace evidence.

## Installed-runtime actions

The owner attended the VM. Each potentially authorizing action used the existing
real-terminal `ready` **before** and `settled` **after** barrier, with no deadline
on human acknowledgement. The harness drove the actual candidate's keyboard
confirmation through a private terminal session; it did not invoke a substitute
mutation CLI or create another runtime. QML remained installed/enabled.

The starting profile was retained privately and reused, never printed or passed
in process arguments. Starting and final state: connected / Routing.

| TUI action | Applied reply | Refreshed runtime state | Public HTTPS probe |
| --- | --- | --- | --- |
| Disconnect current session | PASS | Disconnected; core/TUN 0/0 | Not required |
| Connect original profile | PASS | Original target; core/TUN 1/1 | PASS |
| Change connected session to Full VPN | PASS | Full VPN; core/TUN 1/1 | PASS |
| Restore original Routing mode | PASS | Original target/mode; core/TUN 1/1 | PASS |

Each row required coherent instance/revision, matching desired/actual state,
observed facts, zero auxiliary core and `manualRecoveryRequired: false`.
Connected rows required desired-profile/owned-config match, exactly one visible
Mihomo and one visible/managed TUN. HTTPS was a bounded ordinary request to a
generic public 204 endpoint; it is **not** comprehensive DNS/leak/route coverage
or evidence for any new protocol family. This slice does not add a TCP controller
or change the existing private Unix transport; no new privileged listener audit
is claimed here.

Afterward a separate read-only live check opened/closed the same client with q,
reopened it and used SIGTERM. Both rendered the actual active identity correctly
(compared privately); desired/actual state, revision and runtime/core process
identities stayed unchanged. Final original profile/mode restoration passed.

## Test-tool findings and limits

- An initial PTY assertion searched for a whole phrase in incremental ANSI bytes.
  Cursor-addressed spaces made that a false failure. The slow-action test now
  proves callback entry with a credential-free example-only receipt; rendering
  is independently covered by actual screenshots/TestBackend.
- The first live harness stopped before any network command after attaching a
  tiled terminal failed its screen-readiness check. The corrected harness used
  a fixed-size real PTY and a separate visible authorization terminal. No host
  mutation was retried or bypassed to work around a window-layout problem.
- The first post-close comparison included the CLI response ID, which contains
  the caller PID and therefore changes on every invocation. A separate read-only
  repeat compared runtime result/revision and process identities instead; it
  passed. This was not a tunnel reset or a reason to repeat authorization.

Not claimed: full T2 MVP, x86_64 installed TUI acceptance, packaged/default TUI,
Open app launch/focus, runtime restart during an in-flight action, comprehensive
concurrent plugin/TUI mutation acceptance, durable cross-client receipts, AUTO-1,
V0 completion or DNS/provider follow-up closure. CI is recorded on the PR rather
than frozen here as a promise. No merge/release/marketplace action is authorized
by this acceptance report.

## Post-acceptance hidden-target guard

While preparing the next browsing slice, a deterministic case demonstrated that
an externally renamed profile could disappear from search after refresh while
remaining selected. The follow-up clears invisible selection and rejects hidden
Connect targets both when opening and confirming the action. Focused TUI tests
increase to **27**. This is a narrow admission correction, not a change to wire
commands, runtime lifecycle or the accepted successful network sequence; the
live evidence above retains its original exact head rather than being relabelled.
