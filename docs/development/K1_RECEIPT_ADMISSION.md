# K1 inactive receipt assessment

This candidate stacks on the [locked coordinator](K1_COORDINATOR.md). It adds
only a strict bounded receipt decoder and pure consistency/refusal matrix in
`omavless-netguard::receipt`. There is no production caller, receipt opener or
writer, nft renderer/executor, helper/service/socket, package or host mutation.
The marker and coordinator behavior are unchanged. K1 remains unavailable.

## Identity and evidence boundaries

The candidate record is a flat JSON object, at most 2048 bytes, with exactly
`version`, `enrolled_uid`, `boot`, `host_netns_epoch`, `netns_device`,
`netns_inode`, `operation`, `phase` and `table_handle`. Version is 1. UID,
operation and namespace inode must be nonzero; boot and namespace epoch are
nonzero 16-byte arrays. Duplicate/unknown/missing fields, wrong types, invalid
UTF-8, overflow, trailing documents and unsupported versions are rejected.
This is an experimental model schema, not an installed storage format.

`PendingCreate` and `Retired` require wire handle zero; `Live`, `PendingReplace`
and `PendingDelete` require a nonzero handle. The pending replace/delete states
retain the **old** handle, never a purported replacement identity. All states
retain enrollment, operation and epoch context. No generation, policy contents,
endpoint, interface name, shell expression or arbitrary path can be supplied.
The operation number is retained context, not a monotonic sequence allocator or
replay fence; creation and exhaustion policy remain future work.

`host_netns_epoch` stands for independently proven canonical host namespace
lifetime. It is deliberately separate from boot/device/inode: equality of a
recycled namespace inode must not imply lifetime continuity. This module does
not establish that epoch, pin a namespace or authenticate its inputs. A future
adapter must solve that proof before using an equivalent production model.
Random bytes copied from a receipt are not independent lifetime evidence.

Likewise, decoding `Live` does not establish exclusive creation or durable
root publication. The synthetic `ReceiptRead::Durable` input represents evidence
the future adapter must acquire under the trusted shared directory lock. Unsafe
storage, pending filesystem staging, lost binding and uncertain publication all
map to `UnsafeOrUncertain`; they must never become `Missing`.

An identity match produces only `LiveIdentityConsistent`. There is no conversion
to `TrustedTableIdentity`, `KernelSnapshot`, transaction effect or success
response. Full policy verification, trustworthy kernel observations and atomic
conditional mutations remain outside this module. The root Armed/Closed marker
is deliberately not an input: it represents intent and generation fencing, not
kernel ownership. A receipt can never reset or retire that generation fence.

## Conservative crash matrix

| Receipt and observation | Assessment |
| --- | --- |
| Missing or Retired, current canonical epoch, table absent | AbsenceConsistent only; no mutation authorized |
| Missing or Retired, table present | Recovery required: no provenance |
| Live, same enrollment/boot/namespace lifetime/handle | LiveIdentityConsistent only; no policy/protection claim |
| Live, absent or changed handle | Recovery required: identity mismatch |
| Any pending operation, table absent/old handle/new handle | Recovery required; no automatic promotion, retirement or retry |
| Different boot, namespace lifetime, device/inode or enrollment | Recovery required, including when table is absent |
| Unproven namespace, unreadable table or unsafe/uncertain receipt | Recovery required |

This covers modeled crashes before execution, after kernel commit but before
receipt publication, and after lost acknowledgement. It does **not** inject
real filesystem crashes or exercise kernel transactions. PendingCreate plus a
matching table is still insufficient evidence, even if a future policy parser
would report identical rules. PendingReplace cannot transfer the old receipt's
authority to a new handle. PendingDelete plus absence cannot silently finalize
the receipt. Old-boot receipt retirement also requires a separate reviewed
plan; this candidate intentionally refuses it.

## Unresolved adapter gates

Kernel commit and filesystem publication cannot be one atomic commit. The
future port must durably publish its receipt before reporting success. If
exclusive creation succeeds but the process dies before ownership evidence is
durable, an existing fixed-name table cannot be adopted by policy equality,
comment, intent marker or pending receipt. The current root recovery contract
allows only proven-owned deletion. Disposition of this orphan case therefore
remains an explicit design gate; this candidate does not invent a cleanup path.

The future adapter must also establish canonical namespace lifetime, table
handle reuse semantics, trusted receipt storage and serialization across all
helper/recovery instances. Revalidation followed by a name-only mutation does
not meet the coordinator's atomic conditional execution contract. Generation
fences must survive receipt retirement, reboot, package changes and recovery.
No candidate assessment is a substitute for those proofs.

Required later gates include receipt persistence fault tests, real isolated
conditional kernel mutation and crash tests, root enrollment/IPC/service/boot
ordering, package upgrade/removal, runtime and Mihomo mark integration, DNS and
firewall coexistence, and the [K1 host/physical matrix](../roadmap/KILL_SWITCH.md).
Existing round-trip/packet evidence stays attached to its tested heads.
The separate [VM kernel-capability gate](K1_KERNEL_CAPABILITIES.md) exercises
namespace IDs/cookies, conditional generations, handle deletion and atomic
rollback inside a disposable namespace. It does not authenticate receipts or
resolve orphan disposition, module lifetime or production namespace provenance.

## Deterministic coverage

Tests cover every modeled phase and its handle constraints, all missing and
duplicate fields, bounds/types/versions, changed identity components, reused
inode with a changed modeled lifetime, unproven namespace, enrollment mismatch,
pending create/replace/delete across absent/old/new table outcomes, missing
provenance, and unsafe or uncertain publication. They prove refusal/consistency
decisions only. They do not test fsync, simultaneous real helpers, kernel CAS,
reboot, policy enforcement or host recovery.
