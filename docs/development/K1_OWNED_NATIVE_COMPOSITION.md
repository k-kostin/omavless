# Owned native K1 composition candidate

SOURCE only, unregistered. Composes fixed-client source `24b91bb4` and preparation
source `8608857b` into the existing LifecycleExecutor/NativeLifecycleHost. The
actual #676 client/service boundary result does not accept this new native path.

Historical #679 `netguard-native-scenario` exposed the explicit library function
`lifecycle::developer_protected_roundtrip(existing_executor, profile_id)`. It
consumes an existing native executor, constructs one lazy FixedClient, connects
and explicitly disconnects on success. It never disconnects as error cleanup.
There is no CLI registration, ordinary factory replacement, service installer,
root launcher, package change or default-feature activation. The caller must
already own the ordinary runtime lifetime/desired-store authority; the function
does not acquire a new daemon lock or permit parallel owners. A future executable
driver must prove that custody before calling this function. The source
continuation below removes that bare-executor entry in favor of whole-owner
custody; it is not an additional parallel factory.

## Same-owner qualification continuation

The private `managed_pair::qualified_receipt` decoder derives from exact T3
`4d4747af47d60956b370e588469a3b9472ed42a1`, with a typed family result added.
Strict three-patch close/Meta and four-patch close/omavless0 schemas retain their
exact patch/tag/feature/device/enrollment vocabulary; legacy two-patch parsing
is unchanged. All three families may receive ordinary package compatibility
checks, but protected eligibility and its immediate pre-Arm recheck require
the distinct four-patch family. `ProtectedPairIdentity` still binds the exact
receipt/core/broker digest triple. Neither family recognition nor a decoded
receipt is socket coverage. The pre-validator policy decision remains
unconditionally Unsupported; the later private constructor is unreachable.

`ProductionNativeOwner::protected_developer_roundtrip` is private, feature-only
and unregistered. It consumes an already constructed real owner and its real
unregistered singleton, installs a whole-graph custody guard before acquiring
the original MigrationLock, and requires committed Rust ownership, idle empty
mutation scheduling, no batch, disconnected state and safe auxiliary custody.
The existing executor stays inside its original transaction: ProtectedCandidate
borrows it for the entire sequence, never extracts or replaces it. No ordinary
executor or host is returned. The coordinator is permanently blocked before
entering the private sequence. Named/held singleton socket and owner-lock facts,
the same migration lease, exact ownership generation, login receipt and pending
transaction absence are checked before each local operation/exchange.

Error/unwind retains the whole original owner, singleton and migration lease;
the protected candidate retains its original fixed client and admission too.
Only positive Closed after stop/empty/Disarm and a distinct final healthy Status
with the same closed generation may retire the graph normally. That final Status
is reached only after known successful Disarm, never after uncertainty.
Fatal process loss is unavailable, not descriptor survival or recovery. There
is no ignored executable selector, normal constructor registration, traffic
callback, privileged helper, cleanup or issuer override in this continuation.
Current qualification may reach fixed Status, but the closed issuer still
prevents validator execution and Arm. Real same-owner validation/start/readiness,
protected traffic, stop/Disarm and failure integration remain unexecuted.

Continuation checks: seven receipt/package cases passed; the protected filter
passed 35 cases with the existing ROOT-only renderer case ignored. Strict
all-target Clippy passed for the explicit scenario feature, headless defaults
and ordinary defaults, plus package formatter/diff checks. Tests use inert
package bytes and mock lifecycle/port effects; none executed a native core,
NetGuard exchange, broker, namespace, BPF program or VM action. The initial
new final-Status test transcript lacked its last event and a mock branch was
temporarily placed in the wrong test type; both source-only failures were
corrected before the passing gates. No coverage issuer was enabled.

### Exact four-family validator evidence

ROOT selected the separately reviewed fixed `-t` gate on boot
`682f9d46-d8f0-425f-a9f4-308475bef0b5`. Exact core
`897ada648fe975718ac1b7318702def5b826a9901797a0d13cdd333a012b9fcb`, broker
`4bbba825bc39209cd821af18f7a776825bf762c46a05e43f4354aca283009c70` and receipt
`c6e283d9c8b4c2fa59163e6a363b93788a36b210f33d5d9dee2d8b5eec5bd8f2` were
held/pinned, with actual core file capabilities unchanged. The SAME canonical
Rust-rendered config was held unchanged; only a separate fresh exclusive
`/run/omavless-k1-four-data24` was created by the file-only administrative stage.
Existing runtime cache and T4 scopes were not repaired or removed.

