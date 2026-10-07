# K1 crash/orphan disposition proposal

The [developer service core](K1_SERVICE_CORE.md) implements the conservative
no-adopt disposition: cold Live/Pending/Armed state returns bounded errors and
does not mutate policy/records. Its root emergency command is confined to the
still-live original creator and cannot recover a cold orphan. The historical
design below remains the owning provenance argument; product orphan
adjudication/healthy recovery and actual crash packet evidence remain open.

The default-off [cold-bootstrap successor](K1_COLD_BOOT_RECONCILIATION.md) is a
distinct SOURCE experiment: different-boot Armed/terminal-Live plus complete
current absence may earn a NEW exclusive FullVpn creator before READY and
NetworkManager. It does not adopt or remove an old/present table, manufacture
Closed, repair pending storage, reset high-water or change the default service
classifier. Its installed reboot/ordering and physical gates are not accepted.

Status: inactive design and synthetic counterexample tests, not accepted recovery
policy or a root adapter. Follows [receipt storage](K1_RECEIPT_STORE.md). No
production behavior, root command, service, nft operation, VM configuration or
kernel effect changes. K1 remains unavailable; ambiguous state refuses mutation.

## What the existing evidence can and cannot prove

The Linux nftables transaction and filesystem receipt publication are separate
commits. A successful kernel change can precede a crash, lost reply or failed
receipt fsync. A durable pending receipt proves intended work, not execution.
The new file store makes interrupted *filesystem* publication conservative; it
does not close this kernel/filesystem gap.

Primary-source checks:

