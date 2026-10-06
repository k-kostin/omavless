# T3 default-off original-child executable witness

Developer-only concrete read-only helper, passive tests and same-Session effect gate. No default close
method, effect permit, installer, unit/enrollment creation or service activation.
The prior capless package-path gates remain exact4d/e452/7def evidence; they do
not solve normal capability-enabled `/proc/<child>/exe` access. The helper is
separate from DNS broker and NetGuard: CAP_SYS_PTRACE is never added to either
existing service or the unprivileged runtime.

## Fixed original objects, not a generic privileged reader

The new `omavless-image-witness` crate is empty by default. Its explicit
developer-helper feature provides a separately compiled binary, accepting only
`--development-service`. Runtime's separate non-default developer-image-witness
feature includes the ignored passive gate and test-only same-original Session
integration with exact scoped acceptance below. Default/native product activation still
does not select this provider. No service registration or public caller path exists.

The fixed endpoint is `/run/omavless-image/control.sock`, under root-only writable
ancestry. An explicit ROOT-created root0700 directory holds the root0600,
single-link, bounded fixed enrollment file
`/var/lib/omavless-image/development-enrollment-v1`. Exact complete contents:

```text
omavless-development-current-image-v1
1000
<exact lowercase nonzero core SHA256>
<exact lowercase nonzero development test ELF SHA256>
```

There is exactly one terminal LF and no extensions or general UID/path input.
The helper retains protected no-follow root ancestor chains and original regular
root0755/single/nonempty/bounded core and client files, plus enrollment. It
hashes the original two complete files under its startup budget. Fixed paths:
`/usr/lib/omavless-dns/mihomo` and
`/usr/lib/omavless-image/development-runtime-tests`. Normal `/usr/bin/omavless`
and another test location cannot be silently enrolled into this developer class.
Root-stated source/ELF issuance and trusted launch remain separate; an inode
match does not attest every library/mapping or hostile LD_PRELOAD environment.

The existing exact named-UID ACL discipline is separately applied to ONLY this
new socket: owner rw, enrolled UID1000 rw, owning group none, mask rw, other
none. It never enrolls DNS, edits groups, invokes polkit/sudo, deletes a stale
socket or repairs filesystem state. A failed/partial grant remains failure.
The endpoint/stated hashes do not themselves establish close authority.

## Kernel origin and every fresh image

One synchronous channel accepts exactly one Bind(original child pidfd), then
strictly sequenced Observe requests and terminal Finish. Frames are exactly16
bytes/version1 with zero reserved bytes, no PID/path/command input. Each
successful Observe returns exactly ONE READ-ONLY current executable FD; ACK is
never an authority boolean. At most2048 Observe requests per original channel;
replay/duplicate bind/wrong sequence/capacity/refusal poisons it permanently.

SO_PEERCRED plus kernel SO_PEERPIDFD pins the original peer. Every message
requires its same original SCM_CREDENTIALS, exact cmsg/rights cardinality,
CLOEXEC receive and no TRUNC/CTRUNC. All delivered rights iterators are fully
drained, including surplus/unexpected rights. No reconnect/resend/fallback or
signal/ptrace/getfd API is exposed. This inherits ordinary library/kernel error
semantics, NOT legacy Bundle's stronger hidden-FD/fatal-custody guarantee.

PID is derived ONLY from real pidfs descriptors: mandatory PID_FS_MAGIC,
no PIDFD_THREAD, strict positive single-namespace Pid/NSpid from original
self/fdinfo procfs. Older unsupported kernels refuse, not adopt scalar input.
The helper retains original peer/child pidfds and proc directories. Pre/post
checks compare current named row dev/inode, non-exited status/stat, leader/Tgid,
all four UIDs, original PPid, positive start time and original pid/user namespace
objects. Caller/helper/child must share these namespaces; procfs's own PID view
is checked against the helper. Reaping/reuse, reparenting, dead group or namespace
drift refuses. R/S/D scheduling may legitimately change between reads; that is
not process identity. All proc text is bounded16KiB plus overflow detection.

Every Observe reopens the ACTUAL kernel exe link of the same original child,
checks its full dev/inode/UID/GID/mode/link/size/ctime/mtime against the retained
fixed core, and repeats peer/child/source/origin/live checks before returning.
Wrong-image children cannot turn the helper into an arbitrary-file oracle.
No cached hash replaces a fresh current-exe acquisition. The returned FD is
data/evidence, not CandidateEffectPermit or product authority.

