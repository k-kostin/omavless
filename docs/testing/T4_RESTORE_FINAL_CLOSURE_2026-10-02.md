# T4 inactive restore final-closure candidate — 2026-10-02

This follows the terminal-receipt, replacement-slot and fixed-artifact
retirement drafts. It is an internal Rust candidate only: no installed
restore button, CLI/IPC method, startup call, service action or VPN effect.

The previous retirement result deliberately kept
`restore-finalization.pending` as a startup/mutation fence. The new **separate**
closure step first proves that every fixed staging member, decision journal
entry and replacement slot is absent. It then rechecks the same durable
terminal receipt, exact Rust owner generation, disconnected/idle native owner,
fresh absence of owned host activity and the live pair's expected digests
under one migration lease. Only then does it unlink the final receipt and
synchronize the state directory. Any surviving artifact, changed live pair,
owner/desired/host/queue drift or failed check retains the receipt.

Synthetic tests cover committed and aborted restores, refusal before cleanup,
surviving slot, changed live pair, late owner-gate refusal and SIGKILL at the
last unlink/sync checkpoints. On abrupt process loss the observed result is
either the intact receipt with a valid terminal pair or a fully cleaned state
without the receipt; no test touches real profiles. The native-owner
composition test additionally confirms that the receipt blocks normal
mutations until this explicit final step and that foreign visible VPN counts
are neither stopped nor treated as OmaVLESS-owned.

This does **not** make backup/restore available to users. Whole-flow startup
and crash recovery, interface/IPC semantics, packaged/VM acceptance, review of
all filesystem fault and concurrent-replacement cases, and a product decision
about handling an ambiguous post-unlink result remain open. The test simulates
process loss, not a physical power cut. A failed directory sync may report an
ambiguous result even if the receipt is no longer named; no success is claimed
in that case. Do not expose this internal call before those gates are closed.
