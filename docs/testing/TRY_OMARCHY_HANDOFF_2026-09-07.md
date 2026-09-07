# Try Omarchy handoff — 2026-09-07

Owner requested a safe stop before powering off. All writers are stopping;
resume by fetching GitHub, reading AGENTS.md and the canonical roadmap/workflow/
acceptance/Rust migration documents. Do not assume this checkpoint is current
without fetching. No credentials or private fixtures belong in this report.

## Main and completed work

- Starting main: `1d06c2f1f334df0b53415d6e6af4230e3ab94ab0`.
- PR #175 **merged** at `5325617a75ef1687f486328a8a05111823b51dae`, current
  verified main. Accepted helper head:
  `4d7485635be46fbfca9d24cb0d164bcca982eb9b`.
- Fixed Rust client-only clipboard read/copy, chooser-to-file-contents,
  file read, editor, QR PNG, atomic 0600 export and stale editor cleanup.
  Cancellation is exit3 with empty output. No daemon IPC or installed QML
  switch, no new privileged command path.
- Exact acceptance: **510 Rust passed / 4 ignored**, installed Mihomo opt-ins,
  fmt/Clippy/parity PASS; Python270 run/3 skipped; QML/i18n/search PASS;
  CI34133079699 PASS. Actual Try Omarchy clipboard roundtrip/restoration,
  chooser contents, QR, export0600 and file-read PASS. Real smoke found editor
  incorrectly requiring daemon directory; fixed dedicated safe0700
  `omavless-desktop` directory. Final real editor PASS, unchanged synthetic
  contents and seed cleanup verified. See PR175 body for exact evidence scope.
- Installed plugin was not replaced: Python still owns normal production.

## Draft work to resume

| PR | Branch | Saved checkpoint | State |
| --- | --- | --- | --- |
| #174 | `codex/r5-custom-rule-mutations` | `920fd0fa0f527692fc003f2a007a4dc6832bd54f` | New actual-core rule observation test FAILS; do not merge yet |
| #176 | `codex/r5-startup-policy` | `f15cb817f161d76b7506018dba5e06111b407005` | Offline unregistered foundation green and reviewed; not rebased over #175 |
| #177 | `codex/r5-diagnostic-operations` | `8532e03171a5cd9c08b1ff775c7b44f6f8c9495a` | Latest hardening/test changes uncompiled; actual-provider shape mismatch investigation open |
| #179 | `codex/r5-routing-preset-transactions` | `6051cbf3beef306441521ddbaa12547ef773f905` | Partial durable-sentinel changes saved as unvalidated Draft |
| #180 | `codex/r5-route-check-fastpaths` | `46eb7f0a6bd774e30ea952dcf55ac2c2b4d80c07` | New independent fastpath checkpoint, uncompiled |

These branches were independently based on `1d06c2f`, not stacked. Refresh,
reconcile, and independently rebase as appropriate before final exact-head
integration. Do not mistake old-head CI for new code acceptance.

### #174 custom rules

Native exact add/delete, shared revision/replay/ownership lock, trusted IDs,
active config replacement compensation, fixed CLI with private rule value on
stdin. Actual Python32-case differential PASS. Previous head `84f8c80` passed
504 Rust/4 ignored, Python270/3 skipped, QML/static/CI. Production code is
unchanged in latest test-only head, but its new installed-Mihomo host test
failed: expected synthetic Domain rule was absent from observed rules. Compile
succeeded; **this host gate is not green**. Investigate exact config and
controller observation without printing any real profile/store data.

### #176 startup policy

504 Rust/4 ignored, Python270/3 skipped, QML/static/plugin validation PASS;
exact-head CI34133783399 PASS. Independent review found no blocker for the
declared offline/unregistered boundary. Eight synthetic actual Python
`configure_startup` whole-store digest comparisons pass. Preserves current
connection intent; does not register startup.configure, add login unit, or
implement working login autoconnect.

Issue #178 records packaged `NoNewPrivileges=yes` versus file-capability TUN
acquisition conflict. Do not remove hardening casually. A once-per-login
trigger must differ from daemon-restart reconciliation; preserve legacy
implicit unit enablement. Before activation also handle SIGKILL-orphaned
`.startup-check.yaml` safely under ownership lock (current create_new refuses it).

### #177 diagnostics

