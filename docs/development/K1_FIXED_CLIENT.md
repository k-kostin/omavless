# K1 optional fixed client candidate

Status: SOURCE-only successor of immutable
`9ed6b286fc94536670905cbee1c82c5a1f02fe85` / #671 on dev/k1-fixed-client.
No client/socket/backend/native/VM activation or product registration. The
[runtime ordering candidate](K1_RUNTIME_PROTECTION_SEAM.md) still has no
owner/coordinator factory or native protected readiness; the prepared-config
attestation gap remains separate work. Frozen #663 service evidence remains
exact application d3b24a36 and is not borrowed for this new client.

## Fixed scope and trust

Optional netguard-client-candidate adds only existing nix's poll feature. The
private runtime ProtectionPort implementation is compiled by the existing
opt-in runtime candidate; defaults, daemon/CLI/IPC methods and templates remain
unchanged. FixedClient::new/default is lazy and performs no filesystem or
network operation. No constructor accepts a path, UID, FD, namespace, mark,
interface, unit, shell fragment or arbitrary bytes. Only typed v2 Status,
Arm(full,N), Disarm(N) exist; no root Recover.

Caller must be the admitted canonical user/mount/network-namespace runtime
under trusted root/package custody. No new manager attestor, PID lookup or
namespace-number token is introduced. Peer UID0 is root authority only under
that user-namespace/root trust assumption. Neither pathname equality nor peer
metadata authenticates a creator or hostile-root installation.

The supported unit has User=root/Group=root: actual SO_PEERCRED UID/GID=(0,0)
and positive PID are required before any request write. Pin the complete
observed tuple at initial successful Status, compare later distinct exchanges
and after reply. This is a connection/listen-time credential snapshot, NOT
process liveness or original-process identity across PID reuse. Package ACL GID
is independently resolved from the fixed bounded root-owned group binding;
it is not substituted for the root peer's primary GID.

Retain no-follow root:root /run directory, literal root:package-group0750
parent and root:package-group0660 single-link socket O_PATH leaf. Recheck
named/held identity, metadata and group binding before connect/write and after
decode. This is fixed path consistency under trusted ancestry, not hostile
path-ABA resistance or a filesystem label-to-descriptor authority proof.
Service still independently authenticates enrolled request sender credentials.

## One original exchange and positive completion

Create a fresh nonblocking CLOEXEC owned Unix socket for each DISTINCT exchange.
Store its positively returned owner BEFORE post-create time/check cuts. Connect
once; known EINPROGRESS may wait then check SO_ERROR/connected peer. UNIX
backlog EAGAIN, connect EINTR and all unexpected errors are terminal, not retry.
Read/write known WouldBlock may wait and continue the SAME original byte cursor
under the SAME budget; no accepted bytes are resent. Interrupted network calls
and poll failures do not retry.

One original whole2s monotonic deadline covers admission/socket/connect,
four-byte big-endian framing, request write, bounded response read, strict v2
decode and named/path/peer/final gates. Sample before/after bounded backend
calls and decoding; late final progress/postgates refuse. This is not hard
syscall preemption or a bound on later return scheduling. Frame length must be
positive and at most8192 before allocation. No raw parser/OS/peer values are
logged or returned in an error.

The client ITSELF requires a request-specific healthy positive reply before
marking completion: Status Verified/current policy; Arm exactly Armed(N);
Disarm exactly Disarmed(closed_generation=Some(N)). Error/manual/malformed/
oldv1/mismatched/late replies poison and retain the original stream BEFORE
returning, not after a runtime wrapper notices a mismatch.

The root service handles one frame per connection. A later DISTINCT operation
may retire the previous stream only after that positive reply AND every
postgate completed. Keep the completed socket owned through successful return;
retire it on the next separately requested operation before creating another.
One active/completed/quarantined socket slot, no unbounded owner list. A failed
port cannot allocate a second stream, resend, reconnect, query Status as repair
or compensate with Disarm. Original graph is retained while alive and forgotten
on uncertain/in-flight Drop; fresh/known-complete Drop may close normally.
No descriptor-survival-after-process-death or actual kernel-custody claim.

Read-only constructor/group-helper/intermediate FDs preserve ordinary drop-on-
Err contracts. No root operation exists before a request is sent, and the client
does not claim custody of unreported descriptors or every builder temporary.

## Focused controls and remaining gates

Mocks exercise lazy/fixed input closure, pre-write wrong root/group/PID and
changed peer, held/named-path substitution, positive-late socket creation,
connect/poll refusal, exact partial cursors/WouldBlock, late final read/write/
decode/postgate, oversized/truncated/duplicate/oldv1/error/manual frames,
request-specific Arm/Disarm mismatch, unexpected panic, no second allocation
after poison and peak one socket across positive distinct operations. They use
no Linux backend path/socket call; counters model original owners only.

Local SOURCE gates: 11 new mock controls and 33 existing protected/ordinary
lifecycle controls passed (zero ignored), candidate check and strict client/
runtime all-target Clippy passed, plus default runtime check, fmt/diff/links.
No Cargo.lock/version/vendor/unit/protocol/producer/native-host change. Initial
ordinary test compilation lacked Debug on the finite phase enum; initial
Clippy found a div_ceil style issue. Both were fixed before the frozen code.

Separately, ROOT's immutable9ed default NetGuard gate completed original0 with
396pass/0fail/51ignored, no ignored selected. Its first attempt's19 SUN_LEN
failures came from an overlong chosen HOME TMP path; shorter private HOME TMP
resolved that harness issue without source change. That broader predecessor
gate is not a real-client or native acceptance result for this successor.

Before activation: complete primary/independent reached-code review, actual
fixed namespace/package/enrollment/socket admission and transport gates, bounded
owner factory/operation/revision integration, post-prepare exact config/mark/
omavless0 attestation, real core/socket-path coverage and product/physical
acceptance. No readiness or new orphan-recovery guarantee follows from these
source controls.
