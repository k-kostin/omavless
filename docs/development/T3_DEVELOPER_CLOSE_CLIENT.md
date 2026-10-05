# T3 development connection-close client

Source successor of #666 `bb83f851f9d8cf1da638056be1fc34f49e54ba8e`.
The tested developer socket checkpoint remains recorded in
[T3_DEVELOPER_PAIR_PERMIT.md](T3_DEVELOPER_PAIR_PERMIT.md). This client is not
released-pair adoption, default product close, installed UI acceptance or a new
authority constructor. Main/RC/release/package pins are unchanged.

## User task

Review private connections from one original owner and explicitly close exactly
the selected opaque row. Selection/navigation alone never mutates. The target
list is independent of the existing ID-free `runtime.connection_rows` page:
handles are never joined by destination, display equality or row index.

Build with non-default `developer-conditional-close`, then explicitly choose
`omavless tui --developer-conditional-close`. Ordinary `omavless tui` keeps its
existing read/action/job paths. Headless/default builds cannot discover or use
this command. No environment fallback, shell, privileged transport, service
startup, installer or raw method/JSON entry point is added.

The separate development workspace reuses the TUI's terminal guard, bounded
catalogue/plain-text sanitizer and authenticated Rust unary socket client. It
exposes no ordinary Connect/Disconnect/Mode/job actions. Development pair
authority still requires the existing root-provisioned receipt with
`production_adoption=false`; API advertisement is not proof that it is present.

## Finite interaction and transport

- `r` explicitly performs hello/capabilities/snapshot once. All four development
  methods must be advertised and native runtime ownership must be true. There
  is no initial/background refresh, token renewal, reconnect or automatic retry.
- At most 128 exact private rows are accepted. Handles are canonical nonzero
  lowercase 256-bit tokens, unique within the snapshot; display is exactly the
  existing one-row destination projection. No raw controller ID/token appears
  on screen, in Debug, ordinary status, activity history or diagnostics.
- `j/k` moves only the visible cursor. `x` prepares that exact held handle.
  The prepare response must match original instance, snapshot revision, handle
  and entire display. It supplies one single-use ticket. Enter submits only
  after that response and explicit visible confirmation; repeated keys cannot
  submit. Esc cancels only a not-yet-submitted confirmation.
- The window's conservative five-second selection lifetime begins BEFORE hello
  is dispatched and never renews. The owner independently enforces its original
  expiry. A delayed prepare, expired selection, malformed reply, wrong owner or
  changed revision cannot certify authority. The minimum submit/review viewport
  is 70 columns by 24 rows; resizing smaller blocks submission.
- Before enqueueing confirm, retain one random operation ID with the original
  instance/revision/handle/ticket and mark submitted. There is exactly one
  close submission per such context. A capacity-one worker/request/result queue
  runs fixed unary calls outside the UI loop; it cannot create unbounded work.
- After submission, only `u` may read that exact operation receipt. Timeout,
  transport loss, invalid receipt and not-found remain unresolved: no confirm
  resend, fresh operation, local acknowledgement or optimistic close follows.
  An exact terminal receipt may resolve transport ambiguity. A verified terminal
  Unknown remains sticky; subsequent data cannot promote it into success.
- Closed, Missing and other terminal non-Unknown outcomes remain distinct.
  They clear the spent snapshot; another user action requires a NEW explicit
  refresh. An action reply is not VPN/network-health evidence.
- `q`, Ctrl+C, signal/terminal loss close only the client. They cannot cancel or
  compensate a submitted operation or change VPN desired state. Runtime-owned
  receipts remain subject to #666's existing capacity/non-eviction contract.
  Client correlation is window-local, not a durable cross-restart recovery
  journal. Reopening never retries an old close; it cannot resolve a lost
  window's operation ID. Do not advertise automatic recovery.

Every wire call uses the existing private 0700 directory/0600 socket, same-UID
connected-peer authentication, bounded framing and correlated response ID.
The server validates the requested actual instance; changed server identity
cannot adopt old handles. No frontend evidence or copied receipt grants a
permit. Backend owner/scheduler/ProofFlight/revocation/expiry are unchanged.

## Evidence matrix

| Boundary | Current status |
| --- | --- |
| Existing #666 original developer socket scenario | Retained exact-head scoped PASS; not repeated or borrowed as this UI's evidence |
| Client state/parser/target/error functions | New deterministic source tests required on this head |
| EN/RU / long-private-text / constrained terminal | Synthetic TestBackend checks; installed rendering pending |
| Actual client → same owner/socket → selected tunnel close | New separately reviewed VM/terminal gate pending; ROOT sole operator |
| Lost UI response / Unknown / expired selection / owner change | Synthetic controls; affected real client fault gate pending |
| Default/product close | Not activated; released-pair adoption, ARM64 and installed product gates pending |

The UI-review and localization skills shaped explicit target/confirmation,
wrong-target/stale/error cases and bounded EN/RU labels. There is no available
rendered/installed session in this SOURCE task; synthetic rendering does not
prove installed terminal clarity, keyboard/resize/signal behavior or real VPN
effects. No VM, old stopped scope/capture, host network or service was inspected
or changed by the author.

Local SOURCE checkpoint: the full feature-enabled TUI suite passed 161 tests,
zero failures; 13 are new workspace controls. The five focused development
protocol tests passed, including the new real client-function/canonical-parser
composition. Strict feature all-target Clippy, default and headless checks,
325-key EN/RU catalogue, documentation navigation, format and diff checks passed.
These are not the full combined CI or an installed UI/VM gate. During iteration
a misplaced rendering edit failed compilation, followed by an outdated screen
assertion failure; both ordinary SOURCE negatives were corrected and affected
checks repeated. No actual operation ran in either failure.

The next meaningful gate is a separately admitted disposable-namespace scenario
using #666's same original coordinator/developer pair and two echo tunnels:
drive this workspace's real input/call/accept functions through the actual socket,
prepare row B while A survives, submit once, resolve/re-read the exact receipt,
then prove B terminates/A echoes and desired intent is unchanged. Render EN/RU
confirmation privately in the actual terminal, test resize/cancel/client close,
and exercise a dropped reply using receipt reads only. No second privileged
dispatcher or new pair permit is needed. Its affected test/launch patch must be
reviewed before execution; ROOT keeps the only VM lease.

The owner-approved [execution policy](EXECUTION_POLICY.md) governs NEW work;
the historical #666 receipt and all stopped experiments keep their original
scope. Risk-proportionate affected review plus independent authority-boundary
review is required before the new actual gate. No main merge or release follows
from source tests.