Stage `dace83e6`, validator `18c9a740` and unchanged observer `f4f9d506` completed
the original-owned validation gate: ROOT run `5a892d` / completion `f378b1`
returned original SSH0 and `K1_VALIDATOR_ORIGINAL_ZERO_NO_SOCKET_EVENTS 10`.
The strict parser, held-input/empty-data postchecks, original observer0 and
core0 statuses and reaps preceded that marker. Separate bounded observation
`8e04e3` reported one exec, four thread creations and five exits; no socket,
network or mark events, with both stderr captures empty. Trace386 bytes SHA256
`88dacbfb68934df3b4dd20f883e1bd4d523f747e74f5de750addb1d8f08acdb0`;
private core stdout485 bytes SHA256
`7de65fb28946303065606b47ebccdafce2f3bc1eef94a36bf0151d0b47d25857`.
The snapshot's two unrecognized lines were the exact allowed blank finalizer
trailer, not unknown events accepted by the parser.

This accepts only that bounded exact-core/config/fresh-data validation case.
It does not execute the native owner candidate, establish runtime failure-path
coverage, authorize Arm, or open the private coverage issuer.

## Owning contract

The private sealed ProtectedHost trait adds a move-only associated admission;
it does not broaden public LifecycleHost into a privileged command interface.
ProtectedCandidate still exclusively owns the same executor and protection port.

1. Verify disconnected/empty owner and managed-pair eligibility; obtain one
   verified Status and choose a generation strictly above both known floors.
2. Generate and exclusively stage the canonical #674 policy. Hold core/config
   identities and bytes, store digest, exact desired state and data-directory
   identity. This supersedes #674's two-record-handle count: the bound record now
   also retains one data-directory descriptor. The count is not peak descriptors.
3. Ask the private policy decision gate for this exact core and pair identity.
   **The gate always returns Unsupported.** No digest
   is currently admitted, no caller boolean/JSON receipt overrides this, and no
   real validator can be spawned by the current candidate. Neither package
   provenance nor readiness supplies socket coverage.
4. The implemented downstream validator clears its environment to LANG=C, runs
   the exact core with fixed `-t -d -f` shape, and moves the original Child plus
   entire bound inputs into Validation. Only original observed/reaped success,
   within budget and with input postchecks, returns those bindings. Rejected
   original exit returns no admission. Timeout, poll error, late exit, changed
   input, pause panic or unwind retain the original child and input graph together;
   there is no kill, PID reopening, retry or generic discard. Retention lasts only
   as long as the process: process death is not an independent custodian.
5. Only validated bindings plus accepted coverage can construct ArmAdmission.
   Reserve generation durably, then recheck admission immediately before Arm.
   No subsequent start is possible without consuming that admission. Uncertain
   preparation is terminal, unlike ordinary prepare/restored behavior.
6. Protected start stores its returned OwnedCore BEFORE readiness checks. It uses
   fixed environment and exact protected staging path, not ordinary staged YAML.
   Typed readiness checks Full selectors, managed DNS ready, fixed device and
   routing flags; fixed device also scopes TUN observation. The core's serializer
   omits false auto-redirect, so absent or false is accepted; true/mistyped refused.
   These checks are not mark coverage. Original PID/controller and interface
   identity still come from existing OwnedCore/NativeLifecycleHost observation.
7. Protected commit retains the exact launch config rather than overwriting
   ordinary config.yaml. Disconnect writes intent, calls existing owned stop,
   confirms no owned core/TUN, and only then Disarms. Protected discard preserves
   the staged record and grants no unlink/retry/restart/adoption authority.

Normal spawn retains its previous environment semantics; only the new protected
spawn clears environment. No ordinary template or active config is rewritten.
The developer roundtrip does not expose ordinary route/UI/diagnostic consumers
which expect active config.yaml. Integrating those consumers is later product
registration work, not silently enabled here. Unknown or abandoned armed owners
remain retained, including the same FixedClient. No release/marketplace authority.

## Exact-core software coverage experiment: proposal, NOT execution authority

ROOT reported these read-only managed-package facts from the restored VM:

- Core `/usr/lib/omavless-dns/mihomo`, 61079712 bytes, SHA256
  `1da6469cd2d122ddc9073835ba5fcee083509e1c76845fb67b8efacb8f448619`.
- Broker, 5125440 bytes, SHA256
  `c1cd46ca4fd13732b46ef75a89dc6eb1640c579ca89fcbfc504e0229a2304d76`.
- Corresponding-source archive SHA256
  `96920bc6bef3e0f4d2b6e848b6fc66ef771e30b3258a0ef4594e6ca06dc88cdc`.

Before a run, ROOT must freeze the full source-receipt digest, selector bytes,
installed root-file identities, capabilities/xattrs, broker unit/enrollment and
trust-store baseline, exact package build options/dependencies and source mapping.
An unrelated `/usr/bin/mihomo` is not this pair. No acceptance of this pair is
implied merely by matching ManagedPair's source receipt. Preserve previous VM
disk/NVRAM and use a disposable clone; ROOT remains sole VM operator.

