# K1 BEGIN and operation ACK observation loss

This test-only continuation starts at sealed composition [#569](https://github.com/k-kostin/omavless/pull/569),
`1bbd358e53b99b5aa6156817fc130b17a7429d0e`. The earlier
[END observation-loss evidence](K1_END_ACK_OBSERVER_LOSS.md),
[real receive truncation](K1_REAL_RECEIVE_TRUNCATION.md) and
[creator lifecycle](K1_CREATOR_LIFECYCLE_RESEARCH.md) keep their own tested heads.
No source correctness defect or pre-fix failing regression is claimed.

## Audited guarantee and gap

The actual creator's `BatchReplies` requires BEGIN, every operation and END.
The conditional delete collector requires all three ACKs. Post-send failure
seals the retained session before ordinary readback; pending LockedState records
prevent fresh observations or reopened writers from granting another effect.
Existing actual observer-loss coverage consumed END after the accepted prefix.
Existing FullVpn pure missing-reply coverage uses a different GETGEN/barrier
decoder, which deliberately excludes BEGIN. It does not exercise a lost BEGIN
or operation observation followed by actual END delivery on these effect paths.

The new private `cfg(test)` observer consumes exactly one structurally valid
zero-error BEGIN or operation ACK from successful real `recvmsg`, validates it
against the actual outgoing request header and recipient port, and withholds
only that frame from the existing collector. Sender port/groups must be zero,
receive flags empty and the success ACK exactly 36 bytes. Every non-target
frame, including END and any coalesced neighbors, passes byte-for-byte with
its actual metadata. The test observer never creates or alters an ACK on the
effect path. Constructed frames belong only to explicitly pure tests.

The exchange cannot finish on loss alone. It must first accept every non-target
ACK, including END, remain nonfailed/non-generation-refused, and have exactly
the targeted ACK missing. Only then does it record one complete fault receipt,
poison the collector and return uncertainty. A duplicate target cannot redeem
the loss or be consumed twice. A missing remaining reply, malformed frame,
timeout or earlier refusal cannot produce the successful scenario receipt.
This is observer withholding after real receive, not kernel ACK dropping,
network loss, timer refusal, receive truncation or post-readback adapter loss.

## Deterministic coverage and actual fixture

Pure tests omit every BEGIN/operation position for both create and atomic
replacement, in forward and reverse receive order. Delete covers BEGIN and
operation loss in all six reply orders. Tests require END/remaining delivery,
permanent closure against subsequent ACKs, coalesced adjacent-frame preservation,
exact header/metadata validation and one-shot behavior. Strict receipt tests
reject missing/duplicate/wrong-case receipts, altered effect/retry/observation
counts, extra suffixes and invalid UTF-8.

The opt-in actual gate has six fresh loopback-only user/network namespace
scenarios: create, replace and delete, each losing BEGIN or operation index 1.
For replacement that operation is deletion of the old handle inside the same
atomic full-policy replacement. Create index 1 is exclusive table creation;
delete index 1 is its sole table-handle deletion.

The gate reuses real LockedState pending writers, retained socket descriptor/
inode/port and namespace checks, exactly one additional attempted mutation,
repeated Status/Arm/Disarm refusal, unchanged private record bytes, fresh
read-only presence/changed replacement-handle/absence observations, writer
reopen and zero-effect new-creator refusal. Every child must emit exactly one
case-specific receipt with `consumed=1 remaining_delivered=1 effects=1 retries=0
reopened_effects=0`; the parent rejects any missing, altered or extra prefix-loss
receipt. Namespace teardown remains fixture cleanup.

```sh
OMAVLESS_K1_PREFIX_ACK_LOSS_VM=1 frozen-netguard-tests --ignored --exact \
  kernel_observer::creator_lifecycle::tests::real_prefix_ack_observer_loss_in_disposable_vm \
  --nocapture
```

## Gates and limits

Actual VM execution is **pending**, requiring an explicit exclusive development
VM lease and reviewed preservation guard. Ordinary CPU tests do not prove that
the kernel emitted or the observer consumed any ACK. Use a frozen executable
outside Cargo, private home target/temp roots and exact code/artifact identity;
never rebuild a running self-reexecution binary.

Production still has no EffectPort or canonical namespace authenticator.
Synthetic epoch/Canonical fixture labels remain untrusted local history.
Reviewed safe namespace APIs, trusted launch/no-switch authority, nft continuity,
durable orphan adjudication, installed package/service/runtime integration,
core mark/DNS/firewall behavior, power-cut/boot and physical-host gates remain
open under [KILL_SWITCH](../roadmap/KILL_SWITCH.md). This introduces no privileged
IPC, normal namespace adoption, dependency or installed caller. No main/RC
merge, product activation, release or marketplace publication is authorized.
