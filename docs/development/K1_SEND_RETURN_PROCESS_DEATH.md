# K1 process death after send return, before receive

This test-only continuation starts at #584 report
`dac65a9ced60a43c2e65833d9930b6b06c3e1d37`; its tested shared observation code
and retained evidence remain unchanged. No production executor, socket effect,
dependency, helper, namespace authority or runtime policy is added.

## Distinct failure boundary

Existing creator lifecycle SIGKILL cases occur **after complete raw ACKs and
readback**, before adapter return. Receive-truncation and ACK-observer-loss cases
withhold observations but keep the writer alive through classification. Neither
is writer death immediately after its one send returns and before its first
receive. This fills that bounded process-lifetime gap in the
[crash disposition contract](K1_CRASH_DISPOSITION.md); it does not claim death
inside kernel commit processing or solve kernel/filesystem atomicity.

`OneShotSendCut` exists only under `cfg(test)`. It is armed for a fixed create,
replace or delete, retained socket and holder-owned private fixture. Both
existing senders invoke it after successful full-length `sendto`, before
`recvmsg`, ACK collector delivery or readback. It requires the expected fixed
batch shape, zero accepted ACKs and an incomplete/unpoisoned collector. An armed
invocation is consumed even on mismatch. Default is inactive. No generic
callback, caller command, public checkpoint or normal API is introduced. Fixed
checkpoint counts describe the targeted transaction, not the preliminary
successful Arm used to set up replace/delete.

The hook publishes a fixed operation-tagged private checkpoint and waits for
the holder. Its fallback timeout refuses, never receives or retries. The
checkpoint is **not an ACK or commit receipt**. The holder verifies exact bytes,
owner/mode, corresponding durable Pending phase and Busy root-state lock while
the writer remains alive. `UnreapedWriter` polls with WNOWAIT, then SIGKILLs and
reaps that exact still-owned child. Unknown child ownership cannot authorize a
signal/reap; outer owned-group supervision must classify unsettled failure as
NONPASS. Ordinary process tests exercise the real child anchor and cleanup
without namespaces or networking.

The new parent path does not use the older lifecycle `ChildGuard`. It requires
the external supervisor's process group, creates no nested group, retains the
holder with WNOWAIT, drains bounded nonblocking stdout/stderr through EOF, and
checks exact group membership before reaping. The successful holder checks no
writer/descendant remains before fixture removal. Any new-case unwind retains
its private Pending/checkpoint directory. Unknown wait/proc ownership ends the
case without catch-and-clean; the external supervisor must contain the owned
group and retain failure evidence. This does not retroactively strengthen the
older, separately tested lifecycle harness.

## Three fixed disposable cases

Each create/replace/delete case has a fixed ignored parent selector. The existing
holder creates a fresh user/network namespace, pins and checks the distinct
parent namespace, verifies loopback-only isolation and the fixture UID, and
validates its exact private path before any effect. The synthetic
`NamespaceObservation::Canonical` seam stays test-only; it does not attest the
host namespace.

After SIGKILL, the holder compares durable records byte-for-byte with Pending
and performs a fresh complete **untrusted** observation. Absence is reported only
from the successful reader. A present result must retain the persistent table
with released OWNER flag and independently match raw FullVpn rules; it remains
`OtherUntrusted`, never owned or adopted. Parser errors are errors, not missing
policy. A send return does not select the observed outcome or prove commit.

Regardless of absence/orphan outcome, fresh Status, Arm and Disarm all refuse,
with unchanged durable records and zero new effects. The exact sanitized receipt
names the operation and observed untrusted result, one target send, zero target
ACK/readbacks before death, one killed writer, zero retries and zero reopened
effects. Only validated fixed receipts are forwarded by the parent; raw
kernel/private data remains outside Git.

Run only after exact-source review and an explicit exclusive disposable-VM
lease. Fixed selector suffixes under
`kernel_observer::creator_lifecycle::tests::` are:

- `real_send_return_create_death_in_disposable_vm`;
- `real_send_return_replace_death_in_disposable_vm`;
- `real_send_return_delete_death_in_disposable_vm`.

Each requires `OMAVLESS_K1_SEND_RETURN_VM=1`; a frozen test ELF and fixed
supervisor/strict before-after guest guard are required. Ordinary tests skip
these ignored selectors. Any NONPASS stops later cases; no blind retry,
canonical service/package/network mutation or cleanup follows failure.

## Evidence and limits

Final source/full Rust gates and frozen identities are pending. The new VM cases
have **not run**. Earlier kernel evidence does not cover this new crash boundary.

This supplies no installed ownership, nft-subsystem continuity, safe namespace
API adoption, canonical system-manager launch, automatic orphan adjudication,
root service/package integration, reboot/power-cut or physical-host acceptance.
Missing policy means protection absent/unknown, not protected merely because
mutation refused. No main/RC merge, activation, release or completed K1 is implied.
