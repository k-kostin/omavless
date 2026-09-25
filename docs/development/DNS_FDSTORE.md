# DNS lease quarantine: systemd descriptor-store proof

Status: test-only foundation, not an installed DNS helper or completed #270.

## Problem and boundary

Keeping an actual TUN descriptor in the broker prevents ordinary core shutdown
from deleting that nonpersistent interface. It does not survive a broker crash
unless another trusted process retains the same descriptor. A DNS operation
already submitted to systemd-resolved can outlive the broker that submitted it;
closing its connection is not a cancellation acknowledgement. Prematurely
releasing the last TUN descriptor can therefore permit interface-index reuse
while an old operation has an unknown outcome.

The systemd service manager's descriptor store is a candidate independent holder.
It can retain the original kernel object, without moving route ownership into
the broker. This document proves only descriptor-store mechanics with an
ordinary unlinked temporary file. It does not exercise TUN, host DNS, a system
service, production authorization, or a VPN connection.

## Verified mechanics

Tested locally with systemd 261.2 on ARM64. The explicit probe creates three
uniquely named transient **user** services; it writes no permanent unit files.
All worker files contain synthetic data, with a private scratch directory and
mode-0600 atomic checkpoints. Results contain eleven fixed booleans only.

| Operation | `Preserve=restart` | `Preserve=yes` |
| --- | --- | --- |
| Main process SIGKILL followed by automatic restart | Same object inherited | Same object inherited |
| Explicit service restart | Same object inherited | Same object inherited |
| Explicit service stop | Store released | Store retained |
| Start after explicit stop | Not tested/relevant to quarantine | Same object inherited |
| Explicit named removal | Not separately exercised | Store released |

The worker closes its local copy before recording readiness. Subsequent workers
verify the inherited descriptor's file type, synthetic content and device/inode
identity. Thus the result is not merely survival of a numeric descriptor value.

The negative control is essential: with `FileDescriptorStoreMax=0`, sending
`FDSTORE=1` and completing a separate `BARRIER=1` **still succeeds**, while
`NFileDescriptorStore` remains zero. Notification delivery and barrier completion
prove processing, not successful admission into the store.

Normal/error cleanup targets only the validated generated unit: stop, explicitly
clean its descriptor store, reset its failure state, then verify both zero stored
descriptors and no running main process. Worker start/stop/timeouts and restart
rate are bounded. Unexpected kernel/service-manager behavior is FAIL, not PASS.

Run the actual opt-in experiment as an ordinary logged-in user:

```sh
python3 tests/dns_fdstore_probe.py
```

The deterministic guard tests do not start services:

```sh
python3 -m unittest discover -s tests -p test_dns_fdstore_probe.py
```

## Proposed production ordering, not implemented by this probe

1. A root-owned fixed-purpose service uses `NotifyAccess=main`, an explicitly
   bounded nonzero store capacity, and `FileDescriptorStorePreserve=yes`.
   `restart` is insufficient for quarantine during an explicit stop.
2. Recover and validate any inherited lease before accepting new work. An
   unexpected existing descriptor or transaction is a quarantine/recovery state,
   not an empty baseline. The kernel admission leaf must validate the actual TUN
   descriptor again; names or descriptor numbers alone are not proof.
3. Store the verified descriptor under a fixed generic name, with `FDPOLL=0`
   if automatic HUP/ERR eviction could release an unresolved lease. This choice
   intentionally requires explicit release; it is not automatic cleanup.
4. Complete the notification barrier and verify store admission through the
   manager's typed store metadata, including expected count and object metadata.
   A count alone is not sufficient if an old slot can exist. The proof script's
   fresh single-slot units are narrower than production recovery.
5. Persist the pending transaction/quarantine decision before issuing the first
   DNS write. Broker death must not erase the information that an outcome is
   unknown. No DNS write may rely solely on a successful notification send.
6. Report DNS readiness only after the fixed mutations and their readback are
   proven for the held interface. Release requires a proven safe transaction
   boundary, followed by named removal and verified manager-store readback.

**The descriptor store does not prove that a pending D-Bus call has completed.**
A query on a new connection after broker restart is not established here as an
ordering fence for calls queued by an old connection. Timeout, EOF, core exit,
broker restart, and a fresh `GetAll` must not by themselves release an unknown
lease. Until an actual ordered-completion mechanism is implemented and tested,
retain the original descriptor in quarantine and fail closed to new DNS work.
Explicit recovery must reconcile this uncertainty; blindly dropping the store
is not safe recovery. Reboot establishes a different lifetime boundary, but is
not the normal intended UX.

The root unit, durable transaction record, exact metadata verification,
recovery protocol, real resolved calls and queued-call fault injection remain
implementation/acceptance work. This is not evidence that prompt-free DNS is
ready to install.

## Stop, upgrade and removal semantics

`Preserve=yes` deliberately leaves descriptors held after stopping the broker.
Do not make `ExecStopPost`, package removal, upgrade, or generic cleanup silently
invoke `clean --what=fdstore` for an unresolved transaction. Unit configuration
changes, including reducing store capacity, require the same lifecycle review.
Authorized administrator intervention can override the mechanism; administrator
host control is outside the ordinary same-user/crash threat boundary.

The disposable user-manager experiment does not establish survival of logout,
user-manager death, PID 1 failure or machine reboot. A future system service is
owned by PID 1; that integration needs its own tests. The design must not rely
on an incidental user-manager lifetime for production quarantine.

## Authoritative references

- Installed systemd 261 manuals: `systemd.service(5)` sections
  `FileDescriptorStoreMax=` / `FileDescriptorStorePreserve=`, and `sd_notify(3)`
  sections `FDSTORE=`, `FDSTOREREMOVE=`, `FDPOLL=` and `BARRIER=`.
- [systemd descriptor-store design](https://systemd.io/FILE_DESCRIPTOR_STORE/)
  describes service-manager retention and delivery on subsequent execution.
- [Version-matched service implementation](https://github.com/systemd/systemd/blob/v261/src/core/service.c)
  implements capacity checks, insertion/removal and preserve-state handling.
- [Version-matched service D-Bus properties](https://github.com/systemd/systemd/blob/v261/src/core/dbus-service.c)
  exposes `NFileDescriptorStore` and `DumpFileDescriptorStore` metadata.

These references establish API semantics. The checked-in probe independently
tests the specific local manager behavior; neither substitutes for the remaining
production crash/queued-operation acceptance.
