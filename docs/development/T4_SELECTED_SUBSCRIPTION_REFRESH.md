# Cancellable selected-subscription refresh

Independent T4 source slice under the October25 owner decision. Installation
and integration remain ordered after Restore739, T3 and WG/AWG. This is not
activation of the dormant schedule, OS-resume or automatic-retry prototypes.

## User outcome and ordinary entry

An explicit selected-feed refresh can outlive a unary exchange without losing
its cancellation/status handle:

```text
omavless hello
omavless subscription list
omavless subscription refresh-selected INSTANCE_ID OPERATION_ID SUBSCRIPTION_ID REVISION
omavless operation get INSTANCE_ID OPERATION_ID
omavless operation cancel INSTANCE_ID OPERATION_ID
```

The caller uses the current hello instance, a new operation ID, the selected
existing subscription ID and the current revision. The fixed CLI maps to the
separately advertised `subscriptions.refresh_selected` v1 method. Its exact
required parameters are `instanceId`, `operationId`, `subscriptionId` and
`expectedRevision`. No URL, deadline, path, list, provider code or concurrency
setting is accepted. Existing `subscriptions.refresh` and
`subscriptions.refresh_all` requests and semantics remain unchanged.

The normal same-UID framed socket, committed Rust owner, unified operation-ID
ledger, single supervised worker, four-fetch pool and existing provider/whole
job deadlines remain authoritative. Admission snapshots only the selected
subscription under the migration lease. Work fetches only that feed outside
the dispatcher/store locks. Completion rechecks ownership, global revision,
selected identity/URL/timestamp and cancellation before the existing atomic
store publication. Other subscriptions and their managed profiles remain
unchanged. No connection/lifecycle action is introduced.

## Completion, cancellation and uncertainty

The existing credential-free long-operation projection identifies the new
fixed method with total1; success advances the revision once. Safe import
summary/reasons use the same bounded representation as refresh-all. No target
ID, URL, profile content or remote error is added to the projection.

Poll/cancel take only the original instance and operation, not a target. An
exact repeated start returns the original projection without a second GET;
changing the target or revision under the same ID conflicts. Accepted cancel
before the commit fence prevents the store write even when an in-flight GET
later returns successfully. A cancel after that fence cannot undo success.
Status/other peers remain responsive during the bounded fetch.

Lost/incomplete/malformed start reply returns CLI exit73 (unknown), with no
automatic resend and a reminder to retain the original operation. A missing or
unsafe socket before exchange remains the existing ordinary refusal. The caller
queries the original handle. A daemon restart does not resume the operation:
old-instance get/cancel returns not_found, not a successful/failed/cancelled
receipt. Renegotiating a new instance must not automatically resubmit. Ambiguous
store I/O retains the existing manual-recovery barrier; no reset or recovery
authority is added. Terminal retention/eviction is unchanged.

## Source completion test

The semantic CLI parser feeds the actual RuntimeServer private Unix socket and
production HTTP transport against synthetic loopback. The affected test covers
selected success, cancellation, provider error, owner withdrawal, replay,
sibling preservation and the unchanged refresh-all cases. Additional controls
cover exact request shape/digest, stale/missing target, unsafe store, shared
admission, restart/old-instance refusal and no private projection. Full affected
domain/runtime regressions retain real crypto and serial deadline cases;
strict lint/format and independent Standards/Spec reviews bind the exact patch.
The actual CLI executable additionally exchanges canonical framed projections
with an inert same-UID peer: fixed start/get/cancel shapes, lost-reply exit73 and
no second connection. This peer test is not native-owner/store proof; the
separate actual RuntimeServer/HTTP test provides that source composition.

This is source/inert evidence, not installed native-core/provider/VM proof,
whole T4, scheduled refresh, TUI/QML controls, final CI or release admission.
The ordinary CLI already provides get/cancel; no new helper/driver is needed.
