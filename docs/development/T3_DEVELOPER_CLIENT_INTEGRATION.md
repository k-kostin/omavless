# T3 development client integration gate

SOURCE successor of #677 `edc2aadd6674054c237a542dfca99613c9b7102c`.
The [client contract](T3_DEVELOPER_CLOSE_CLIENT.md) and existing
[#666 developer pair/socket contract](T3_DEVELOPER_PAIR_PERMIT.md) are unchanged.
No production method, permit, default CLI, package or installed state changes.

## Two separate, explicitly ignored gates

### Real Workspace → original owner/socket → real selected close

`actual_owner_developer_pair_tui_workspace_in_dev_vm` requires
`OMAVLESS_CLOSE_DEVELOPER_CLIENT_VM=1`, the existing non-default developer feature
and TUI, and the exact administrator-provisioned pair. It must run only in the
separately reviewed disposable PID/mount/network namespace, loopback only, after
dropping to the original admitted non-root user. This is not normal installed
TUI invocation or released-pair adoption.

The test extends the **existing** composed-core/SocketFixture graph. The same
original coordinator and genuinely parent-owned core transfer into RuntimeServer;
the passive host has no test fixture effect permit. It does not introduce another
coordinator, privileged dispatcher or pair authority. All ordinary hello,
capabilities, snapshot, prepare and receipt calls use the real private socket
client. Workspace's real input/call/accept functions produce every request.

Two synthetic loopback echo destinations make A and B visibly distinguishable.
The target comes from that SAME original opaque row's prepare display, not a raw
controller ID or a join with the separate ID-free connection list. The selected
B target is reviewed, cancelled, reviewed under a too-small viewport, and then
allowed to genuinely expire before any confirmation. Both original streams must
still echo after every no-effect cut and desired bytes remain unchanged.

A new explicit refresh/prepare emits exactly one confirm. The test authenticates
the connected same-UID socket and completely writes that original frame, then
drops the client **without reading a reply**. It feeds that actual transport
ambiguity to Workspace, which must block Enter/x/r. Before competing receipt
reads, B must terminate while A still echoes. Only Workspace's exact receipt call
may resolve the pending operation; repeating that already emitted read must give
the same Closed receipt without another confirmation, operation ID or refresh.
Final desired bytes are unchanged and A echoes again.

The original helper's four-argument entry and prior scenarios remain. The new
mode adds one target listener (two total), keeps exactly two echo peers/clients
and one server/core, and adds no unbounded acquisition loop. Echo admission is
still bounded by 15 seconds. Only the NEW mode uses a 20-second peer idle read
timeout so the genuine five-second expiry does not destroy its negative-control
streams; old modes retain five seconds. New client write/read waits are one
second, selected-closure and receipt rounds each have a four-second test budget.
The actual runtime's snapshot/effect/expiry/proof-flight budgets are unchanged.

The original fixture teardown and disposable namespace-init containment are
inherited. They are not product recovery, unknown-child FD custody, installed
service cleanup or a grant to reopen any old stopped scope. No test opts in
automatically in CI or on the author's host.

### Private real-terminal visual review with CLOSED synthetic replies

`developer_close_private_terminal_demo` requires a real terminal and exactly one
of `OMAVLESS_T3_CLIENT_TERMINAL_DEMO=empty|rows|closed|unknown`. ROOT may select it
as a normal user after source review. `OMAVLESS_LOCALE=en|ru` uses the existing
process-local bounded locale policy; OS/plugin settings are unchanged.

This invokes the actual `developer_close::run`, including its input loop,
terminal restoration, worker and real rendering, with a closed synthetic
callback. It has no RuntimePaths, socket, core, services, network, privilege or
real profile. UI entropy/terminal/signal handling remain ordinary client backend
operations, not fake authority. Fixed `.invalid` names and RFC5737 IPs are
synthetic private screen data; generated handles/ticket/operation are never
Debug-printed. Every allowed method and exact parameter shape is checked, one
original operation is retained, invalid/unprepared confirmation cannot mint a
terminal receipt, and a second confirmation is refused.

`empty` shows a genuine synthetic empty snapshot. `rows` supports selection and
full-target confirmation, then Pending → explicit `u` → Closed; `closed` gives
an immediate synthetic terminal reply; `unknown` gives Pending → `u` → sticky
Unknown. `r`, j/k, x, Enter, Esc, resize and q/Ctrl+C can be reviewed in EN/RU.
Terminal close is client loss only. This gate establishes rendered interaction,
not installed/live-close or product authority. Keep captures private; report
only the exact source/binary and sanitized screen/state matrix.

## Readiness and evidence

- Both resource/terminal scenarios are explicitly ignored and unexecuted by the
  author. No VM, private capture, old scope or host network/service was touched.
- Compile-only feature library succeeds. The CLOSED demo backend's one pure
  method/operation/reentry test passes; strict feature all-target Clippy passes.
- This checkpoint ran the full feature TUI suite (161 passed), five focused
  canonical development protocol tests, four static feature-boundary controls,
  default/headless+developer checks, catalogue/navigation and format/diff checks.
  They are SOURCE evidence, not either actual scenario. Combined local fixture
  limitations retained on #677 are not silently converted to PASS.
- Included test sources receive explicit rustfmt checks in addition to the normal
  workspace formatter, which does not follow `include!` bodies.
- ROOT/independent affected review, exact compiled public image identity,
  separately rebound fixed delivery/launch and original whole results precede
  any actual scenario. No early PASS, main merge, release or activation follows.

The first actual question is whether the real client-produced B confirmation can
lose its reply and still resolve solely through the retained original receipt
while A and desired intent survive. The terminal demonstration is a separate
usability question; it cannot replace that operation gate.

## Original21d actual outcomes and Hello successor

ROOT's exact21d/SHA3acc16… real Workspace/socket scenario returned original
SSH101, **NONPASS**. Separately selected file observer returned original0:
stdout355 bytes SHA5376ac6625aa2cf7d36c68659efdc55a2c14064269f0c9cf991cf79506480041;
stderr332 bytes SHA6062bf8336c87f1668af4defc2edf378f55f939b2d0bcaafb28caa5e3ea892c2.
A separately admitted bounded projection returned original0 and identified
public client_integration.rs50:13/success_reply_assertion. That capture does not
contain the actual method/error code and does not prove no effect, current
custody, recovery or a retry entitlement. No cleanup/retry was selected.

Source inspection found a definite protocol defect: Workspace emitted empty
system.hello params, but both canonical read/native dispatch require exactly
versions:[1], otherwise InvalidArgument. The source therefore predicts refusal
of the first Hello; this is a source-supported explanation, not a newly captured
error code. The successor reuses existing Read::Hello.method/params rather than
changing the server/parser/authority predicate. A real canonical-dispatch pure
regression rejects the former empty shape and accepts the existing negotiator.
The CLOSED demo now checks that same shape, and integration failures may emit
only an allowlisted method/stable-error-code pair, never JSON/private details.

All eight exact21d/SHA3acc16… synthetic actualPTY combinations returned original
SSH0 on ROOT's80×24 terminal. Rows/closed/unknown exercised selection, explicit
review/cancel, pending/receipt, q; RUunknown repeated r/x/Enter remained sticky
and Ctrl+C exited only the client. Empty x/Enter had no action. A deliberately
slow initial expiry refused before an explicit fresh snapshot. These are actual
terminal interactions with synthetic closed replies—not a live socket PASS or
installed product acceptance. Graphical Foot/focus/resize review remains separate.
Old21d and original101 are preserved; a new exact artifact/fresh namespace and
original whole result are required for the corrected client integration.

## Exact corrected Workspace/socket gate

Tested implementation `b29a665555f8559f97baa35c1a721670447d650d`.
Incremental release compilation returned original0. Exact image22,008,024 bytes,
SHA7419ca64e039f0e8b1513dceaec7a9e61600aebeb854b635889cb5bb41f669b5.
ROOT's full affected review and eight pure delivery controls preceded
ROOT's fresh isolated namespace selection; the already confirmed developer pair
was not reprovisioned. New namespace/guest delivery scope did not reopen the old
failed review1 captures or change normal installed runtime/profile/network state.

ROOT's selected actual scenario `ad2379`/session40656 returned originalSSH0.
Separately selected two-file observer `11fcd0` also returned original0, complete
exact one-test success grammar: stdout224 bytes SHA
42392602a5be6776dde8b56e1636565f851535e675b0d59faf2f9f2e49bbf04d,
stderr empty. The actual source asserts original Workspace→same coordinator/
socket/developer permit, cancel/resize/genuine expiry with both streams alive,
selected B closure/A echo, genuinely lost confirm reply resolved solely by the
original receipt call, exact read repeat, one confirmation and unchanged desired
bytes. No synthetic effect permit or guessed raw-ID/public-row join was used.

This closes that scoped developer client-function/socket gate, NOT a normal
installed CLI/Foot path, released-pair/default adoption, ARM64 or whole T3/C1.
Disposable namespace-init containment and ordinary trusted tooling limits are
unchanged; successful teardown is not product recovery/unknown-child custody.

ROOT also inspected actual Foot on b29's CLOSED synthetic ENclosed demo:
selected B/full target readable, Enter→Closed. Prior21d RUunknown Foot returned
originalSSH0 after Unknown, resize1110×1198→560×330, a visible70×24 requirement
and wrapped readable receipt-only footer, then q. ENempty correctly had no x/
Enter action. Private captures remain outsideGit. These observations exposed
phantom row/confirmation hints in Empty/Closed footers, recorded for a separate
UI-only successor; they do not retroactively correct b29's presentation or prove
live socket behavior through the ordinary executable/terminal path.

The later UI-only footer successor addresses these observed gaps: Initial/
Empty advertise only r+exit, actionable rows navigation/review, prepared target
Enter/Esc, in-flight wait/exit and Pending/Unknown receipt-read-only. A verified
non-Unknown terminal receipt now permanently spends this developer window:
q/Ctrl+C only, including hidden r refusal. A new window is required for a new
operation. Backend/API, tokens, permit, ordinary/default TUI and desired state
are unchanged. New EN/RU TestBackend/state/no-hidden-refresh control passed;
full TUI162, protocol6, strict feature Clippy/default/headless, catalogue330,
format/navigation/diff passed. This source correction is not retroactive b29
rendering acceptance; actual updated frontend/Foot remains pending.

## Separately ignored real primary CLI + Foot gate

`actual_owner_developer_pair_real_cli_foot_in_dev_vm` adds a separate mode to
the SAME composed-core/SocketFixture helper. Old scenarios keep their dynamic
ports/timing. ROOT must select a fresh reviewed disposable PID/mount/network
namespace, exact developer pair and UID/GID1000; the driver is its PID1. Two
private-netns synthetic echo destinations are A:19180/B:19181, not raw-ID/public
row joins. The actual `omavless tui --developer-conditional-close` communicates
with that original RuntimeServer/socket. No CLOSED callback, CLI fixture API,
second coordinator, installed service or new effect authority is used.

The primary CLI is a separately byte-pinned8,571,248-byte artifact, SHA
75263631e03f8b99bc8d1dd511901148cb7798fa5ad35b71797e9efdbed1a304,
compiled from the UI-footer successor. Its fixed guest path is
`/home/kdk_vm/.cache/t3-live-ui-r1/omavless`: UID/GID1000, single regular inode,
mode500, nofollow original File retained after exact hash/metadata/named checks.
The library-test image needs a separate exact build/hash and reviewed delivery;
the CLI hash does not identify or authorize that driver image. Neither is run by
the author. The private fixture is exactly one mode700 direct child of the
fixed `t3-live-ui-r1/tmp` parent; no caller-selected runtime/socket is admitted.

One original `/usr/bin/foot` runs one explicitly ignored library-test TTY helper,
which spawns and waits the one original primary CLI Child. Foot uses only the
normal UID1000 Wayland endpoint, no server/no-wait mode, `/dev/null` configuration
and fixed `OmaVLESS-T3-Live` app-id/title. `/usr/bin/env` replaces ONLY its child
runtime path with the original private fixture, and passes the original PID
namespace dev/inode for a same-namespace check. The helper checks UID/GID1000,
EN/RU and the pinned CLI, clears the CLI environment, then provides that private
runtime path. Normal installed runtime, desktop config, VPN intent and services
are not changed. ROOT selects one locale and inspects/types the actual window.

Completion requires **both** original Foot wait0 and the strict exclusive
`cli-original-zero` receipt from the helper's original CLI wait0, then actual
B termination/A echo/unchanged desired bytes. Foot's ordinary trusted delegated
child-status contract is explicit: upstream [foot(1) EXIT STATUS](https://codeberg.org/dnkl/foot/src/branch/master/doc/foot.1.scd),
`main.c` shutdown callback/return and `terminal.c` original slave WEXITSTATUS.
This is a trusted administrator-installed terminal/backend assumption, not
toolchain attestation or imported runtime authority. The helper returns0 only
after exclusive mode600 single regular UID/GID1000 receipt write/sync/held+named
identity/size and sampled-deadline checks. The admitted CLI File and named
binding are checked again after original CLI wait0. Parent reads the exact31 bytes
under its own five-second sampled budget and rechecks both held and named inode.
A physical partial/late receipt can remain after ordinary I/O failure: it is
**not accepted**, because the helper parks rather than returning0. No marker is
attempted after nonzero/timeout/late original CLI wait. No marker alone, Foot0
alone, screen or guessed child PID is sufficient evidence.

Each original UI wait has a900-second sampled pre/post gate and20ms polling,
not a five-second whole-scenario limit or a preemptive kernel timeout. The row
lease remains five seconds: fresh snapshot/review is explicit, no automatic
renewal. The new echo peer idle timeout is900 seconds; on an idle-timeout error
the peer remains parked, not dropped. Old five/twenty-second modes are unchanged.
On Result refusal the helper retains original CLI/binary/reported marker, and
the outer driver retains original Foot/reported completion File plus the SAME
server/core/clients. No kill, second wait,
restart, resend or cleanup is selected on refusal; ROOT may separately
administer this disposable unavailable scope. Positive completion retains the
existing fixture teardown and namespace-init containment. Other ordinary socket
errors, partial spawn internals, panic/fatal loss, Foot's own abnormal PTY
shutdown and process-wide backend FD custody remain excluded—not product
rollback/recovery or fatal-FD survival claims.

Finite extra live roles are one Foot, one TTY helper, one CLI Child, the retained
public CLI File and one completion File, on top of the existing single server/
core, two destination listeners/peers/clients and private fixture. Existing
ordinary stdio/Wayland/font/config-free terminal backend resources are trusted,
not an invented global memory/FD ceiling. Pure wait controls cover positive,
pending→positive, nonzero, error, pre-expiry and positive-return/late-postgate
without callbacks after refusal. Real path/receipt predicates reject other
runtime roots, malformed leaf shapes and nonexact marker bodies. Fixed-artifact
controls and compilation do not
execute any UI/socket/actor. Actual original whole/terminal/closure evidence and
fresh delivery are pending ROOT plus independent affected review. Default/
released-pair adoption, installed package/frontend and ARM64 still remain open.

SOURCE checkpoint: three new real-UI wait/path/receipt controls, six canonical
development protocol tests, the full162-test feature TUI suite, four static
feature guards, strict feature all-target Clippy, default/headless feature
checks, catalogue330/navigation and explicit included-source/workspace format
checks returned original0. Initial compile-only attempts found a missing Digest
trait and an ambiguous Read/Write.by_ref call; both ordinary SOURCE failures
were corrected, not labeled native/VM failures. The two new actual entries
remain ignored; no real Foot, CLI child, controller/socket or VM was executed
by the author. Final private image/delivery and actual evidence remain separate.