Exact summary/rules/providers methods, detached bounded Unix-controller reads,
sameUID/private socket checks, ownership/revision recheck after I/O. Canonical
private redaction inputs; latest snapshot cap512 fragments/64KiB fails closed,
large fields conservatively redacted, projection deadline enforced.
Earlier focused tests and30-case Python oracle passed; latest code needs fresh
build. Actual installed Mihomo rule reads passed. Initial test `/proc/PID/fd`
inspection failed because file-capability core is non-dumpable: do not weaken
OS security. Test now stages a byte-identical capability-free private copy for
its no-TUN inspection. Keep both empty and inline-provider cases. Standalone
shape probe reports an object for providers; do **not** normalize null in
Python/Rust based on an unconfirmed earlier mismatch. No Python production
change was justified or made. Investigate fresh-build artifact/wiring first.

### #179 routing presets

Fixed three bundled templates; store/template/desired compensation, keepMode,
first-run missing-template guarded creation/removal and generation-monotonic
rollback. Previous focused7 tests/18 Python cases passed; expanded24-case
oracle and newer tests need rerun. Crash between member writes is a real
blocker, not a claim of atomicity. Partial fixed private durable sentinel work
is being preserved: mark before effects; retain through candidate verification;
clear only after verified commit or complete recovery; any pending/unsafe
marker blocks native startup/mutations with manual recovery. No generic
journal/shell API. Review and compile this partial checkpoint before trusting it.
Resume detail: adapt direct plan tests to finalize the sentinel using
`clear_verified(false)` after commit and `clear_verified(true)` after restore;
active failures use `finish_outcome`. Add crash-after-each-member/drop/reconstruct,
malformed/symlink sentinel, unrelated mutation/replay refusal, and failed old-core
recovery retention cases. `PreparedPrivateStoreWrite.verify_outcome_locked` was
added but is not yet gated in this WIP head.

### #180 route-check fastpaths

Independent bounded mode/custom-rule/disconnected results and78-case Python
oracle prepared. Pure Python six-case smoke passed, Rust not compiled yet.
Required live observation returns capability_unavailable, never fabricated
success. Live Python route-check opens a CONNECT and inspects connections/hit
counters; implementing it needs whole-call deadline and attribution care.
Latency probes still require DNS pinning and isolated core orchestration.
Controller /traffic rates are not parity with current QML sysfs TUN counters.

## Local workspace and build discipline

Root checkout `/home/kdk/Work/omavless` remains on `codex/r5-startup-policy`.
Other session worktrees end in `-rules-session`, `-helpers-session`,
`-diagnostics-session`, `-preset-session`, `-route-check-session`.
All useful implementation checkpoints are pushed; Drafts are intentional.
Do not remove older batch-owner/scheduler or evidence worktrees blindly.

This VM has about4GiB RAM. Parallel coding helped, but **serialize Cargo**.
Shared `/home/kdk/Work/omavless/target` reused stale artifacts across worktrees,
producing unresolved APIs despite present sources. Before switching build
ownership, clean the workspace packages (not all external dependencies):

```sh
cargo clean -p omavless-domain -p omavless-runtime -p omavless-mihomo \
  -p omavless-profile -p omavless-store -p omavless-control-protocol \
  -p omavless-parity
```

Use `CARGO_BUILD_JOBS=2`; do not run simultaneous worktree builds. Preserve and
hash exact binaries before another branch builds. Tests using isolated Unix
sockets/actual Wayland may require sandbox escalation; do not label sandbox
denials as product failures or fake PASS through empty piped output.

Useful local logs: `/tmp/omavless-startup-policy-{rust,python}.log`,
`/tmp/omavless-desktop-helpers-cold-rust.log`,
`/tmp/omavless-custom-rule-mutations-{rust,python}.log`,
`/tmp/omavless-rule-host-exact.log`,
`/tmp/omavless-native-diagnostics-rust.log`.
Temporary logs/binaries are not durable cross-machine dependencies.

## Final runtime / safety

Verified outside sandbox before shutdown: plugin enabled, disconnected,
routingMode `global` (preserved), statusFailures0, both user services inactive,
supervisor/Mihomo/TUN **0/0/0**. No live case or real private-store mutation was
performed this session. Test dialogs completed and editor seed was removed.
No OS security relaxation or arbitrary privileged/shell IPC was introduced.

PR #30 remains Draft at accepted `a643db595ad5369b5fea200ebc601b4f0f70f18f`,
implementation and XHTTP evidence unchanged. #135 and #161 remain untouched.
R5/R6 are **not complete**; Python cannot yet be removed. Next session should
finish the saved gates and integration, not restart these implementations.
