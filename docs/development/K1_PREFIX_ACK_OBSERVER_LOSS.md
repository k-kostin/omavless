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

CPU source checkpoint, 2026-10-03: tested code is
`ace50c0a0401ce54600b9457f8cd2df578918f7a`. The ordinary netguard suite passed
207 tests, with 33 opt-in tests ignored, including doctests. Formatting and
strict all-target crate clippy passed. The full source suite passed 503 tests
with two existing skips, 93-link documentation navigation and native/QML checks.
Its first HOME-temp invocation had three unchanged native-V0 private-file test
errors because the HOME ancestor contains a `.git` marker. The source-only rerun
used an owned outside-Git temporary directory and preserved that privacy guard;
no test assertion or HOME marker was changed. The full workspace Rust script
passed with 2,009 successful test invocations, zero failures and 71 ignored
invocations across its ordinary/feature/serialized suites (not a unique-test
count), all 12 terminal checks, strict workspace/feature clippy, feature checks
and two-case parity. Exact final documentation-head CI remains a separate gate.

The frozen x86_64 test executable has SHA-256
`8538bdab3f08ca86b108ca6c0db4fee3d960266ba3d62b25f45d696584e94362`.
It was copied outside Cargo and has no file capabilities. It is a test artifact,
not a package or installed helper. Later documentation changes do not imply
execution of modified code.

Actual VM execution passed on 2026-10-03 under an exclusive development VM
lease: 20 independent invocations, each validating all six child receipts,
gave 120 actual BEGIN/operation observation-loss scenarios. Every invocation
reported exactly one aggregate `scenarios=6` receipt and one passing ignored
test. These are deliberate observer losses after exact valid kernel ACK
decoding, not kernel loss or a timer-only refusal.

The reviewed preservation guard has SHA-256
`f2ec8bbf15bc5717904e1dc5116a383ab310e65533ea90d6ea417449549054fb`.
All four canonical private file hashes, user service/PID and executable,
outer namespace, core/TUN inventories, resolver and resolv.conf remained
unchanged. Every address and IPv4/IPv6 route/rule field matched except the
explicitly reviewed nonincreasing valid/preferred address lifetimes.
The private artifact archive, containing all 20 aggregate receipts, raw before/after
baselines, exact guard and frozen executable, has SHA-256
`5fc978291aee3ed62e176843bd9ea7af3026360f3a24a727749a3b26f8ac2c5a`.
No raw private baseline is committed. After durable retention, only this
fixture's guest executable and verified-empty temporary directory were removed;
private receipts/baselines were retained and the lease released.

Use a frozen executable outside Cargo, private home target/temp roots and
exact code/artifact identity; never rebuild a running self-reexecution binary.
Ordinary CPU tests alone do not prove kernel ACK emission or consumption.

Production still has no EffectPort or canonical namespace authenticator.
Synthetic epoch/Canonical fixture labels remain untrusted local history.
Reviewed safe namespace APIs, trusted launch/no-switch authority, nft continuity,
durable orphan adjudication, installed package/service/runtime integration,
core mark/DNS/firewall behavior, power-cut/boot and physical-host gates remain
open under [KILL_SWITCH](../roadmap/KILL_SWITCH.md). This introduces no privileged
IPC, normal namespace adoption, dependency or installed caller. No main/RC
merge, product activation, release or marketplace publication is authorized.
