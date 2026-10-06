# T3 default-off original-child executable witness

Developer-only concrete read-only helper and passive test. No default close
method, effect permit, installer, unit/enrollment creation or service activation.
The prior capless package-path gates remain exact4d/e452/7def evidence; they do
not solve normal capability-enabled `/proc/<child>/exe` access. The helper is
separate from DNS broker and NetGuard: CAP_SYS_PTRACE is never added to either
existing service or the unprivileged runtime.

## Fixed original objects, not a generic privileged reader

The new `omavless-image-witness` crate is empty by default. Its explicit
developer-helper feature provides a separately compiled binary, accepting only
`--development-service`. Runtime's separate non-default developer-image-witness
feature presently includes ONLY the ignored passive gate; no production Session
uses this provider yet. No new effect/registration or public caller path exists.

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

## Next native integration, not assumed by this passive slice

After the first exact passive gate, the SAME original OwnedCore/Session must
retain this channel/child pidfd. Capture and EVERY image recheck must use fresh
helper FD and compare it to the already retained core/source and pair evidence.
Blocking RPC must run OUTSIDE the lifetime/scheduler gate under the existing
counted ProofFlight; reservation/cancellation/expiry/durable EffectProof are
rechecked immediately before any effect/final publication. No cached-image or
fallback bypass, second coordinator or token-only promotion is permitted.
Current Session implementation remains unchanged; inaccessible direct image
still refuses. Default product exposure, service/package distribution, both
host families and whole T3/C1 remain open.

Local SOURCE gates:8 pure helper tests, strict all-target helper and headless
runtime feature Clippy, compile-only ignored native entry, runtime default
check,6 scheduling/default/passive retention guards, formatter and document
navigation passed. The earlier ordinary source compile failures named pinned
Pid/dup/socket API spellings, and strict lint found three local formatting/
parser style defects; each was corrected before this checkpoint. None executed
the helper, a real socket or the ignored test. These source controls do not
substitute for the fresh original helper/native-child VM statuses and matrix.