Use a separately reviewed controlled VLESS/TCP/TLS peer on a routed RFC1918
numeric endpoint reached through a veth peer network, NOT loopback. Exact source
`component/dialer/mark_linux.go` skips non-global-unicast destinations; a loopback
socket cannot establish the required mark path. Keep the real service/runtime in
canonical guest namespaces; isolate only test peers, not service origin.

Use a synthetic UUID and ephemeral fixture-only CA/server keys; no real user
profile, provider, credentials or production keys. Retain keys privately, never
in Git/output. The TLS peer verifies/decodes VLESS and forwards only to two fixed
test destinations: a bounded HTTP responder and a TLS DoH responder. Certificate
verification stays enabled; install the ephemeral CA only in the disposable VM's
ordinary trust store by a separately reviewed admin step. Give VLESS its fixture
SNI and DoH an IP SAN for the unchanged `1.1.1.1` endpoint. The controlled proxy
maps that fixed tunneled destination to the local DoH peer; do not change the
generated policy to an arbitrary resolver or allow internet forwarding. DNS
replies map one `.invalid` fixture name to the fixed HTTP endpoint. Run an
independent UDP/TCP port53 sink to detect forbidden direct bootstrap/fallback.

First examine exact `main -t` -> config.Init/Parse source closure for filesystem,
network, subprocess, DNS, CA, geodata/provider and cache side effects. Then execute
the canonical candidate with `-t` alone under the exact intended credentials and
fixed environment, with bounded passive syscall/network observation and before/
after data-directory/file inventory. Require original successful reap, no child
descendants, no network connect/send and no unadmitted data-dir mutation. Syntax
success alone is insufficient. Do not give the validator or tracer a bypass mark.

For runtime coverage, separately review passive socket/packet instrumentation
that preserves the actual file capabilities/credentials. Ordinary ptrace around
a file-capability executable can alter its privilege transition and is not
automatically equivalent evidence. Prefer a bounded root-observed kernel socket
mark/connection trace plus counter-only observation of the fixed test egress;
freeze the exact instrumentation before ROOT executes it. No diagnostic accept
rule, broad UID bypass, SO_MARK injection or modification of foreign firewall
policy. Correlate original process/socket identity, destination, SO_MARK assignment
before connect/send and marked egress with the peer's bounded request facts.

Cases must include numeric HTTP success, DNS-over-proxy then HTTP success, repeated
connections and reconnect, TLS rejection, wrong UUID, peer reset/refusal/timeout,
DoH certificate rejection/SERVFAIL/reset/timeout, cache cold/warm and repeated
startup/stop. Prove no direct UDP/TCP DNS, no unadmitted DoH path, no new socket
class on retries/failures, and no marked auxiliary probes. IPv6/UDP/alternate
transports, domain proxy endpoints and unsupported options remain refused. Record
source audit of DefaultSocketHook/custom-dialer reachability: absence in one
packet capture is not proof that another path cannot exist.

Only after the exact-core policy closure and tests are independently accepted may
ROOT authorize a source change to the private issuer. Then run the SAME native
driver under real Status/Arm/start/readiness/stop/Disarm with packet tests scoped
separately. No post-start observation retroactively authorizes Arm. Physical
NIC/suspend/boot, production ownership/factory registration and release acceptance
remain separate; this plan neither executes nor grants those effects.

## Source gates

### Fixed native observation interval candidate (issuer still closed)

The private same-owner roundtrip now places one move-only traffic interval
between protected readiness/commit and explicit disconnect. This is not a new
owner, service, public callback, observation receipt or caller-provided approval.
The pre-validator policy decision still returns `Unsupported`, so this source cannot reach Arm or
execute the interval through the native driver. ROOT must separately approve
exact-core qualification and review an issuer change before activation.

The interval uses only the ROOT-approved VM-only, root-owned `0755` test ELF
`/usr/lib/omavless-netguard/development-native-tests`. Its fixed ignored Rust
selector has no path/URI/command input: after one stdin release byte it issues
one A/IN query for `probe.k1.invalid` from `198.18.0.1:40531` to
`192.0.2.53:53`, requires the exact zero-TTL `192.0.2.80` answer, and sends the
fixed HTTP request to `192.0.2.80:80`. No mark or bypass is assigned. The helper
exists only in the test binary, not a product CLI or package registration.
ROOT will independently admit the exact ELF bytes when provisioning that VM
artifact; a root-owned arbitrary executable is not package qualification.

Before releasing traffic the native parent retains the original `Child`, actual
child executable FD, original source ELF and private captures, and checks the
actual image identity, original parent/UID and zero permitted/effective/ambient
capabilities. The child waits on its original stdin barrier. The 30-second parent
deadline and 20-second fixed traffic deadline are finite, without retries. The
child sets its file-size limit to 4096 bytes before traffic; parent capture reads
also enforce that bound. Whole stdout/stderr grammar is checked only after
original zero WNOWAIT, then source/capture ownership and original reap are
required. Live polling reads only bounded metadata, not a falsely stable live
byte snapshot.

