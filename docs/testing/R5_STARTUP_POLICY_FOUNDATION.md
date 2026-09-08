# R5 startup policy foundation

This checkpoint implements **offline** startup preference transactions. It does
not register `startup.configure` with production IPC, add a CLI command, enable
a login unit, change QML, or replace the installed Python startup owner.

## Implemented boundary

- Exact typed startup request and shared revision/replay admission.
- Whole-store validation, private atomic replacement, compare-before-write,
  and the existing store-only compensation/manual-recovery barrier.
- `last` versus explicit profile selection and Routing versus Full VPN policy.
- Validation of a future connection without staging or stopping the current
  core. The fixed native preflight checks capabilities and a separate private
  config; subprocess output is bounded and never returned.
- Eight synthetic cases compare normalized complete store digests with actual
  Python `configure_startup`, with every Python host effect mocked. Current
  connection pointers and unrelated store extensions remain unchanged.

Python remains both production owner and parity oracle. Synthetic protocol
input here is not live interoperability evidence for any protocol family.

## Required next integration, not silently assumed

Enabling preferences is not equivalent to implementing login autoconnect.
Production registration must land with an explicit once-per-user-manager-login
trigger. Applying login preferences on every runtime process restart would
incorrectly replace an existing requested tunnel or ignore an explicit
disconnect; a runtime restart must instead reconcile durable current intent.

Preferred integration to validate: a fixed-purpose, `RemainAfterExit` user
oneshot ordered before the runtime, surviving daemon restarts within the same
user-manager lifetime. Its offline operation must hold migration and owner
locks, validate exact ownership, prove there is no live core/TUN/controller,
then write the next desired generation. No generic systemctl or shell IPC.
Do not install this proposal until its behavior is covered by host tests.

Legacy `startupConfigured=false` is **not** an explicit disabled preference:
Python historically derives enablement from existing user units. Cutover must
snapshot/convert that state or refuse an ambiguous migration before committing
ownership. It must never silently turn legacy autoconnect off.

Host acceptance must cover login enabled/disabled, daemon restart after explicit
disconnect, missing selected profile, invalid Routing configuration, failed
validation, exact previous unit-state rollback, and no duplicate core/TUN.
The packaged service currently sets `NoNewPrivileges=yes`, while the native
host expects Mihomo to acquire file capabilities on exec. This conflicts with
the documented [systemd execution contract](https://github.com/systemd/systemd/blob/main/man/systemd.exec.xml):
the setting prevents children gaining filesystem capabilities. Startup
preflight therefore fails closed under `NoNewPrivs`, instead of reporting ready
from `getcap` alone. The real service/TUN architecture needs its own accepted
host solution; this checkpoint does not remove or weaken the setting.
The owning follow-up is [issue #178](https://github.com/k-kostin/omavless/issues/178).

R5 and R6 remain incomplete. Saving this policy does not make Python removable.
