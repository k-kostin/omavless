# K1 inactive transaction coordinator

This Draft stacks on the [root-state checkpoint](K1_ROOT_STATE.md). It composes
the existing fixed transaction planner with the durable root-state store and a
synthetic kernel port. There is no production caller, root executable, socket,
service, nft subprocess, firewall change or package artifact. The feature
remains unavailable to users.

The coordinator holds the store's exclusive lock from initial observation
through all effects and final verification. It obtains the marker itself,
never from a peer request. A storage read error (including an unsafe directory,
pending stage or lost directory binding) refuses all mutation; only a decoded
invalid document in a still-trusted store may enter emergency reconciliation.
The kernel port supplies a complete table snapshot and a separate boot/netns/
handle identity which a future root adapter must establish from an independent
durable ownership receipt. The fixed table name, policy equality, root-state
marker and an nft command's exit code are not ownership evidence.

Before each effect the coordinator re-reads marker, table and identity. Create
requires proved absence. Replace/delete pass the exact independently proven
identity to a future atomic conditional executor. Create/replace must return
the newly published receipt identity; subsequent readback must match that
identity and the complete expected policy. Persistence is acknowledged only
after the real store write and readback. Closed is durably written before any
delete; Armed is written only after policy verification. Final success requires
a fresh marker/table/identity observation matching the transaction response.
Any uncertain effect or changed final observation poisons this coordinator
instance and requires a new locked reconciliation from independently proven
facts. It never silently replays an uncertain effect in the same instance.

Synthetic tests exercise durable arm/disarm and closed-generation replay,
exclusive store lock, storage change before Closed prevents delete, changed
ownership, missing ownership receipt, create refusal, final-readback drift,
decoded invalid-state emergency, and refusal when storage is unsafe or rebound.
They inject storage changes during the kernel observation, including before
create and during final status, to verify the second marker read. These
tests establish orchestration behavior only, not kernel enforcement.

The missing root adapter must independently solve durable table-receipt
publication, uncertain create/replace/delete outcomes, same-boot/netns/table
identity and reboot recovery before it implements this port. Authenticated
peer enrollment, provisioning, service/boot ordering, Mihomo mark integration,
physical-NIC and DNS/suspend acceptance, package lifecycle and explicit owner
activation remain separate gates. No existing test in this Draft is a claim of
a working kill switch on the host.
