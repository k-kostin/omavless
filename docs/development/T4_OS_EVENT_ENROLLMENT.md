# T4 OS-event and explicit-Connect enrollment integration

Status: development candidate continuing the dormant maintenance composition
at `eb42ab9a45f027143ae48aa32092d547e7bb633c` (#697). No normal daemon
activation, installed recovery or physical sleep/NIC acceptance is claimed.

The source component at `3cf053e9dee5b30602d85ed91246712f8223dd8f` passed its
fifteen reviewed private-protocol fixtures. The completed-Connect component at
`92e227773c107ab6808fcfd5c7b966f97411929f` passed eight reviewed original-owner
fixtures. Those results belong to those exact component heads. The combined
backend adaptation received primary and independent review at
`7394c480375d01027e875d2390870d668c2b1033`. Its three new
real-private-bus/control-socket scenarios passed once: Connect-to-Ready and one
sleep/resume recovery; owner replacement without recovery/rearm; and source loss
while awaiting, preserving explicit Connect without granting Ready. Exact final
combined/full/CI results belong to the integration PR, not the component pins.

## Combined verification checkpoint

At `f8bac8439211170fe9adde7eef9bc9646a5e2ed5`, the complete Rust script passed:
default workspace/runtime (828 passes, 34 historic/developer ignores and the
separately passed helper), serialized DNS suites, TUI terminal, strict checks
and lint, the existing monotonic example, parity, and the explicit eight
enrollment / three combined-wire / fifteen source scenarios. An additional
all-three-feature workspace gate passed (runtime 829, the same 34 ignores and
one separately passed helper). The isolated source gate passed 499 tests with
two opt-in skips plus JS/QML/navigation checks. No ignored installed/physical
case was promoted by association.

The first three-case combined execution at `7394c480` used test ELF SHA-256
`c690dcd1efa90581f401f1e5a3116d96d19762f7cc5f7db7ce308749b99e2554`.
That executable and exact known-status logs are privately retained outside
rebuildable target storage. Later documentation-only component handoff imports
do not transfer a changed implementation's acceptance. Final hosted checks are
recorded on the integration Draft PR.

The initial hosted checkpoint `9a185ecb` did not pass all gates: the normal Rust
suite reported Busy instead of capability_unavailable at the final revoked-owner
read in an existing route fixture (827 other cases passed). Its precise hosted
contention source is unproved. The test-only successor `acf2eeaa` retains every
terminal/privacy assertion, deliberately verifies original-dispatcher contention
as Busy with no result/host calls, and retries only that fixed host's read-only
Busy refusals. It uses original stop/join supervision rather than a fixed request
count. The two-second Busy-retry budget is additional to existing bounded unary
I/O, not a hard wall-clock deadline. Primary and independent review preceded the
actual affected case, which passed once; normal runtime code is unchanged.
The separate native DNS-pair x86 build on the old head also failed while hydrating
public Go dependencies with HTTP stream errors, before package build/staging.
Those failed results are retained; successor checks are separate exact-head gates.

## Completion matrix

| Boundary | Required outcome | Evidence target |
| --- | --- | --- |
| Sleep source | Exact logind signal, original bus/unique service owner, initialization ordering and terminal loss | Real D-Bus messages on a disposable private bus; no system-bus calls |
| Network source | Kernel receive metadata, bounded complete datagrams, truncation/overrun/loss refusal | Fixed synthetic wire/receive fixtures; no host socket or kernel-origin claim from these fixtures |
| Source lifetime | Pending events and terminal loss participate in observation/effect checks; no source reconnect | Original mutable source and bounded continuity checks, not an empty downstream bridge |
| First permit | Only a newly completed, changed explicit Connect on the original coordinator can qualify enrollment | Actual private control-socket scenario; replay, NoChange, failure and events mint no permission |
| Durable publication | Ready is published/read back under the original migration lease after current target/source validation | Owned private files; pre/post publication uncertainty and subsequent loss refuse |
| Recovery | A due event consumes the original permit at most once; explicit Disconnect/Quit and newer intent win | Combined original-owner source/control/fault matrix |
| Restart and rollback | A new owner does not inherit old authorization; restored or missing records do not reset local uncertainty | Negative instance/receipt fixtures, not cross-restart rollback resistance |
| Real activation | Authentic installed bus/kernel acquisition, production binding and complete DNS/route/protection evidence | Pending separate exact-head integration; VM remains with its designated operator |

## Trust and serialization

OS notifications are hints to inspect existing desired intent. They contain no
profile, source URL, commands or authority to turn Off into On. Network names,
controller liveness and endpoint reachability cannot prove safe recovery.

The Linux source uses fixed logind semantics and routing-netlink notifications,
not a desktop- or NetworkManager-specific shared-domain requirement. Service
owner replacement, connection loss, malformed input and bounded queue/receive
loss revoke that original source. Netlink message sequence fields correlate
requests/replies; they are not an event-loss counter. Only receiver metadata can
attribute a datagram to the kernel. A local event sequence is bookkeeping, not
proof that every kernel notification arrived.

The source-currentness check is momentary. It cannot exclude a future signal
arriving after the final read, and cannot revoke an already admitted atomic
owner operation. The existing dispatcher, quit gate and migration lease define
the operation boundary. A queued Suspend must never be skipped merely because
an intermediate Unix pipe appears empty.

The combined Source holds the original HostEventSource directly, alongside the
retained legacy owned-stream fixture backend. All readable, next and strict
quiescence operations consult that actual backend. A forwarding pipe's emptiness
is not substituted for upstream currentness. The integration preserves the
private Weak-backed original Source identity and receiver-assigned hint sequence.
An outer enrollment drain deadline caps the source's ordinary per-call I/O
budget, including the final quiescence check; it is not renewed per frame.

## Permission provenance

New enrollment must begin before unrestricted startup reconciliation and must
not bypass the existing once-installed lifecycle guard. It begins without a
Ready receipt. A successful explicit Connect retains its original outcome even
if optional enrollment publication fails. Such a failure disables automatic
recovery; it is not rewritten as a failed user connection.

Completed-Connect authority is private, non-cloneable and bound to the original
owner, fully committed desired/store revision and source lifetime. It cannot be
constructed from a successful-looking IPC reply or from receipt absence. After
provisioning, missing/invalid/restored receipt state, terminal source loss and an
uncertain attempt cannot silently issue another permit. A failed recovery is
not an invitation to repeat it on the next notification.

The explicit Connect itself creates link/address/route notifications. First
provisioning therefore uses this fixed order: committed-Connect authority,
bounded drain of **NetworkChanged only**, a new complete ownership and binding
observation, final strict source quiescence, then create-only Ready publication.
Suspend, Resume, source/sequence loss or a continuously busy source refuse this
optional enrollment. A new frame after the fresh observation refuses it; there
is no second drain followed by publication against obsolete facts. Consuming
those network hints is bookkeeping, never the authority to create Ready.

Ready authorizes a future guarded attempt, not an immediate recovery or a claim
that networking is stable. Its owner-local epoch identifies that permission;
it is not an OS-provided trustworthy network identity. A future event still has
to satisfy the existing quiet period and fresh complete safety checks. No new
three-second lease hold or implicit connection retry is introduced by publishing
the first permit.

This integration does not claim production first-use, backup/downgrade or machine
power-loss rollback protection from ordinary private files. Those activation
guarantees and real `ResumeBinding` proof remain explicit separate boundaries.

## Primary contracts

- [Owner continuation](T4_NETWORK_RESUME_CONTINUATION.md)
- [Attempt receipt](T4_RECOVERY_ATTEMPT_RECEIPT.md)
- [Network-transition policy](../roadmap/NETWORK_TRANSITION_RECOVERY.md)
- [Execution policy](EXECUTION_POLICY.md)
- [Acceptance environments](../roadmap/ACCEPTANCE_ENVIRONMENTS.md)
- [logind interface](https://github.com/systemd/systemd/blob/main/man/org.freedesktop.login1.xml)
- [D-Bus specification](https://dbus.freedesktop.org/doc/dbus-specification.html)
- [Kernel netlink contract](https://www.kernel.org/doc/html/latest/userspace-api/netlink/intro.html)
