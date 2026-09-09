# R5 login receipt startup barrier

This checkpoint connects only the **read-side recovery barrier** of the
[offline login transaction](R5_LOGIN_INTENT_TRANSACTION.md) to native owner
construction. It does not call that transaction, install a login trigger,
change the installed Python plugin, or complete #178/R5/R6.

## Contract

Before native startup reconciliation can observe or mutate lifecycle state,
construction holds the existing migration lock and reads the fixed
`omavless-login.receipt` below the same user runtime base. The canonical receipt
parser enforces regular-file ownership, exact `0600`, byte bounds and strict
schema. No second permissive parser is introduced.

- Absent receipt preserves pre-existing startup behavior. Absence is **not**
  evidence that first-login policy was applied.
- Pending, malformed, unsafe or ownership-mismatched receipts refuse startup
  with a bounded manual-recovery error, without changing desired state or the
  receipt.
- A consumed receipt for the exact committed Rust ownership generation permits
  ordinary reconciliation of **current** desired intent. It does not replay
  login preferences, so a later explicit Disconnect stays disconnected.
- A preparing cutover candidate refuses any existing receipt; a previous
  committed owner's receipt cannot authorize a new ownership transition.

The check is read-only and under the same lease as reconciliation. Cooperating
login writers also require this lease (and acquire the runtime owner lock first),
so publication cannot interleave with startup. This does not defend against
arbitrary malicious same-user processes bypassing all application locks.

Epoch hashes are syntactically checked, not authenticated as proof of a fresh
login. This checkpoint has no trusted epoch source. Runtime-directory teardown
or reboot may remove the receipt, so cross-boot recovery is not established.
There is no automatic receipt deletion, repair, desired-state rollback, generic
shell command, privileged helper or new IPC method.

## Reference and acceptance boundary

Python remains installed owner/oracle/rollback. Python has no native login
receipt, so this is an intentional native failure-safety contract rather than
Python file-output parity. Unchanged no-receipt startup and current-intent
reconciliation are the compatibility reference; existing differential suites
remain required.

Deterministic tests use actual private temporary files and migration locks with
an instrumented lifecycle host. Refusal must occur before **any** host callback
and preserve exact desired/receipt bytes. An isolated real daemon subprocess
also verifies bounded refusal, public-error privacy and socket cleanup. These
tests do not alter installed units, private profiles, system DNS or routes.

The declared host gate is local ARM64 execution of the real binary's isolated
startup refusal and existing socket/daemon regressions, not a live provider
probe: this slice adds a pre-reconciliation refusal path, not a new connect
path. Actual login enabled/disabled, trusted trigger ordering, legacy unit
conversion, packaged activation and Full VPN acceptance remain separate gates.