There is one interval slot, never take/reinsert/retry. Before acquisition a
private capacity record counts the current WHOLE process FD inventory (including
its temporary directory iterator) and reserves 24 additional FD roles within a
fixed aggregate 256 and the existing `RLIMIT_NOFILE` soft limit. This includes
the already held native owner/core/receipt/singleton/lease/runtime graph, rather
than pretending the interval starts in an empty process. The 24-role allowance
covers the six persistent roles below, temporary proc-status reads, output
clones and overlapping stdin/exec-error/spawn plumbing; no limit is raised.
The exclusive idle-owner/auxiliary fences precede this admission. Repeated
inventory/rlimit checks refuse growth beyond that reserved ceiling; this is a
software role reservation, not an atomic reservation of future Linux FD numbers
or a promise that allocation cannot fail. Allocation failure retains the
reported original prefix and never evicts an uncertain graph to make capacity.

Persistent interval FD
roles are source ELF, capture directory, stdout, stderr, current executable and
stdin write barrier (six); proc-status reads and standard-library spawn pipe/
duplication descriptors are temporary, so six is not a peak-descriptor claim.
The original child is retained immediately after spawn, before proc/image reads.
On timeout, unknown/nonzero status, acquisition failure after an original prefix,
source/capture drift or unwind, the interval holder retains its owned prefix and
the existing outer guard retains the WHOLE armed native owner, singleton and
migration lease. No automatic stop, signal, kill, compensation or reconnect is
introduced. Fatal process death remains unavailable, not descriptor survival.

After positive original traffic zero/reap, the same candidate rechecks current
desired generation/profile/mode, managed pair, held core/config/store and data
directory identity, actual configured core/TUN and original owner/singleton/lease
before its existing explicit stop, empty-TUN, Disarm and distinct final Status.
Runtime data cache creation is allowed without treating directory timestamps as
immutable; directory inode/ownership/mode/link identity remains bound.

ROOT separately owns the original BPF observer and packet census. Native code
does not receive its PID, result, receipt or boolean and cannot turn it into Arm
authority. A native positive traffic interval alone is not whole acceptance:
ROOT additionally requires its original observer zero, complete strict socket
and mark-before-connect/send census, packet/leak checks and all declared finite
fault cases. Ordinary constructors/defaults and the unsupported issuer remain
unchanged. No ignored selector, real child, network or VM was executed by the
source writer.

Interval SOURCE gates: the complete `protected_` filter passed 43 tests with two
ignored selectors; eight of the passing controls directly cover interval
ordering, post-traffic refusal, one-use, original-prefix Drop/unwind, fixed DNS
bytes, child privilege/parent, capture replacement/overflow and aggregate
capacity boundaries. Feature all-target strict Clippy, default check, scoped
format and diff checks passed. An initial exact-name filter selected zero tests;
it was not counted as evidence. The first protocol test used an incorrect literal
header length and failed before any traffic; the exact-length assertion was
corrected and all affected controls rerun. No ignored test or VM was selected.

Locked/offline targeted protected tests: 30 passed, zero failed/ignored (mock
validation and lifecycle plus inert private-file preparation). The broader
`core_readiness` name filter also passed 11 tests; that filter includes the
existing synthetic shell-child/private Unix-socket foreign-controller test,
not a real Mihomo process or network interface. Strict feature Clippy and default
check passed. No installed service, FixedClient exchange, managed core, VM,
physical interface, nft rule or broker execution was performed for these gates.
Initial test failures were the old prepare/restored expectation and missing new
pre-Arm admission event; both assertions were updated to the intentional stronger
uncertainty contract and rerun successfully. Source review must precede any
accepted issuer or actual native execution.

## Protected Rule policy successor (source only)

The old Global-mode transport matrix does not establish a TCP-only policy:
the qualified core's VLESS constructor enables XUDP for empty packet encoding,
and Global UDP dispatch does not enforce the proxy's `udp: false` option.
The protected renderer now emits Rule mode, first `NETWORK,UDP,REJECT`, then
`MATCH,PROXY`, and `tun.disable-icmp-forwarding: true`. The desired Full intent,
fixed mark/device and same-owner lifecycle are unchanged. The coverage issuer
is still closed; no receipt or matrix result is imported as permission.

The exact four-family corresponding source supplies these bounded path facts:

- `listener/listener.go::ReCreateTun` calls `sing_tun.New` without additions;
  its default additions set an empty SpecialRules and no SpecialProxy. The
  ordinary TUN TCP/UDP handlers construct fresh metadata. No arbitrary inbound
  or tunnel listener is present in the closed generated configuration.
