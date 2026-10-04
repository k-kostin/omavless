# Host authorization during acceptance

Local test-tool correction, 2026-09-11. Normal Omarchy networking authorization
is not a new Rust-only feature. A successful semantic CLI response and empty
Mihomo/TUN inventory do **not** prove resolved/route authorization completed.
The [interrupted Disconnect batch](R5_NATIVE_DISCONNECT_PROCESS_EXIT.md)
demonstrated why unattended repetitions cannot be used as full host acceptance.

## Fixed human barrier

`tests/human_authorization.py` is developer tooling, not a runtime auth API.
Both `installed_native_acceptance.py` and `native_service_acceptance.py` use it.

- A real interactive terminal is required before host/private-fixture access.
  Piped acknowledgements or a headless agent invocation fail closed.
- Before **each** potentially authorizing effect, the human types `ready` only
  after prior authorization has settled and they can attend the next action.
- After that effect, even if it failed, timed out or was interrupted, the human
  resolves all OS dialogs and types `settled`. There is no deadline on that
  human acknowledgement. Existing command/network time bounds remain intact.
- `settled` means no authorization remains pending, **not** that authorization
  succeeded. Actual lifecycle/network/DNS evidence is still independently
  required. A cancelled or failed authorization is not a successful gate.
- Anything else, EOF, terminal failure or an interrupted acknowledgement
  permanently blocks further effects for that invocation. No automatic retry,
  repeated Connect, compensating Disconnect or unit Stop follows that block.
- If cleanup cannot safely proceed, the fixture is retained and output says
  `human_authorization_unsettled` / manual cleanup required. Inspect current
  state with the human before any new explicit recovery action.

Never type OS passwords into this test prompt or chat. The tool reads only
bounded acknowledgement words; it does not log rejected input or raw host
errors. Never script `ready`/`settled`, make a pseudo-terminal to bypass the
barrier, reset PAM counters, change polkit rules, or kill arbitrary auth agents.
Do not infer successful authorization from disappearance of a dialog/process.

### Rejection testing and account lockout

The [RC DNS cancellation test](RC_090_DNS_AUTHORIZATION_2026-09-24.md) exposed a
second acceptance hazard: repeated polkit/PAM failures can temporarily lock the
desktop account, so even a correct password is subsequently refused. Before a
negative authorization case, inspect the effective PAM policy and available
failure counters read-only. Do not assume Cancel is exempt from accounting.
Prefer one deliberate rejected action followed by separately attended accepted
recovery, rather than asking the human to reject a whole chain of dialogs.
If the core continues with more authorization requests, handle that explicitly
in the case instructions; do not start another negative case blindly.

On wrong-password/lockout reports, stop new authorizing effects, inspect safely
and wait for the configured recovery interval with the human. Never reset PAM,
kill the auth agent, alter policy or try the password on the human's behalf.
An empty dialog queue is still not success: verify managed-link DNS readback
after restoration as well as native process/controller state. Record only
aggregate classifications, not passwords, PAM conversation input or raw logs.

The isolated service gate guards fixture Start, Connect, Disconnect and Stop;
every repetition requires new human acknowledgements. The installed gate also
guards explicit socket-inspection authorization and mode restoration. Human
waiting is excluded from reported connect/disconnect command durations.

The installed gate additionally requires an explicitly observed disconnected
result with `manualRecoveryRequired: false` at both baseline and cleanup. A
disconnected desired flag, cached disconnected actual state or empty process
inventory cannot substitute for that proof. Missing/null/string recovery flags,
unavailable facts, nonzero auxiliary counts and booleans used as numeric counts
refuse acceptance. This closes a test-tool gap; it changes neither the runtime
recovery policy nor how host authorization is performed.

## Scope and tests

Deterministic tests inject in-memory terminal streams and harmless callbacks.
They cover missing terminal, wrong/oversized/EOF input, failure/timeout/interrupt,
pre/post-action stop, reentrant effects, cleanup refusal, fixed error privacy,
and valid separately acknowledged cleanup. They never contact polkit, systemd,
the private store or VPN runtime.

Existing unguarded temporary cycle/Quit/removal/recovery helpers in this VM were
retired with an immediate no-effect refusal. Their historical evidence remains
recorded, but those scripts must not be reused by deleting the refusal.
Other acceptance harnesses still need their own per-effect audit before running;
this change is not a blanket claim that every historical test tool is guarded.

No installed runtime/package/security policy changes are made by this slice.
This does not resolve the outstanding DNS/route authorization semantics or
prove R5/R6 complete. Installed interactive acceptance of the guard is pending;
the current session intentionally performs no authorizing host transitions.