Pidfds exclude PID-reuse confusion but do NOT freeze exec. These are sampled
per-fence observations of the trusted fixed non-exec core, not atomic
image-through-an-effect assurance. The fixed core's token/ID conditional compare
and native owner/cancellation/revision remain indispensable for later effects.
Kernel basis: Linux v6.17
[pidfs type, fdinfo and poll producer](https://github.com/torvalds/linux/blob/v6.17/fs/pidfs.c)
and [pidfs magic](https://github.com/torvalds/linux/blob/v6.17/include/uapi/linux/magic.h).
No new unsafe syscall/IOCTL wrapper is introduced.

## Bounds and error behavior

Root helper must actually have UID/EUID0 and permitted/effective capabilities
exactly CAP_SYS_PTRACE (0x80000). It cannot grant that privilege to itself.
ROOT's reviewed namespace launch must supply this narrowly bounded capability,
sanitized environment and original stdio; no host unit/install grant is shipped.
The helper lowers its own hard/soft NOFILE limit to64 before acquisition.

Fixed chains contain4 descriptors each:12 total plus3 member files. Original
binding holds2 pidfds,2 proc rows and6 namespace descriptors. Endpoint holds
the accepted socket and original peer pidfd; the listener is one. Stdio is3.
That is31 fixed accounted descriptors; bind's helper proc row and bounded
fresh fdinfo/namespace/exe/payload temporaries add at most6, and an invalid
ancillary record may transiently deliver2 rights before refusal. This explicit
plan fits64; loader/ordinary-library hidden allocations are not falsely attested
as an exhaustive table. No unbounded thread/channel pool or uncertain-owner
eviction is present. Root actor admission is one channel/epoch, not bypassable
by a different caller fixture directory.

Startup hash budget5s, idle/initial accept budget30s, and each request's absolute
send/receive/proc/postguard budget2s (native caller can only shorten it).
These are sampled guards and cannot preempt a blocking filesystem syscall.
Read-only helper resources may ordinarily close on refusal; native must retain
its actual OwnedCore and permanently revoke its context, not treat helper loss
as cleanup, reopen the channel or accept a late reply. Fatal helper loss is
unavailable, never continued custody. Six fixed helper phase literals contain
no PID, private paths, file bytes, metadata tuples or raw errno.

## First executable passive scenario and negative evidence

The ignored `actual_original_cap_child_image_witness_in_dev_vm` requires explicit
OMAVLESS_IMAGE_WITNESS_VM=1 and one closed case: positive, wrong-parent or dead.
ROOT must stage exact binaries/enrollment/socket directory in a fresh private
PID/mount/network namespace without a user namespace. The sealed core gets
exact Linux filecaps eip0x3400 on a read-only suid bind; no TUN/DNS/broker effect
is requested. Only helper has SYS_PTRACE; native UID/GID1000 parent has zero
permitted/effective caps and NNP0. Original cap child UID quadruple1000 and
eip0x3400 gain must be actually observed before binding.

ROOT's namespace PID1 supervisor retains ORIGINAL helper and test children,
their original pidfds/captures and statuses. Native test is its UID1000 child,
NOT an exec-away reaper. Positive sends Bind then3 Observe requests; each actual
returned FD equals the same original retained root core's complete identity
and exact bound SHA. Native's Original OwnedCore remains live around every
sample. Finish is read-only terminal ACK; reentry refuses. No Session adoption,
controller POST, fixture effect permit or desired mutation occurs.

Whole positive requires ORIGINAL helper0 AND ORIGINAL test0, not just ACK or a
marker. Wrong-parent sends the actual runtime's own pidfd (whose PPid is the
supervisor, not itself); dead sends the original cap child's retained pidfd
after owned stop. A generic Client error is NOT enough to name either case
PASS: actual evidence must include ORIGINAL helper2, ORIGINAL test0 and fixed
before-child-bind/child-bind-refused phases, after separately verified listener
startup, with no alternative preflight failure. These are bind-refusal case
observations, not kernel-errno causal attestation. Pure tests also reject all
identity drift coordinates; actual changed-source/changed-image refusal remains
a separate required matrix before effects.

Existing OwnedCore stop/Drop and namespace containment are ordinary developer
fixture behavior, NOT exact original-core exit0 or UNKNOWN cleanup/recovery
receipts. On failed/hung/uncertain helper/test, PID1 supervisor retains originals
and parks without kill, reap, cleanup or automatic rerun; ROOT reconciles or
explicitly administers the disposable VM under the execution policy. The VM
packet and all actual privilege/resource boundaries need separate primary and
independent review before selection. No actor, socket or ignored test was run
by the author.

## Native integration requirement, not implied by this passive slice

After the first exact passive gate, the SAME original OwnedCore/Session must
retain this channel/child pidfd. Capture and EVERY image recheck must use fresh
helper FD and compare it to the already retained core/source and pair evidence.
Blocking RPC must run OUTSIDE the lifetime/scheduler gate under the existing
counted ProofFlight; reservation/cancellation/expiry/durable EffectProof are
rechecked immediately before any effect/final publication. No cached-image or
fallback bypass, second coordinator or token-only promotion is permitted.
Current Session implementation remains unchanged; inaccessible direct image
still refuses in the tested400c passive baseline. The later source candidate
below does not retrospectively change that evidence. Default product exposure, service/package distribution, both
host families and whole T3/C1 remain open.

Local SOURCE gates:8 pure helper tests, strict all-target helper and headless
runtime feature Clippy, compile-only ignored native entry, runtime default
check,6 scheduling/default/passive retention guards, formatter and document
navigation passed. The earlier ordinary source compile failures named pinned
Pid/dup/socket API spellings, and strict lint found three local formatting/
parser style defects; each was corrected before this checkpoint. None executed
the helper, a real socket or the ignored test. These source controls do not
substitute for the fresh original helper/native-child VM statuses and matrix.

## Exact passive VM checkpoint (ROOT-operated)

Tested production source is `400c648b61d266a84c6732022aa19ffd9eec2ba3`,
not this documentation successor. The immutable helper was570,592 bytes/SHA256
`6c646467e572437ba98b64ba248e2b944745ca06d4bc9eec11784bc600db304b`;
native test ELF21,678,480 bytes/SHA256
`fb61ea171e531f51d5645d0fe45f75a77adecb5a5166077049af956d89148477`.
The exact four-family core remained61,083,808 bytes/SHA256
`897ada648fe975718ac1b7318702def5b826a9901797a0d13cdd333a012b9fcb`.
Primary and independent full source/packet reviews preceded selection.

The first recipe retained writable copied-executable descriptors. Its run
`873207`/session97929 reached HELPER_START refusal and stayed parked; no test
spawn or passive PASS is asserted. Initial fixed-file observer `233300` was
originalSSH1 on an absent test capture. Separately approved projections
`7f83e6` and `10a9cd` returned originalSSH0: helper stderr88 bytes/SHA256
`7b969c0c52dcc0c093c8bb6084dfc2e32d1a5bdc1c694776364bd06403bed57e`
matched the exact closed `text_file_busy` category. This concrete recipe error
was corrected ONLY in a fresh packet: verify full copied bytes and retained
read-only same-inode handle, then positively release its writable handle before
exec. Failed scope1 was not retried, reaped, signalled, queried or cleaned up.

Fresh scope2 kept the same compiled source/artifacts. ROOT admitted only the
known-positive global empty mount anchors through exact read-only identity,
owner/mode/no-xattr/emptiness checks; no old private namespace/resource was
adopted. Seventeen pure packet controls passed before both affected reviews.
ROOT upload `80998c` and preparation `c35d23` returned original0. Positive run
`76b7e1` returned originalSSH0 after exact ORIGINAL helper0/native0 and complete
output/identity grammar checks. Separate observer `5e0abf` returned original0:
helper stderr232 bytes/SHA256
`c4f72de75067b7d2bcc4156163778f87f8275ab3050e37c8488e7af729724c93`
contained seven fixed phases with exactly three fresh image sends; stdout was
empty. Native stderr127 bytes/SHA256
`3950f4560b615e8cb849639adc59f0945a8c43668e934851116174e45707606a3`
matched the positive phases; stdout222 bytes/SHA256
`c3860517d8eb8f054aba99e9b9a6a0e0ae890d260b9f9e7c58b1fcf5fa08bb87`
matched the complete single-test success grammar, without a panic.

Wrong-parent run `523c38` and original-dead-child run `ad2a6906` independently
returned originalSSH0 after expected ORIGINAL helper2/native0, exact bind-refused
phases and native success grammar. Their separate observers `9f2425`/`722d6e`
returned original0. Both helper stderr131 bytes/SHA256
`dc98b3bd954d42750ae6536d8d9fcf820761ffadc7f2fb34591f316469d1cfa7`
and native stderr90 bytes/SHA256
`e2db968bd74639162132566d747f87b156662cf36495e7a624acaa42afb423d3`
matched the exact negative grammar. These are specific bind-refusal controls,
not generic-error or kernel-errno inference. Native test stdout222 bytes/SHA256
`cf4b5c79a024fc5e934590c686fb7b41c9d106caf7cb97731391cf01e3424282`.

This400c checkpoint proves bounded passive same-original cap-child image samples
and two negative controls only. It does not establish actual image/source drift,
NativeOwner/Session per-flight integration or effect-bearing close; the later c9
acceptance below is separate evidence. Default exposure and distribution remain
pending. No source/packet author operated the VM or read raw captures.

## Same-original native Session integration — new SOURCE candidate

This is not400c acceptance or default activation. Only the explicit test-only
developer constructor selects a fixed normal-package-path image provider. It
retains the actual OwnedCore's unreaped lifetime, original kernel child pidfd,
controller directory/socket and fixed source descriptor. Bind/initial Observe
run in detached preparation with NO owner mutex, scheduler or migration lease.
The helper/client protocol and privilege boundary remain unchanged400c code.

The native host retains ONE noncloneable prepared CloseObservation/Session/Client,
not rows, a permission boolean or copied image identity. Preparation requires
fresh actual readiness and the separately qualified normal package, and records
the exact desired/profile/mode/UID/core/controller/config/store context. Initial
test adoption uses only that context plus local original lifetime/source/controller
guards. First capture consumes the SAME observation once; it never reconstructs
or rebinds after failure, missing slot or a second request. Detached discovery
then freshly observes current image/controller/catalog/package before the usual
owner instance/revision/cancellation/expiry retention. Every host prepare/start/
commit/stop/discard hook revokes the prepared context before mutation. A field
presence or the deliberately invalid test preparation cannot adopt authority.

The preparation starts the original shared3s discovery budget; network/catalog
setup cannot renew it. The5s confirmation lifetime still starts at capture and
cannot extend that discovery budget. This first developer window is one-shot;
it is not a general reconnect, refresh-pool or product lifetime solution.

Current-image acquisition is separated from local `check_locked`. Each existing
counted ProofFlight samples a NEW current FD through the original Client outside
the fast gate, compares it to SAME original image/source/package evidence, and
passes a private CurrentImage to all reached pair checks. There is no recursive
refresh, cached-image substitution or direct-proc fallback in selected helper mode.
ProofFlight retains the FD through each effect chunk/final publication. RPC and
terminal read-only Finish complete BEFORE durable lease acquisition; late cancel/
expiry is checked before that lease and again at the effect/terminal gate.
The migration lease drops before the current FD; the flight drain acknowledgment
comes last. A late valid FD/ACK cannot win cancellation. Ordinary read-only
controller requests sample the image but drop that observation flight before
their syscall; this is NOT a held-through-effect claim for all socket writes.

Provider loss/refusal, wrong image, source drift or deadline permanently revokes
the Session lifetime as well as blocking new requests. Before an effect it refuses;
after attempted bytes it remains Unknown. Exact typed receipt replay is unchanged
and never resends. Definitive completion requires the SAME final fresh flight,
durable owner proof, current reservation/lifetime and original expiry. Helper
Finish ACK is still not original helper exit or atomic image-through-effect proof.
Fixed trusted non-exec core and ordinary kernel/library backend assumptions remain.

One Session has at most one counted image flight. Relative to direct evidence,
one Client adds its socket/helper-peer pidfd/child pidfd (three retained handles)
and each current flight adds one temporary image FD. Original image/source and
<=28 package/selection handles remain bounded as before; no FD/history/channel
pool, eviction or stronger hidden/fatal-descriptor custody is claimed. The helper's
existing64/one-channel/2048-observation limits are unchanged and exhaustion refuses.

The new ignored `actual_owner_qualified_cap_image_close_with_witness_in_dev_vm`
uses the original real RuntimeServer/coordinator and two private no-TUN/no-DNS
streams. It requires actual parent0 caps/NNP0, original cap-child0x3400, the exact
qualified pair, no fixture permit, selected EOF/other echo, exact receipt replay
and unchanged desired bytes. Its exact c9 actual result is recorded below;
memory callbacks are not that privileged evidence and do not substitute for it.

The source-drift extension is a separate declared passive case, not a generic
positive panic relabelled PASS. ROOT's new fixed namespace recipe must first
start the original helper and verify listener-ready, then overlay ONLY its
fixed core source with an exact same-byte/capability copy in a different inode
before the native original cap-child launch. Native still uses that actual live
child's pidfd and exact UID/parent/0x3400 evidence; it does not send the wrong
parent or a dead child. One bind must refuse, with the extra closed
`image_witness_original_source_drift_refused` phase, original helper2/native0
and exact complete grammar required externally. No native mutation, retry,
fallback, controller POST, cleanup proof or default effect occurs. Its exact
fresh-scope6 actual result is recorded below, after separate ROOT selection.

Regular regression tests use the existing actual owned child/controller fixture
with memory image-RPC callbacks, never a helper or CAP_SYS_PTRACE. They exercise
off-gate cancellation/late valid FD, expiry/wrong image sticky revocation,
first partial write then next-refresh failure→Unknown, final refresh failure→Unknown,
cancel during Finish, exact replay/no extra POST and invalid prepared-field/
changed-context/second-take refusal. These are source behavior controls only.

Checkpoint source controls returned original0: six RPC behavioral tests (plus
the prepared-field control), nine source-retention/default-boundary controls,
strict all-target headless-feature and default Clippy, default check, formatting
and compile-only release test image. The first compile named a nonexistent test
pause setter; it was corrected before execution. Strict Clippy's two callback
type-complexity and private enum-name diagnostics were fixed without suppressions.
The first source guard incorrectly stopped at an internal cfg(test) block; its
function-range extractor was corrected without a production change.

The first1173-case serial feature suite remains originalEXIT101/NONPASS:
1107 passed,26 failed,40 ignored in1900.08s. Its recovered original complete
source-test result was150176 bytes/SHA256
`67c5182ffe0aad2fc39affc0a3a11c2ce53f368caf2a5bae232a1a9988562866`.
All26 failures report ENOENT at self-reexec output sites, while the author
rebuilt that SAME active debug ELF pathname for focused checks. This signature
is consistent with replacement/unlink of the reexec image, not a demonstrated
application recovery defect; no application fix was made.

A unique immutable debug ELF231713680 bytes/SHA256
`f434f2a33903d51813256cd16ef9130242f7304db2628701145612a0b6ed8efd`
preserved the integrated0c8e source controls before c9's additional ignored
passive-case branch. The first exact failed filter then passed. Its subsequent
whole serial run returned originalEXIT0:1134 passed,0 failed,40 ignored,
1174 total in2564.64s. Durable stdout132919 bytes/SHA256
`9fdcd1204988cfcaf06ac37883f29a61c5397f9d652db3bc63e979e4c0d5e7f2`,
stderr empty. No ignored helper/VM case executed and no live ELF was rebuilt
during this run. This is ordinary source-test evidence only; the old failure
remains retained separately. See [self-reexec scheduling guidance](../../CONTRIBUTING.md#checks).

## Same-original VM integration: earlier supervisor failures preserved

The integrated runtime candidate is exactc9baa5479e36582ba5c73920c95f94c54a6983dd
(production effect source0c8e4864/helper400c648b), immutable release test ELF
21805440 bytes/SHA256
`97d71f9fb48ea713f46b40fdd14eb6a753121c2e6e9466a6955263f1cf950536`.
It passed only the new exact developer gates recorded below, not default activation.

New scope3 upload/preparation returned original0, but source-drift supervisor
parked at HELPER_START before native spawn. Its separately scoped fixed-file
observer returned original0: helper stdout empty, stderr75 bytes/SHA256
`c1bd16bf8459e9a2ca1cf632c09e4cce8ed23ccc50a0d5d79f87e114bfe0b052`
contained listener_ready once and the closed expired category. Source inspection
and a pure counterexample found a live-capture reader could reject concurrent
growth between fstat and pread; a fresh scope4 recipe corrected that reader,
not the production helper or any privilege/authority predicate.

Scope4 upload/preparation also returned original0; its new original supervisor
parked at HELPER_START/fixed_operation/io_error. A
separately selected helper-only observer returned original0 with the same exact
75-byte helper grammar/hash and empty stdout. The helper started, but neither
projection isolates the supervisor's failure: fixed_operation is also reset
AFTER readiness, so the earlier pre-readiness inference was wrong. The live-reader
defect is not established as its actual cause. Neither failed scope is retried,
signalled, reaped, traversed or cleaned by the author. Fresh scope5 SOURCE
diagnostics classified only existing acquisition subphases/returned errno through
closed public categories, without raw errors or additional resource probes.
That new original also parked at HELPER_START/fixed_operation/io_other; it is
NONPASS, not a completed source-drift test.

Reassessment found a concrete recipe ordering incompatibility: RLIMIT_FSIZE
262144 preceded a61083808-byte drift-copy write. A pure frozen critical-order
counterexample produces EFBIG, which the old taxonomy reported as io_other.
This is consistent with the observed outcome, not an attestation of its actual
errno. Fresh scope6 SOURCE prepares the full same-byte/capability, different-inode
read-only copy BEFORE the capture-file limit and helper startup; the fixed
source is covered only AFTER original helper readiness and repeated retained/
named/capability checks. Capture size, helper privileges, all deadlines and
negative/positive status requirements remain unchanged. The new actual selection
requires primary/independent review and ROOT operation.
No observer0, source0 or passive helper gate promotes these runs to PASS.

## Exact same-original cap-image effect checkpoint (ROOT-operated)

Fresh scope6 retained the same runtime c9/source0c8e, helper400c/570592-byte ELF
SHA256 `6c646467e572437ba98b64ba248e2b944745ca06d4bc9eec11784bc600db304b`,
and four-family source4d4747af47d60956b370e588469a3b9472ed42a1/package objects.
The corrected recipe received full primary/independent affected review and38
pure controls before ROOT alone selected each action. Original upload `10cf48`
and preparation `15345`→`824341` returned0. No old failed graph was reused.

The source-drift negative ran first: original `26608`→`234ff8` returnedSSH0
with the exact completion marker, after the recipe independently verified
ORIGINAL helper2/native0, complete fixed grammars and both positive original
reaps. Separate fixed-file observer `6332`→`e7b052` returned0. Helper stdout
empty; stderr131 bytes/SHA256
`dc98b3bd954d42750ae6536d8d9fcf820761ffadc7f2fb34591f316469d1cfa7`
matched ready/before-bind/bind-refused/fixed-refusal. Native stderr134 bytes/SHA256
`9c38f99d6f33c5c56ef086f1f639cec92f095ce8676a4cdfc1288260f88c168a`
matched original-cap-verified/binding-refused/source-drift-refused. Native
stdout222 bytes/SHA256
`02273be623c628423ca4cdcdcb590af730bbfabb1148e2b53e276ce9cbeb2f05`
matched the complete exact single-test grammar without panic. This is the
declared same-byte/different-inode source-drift refusal, not generic error2/101.

ROOT then selected a SEPARATE original-owner positive namespace. Original
`58818`→`5bf48e` returnedSSH0 with the same exact completion marker after
verified ORIGINAL helper0/native0, whole grammars and positive-only original
reaps. Separate observer `52772`→`454f9e` returned0: helper stdout empty;
stderr5413 bytes/SHA256
`94d0d250fc6a0d4c9204560830fca5170331444680119c1e863d8015e9f0d2db`
matched ready/original-binding/current-image-sends/positive-finished grammar.
Native stderr87 bytes/SHA256
`48cac33d2e4043dcfb821cebf955b62c3a1e3fe1a80361d6bd3afd0ddba19bd4`
matched original-cap-verified/owner-selected-closed; stdout234 bytes/SHA256
`9956601e1723918303aee589d02f76dcac970020479849b6a4f881c541cf2814`
matched exact single-test success grammar, without panic.

This proves the scoped real RuntimeServer/NativeOwner integration with SAME
retained Session/Client, off-lock fresh executable FD checks through each counted
effect/final flight, actual cap-enabled original child, selected B EOF while
nonselected A echoes, exact receipt replay and desired bytes unchanged. No
fixture-effect permit was used. Original helper exit0 is separately verified,
not inferred from Finish ACK. Trusted fixed non-exec core and ordinary
stdlib/kernel/fixture Drop assumptions remain; no atomic image lock, unknown
cleanup, fatal descriptor survival or original-core exit0 receipt is inferred.

Default provider selection, helper service/enrollment distribution, public
default close registration, installed frontend/cap-helper acceptance, native
startup/DNS/TUN/broker serving, both host-family release gates and whole T3/C1
closure remain open. Prior capless/CLI/Foot evidence belongs to its own exact
heads; these two new gates neither replace it nor borrow its broader claims.
Raw captures/private packets stayed outside Git. All scopes3/4/5 stay NONPASS.