- `listener/sing_tun/dns.go` intercepts matching port 53 before the ordinary
  packet handler. The fixed numeric DoH `#PROXY` resolver is an intentional
  separate internal SpecialProxy path, not permission for TUN UDP to bypass
  the Network rule. Ordinary UDP reaches `tunnel.match`; Network matching does
  not resolve a hostname. REJECT supports UDP but its packet connection is
  in-memory, not a socket dial. The following Match rule selects sole PROXY
  for ordinary TCP. No provider, rematch or user rule is generated.
- `listener/sing_tun/prepare.go` returns before `ping.ConnectDestination` when
  the ICMP-disable flag is set. In the selected system stack,
  `stack_system.go::processIPv4ICMP/processIPv6ICMP` discard non-echo/code-nonzero
  input; echo can receive a locally synthesized response after that nil route.
  A local echo reply is explicitly not a remote reachability result.
- Controller readiness reads actual Rule mode, exact ordered enabled Network
  and Match rows, empty rule providers, sole currently selected PROXY member,
  managed DNS ready and explicit ICMP-disable true. This remains a read-only
  readiness assertion, not a permanent lock against controller mutation and
  not socket coverage or Arm authority.

Before an issuer decision, separately render using the existing ignored Rust
peer renderer from this exact successor (never reconstruct JSON in the fixture),
validate with the held original core, then observe fresh DNS/TCP success,
non-DNS UDP rejection and ICMP no-network cases. Each rejection case needs
positive inner probe delivery plus no corresponding outer socket/packet; a
missing packet or fake echo alone cannot certify rejection. Preserve all old
Global matrix evidence under its original hashes. Native Arm/observation/stop,
leak/crash and repeated-cycle acceptance remain separate required work.

The private readiness expectation distinguishes desired Full/Global intent from
actual protected core Rule mode. All three existing NativeLifecycleHost intent
checks (close capture, fresh observation and lifecycle observation) use that
mapping; ordinary readiness still requires literal mode equality. Mode matching
alone never grants controller/TUN readiness or coverage.

## Private Rule issuance structure (still closed)

The Rule issuer continuation retains a private move-only renderer token with
policy version `RuleTcpVerifiedTlsDohV1` and the exact staged config digest.
Only the closed canonical renderer constructs it. No arbitrary JSON decoder,
caller boolean, observer/test receipt or public token can select this policy.

A protected-only package holder captures the original selected-user600 selector,
root755 broker and root644 receipt, checking full named/held metadata and bytes
on every recheck. It admits only the exact897ada/4bbba/c6e triple documented
above after strict four-family verification. The original core is the existing
Bound file; its exact security.capability bytes are read through that descriptor
with feature-only rustix/fs. Ordinary package detection is unchanged. Same-byte
replacement of any captured member refuses; this is new original-file custody,
not a guarantee previously supplied by ManagedPair's path/hash snapshot.

Before staging/acquisition a bounded capacity record counts the whole current
process inventory and reserves16 additional roles within256 and the unchanged
RLIMIT_NOFILE. These cover three package originals, config/data/publication and
validator plumbing plus transient reads; they are not merely a persistent FD
count. The later interval separately reserves its existing24 roles against the
then-current whole graph. Neither admission raises limits or evicts originals.

The no-argument `approved_policy_decision` still returns Unsupported before any
validator spawn. Thus this source cannot execute validation or Arm through the
native driver. Downstream structure now constructs Coverage only after the
same Validation owner returns original observed/reaped zero with held/package
postchecks; its actual Child adapter uses try_wait, not WNOWAIT. Bound is restored
into the same host before any late fallible issuance step. An acquisition guard
retains Bound on a spawn error/unwind. A private nonescaping validated view then
binds exact policy version/config/core/package into Coverage and ArmAdmission.
Immediate pre-Arm/start and interval checks retain the same original binding.

Enabling requires a separate accepted Rule-policy decision and full source
review. The old Global matrix, actual Rule validator, new UDP/ICMP case outcomes,
and native observer/interval proof remain separately scoped evidence. No current
nonzero/unknown observation becomes acceptance by this structural source change.
There is no native activation, ordinary-default change or successful test-only
substitution of an installed package. Focused controls use inert files and
memory: exact-member mismatch, same-byte inode replacement, mutation/mode/link,
missing capabilities, token mismatch, capacity refusal, closed entry and owning
post-validator constructor placement; inherited validator/lifecycle fault cuts
remain applicable. Ignored VM selectors must remain unselected by source gates.

This structural checkpoint passed53 protected-filter tests (three VM selectors
ignored), strict all-target feature Clippy, strict all-target ordinary-default
Clippy, scoped runtime formatting and diff checks. The successful Coverage path
was not exercised with a real installed package; no core/validator, service,
socket traffic, privileged operation or VM action was selected. The optional
rustix dependency reuses the existing locked1.1.5 version; no new version or
default feature is introduced.

