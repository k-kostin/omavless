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

## Actual v2 boundary: scoped development PASS

On 2026-10-06 ROOT executed the reviewed fixture on a disposable pre-K1-baseline
VM overlay, preserving the previous working disk/NVRAM. Tested client source:
`3d217134e0343eec8cdf764efe305074240a73d0`; service source:
`9ed6b286fc94536670905cbee1c82c5a1f02fe85`. This documentation-only successor does
not change or rebuild either tested executable.

The client artifact (10189360 bytes) has SHA256
`4323b577c5f7da741582c0e82a84af626d01b46c576b63a5f24f254600e5d516`;
the service artifact (56368800 bytes) has SHA256
`f060fd6058d784e29e118e4c5906331c80e6ede19ec20e6ac3d64a8ceded4ad5`.
Unchanged unit SHA256:
`7fc59fa8ae3915a65a0470909b7ff7d908ecca26f7c5fbfe1630e47d4368a94c`.

Following independent source reviews, ROOT separately selected baseline admission,
provisioning, current-invocation startup verification, one enrolled-user run and
the prescoped observer; every final phase returned original exit 0. One SAME
FixedClient completed the exact four exchanges without retry or extra queries.
Its exact success capture was 36 bytes, SHA256
`d32fb31e6b9934e01b855f95cf6f6cdee85112b17494310d11adfd0a0c2bea7b`;
stderr was empty and the exit capture was `0` plus newline. The complete final
nft dump showed the reserved table absent and full foreign ordered firewall
structure equal to its baseline, excluding only live packet/byte counter values.
This is endpoint equality, not uninterrupted preservation proof. Initial read-only
preflights refused incorrect empty-firewall assumptions before any provisioning;
the admitted fixture preserved existing stock UFW rules without adopting them.

[Public execution evidence](https://github.com/k-kostin/omavless/pull/676#issuecomment-6004889916).
This is agent-attended fixed-client/actual-v2-service boundary evidence only.
No core was started; native protected lifecycle, packet enforcement, mark and
resolver coverage, physical NIC/suspend/boot, released-pair adoption and product
activation remain unaccepted. No physical-host networking was changed. Neither
the old v1 service nor unexecuted protected preparation inherits this PASS.