- Linux `nf_tables_newtable` assigns a per-subsystem table handle and stores the
  creating netlink port ID for owner tables. `nf_tables_valid_genid` provides a
  nonzero-generation conditional transaction under the commit mutex; this is
  concurrency control, not application identity. See the version-pinned
  [kernel implementation](https://github.com/torvalds/linux/blob/v6.18/net/netfilter/nf_tables_api.c).
- The same source's `nft_rcv_nl_event` handles socket release: owner-only tables
  are removed, while persistent tables lose the owner flag. Subsystem per-net
  initialization resets its state. Matching boot/netns/handle therefore does
  not independently attest uninterrupted nft subsystem/table provenance.
- The [nftables manual, table flags](https://www.netfilter.org/projects/nftables/manpage.html#TABLES)
  documents orphan adoption with `owner,persist`. Kernel permission to acquire
  an orphan is not proof that it belonged to this application. `owner` alone
  is unsuitable for K1 because protection must survive helper/socket death.
- An open namespace FD pins its lifetime, according to
  [namespaces(7)](https://man7.org/linux/man-pages/man7/namespaces.7.html).
  A receipt's namespace bytes are not that open FD or trusted launch provenance.
  Reboot ends the old kernel lifetime; a current-boot observation is still needed.
- [fsync(2)](https://man7.org/linux/man-pages/man2/fsync.2.html) distinguishes file
  durability from containing-directory durability. Neither primitive transacts
  with nftables. The fixed pending guard is retained across the receipt rename.

The kernel source is a review baseline, not proof of installed-kernel equality.
The [separate VM experiment](K1_KERNEL_CAPABILITIES.md) checked namespace IDs,
generation/handle conditions and rollback on kernel `7.2.5-3-omarchy`; it did
**not** check owner/persist lifetime, module unload/reload, reboot or power loss.
This change runs no new host/kernel experiment.

## Indistinguishable histories

These are architecture counterexamples, not assumptions that privileged foreign
software is malicious. Other root firewall managers and interrupted upgrades can
legitimately create/remove objects; they must not accidentally be adopted.

1. **PendingCreate + table present.** Our exclusive create may have committed
   before death. Alternatively another manager won the create race, our create
   failed, and we died before recording the failure. A fixed name, handle,
   identical policy or public userdata/comment cannot select the right history.
2. **Live + matching boot/netns/handle.** The original table may have survived.
   Alternatively, after interruption, the table/subsystem may have been removed
   and reinitialized, then a foreign table received the recycled identifier.
   Namespace identity alone does not distinguish these subsystem lifetimes.
   `LiveIdentityConsistent` deliberately remains weaker than ownership.
3. **PendingDelete + absence.** Our delete may have committed, or another actor
   removed the object. Present absence does not prove the whole disconnect,
   receipt retirement, intent update or acknowledgement completed.
4. **Old-boot receipt + absence.** Reboot removed old kernel objects, but it did
   not revoke persisted Armed intent or erase Closed generation fences. Neither
   a clean kernel nor a new boot makes delayed old arm requests acceptable.

`tests/crash_disposition.rs` records these distinct hidden histories with equal
observable evidence and checks the existing assessor's conservative results.
It also covers pending replace/delete with old/new/absent table, changed boot
or namespace, and every marker kind. It is a deliberately small abstract model:
not a model of all Linux transitions, formal proof, power-cut simulation or
independent provenance provider. No new public recovery result/type is exposed.

## Safe immediate disposition

This table describes refusal and review requirements, **not executable steps**.
Every observation must later come from a complete, locked, independently bound
canonical-namespace read; partial dumps or guessed absence are never sufficient.

| Observed state | Required disposition; no automatic repair |
| --- | --- |
| Unsafe store, leftover stage/guard, unreadable table or unproven namespace | Preserve evidence/fence; refuse state writes and kernel mutations |
| PendingCreate/Replace/Delete, whether table is old/new/absent | Preserve pending state; review interrupted operation, never promote/retire/retry automatically |
| Missing/Retired receipt and table present | Unattributed collision: do not adopt, replace or delete; protection unknown |
| Live receipt and inconsistent epoch/handle/table | Preserve receipt; independent provenance/lifetime review required |
| Live identity consistent | Still require independent ownership, subsystem continuity, complete policy verification and conditional execution; not protected merely by identity |
| Current absence plus valid Armed marker | Protection is absent, not healthy/disarmed; preserve generation and block a protected-connect claim; restoration needs the future reviewed adapter |
| Current absence plus valid Closed marker | Retain fence; never revive its generation; unresolved receipt still requires review |
| Current absence plus missing/invalid marker | Not a protected state; missing and corrupt/enrollment-unknown are different recovery cases |
| Changed boot/netns or enrollment | Do not reinterpret old records as current ownership; retain history/fence until reviewed migration/retirement |

"Fail closed" here means **refuse uncertain mutation and success claims**. It
does not mean this offline code enforces packet blocking. If no policy exists,
or a foreign policy is present, refusing to touch it cannot guarantee no leaks.
The UI must say recovery/protection unknown or unavailable, not failed-protected.
Boot admission must prevent claiming a protected session before restoration is
actually verified; ordering alone is not evidence of packet enforcement.

## Policy choices that cannot be silently resolved in code

The current recovery contract permits deletion only of independently proven-owned
objects. It supplies no authority for the ambiguous orphan in example 1. There
is no safe automatic adopter derivable solely from the present receipt schema.

Recommended conservative next decision: retain **no automatic adoption** and
specify an administrator-mediated *adjudication* flow separately from normal
`recover`. The unresolved policy must decide how an administrator establishes
fresh ownership authority (or explicitly overrides it), how that exceptional
authority is scoped to an exact current observation and invalidated on drift,
and how intent/fences are preserved. A generic `ready`, sudo grant, `--force`,
fixed table name or matching policy is not such evidence. No override is added
here; existing `recover` must continue refusing unattributed objects.

Alternatives requiring separate review:

- **Kernel-enforced persistent application identity/provenance.** A new durable
  credential/attestation design must address replay, subsystem reset, table
  replacement and package rollback. A nonce or MAC copied into readable userdata
  is not automatically a nonreplayable proof; the existing comment prohibition
  is not relaxed. This may require primitives beyond the current receipt model.
- **Surviving broker/socket holder.** It can preserve live ownership across a
  worker restart, but introduces another privileged component and cannot by
  itself solve broker crash or reboot. No helper is introduced speculatively.
- **Privileged boot restoration after current absence.** It can re-establish a
  fresh exclusive table while retaining old records/fences, but needs a reviewed
  boot/network gating and receipt-retirement transaction. It does not authorize
  removal of a present orphan or justify rebooting the user's machine as cleanup.

Owner approval of a recovery exception, or a reviewed new provenance mechanism,
is necessary before claiming complete automatic crash recovery. It is not needed
to retain and test the current refusal behavior. No option changes the K0 promise
that normal helper death must not itself remove active kernel protection.

## Next independently reviewable gate

A useful next **test-only** experiment is owner/persist socket-lifetime behavior
inside a fresh disposable user+network namespace: empty tables only, no hooks,
rules, routes or links; a foreign sentinel; bounded child sockets; independently
verified isolation before any netlink send; no sudo fallback. Verify owner-only
removal, persistent survival/released owner and foreign-socket refusal/adoption
behavior without interpreting adoption as application ownership. It requires
explicit coordination and exact VM-kernel evidence; it is proposed, not run.

The follow-on [opt-in socket-lifetime harness](K1_OWNER_LIFETIME.md) implements
this bounded experiment. Its owning Draft PR carries exact-head VM evidence;
it does not enable production adoption or close the authority gate.

Before production, add process-kill/crash points across exclusive kernel commit,
receipt/fsync and reply, real supported-filesystem reboot tests, trusted canonical
namespace/subsystem continuity, enrollment/boot/service/package integration,
and all existing [K1 physical/host gates](../roadmap/KILL_SWITCH.md). Main/RC,
installation, release and marketplace remain unchanged.
