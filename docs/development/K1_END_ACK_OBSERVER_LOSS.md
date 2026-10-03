# K1 actual END ACK consumption at the observer

This bounded test-only slice starts at sealed [#563](https://github.com/k-kostin/omavless/pull/563)
`a6137e96d5ac5369af75491a7ef702e992b1c42d`. Its
[truncation evidence](K1_REAL_RECEIVE_TRUNCATION.md) and underlying
[creator lifecycle evidence](K1_CREATOR_LIFECYCLE_RESEARCH.md) remain separate.
Current-RC agent/workflow rules forbid automatic documentation merges; this
branch does not update main/RC or publish anything.

## Precise fault

Existing pure collectors already refuse missing END completion. No concrete
source bug or pre-fix failing correctness regression is claimed. This adds the
previously unexercised actual partial-delivery path: successful full-size real
`recvmsg` on the same retained netlink socket consumes a structurally exact,
zero-error kernel END ACK, but a private one-shot observer intentionally
withholds that **actual message** from the existing collector.

The receive metadata must be kernel sender port/group zero and empty receive
flags. The actual END sequence, recipient port, NLMSG_ERROR type, flags, exact
36-byte length, zero-error code and outgoing request-header echo are checked.
Other messages are delivered byte-for-byte with their actual sender/flags;
coalescing does not discard adjacent messages. No ACK is constructed, altered,
replayed, peeked or synthesized in the VM effect path. Constructed frames exist
only in explicitly labeled pure decoder tests.

At the loss point the existing collector must have accepted **every** real
BEGIN/operation ACK, be nonfailed and non-generation-refused, and still have no
END ACK or complete receipt. Only then is the local collector poisoned and the
operation returns uncertainty immediately. There is no timeout or second send.
The containing creator session is poisoned and its live ownership is withdrawn.
Ordinary conditional-delete `consume()` retains its existing external shape;
fault delivery is private and test-only.

This is deliberate loss **between real receive and collector delivery**. It is
not kernel ACK dropping, a malformed kernel reply, network loss, selective
kernel failure, receive truncation, timeout, or the separate post-ACK/readback
adapter-result loss. The test observer itself knows it consumed a valid END;
that knowledge is deliberately not delivered as an effect result and does not
repair the durable transaction. It does not establish canonical host authority.

## Three actual scenarios

Create, atomic complete-policy replacement and inventory/generation-conditioned
delete each use their existing retained-socket transactor in a fresh
loopback-only user/network namespace. Socket descriptor/inode/port and namespace
stay the same. Each requires exactly one additional attempted mutation and one
actual END consumed only after the complete operation-ACK prefix is accepted.

Real LockedState writers retain matching PendingCreate, PendingReplace with
the old handle, or PendingDelete with the old handle and Closed generation.
Repeated Status/Arm/Disarm on retained state, reopened private writer with the
same creator, and reopened writer/new creator all refuse without further effects
or record changes. A separate read-only socket observes actual FullVpn policy
presence/changed replacement handle or delete absence, but neither observation
redeems an acknowledgement or retries an effect. Namespace teardown is cleanup,
not orphan adoption or production recovery.

## Gates and limits

The gate is opt-in and requires an exclusive disposable-VM lease:

```sh
OMAVLESS_K1_END_ACK_LOSS_VM=1 frozen-netguard-tests --ignored --exact \
  kernel_observer::creator_lifecycle::tests::real_end_ack_observer_loss_in_disposable_vm \
  --nocapture
```

## Exact-source evidence, 2026-10-03

Immutable executed code:
`4aed4d35768a3bb68546706e9863d7aa01c3495d`. Frozen standalone test executable
SHA-256:
`f973e95cde47f8a312b87e31ba8adea82c8b657c1c47919a59d6f7517eb0a064`.
It was copied outside Cargo and reverified after guest transfer, with no file
capabilities. All Cargo work finished before self-reexecution; no rebuild
occurred during the VM suite. Previous frozen #560/#563 artifacts and their
branches/evidence were preserved; this slice used its own home target/temp.

Local gates passed: two focused exact-END filter tests; complete netguard 199
passed /32 opt-in ignored including doctests; strict all-target crate clippy and
formatting; source 503 passed /two existing skips, native/QML checks and 93-link
navigation. The complete current-RC Rust script passed with four test threads,
including serialized special fixtures, strict workspace/TUI clippy, terminal
fixtures and parity. Its repeated result lines total 1585 successful test
invocations, zero failures and 44 ignored (not a count of unique tests). Cloud
checks are independently attached to exact heads in
[#565](https://github.com/k-kostin/omavless/pull/565); local/VM success alone does
not assert a cloud check outcome.

Omarchy development VM x86_64/KVM, kernel `7.2.5-3-omarchy`, exclusive delegated
lease: all three new actual END observer-loss scenarios passed ten complete
repetitions (30 consumed real END ACKs, each after the entire actual accepted
BEGIN/operation prefix). The same frozen executable also passed all thirteen
predecessor lifecycle scenarios and all three actual receive-truncation
scenarios once. The lifecycle regression includes three actual SIGKILL cuts;
the thirty END losses are not SIGKILL, truncation or invented kernel errors.

All 46 scenarios passed one-invocation private before/after preservation:
unchanged parent network namespace, canonical **user**
`omavless-runtime.service` active/running at PID 86349, runtime executable and
private profile/desired/ownership file fingerprints. Exact deep JSON network
comparison preserved all interface identities/names/flags/MTU, address/prefix
fields, route tables, rules and metrics. Only confirmed decreasing numeric
`address[*].addr_info[*].preferred_life_time` and `valid_life_time` counters
differed; no other structure/type/key, route or process change was ignored.
Private values, hashes and raw network snapshots were not published.

Every namespace child was reaped; fixture scratch was empty after each gate.
The exact owned guest scratch, transferred binary/guard and private snapshots
were removed. Final no-process/no-scratch and unchanged user-service/PID/parent
namespace checks passed. The exclusive VM lease was explicitly released, with
no further guest calls. The immutable host artifact remains outside Cargo.
No parent-VM firewall/route/service/profile or primary-PC network action was
performed. Later evidence-only edits do not claim execution of modified code.

Ordinary tests do not establish actual kernel delivery. The synthetic local
epoch/Canonical model seam is still untrusted fixture history. Production has
no EffectPort, canonical namespace authenticator, helper installation or kill
switch activation. Reviewed safe namespace API/no-switch launch, nft continuity,
durable/orphan adjudication, installed package/service/runtime integration,
power-cut/boot and physical-host gates remain open under
[KILL_SWITCH](../roadmap/KILL_SWITCH.md). No privileged generic IPC, unsafe local
namespace shortcut, production dependency or package change is introduced.
