# Fixed client four-exchange developer scenario

Opt-in executable `omavless-netguard-client-scenario`, enabled only by
`netguard-client-scenario`. It is not packaged, registered, a privileged launcher,
or native K1 activation. Running it DOES request real policy mutation; compilation
and its pure mocked unit tests do not. No service/client execution is part of
the source gates.

The executable takes no arguments and refuses effective root. ROOT must first
admit a disposable clean-baseline VM with its real PID1, canonical namespaces,
unchanged v2 service, trusted package custody and actual enrolled nonroot account
with package supplementary group membership. This program does not establish
those prerequisites. It does not install, enroll, start, inspect or repair a
service, core, group, namespace, file or nft table.

One lazy FixedClient performs Status, Arm Full generation 1, Disarm generation 1,
then Status. Each reply must equal the current-policy Verified response with,
respectively, Disarmed(null), Armed(1), Disarmed(Some(1)), Disarmed(Some(1)).
Every error or nonexact positive stops immediately. There is no retry,
compensating Disarm, subsequent diagnostic Status, alternate owner or socket path.
The client retains its existing unknown-outcome behavior. Process termination
still closes kernel descriptors; this executable is not a surviving recovery
custodian. Panic is terminal with no followup operation.

Output is one finite public literal after the operations finish; exit 0 means
only that all four exact responses passed the existing client checks. Exit 2
identifies invocation refusal, one of the four refused phases, or panic/unknown.
There is no arbitrary response/error text or private data output. The client's
two-second per-exchange deadlines apply; output delivery and OS scheduling are
not a hard whole-process wall-clock deadline. Use locally captured bounded output
and out-of-band VM control: Arm can block the network carrying SSH. Do not use
network connectivity to sequence these four operations.

The fixture never starts a core. Thus Disarm here is authorized fixture policy
cleanup with no core by construction, not proof of native lifecycle quiescence.
Success is only fixed-client/v2-service boundary evidence. It says nothing about
packet enforcement, transport/resolver mark coverage, physical NIC/suspend/boot
acceptance, readiness or a running VPN. Failure after possible Arm may leave
protection active and requires ROOT's separately reviewed recovery decision.

Source gates (locked/offline): build this binary; run its three pure mocked unit
tests; strict Clippy for this binary/tests; formatting and diff checks. Tests cover
exact order, every transport-error cut, and 192 nonexact response/cut combinations
without invoking FixedClient, service, network or privileged effects.
