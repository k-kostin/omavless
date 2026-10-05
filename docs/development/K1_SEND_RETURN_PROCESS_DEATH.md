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

Tested code is `481a14c0a43a40bf66077e7ac1d185307e15c7b5`; this evidence update
changes documentation only. Gates on that code passed: full Rust 2029 successful
test invocations / 75 ignored, strict workspace and feature clippy/check, parity
2 cases; source 512 tests / 2 expected skips. The exact full-workspace test ELF,
frozen outside Cargo, independently passed 174 ordinary tests / 16 ignored.

Two separately approved disposable-VM invocations each passed all three fixed
selectors (**2 × 3 = 6 cases**). Each actual writer was killed after its sole
target send returned and before its first receive. Both invocations observed
create and replace as `present_untrusted`, delete as `absent`; these are actual
observations, not a prediction from send return. Every reopened operation
refused, with unchanged durable Pending records, zero new effects and no retry.

Both invocations passed exact typed/Rust receipts, owned-group quiescence and
empty scratch checks. All eight canonical preservation categories were unchanged:
private files, user service/PID, executable, namespace, core inventory, TUN
inventory, resolver and resolv.conf. Full IPv4/IPv6 address/route/rule comparisons
allowed only verified numeric nonincreasing address lifetimes. No canonical
runtime/package/network changes, third invocation, blind retry or stage cleanup
occurred. The VM lease was returned; independent retained-archive review passed
46 safe members, both stages, six selector receipts and unchanged stage-1 bytes.

Exact immutable artifact hashes:

| Artifact | SHA-256 |
| --- | --- |
| Executed workspace test ELF | `3657488fb77a09a70f4e9861f37bbe6e3900678349b93bf5ccb457acc4a568d6` |
| Fixed supervisor | `05e0c73f1a399763e540f924728b3f5b9f0725da1651bd2477b742a3fae0d859` |
| Strict guest guard | `2feed815e0c5eb22abfb1ef75bd2a965dd7ddae882e95891b8d8e1bbc7a53c7b` |
| Combined private evidence archive | `ebfcaa2ff5740b7b601e1fa50256bedca854a3fae04ad1d33f76d491afd0b95f` |

The supervisor was corrected before execution: the first uncertain WNOWAIT
query permanently quarantines its anchor without querying again. Success and
cancellation reap only through exact-PID raw `waitpid`, never the `Popen.wait`
ECHILD-to-zero fallback. Unknown/zero/wrong-PID/nonterminal reap outcomes forbid
further query, signal, reap or command launch. Fourteen supervisor tests and
three exact receipt/network guard tests passed. Earlier wrapper evidence did
not cover these specific uncertainty holes and is not retroactively upgraded.
Raw logs and archive remain private, mode 0600 under mode-0700 parents.

At this report cut, both package CI jobs passed; the CI test job remained
pending. Local gate success is not a claim that pending CI passed.

This supplies no installed ownership, nft-subsystem continuity, safe namespace
API adoption, canonical system-manager launch, automatic orphan adjudication,
root service/package integration, reboot/power-cut or physical-host acceptance.
Missing policy means protection absent/unknown, not protected merely because
mutation refused. No main/RC merge, activation, release or completed K1 is implied.