## Qualified development Rule decision (successor to the closed checkpoint)

The private static decision now permits only the existing development feature's
closed Rule policy, after independent qualification on the disposable x86_64 VM.
Exact Rule renderer head: `f1f575d13e7a43a04f73dbc4bdfde20ad3c6a008`.
The original success, non-DNS UDP rejection and ICMP no-network runs all exited
zero with complete target socket census and original child completion. Every
case included DNS/HTTPS success; rejection cases delivered exactly one inner
probe with no corresponding forbidden outer packet. The unconditional
wrong-peer DROP rule rejected six packets before each probe window, with no
increment during the probe. This is not a zero-rejected-traffic or kernel-RST
origin claim. Existing loopback and established SSH exceptions remain explicit.

The earlier Rule41 refusal and old Global matrix retain their original outcomes
and guarantees. Runtime observer output is qualification evidence, not an input,
receipt, token or executable authority for the native issuer. The static
decision introduces no flag or caller-controlled permission. The original
ProtectedPackage still enforces the exact qualified core/broker/receipt triple,
capabilities, retained file identities and package policy. Original validator
zero/reaping and restored same-host Bound postchecks still precede private
Coverage construction; a synthetic or absent package refuses before spawning.

The ignored installed-native launcher uses a valid fixed UUID record ID, with
`k1-native-fixed` only its fixture display name. A normal-store-parser regression
prevents restoring the invalid old display-name-as-ID. No parser relaxation or
product registration is introduced. Native Arm/start/stop/Disarm, repeated-cycle
and fault/leak acceptance are still required; neither this decision nor pure
controls establish those results. Ordinary defaults and release behavior remain
unchanged.

### Installed-native refusal and diagnostic successor

The first composed installed-native attempt used source
`eaeac0fa3c5802c0c269b9c92dce5da0ef5b96c6` and test ELF SHA256
`64019a7c110a34fdae649b7505642c56a43c4664e4bb305647822d59f56a64e9`.
The genuine current-owner constructor completed, but its protected roundtrip
refused before a core exec. No Arm/start/traffic/Disarm acceptance follows.
The original supervisor retained the uncertain graph. Later shutdown was
separate disposable-VM administration; the original SSH ended255, not a
successful original completion, cleanup or product recovery.

The successor adds only test-and-feature-gated, thread-local, closed source
labels. Each label precedes an existing operation; there are no extra probes,
callbacks, retries, authority inputs or altered custody/budgets. Failure prints
one last-entered label only after whole-owner retention. It is not an exact
failed-predicate diagnosis. A print panic cannot bypass the existing park.
Default production builds do not contain this diagnostic module.

Focused diagnostic controls passed56 tests, with three explicit VM selectors
ignored, both with one and two test threads. Strict feature/test and default
library Clippy and scoped formatting/diff checks passed. An earlier unrestricted
parallel invocation had three existing preparation tests fail with Changed;
that invocation remains NONPASS. Process-wide FD inventory can interfere across
concurrent fixtures, but the failing predicate was not captured, so the cause is
not claimed as established. Serial gates do not waive the unchanged production
256-FD bound or count as a passing unrestricted parallel suite.

### First exchange localization (development successor)

Later disposable-VM attempts kept the original nonpassing outcomes. Native44
refused on the private fixture prelude; Native45 never released its native
barrier because the observer's TUN module hooks were unavailable. These are
not runtime Arm/start results. Native46 added early read-only module/BTF
preflight and separately loaded the module by ordinary VM administration.

Native46 used the same runtime `2f2586fc0f3aba48ed215922add4f195cf970483`
and ELF `7cd8844a32763629ebf00ba119f48e55d80aa9f84a5a3c2dce56c3b58b090ea9`.
Its current owner passed construction, but the first FixedClient Status exchange
refused before validator/core execution. The closed last-entered label was
`status_exchange`, not `status_interpretation`. Ordinary service startup reached
READY, published the required endpoint modes/group, and had an empty retained
state domain. This does not prove whether transport, endpoint validation or
decoded response verification failed; no new Status/Recover was sent.

The new explicit `netguard-client-diagnostics` feature distinguishes those
existing client operations through thread-local closed source labels. It is
selected only by the private native developer scenario, not defaults or the
service feature. The ignored launcher prints the last client label only after
whole-owner retention. No response payload, errno, new probe, retry, deadline
change or authority input is introduced. Like the earlier labels, this is the
last operation entered, not necessarily its exact failed predicate. The
earlier attempt remains NONPASS; a separately admitted exact successor and
original completion are still required.

### Post-startup service refusal localization

The next exact developer interval passed client endpoint/peer checks and wrote
its first request, but stopped at `read_prefix` without any core exec. READY is
only startup evidence: every original idle/accept/exchange boundary still
rechecks retained manager/package/namespace authority, and refusal parks the
whole service with its listener retained. Neither transport timeout nor an open
socket proves a particular failed predicate or allows resending the request.

