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
