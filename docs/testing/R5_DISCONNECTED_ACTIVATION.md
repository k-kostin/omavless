# Explicit disconnected native activation

This R5 checkpoint exposes exactly `omavless cutover activate`. It admits a
disconnected legacy installation and invokes the existing cutover transaction,
fixed production host and generation-fenced frontend selector. It adds no IPC
method, caller paths, service names, shell/JSON input, force flag or rollback
shortcut. Python remains the installed owner until successful execution and
the retained rollback/oracle afterward. This checkpoint does not prove R5/R6,
login activation, connected adoption or provider interoperability.

## Admission

The running executable must be the root-owned, non-writable-by-group/other
`/usr/bin/omavless`, with the same device/inode as `/proc/self/exe`. The fixed
`/usr/lib/systemd/user/omavless-runtime.service` must be a root-owned ordinary
file matching the exact checked-in unit embedded when building that binary.
Integration with a changed package unit requires rebuilding the binary.
`OMAVLESS_HOME` overrides refuse. The installed host gate must verify that the
coordinator and user manager share the same HOME/XDG runtime and state roots.

Under the shared migration lease, activation requires legacy ownership,
repeated strict process/TUN inventories, both fixed services inactive with zero
MainPID, and absent legacy/native controllers and native control socket. Any
unavailable inventory refuses. Both units must report disabled startup, no
drop-ins and no pending daemon reload; the runtime fragment must be the fixed
packaged path. This operation never enables or disables a unit.

The complete private store must validate, have explicitly configured disabled
startup, and require no compatibility-pointer repair or missing-profile pruning
when the candidate reconciles disconnected state. Existing desired state must
be valid and disconnected. A login receipt or interrupted-preset barrier
refuses. Template mode and desired-generation capacity are checked before
preparing ownership. Private data is neither printed nor repaired to force
admission. Use the existing legacy operations and `store-compatibility` guidance
to resolve ordinary prerequisites before retrying.

## Failure and recovery contract

| Boundary | Result and recovery |
| --- | --- |
| Invalid command, package mismatch, unsafe/incompatible store, connected host, startup enabled/unconfigured, unit mismatch, receipt/barrier, incomplete observation | Refusal before ownership/service/bridge effects. Existing helpers may prepare the fixed operational lock/state directory. |
| Desired staging or candidate start/bootstrap/hello/status/bridge failure with unchanged preparing marker | Existing reverse compensation returns the bridge to legacy, stops native service, verifies strict emptiness and acquires any existing native owner lock, restores exact original desired bytes or original absence, then commits verified legacy ownership. |
| Desired publication reports an error after replacing the file | The host retains the exact candidate before publication; compensation can identify and restore that candidate. Unknown current bytes are never overwritten. |
| Native stop, socket/controller/process/TUN absence or owner-lock proof fails | Manual recovery; no legacy restart or ownership restoration. |
| Lock handoff cannot be reacquired, marker diverges, or compensation cannot be verified | Manual recovery; never guess a replacement owner. |
| Marker write reports an error | Existing coordinator rereads durable state: exact target is committed, unchanged source permits the corresponding compensation, ambiguous state requires manual recovery. |
| CLI dies while preparing | Durable preparing ownership keeps both mutation paths closed. No automatic resume/rollback exists and no claim of crash-safe automatic recovery is made. |
| Commit succeeds but client loses output | Ownership is already Rust. Read the committed launcher target and runtime status; do not repeat activation or infer rollback from missing output. |

The desired snapshot is private, bounded and **in memory**, not a durable backup
or recovery journal. The existing preparing-marker contract supplies crash
refusal, not reconstruction of a lost snapshot. Operator recovery must verify
the exact private state, native service/process/controller/TUN absence, and
frontend generation before choosing a separately reviewed recovery action.
There is deliberately no command that deletes a marker or blindly restores
legacy ownership. Connected adoption remains unexposed: its existing internal
config-regeneration and legacy-restart compensation requires separate review
and exact host evidence.

## Validation and remaining host gate

Synthetic tests cover exact command/unit facts, activation success using fixed
service/private-socket doubles, startup/receipt/preset/TUN refusal, the existing
transaction fault matrix, exact desired formatting/absence restoration,
unrecognized replacement refusal and an externally held native owner lock.
The pre-existing language-neutral ownership/transaction contract is the
reference for this composition; no new Python behavior is substituted.

No real cutover, service action, installed frontend acceptance or Cargo gate was
run by the activation helper agent. The integrating agent owns compilation,
full tests and exact-head installed acceptance. Required host checks include
package/unit identity and environment agreement, actual disconnected activation,
same-instance mutation promotion, QML and CLI ownership, service restart,
disconnect/cleanup and the accepted failure/recovery matrix. Keep private
profile/subscription/config data and process environments out of public logs.
Python cannot be removed based on these synthetic tests.
