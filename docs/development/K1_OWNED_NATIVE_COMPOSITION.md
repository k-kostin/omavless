# Owned native K1 composition candidate

SOURCE only, unregistered. Composes fixed-client source `24b91bb4` and preparation
source `8608857b` into the existing LifecycleExecutor/NativeLifecycleHost. The
actual #676 client/service boundary result does not accept this new native path.

`netguard-native-scenario` exposes the explicit library function
`lifecycle::developer_protected_roundtrip(existing_executor, profile_id)`. It
consumes an existing native executor, constructs one lazy FixedClient, connects
and explicitly disconnects on success. It never disconnects as error cleanup.
There is no CLI registration, ordinary factory replacement, service installer,
root launcher, package change or default-feature activation. The caller must
already own the ordinary runtime lifetime/desired-store authority; the function
does not acquire a new daemon lock or permit parallel owners. A future executable
driver must prove that custody before calling this function.

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
3. Ask the private coverage issuer for this exact core and pair identity (receipt,
   core and broker digests). **The issuer always returns Unsupported.** No digest
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