The explicit default-off `netguard-service-diagnostics` successor retains only
thread-local closed source-stage labels and the first already-computed origin
failure frame after startup. Marking introduces no I/O, new manager/kernel
query or authority input. Immediately before the existing terminal park it
writes one finite stage/reason frame and, if present, that finite origin frame.
There is no per-idle output; repeated terminal reporting is suppressed before
the first write. Failed or partial output parks without retry/unwind/teardown.
Existing guards, latches, budgets, listener and recovery semantics are unchanged.
The original failed interval remains NONPASS. A separately admitted exact
service/native successor must establish the actual failure and any later fix;
these diagnostics are not themselves K1 acceptance.

### Original-envelope diagnostic slice

The Native50 attempt remains unknown/nonpassing. Its last-entered
`origin_envelope` label does not identify which envelope predicate refused or
which repeated invocation reached it. The separate test-only successor splits
the existing short-circuit expression into the same four checks, in order:
original migration lock, ownership marker, pending transaction fence, login
receipt. Each has a closed pre-operation label; no predicate is evaluated twice
and no new syscall, authority input or fallback is introduced.

The ignored launcher preserves its existing three diagnostic lines and appends
`K1_NATIVE_ORIGIN_DIAGNOSTIC <site> <ordinal>` inside the same catch-unwind after
whole-owner custody retention. The thread-local ordinal increments only at the
existing origin closure entry and saturates at255. Sites are fixed initial,
local, status, arm, disarm, preparation, interval_before and interval_after.
They are source locations, not successful effects or identity/generation data.
On the straight-line connect path ordinals7/8/9/10 enter the reserved-Desired
write, Arm exchange, connected-Desired write and owned-core start respectively;
an error branch is interpreted from its exact source, never from that mapping
alone. Ordinal saturation has no effect on execution. Reset and thread isolation
are tested. Default/non-test builds contain neither this state nor the output.

No failed Native50 context is resumed, no installed run is accepted, and no
timeout, custody rule, pending fence, package qualification or issuer changes.
The serial protected filter passed59 controls with three VM selectors ignored;
strict headless scenario all-target and ordinary-default library Clippy, scoped
formatting and diff checks passed. The new actual mock-candidate call-sequence
control checks the above ordinals through connect, interval and explicit close.
An initial redundant match fallback produced a compiler warning; it was removed
before the final strict gates. These are SOURCE controls, not real native effects.

### Nested startup-receipt diagnostic (SOURCE only)

Native51 remains NONPASS/unknown. Its closed `origin_login`, `local`, ordinal10
labels reach the startup-receipt check before owned-core start. A later fixed
read-only projection found a consumed receipt matching the Rust ownership
generation and absent pending members; that does not reconstruct the original
failed read or prove process descriptor exhaustion. Desired/NetGuard generation
advances are distinct from ownership generation; this checker has no clock or
user-manager epoch query.

The successor observes failure branches of the existing startup checker and
its original receipt reader, only in the native test executable. It performs no
additional read, query, retry or guard evaluation. An invocation-scoped
thread-local observation resets at checker entry, records only the first closed
failure class, and deactivates on return/unwind. Other login calls cannot replace
that record. The existing four diagnostic lines remain; a fifth fixed line,
`K1_NATIVE_LOGIN_DIAGNOSTIC <reason> <io-class>`, is emitted only inside the
existing post-retention catch-unwind. `no_failure` means no recorded branch,
not a successful operation receipt. Phase/generation mismatch is deliberately
one class, preserving the original combined short-circuit predicate.

Existing metadata errors can expose finite errno classes without another
syscall. The store reader erases open/read errno into its existing `Io` error;
`store_io_opaque` explicitly preserves that uncertainty, never claims EMFILE.
No raw error, path, receipt field, epoch or generation is printed. Default and
non-test builds have neither observation state nor output. Real filesystem
controls invoke the actual startup checker for consumed/absent receipts,
Desired-only advancement, ownership mismatch, pending phase, malformed/schema/
UTF-8/mode failures and pending-before-receipt short circuit. They are local
SOURCE controls, not native admission or recovery of an earlier VM attempt.

Focused serial gates passed21 login controls and59 protected controls with three
VM selectors ignored. Strict headless-scenario all-target and ordinary-default
library Clippy, scoped formatting and diff checks passed. The first test compile
refused an ambiguous diagnostic-module import; the explicit module path fixed
that test-only error before the final gates. No VM invocation or release/native
ELF rebuild is included in this SOURCE checkpoint.

### Development VM prerequisites before network-preparation admission

The disposable VM's ordinary prerequisites must be verified **before** selecting
the fixed network-preparation recipe, not discovered through repeated effecting
wrappers. A fresh boot alone does not imply these prerequisites are ready:

