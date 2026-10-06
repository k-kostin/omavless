# T3 product witness epochs

Status: default-off SOURCE candidate based on `5cfbf2fd`; no product activation,
service/enrollment installation or VM evidence. The existing
[original-image contract](T3_IMAGE_WITNESS.md) and its exact installed-development
checkpoint remain authoritative for the unchanged path. No actual acceptance,
K1 authority or schema1 DNS consent is promoted by this successor.

## First executable boundary

The optional helper feature `product-epochs` selects a distinct class and the
explicit invocation `--product-epoch-service`. Defaults expose no helper.
Existing developer classes keep their single channel, initial thirty-second
wait and fixed UID1000. This cut ships no unit, installer, enrollment writer,
runtime flag, public method registration or automatic startup selection.

Closed product paths are runtime `/usr/bin/omavless`, managed core
`/usr/lib/omavless-dns/mihomo`, root record
`/var/lib/omavless-image-product/runtime.enrollment`, and endpoint
`/run/omavless-image-product/control.sock`. The record is the original held
root-owned regular single-link0600 file, at most512 bytes, under retained
root-owned nonwritable ancestors. Its exact five LF fields are schema
`omavless-product-current-image-v1`, one canonical decimal nonroot UID other than
`u32::MAX`, nonzero lowercase64hex core and runtime SHA256, and final empty field.
No duplicate fields, caller UID/path/PID, controller ID or receipt is admitted.
One helper admits only the UID selected by this root record, not arbitrary users.

Socket ancestry is root-only writable but searchable by the user; exact named-UID
ACL, kernel-derived original peer and child pidfds, proc-row continuity, all four
UIDs, original parent, PID/user namespaces and fresh executable checks remain.
Client UID is locally observed only for ACL checks; the helper independently
requires the root record's UID. Wire protocol has no UID/PID/path input, fallback
or reconnect. The helper still requires root and exactly CAP_SYS_PTRACE, and
retains its own created listener rather than an inherited-credential socket.
No caps are added to native/DNS/netguard. Ordinary hidden backend allocation and
fatal-loss exclusions remain; the existing64 FD ceiling is unchanged.

## Idle, active context and terminal history

Only this product listener may wait in one-second poll slices with **no** Binding,
Session, capability, request, flight or effect context. Each slice has a bounded
two-second original source/root/node recheck envelope. Drift refuses, never
repairs/recaptures. This is not renewal of an accepted operation's budget.
Deadlines are sampled absolute checks, not a promise to preempt a blocked kernel
filesystem operation.

Before accept/peer/child acquisition reserve one of128 nonevicting history slots.
Exactly one channel is active and borrows the SAME retained Roots. Active idle
remains thirty seconds; every request has an absolute two-second deadline.
Every accepted error, expiry, unexpected frame/credential/FD or uncertain send
ends that provider context; there is no automatic next admission after error.
Finish rechecks the same binding/current image, sends its original ACK and checks
the same deadline. All per-channel FDs drop before history is appended and a
next channel is admitted. Entry129 refuses before accept. Shared Roots/listener
are retained once and history adds no FDs. ACK is provider completion, not an
effect receipt or atomic no-exec-through-effect proof.

## Required caller cut before product use

There is deliberately no NativeHost product constructor yet. Finish alone does
not prove business completion, drained flights or absence of an unresolved
receipt. Reuse the SAME coordinator/Session/CurrentImage/ProofFlight/EffectProof.

The next concrete integration must take the old SAME snapshot/session into an
explicit retirement task under its owner, then run Finish outside owner and
migration locks. Only known terminal completion or positively before-effect
cancel plus all original flights/current FDs drained may admit an explicit NEW
snapshot. Re-enter the owner to verify retirement identity and exact current
context/revision before new original core/controller/qualified-pair acquisition.
Busy, drift, refusal, timeout, late reply or partial/Unknown delivery stays
sticky; no automatic retry or old rows/tickets become renewed authority. Preserve
all receipt/token/lifetime history without eviction and refuse before acquisition
at capacity. Exact replay remains read-only, never another effect.

Current capture invalidates/drops the old Session while its owner is held, and
Drop does not positively Finish the provider. Wiring the new helper into that
unchanged path would be incorrect and is not done here. Three-second snapshot,
five-second confirmation and existing effect budgets/cancellation/per-chunk/final
proofs are not widened or bypassed.

Pure controls exercise the real decoder/admission/terminal functions: nonfixed
UIDs and bounds, cross-schema refusal, idle/no-context versus active/Busy/expiry,
sticky errors and late success, two positive terminals, entry129 refusal and
nonevicting first history. They do not open a privileged listener/core or prove
actual sequential FD behavior. Independent full boundary review and concrete
caller integration precede ROOT's fresh normal-owner two-epoch VM scenario and
wrong-UID/dead/drift/expiry/upgrade tests. Default public registration, packaging,
enrollment and ARM/Nix activation remain OFF/pending.

First-cut source results:16 product-feature and10 unchanged developer-feature
library controls passed; default helper check, strict all-target product and
developer Clippy, workspace formatting and whitespace checks passed. The first
compile/style failures were corrected before these results. No helper/native/VM
execution or source-to-product acceptance is claimed.
