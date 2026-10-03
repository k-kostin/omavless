# T4 real Off-startup caller research

Base: #544, `b57c8480de6e2e70ba910097f7ab351f4c0d302e`.

The checkpoint below describes the original test-only implementation. The later
[inactive System bridge](T4_SYSTEM_HISTORICAL_OFF_ADAPTER.md) compiles only its
opaque startup witness and bounded startup alternative normally, with no normal
dispatch and no owner escape. It does not carry this checkpoint's VM evidence
forward or promote the test-only mutation research.

## Actual code exercised

The test-only `ProductionNativeOwner::initialize_off_research` consumes the
non-cloneable retained epoch/current-Off witness and enters the same
`initialize_under_lease` implementation used by normal initialization. The
coordinator, connection transaction, lifecycle decision and exact-byte pointer
commit are the real implementations, not replacement models. The ordinary
startup variant still requires normal receipts and existence fences; the
historical variant cannot be selected in a production build.
Ordinary transaction admission preserves its original **desired-directory**
pending fence even when the public generic initializer is supplied a distinct
cutover directory. A regression fixture reproduces four host calls in the
intermediate candidate versus zero in the parent; the corrected path again
refuses before any observation/effect. Historical research independently binds
both caller paths to its retained snapshot rather than borrowing this latitude.

After source/package/manager/receipt-bound resync, the original descriptors and
same lease remain retained across initializer checks, observation and pointer
planning/readback. Research accepts only already-settled Off and a pointer plan
whose actual commit is `NoChange`. Stopping an owned core, starting/recovering a
connection, repairing compatibility pointers or enabling startup is forbidden.
Changed proof, source or caller paths refuses. No historical record is unlinked,
rewritten or converted into a generic pending exception.

The returned opaque `OffResearchOwner` contains the real owner, marked stale and
login-disabled, but exposes only existing startup observations. It cannot yield
that owner, coordinator, dispatcher or registration authority. Marking ownership
stale alone would not suffice: internal dispatch delegates to the coordinator,
which is why this wrapper intentionally provides no such access.

The test-only fixed-path `current_off_research` composes existing System epoch
proof and native host observations, without normal `cleanup_probe_orphans`.
It requires the existing lock; research marker reads use `read_marker_existing`
after proof review rather than the ordinary potentially creating reader.
Likewise lifecycle/transaction desired reads use the existing read-only snapshot
reader in research; normal reads retain their existing prepare/chmod behavior.
Success tests assert the original state-directory ctime is unchanged.
It is compiled, not executed against installed state in this checkpoint. Fixture
tests enter the same initialization seam with private real files and an explicit
host whose effect methods panic; no primary-host runtime operations occur.

The five research tests (44.58 seconds) and ordinary desired-directory regression
(0.06 seconds) passed sequentially in the isolated Dev VM at corrected
implementation `dc781edbeb4bf761714e9d81b26ae3e9e064c034`, test-binary SHA-256
`54eb7ab53100527e32c30203a49f39b63d00eb04526e907a4178664cfc6ac7ed`.
The installed runtime PID and fixed desired/profile/template fingerprints were
unchanged. Only the uploaded test binary and empty private scratch directories
were removed afterward. This is actual-caller synthetic filesystem acceptance,
not installed System-proof/current-path, normal admission or network acceptance.

## Evidence and limits

Five focused real-caller tests cover Commit/Abort no-change success with complete
snapshot/member preservation, normal initialization still refusing afterward,
owned-stop refusal, late receipt loss, stale manager, missing receipt, redirected
desired path, an otherwise valid live store needing pointer repair, and a renamed
original state directory that must not be recreated on refusal. Earlier
epoch/resync crash tests remain in the full suite; this slice adds no new write
or durable protocol. Exact full-suite results belong to the PR/head report.

Product policy remains unapproved. This is a Dev-only test of the proposed
same-UID/exact-generation history interpretation, not permission to admit normal
mutations. The remaining contract caller matrix still requires actual effect,
replay and background integration; subsequent restore/ownership rollover cannot
reuse historical authority. Installed package/manager/host acceptance, normal
restore UX/IPC and missing/new-manager receipt recovery remain separate work.
No new conditional-close-style companion ABI is invented or required here.