- Confirm the exact newly admitted boot, inactive normal runtime, absent fixed
  TUN/link/netns/recipe scope paths, and the unchanged pinned service image.
- Confirm `/sys/module/tun` is the admitted root-owned directory and
  `/sys/kernel/btf/tun` the admitted root-owned regular BTF file, neither writable
  by group/others. If absent, the VM operator may separately authorize the fixed
  ordinary `modprobe tun` action, then verify both paths. Never substitute a
  missing BTF file or weaken the module/type/ownership checks.
- Confirm the normal user login-prepare unit completed successfully for this
  boot and its genuine `/run/user/1000/omavless-login.receipt` exists with the
  required user ownership, single-link0600 shape and bounded size. Do not copy
  an old receipt or manufacture a current one. Preparation's metadata check is
  readiness only: the normal Rust constructor still checks the original startup
  receipt, ownership generation and pending fences.
- Confirm the genuine installed native driver and its ordinary prerequisites:
  the exact selected test ELF and managed core/package identity, the normal
  DNS broker's enrolled fixed device/policy and active service, and NetGuard's
  original enrolled service with its pinned image. Both fixed control endpoints
  must be actual root-owned mode0660 sockets. A genuine current login-prepare
  receipt, DNS enrollment and NetGuard enrollment are separate prerequisites;
  none is manufactured by the fixture or inferred from a fresh boot.

The fixed network-preparation successor now checks the pinned NetGuard public
image, then the exact root-owned0755 DNS-broker image with its bounded size,
group and xattr policy, and both fixed socket metadata shapes before the current
receipt, scope publication and first network command. Its bounded read-only
preflight does not send a request, construct the native owner, prepare a login,
enroll a service or grant Arm/cleanup/retry authority. A separately completed
ordinary VM module/service preparation and original-zero network preparation
establish only those prerequisites. The local frozen supervisor and preflight
remain outside Git; they are neither installed product code nor native lifecycle
acceptance. The original current-owner constructor remains an effecting operation
under its own whole-owner custody guard.

In the fixed recipe, module/BTF checks precede service-image and receipt checks;
receipt absence alone cannot explain a refusal when the earlier module checks
also fail. A known original exit2 at `PREFLIGHT`, with `attempted=false`, proves
the wrapper did not enter its `NETWORK_COMMAND`/spawn path. It does **not** alone
prove no filesystem publication: the scope mkdir immediately precedes that
phase assignment. A separately admitted bounded scope/link/netns absence check,
the unchanged no-cleanup source and no intervening removal premise are required
before classifying an exact refusal as pre-effect and admitting a new ordinary
preparation. Read-only builder descriptors may have existed; do not claim all
descriptor custody from this classification.

This does not permit retrying a parked native original, compensate an uncertain
effect, adopt old durable/kernel state or declare native completion. Preserve
each refused preparation and its original result separately from a later newly
admitted preparation, and require the latter's original zero before selecting
the one retained foreground native run. Native effect selection remains solely
with the VM operator; preparation never executes the native root through SSH.

### Preparation-first interval scratch portability successor

Ordinary `NativeHostPaths::current` uses the same config/data directory. The old
interval created its `.k1-native-interval` child after capturing that directory's
original identity. On filesystems where child `mkdir` increments the parent's
link count, the unchanged strict post-interval check could refuse the program's
own publication. This is a bounded portability defect, not an established cause
of an earlier btrfs native refusal.

The source successor exclusively publishes and captures the private scratch
before `Bound.data` captures its original. Its move-only holder survives every
failed acquisition or abandoned prefix and transfers once into the existing
interval holder. Later traffic captures are regular files inside that original
scratch. No original link count is rebased, masked or permitted to drift; device,
inode, full mode, UID, GID and link count remain exact for both held and named
data-directory observations. The pre-validator, validation and pre-interval
checks also recheck the same scratch original. Collision, metadata drift or
failure supplies no overwrite, unlink, cleanup or retry permission.

The additional held scratch role raises the preparation reservation from16 to17
without changing the256 aggregate bound or RLIMIT_NOFILE. The interval's existing
24-role reservation includes the already held scratch through its whole-process
inventory and then consumes that descriptor. Private captures and the scratch
remain owned by the original prepared/interval graph; uncertain graph retention
and fatal-process-loss limits are unchanged.

Deterministic controls reject a `+1` link-count change and every other identity
field change. A real HOME-backed ordinary aliased config/data fixture exercises
the actual staging and capture-file code without an executable core, socket,
service, admission or network action. A separate HOME child-mkdir control reports
`K1_HOME_CHILD_NLINK_INCREMENT_UNAVAILABLE` when that filesystem keeps the count
unchanged; that branch is not real-filesystem `+1` reproduction. Native execution,
whole K1 acceptance and product/release activation remain separate exact-head
gates, and all earlier NONPASS native outcomes retain their original identity.
