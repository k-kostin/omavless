# R5 offline login-intent planner

This checkpoint makes the desired-state decision for a future once-per-login
trigger per user-manager lifetime testable. This is not necessarily each
graphical login: lingering managers and multiple sessions share a lifetime.
It does not install that trigger, register startup IPC, run
commands, touch files, enable services or transfer production ownership.
Python remains the installed startup owner and migration reference.

Reference: Python `resolve_startup_profile` and `startup_connect` in `backend.py`
select last/explicit profile and reject disabled login. Python has no equivalent
Rust desired-state file. Clearing a prior-session Rust connected intent when
fresh login is explicitly disabled is therefore an intentional native contract,
not byte-for-byte Python output parity. Otherwise restart reconciliation could
reconnect the previous session despite disabled login preferences.

## Inputs and decisions

The caller supplies a validated canonical private store, current durable desired
state and an explicit trigger category. These are private inputs; selection IDs
must not appear in public errors or debug formatting.

| Trigger | Policy | Decision |
| --- | --- | --- |
| Daemon restart / login already consumed | Any login preference | Preserve exact current desired state |
| First login | Legacy configuration not resolved | Refuse; require existing-unit enablement conversion |
| First login | Explicit configured-disabled | Desired disconnected, empty profile; retain current mode |
| First login | Explicit enabled | Resolve canonical last/explicit target and Routing/Full VPN mode |

Current desired state is validated even on a preserve path. Changed desired
state advances its bounded generation; an identical decision does not write or
increment it. Exhaustion refuses the change. A no-op plan does **not** mean the
future host can skip consuming the login trigger: once-per-login consumption
and desired-state generation are distinct concerns.

A preserve path does not require the current profile to remain in the store:
normal reconciliation owns that missing-target failure. Nor does it consult
legacy login enablement, because restart is not a new login policy decision.

Selection reuses the existing `PrivateStore` startup resolver, including its
Python-compatible first-profile fallback when last selection is empty. Canonical
store normalization may disable stale missing selections; the planner does not
invent a second raw-store parser or reverse that accepted normalization.

## Future host obligations

This pure function does not prove ownership, configured readiness, capability
availability or absence of another owner. Before applying a first-login plan,
the fixed host must hold the migration/owner locks, verify the current marker
and generation, validate the one-shot trigger, prove no conflicting core/TUN/
controller, and validate the generated candidate where connection is requested.
Only then may it publish intent and consume the trigger under the
[ordered publication/recovery contract](R5_LOGIN_INTENT_TRANSACTION.md).
Each file replacement is atomic, not the combined two-file update; a pending
receipt blocks unsafe retries. A failed validation must not leave an unverified
connected intent or falsely consumed success.

Ordinary daemon restart must reconcile durable current intent, including an
explicit disconnect, rather than call login policy again. No login unit or
production caller is introduced here. The preferred future RemainAfterExit
oneshot and its host acceptance remain described in
[R5_STARTUP_POLICY_FOUNDATION.md](R5_STARTUP_POLICY_FOUNDATION.md).

This closes only an offline decision boundary. It does not close #178, complete
R5/R6, make startup preferences effective in Rust, or permit Python retirement.
