# OmaVLESS Full VPN fail-closed contract

Status: accepted K0 threat model and host-integration contract, 2026-08-28.
This document makes K1 implementation-ready. It does not install a helper,
change nftables, alter routes or advertise a working kill switch.

September 29 development candidate: the [offline foundation](../development/K1_FOUNDATION.md)
adds strict fixed protocol, acknowledged helper transactions and symbolic policy
tests. An [offline fixed renderer candidate](../development/K1_RENDERER.md) now
adds JSON creation and strict readback with independently supplied ownership
facts; it has no executor or production caller. K1 implementation,
host acceptance and product availability remain incomplete.
The next [isolated packet-policy harness](../development/K1_PACKET_GATE.md)
tests synthetic egress and near-miss exceptions without a production service;
its results do not replace this contract's host/physical acceptance matrix.
The later [inactive transaction coordinator](../development/K1_COORDINATOR.md)
connects the planner and durable marker using a synthetic kernel port; it is
not a root executor or a working host kill switch.
An [inactive receipt assessment](../development/K1_RECEIPT_ADMISSION.md) now
models bounded receipt decoding and uncertain-outcome refusal. It supplies no
kernel ownership authority, receipt storage adapter or automatic orphan recovery.
The subsequent [shared-lock transaction candidate](../development/K1_LOCKED_STATE.md)
combines durable marker/receipt writes with synthetic kernel effects and strict
pending-phase refusal. It provides no production ownership or recovery adapter.

## 1. Decision summary

K1 is an opt-in, **Full VPN only** kill switch enforced by a separately
packaged root-owned service, provisionally named `omavless-netguard.service`.
The service owns one dedicated nftables table and a small fixed-purpose API.
The user runtime coordinates it, but cannot supply firewall expressions,
commands, paths, unit names, marks or interface names.

Protection follows persisted desired VPN state, not Mihomo, TUN, QML, terminal
or user-runtime liveness. While protection is armed, ordinary unmarked traffic
cannot leave the host through a physical IPv4 or IPv6 route. The policy remains
in the kernel if the core, runtime, UI or NetGuard process crashes.

The first K1 slice is not permanent Lockdown, Routing-mode leak protection, a
general firewall manager, a hostile-root defense or a network-namespace policy.
Explicit disconnect releases protection. LAN application access is blocked in
the first slice; a later opt-in LAN exception requires its own threat review.

## 2. Protected asset and threat boundary

The protected asset is the confidentiality of traffic from ordinary host
applications while the user expects a Full VPN connection. K1 prevents that
traffic from silently falling back to a physical default route or direct DNS.

The initial threat model assumes:

- the kernel, root account and package signatures are trusted;
- the enrolled desktop user is trusted and may explicitly connect,
  disconnect or invoke an administrative recovery flow;
- the fixed OmaVLESS TUN name and core bypass mark are package-owned values;
- foreign root/CAP_NET_ADMIN software can change firewall policy and is outside
  the protection claim;
- container, VM and other network-namespace forwarding is outside K1's first
  supported surface and must not be described as protected.

K1 does not defend against a malicious enrolled user. That user already owns
the private runtime socket and can request explicit disconnect. The privileged
boundary exists to keep broad networking authority out of the user runtime,
not to make the runtime an authorization boundary against its owner.

## 3. Failure classification

### Confidentiality leaks

A confidentiality failure means ordinary application traffic reached a
non-OmaVLESS physical path while desired Full VPN state remained connected.
K1 must cover:

- Mihomo crash, SIGKILL, failed start or failed restart;
- runtime crash, SIGKILL or user-service restart;
- QML, TUI, terminal or complete shell exit;
- partial TUN setup or unexpected TUN disappearance;
- a physical default route installed or replaced by NetworkManager;
- direct IPv4 or IPv6 egress;
- direct UDP/TCP DNS, including a local stub whose upstream request is emitted
  by another system process;
- Wi-Fi loss/rejoin, Ethernet appearance/disappearance and interface switch;
- reconnect attempts and endpoint re-resolution;
- suspend/resume and boot-time network activation;
- stale or partially updated protection rules.

### Availability loss

Fail-closed behavior intentionally prefers blocked internet to a leak.
Availability loss includes:

- traffic blocked while the core reconnects;
- captive-portal or LAN access unavailable while protection is armed;
- a failed core transition leaving the host protected but offline;
- an unavailable helper preventing a new protected connection;
- explicit disconnect waiting for verified core cleanup before disarm.

These states must be visible as `reconnecting`, `failed protected` or
`manual_recovery_required`, never disguised as an ordinary disconnected state.

### Stale-protection lockout

A stale lockout means protection remained after desired state became
disconnected. K1 must make these cases diagnosable and recoverable:

- runtime failure during explicit disconnect;
- helper state/rules disagree after a crash;
- package upgrade or rollback interruption;
- plugin disable while runtime communication is unavailable;
- stale policy after package removal was attempted;
- corrupt root-owned state;
- administrator stops the helper while rules remain installed.

A stale lockout is not fixed by silently flushing a host firewall. Recovery
removes only the OmaVLESS-owned table and state through the fixed root command
defined below.

## 4. Desired and protection state model

Protection is orthogonal to actual core state:

| Desired | Actual | Protection | Meaning |
|---|---|---|---|
| disconnected | disconnected | disarmed | ordinary baseline |
| connected | starting | armed | connecting; direct traffic blocked |
| connected | connected | armed | healthy protected Full VPN |
| connected | reconnecting | armed | core recovery; direct traffic blocked |
| connected | failed | armed | protected failure; user action required |
| disconnected | stopping | armed | disconnect transaction still cleaning up |
| disconnected | disconnected | disarming | kernel policy removal being verified |
| disconnected | disconnected | stale/armed | lockout; reconcile or recovery required |

`connected + disarmed` is never a valid K1 state. The runtime must not start the
core or publish protected connecting state until NetGuard confirms that the
kernel policy is armed.

### Protected connect transaction

1. Validate that mode is Full VPN and K1 is enabled and available.
2. Validate the fixed core mark/TUN capability and any unprotected setup data.
3. Ask NetGuard to arm generation `N`.
4. NetGuard atomically installs and verifies its restrictive nftables table,
   then durably records the armed generation before acknowledging success.
5. Persist desired connected with the same generation.
6. Start Mihomo, verify the owned core, TUN, controller and protected probe.
7. Publish connected/armed or reconnecting/armed.

If the runtime dies after arm but before desired connected is committed, its
restart observes desired disconnected and requests a normal disarm. This is a
temporary lockout, not a leak.

### Explicit disconnect transaction

1. Persist desired disconnected.
2. Stop the owned core and verify service/process/TUN/controller cleanup.
3. Ask NetGuard to disarm the matching generation.
4. NetGuard durably replaces the armed marker with a closed-generation fence,
   atomically deletes only its proven-owned nftables table, verifies absence,
   then reports disarmed. That generation and every older generation can never
   arm again, including after restart.
5. Publish disconnected/disarmed.

If core cleanup or disarm cannot be verified, return
`manual_recovery_required`. Do not release protection merely because a stop
command was issued.

## 5. Selected privilege boundary

### Root-owned NetGuard service

The Arch package installs a small root-owned system service and fixed recovery
binary. It is separate from `omavless-runtime.service` and from the marketplace
plugin. Installing/enabling K1 is an explicit administrator action.

NetGuard alone may:

- create, replace, inspect and delete `inet omavless_netguard`;
- persist the armed generation below `/var/lib/omavless-netguard/`;
- create its private runtime socket below `/run/omavless-netguard/`;
- reconcile the fixed table after its own restart and during boot;
- perform the root-only emergency recovery operation.

It may not read the profile store, controller secret, generated Mihomo config,
subscription data or provider content.

### Client authorization

Initial K1 supports one explicitly enrolled non-root desktop UID. Enrollment is
an administrator action which validates an existing local account and records
only its numeric UID in a root-owned mode-`0600` configuration. The inactive
reader pins `/etc/omavless-netguard/enrollment-v1.json`; provisioning and
account validation remain separate gates. The socket is
root-owned, accessible through a dedicated package group, and every request is
checked against kernel `SO_PEERCRED`; a caller can act only for its own enrolled
UID. The request never carries a selectable UID.

An inactive first-enrollment producer now covers the missing *initial write*
side for synthetic tests. It resolves one unique, non-root local login account
from a bounded, root-owned `/etc/passwd` snapshot and writes only its canonical
numeric UID record to an exclusively created private directory/file. It
synchronizes and reopens that record through the existing enrollment reader;
an existing enrollment, root-state directory, unsafe account source or
interrupted publication refuses without replacement or automatic cleanup.
The producer has no administrator command or installed caller. It does not
create the package group, enable the helper, grant current-session socket
access or report the kill switch as ready. Actual provisioning must also
coordinate with the later package/service lifetime and account-removal policy.

Group membership alone is not authorization. Multiple concurrently protected
desktop users are rejected in v1 rather than given ambiguous host-wide policy.

### Fixed-purpose API

The API is bounded to one 8 KiB request and one 8 KiB response, strict UTF-8
JSON with duplicate/unknown-field rejection and a versioned envelope. It has
only:

- `status` — armed/disarmed, generation, policy version and safe health codes;
- `arm` — non-negative generation and the exact mode enum `full`;
- `disarm` — matching generation;
- `reconcile` — root/internal boot operation, not exposed to the user socket.

The caller cannot provide nft syntax, commands, executables, paths, marks,
ports, addresses, DNS servers, interface names, systemd units or arbitrary
environment values. Responses contain no endpoint, credential or rule dump.

An inactive transport candidate tests one four-byte big-endian length plus one
bounded JSON request/response on an already connected Unix stream. It checks
the fixed root enrollment binding and kernel `SO_PEERCRED` UID before reading,
revalidates enrollment after decoding, and applies one two-second total receive
budget so a peer cannot indefinitely drip bytes. A later inactive composition
dispatches at most one request through the shared-lock transaction with that
same pinned enrollment and rechecks it before a response. It does not bind a
socket, supply a real kernel port, package a service or change nftables. Lost
socket acknowledgement is not an automatic retry ticket. The future service
still needs reviewed provenance, listener lifetime, provisioning and host
acceptance; framing alone grants no privileged authority.

An additional inactive session-owner candidate holds one already-bound Unix
listener, the same locked state and one kernel port across sequential clients.
Each poll accepts at most one client and performs one existing exchange; a
failed accept does not spin, a stalled client has the bounded receive deadline,
and lost enrollment seals the owner before the next accept. Dropping a listener
does not disarm or erase a durable armed record. Synthetic tests cover restart,
replaced enrollment, failed replies and poisoned transactions. This is not a
socket publisher or installed service: the listener path, ownership, group,
backlog, namespace and real kernel-port provenance still need separate proof.

An inactive listener path-admission candidate now pins the already-bound
`/run/omavless-netguard/control.sock` directory and filesystem entry, checks
root/package-group modes, and refuses replacement before or after accept.
This is not complete descriptor provenance: a socket's reported path is not
proof that its descriptor corresponds to the *current* path entry after
unlink/rebind. The future trusted service must bind while group access is
closed, prove that lifetime, then publish access; package-group resolution,
backlog, namespace and installation remain separate. Tests use private
temporary listeners only; no host socket is created or used.

The user runtime never invokes sudo or pkexec. Administrator setup/recovery is
performed intentionally in a terminal.

## 6. Enforcement policy

### Dedicated atomic nftables table

NetGuard owns only `inet omavless_netguard`. Every change is constructed from a
compiled/audited template and committed as one nftables transaction. It never
uses `flush ruleset`, rewrites `/etc/nftables.conf` or adopts another firewall's
table. Observations distinguish proven absence, independently proven ownership
(verified or unrecognized policy), foreign ownership and unreadable/incomplete
facts. Foreign/unreadable tables refuse normal and restart mutations; a fixed
name alone never authorizes replace/delete. Create must refuse a raced table,
and replace/delete must revalidate ownership before committing.

The output policy is route-independent and evaluated late enough that an
unmarked physical path cannot be accepted merely by an earlier base chain.
Root-owned integration tests must verify actual hook/priority interaction with
the stock Arch/Omarchy firewall and common package firewalls before K1 merge.

While armed, the fixed template permits only:

- loopback traffic, excluding any claim that a local DNS stub's upstream
  physical traffic is trusted;
- traffic whose selected output interface is the fixed OmaVLESS TUN;
- physical egress bearing the fixed OmaVLESS core socket mark;
- the minimum DHCPv4, DHCPv6 and IPv6 neighbor-discovery traffic required to
  retain or acquire local link configuration;
- replies explicitly proven necessary for those link-maintenance exchanges.

All other IPv4 and IPv6 output is dropped, including existing unmarked flows.
Rules do not grant a generic `established` physical bypass because connections
opened before arm would otherwise continue leaking.

### Core bypass

Mihomo's package-owned configuration applies one fixed socket mark to its
physical outbound and resolver sockets. NetGuard recognizes only that compiled
mark; it does not accept a mark from the user request. K1 preflight refuses to
arm if the installed core cannot apply and retain it.

The K1 implementation must test the fixed mark on current Arch nftables and
Mihomo. A cgroup-v2 socket match is the bounded fallback candidate if marking
cannot cover every required core socket, but it is not combined speculatively
with a broad UID allow rule. `meta skuid` alone is rejected because core and
ordinary applications run as the same desktop user.

### DNS

Ordinary DNS is not exempted. Application DNS captured by the Full VPN TUN is
allowed through the TUN. Marked Mihomo resolver traffic may use the physical
network. A query to a loopback stub does not create a bypass: the stub's
unmarked upstream physical packet is dropped.

K1 acceptance must test UDP 53, TCP 53, the active host resolver path and at
least one encrypted DNS path used by the validated Mihomo configuration. A DNS
timeout is acceptable during failure; direct resolver success is not.

### LAN and local link

Loopback and narrow DHCP/neighbor discovery are not a general LAN exception.
K1 v1 blocks ordinary RFC1918, CGNAT, ULA, link-local application and multicast
service traffic while armed. This is the least ambiguous initial guarantee.

A future `Allow LAN` option must derive validated connected prefixes and
interface identity inside NetGuard, remain off by default, cover IPv4/IPv6 and
document hostile-LAN consequences. Arbitrary user-supplied CIDRs are not added
to the K1 API.

### Route and interface changes

Because the terminal rule blocks all unmarked non-TUN egress, installing a new
default route or physical interface does not create a bypass. No interface
name learned from a NetworkManager event is trusted as an allow rule.

NetworkManager dispatcher integration may request fixed reconciliation after
up/down/DHCP/DNS events, but it is never authoritative. Upstream documents that
clean `pre-down` is not emitted for forced carrier loss, so dispatcher scripts
alone cannot implement fail-closed behavior.

## 7. Persistence, boot and recovery

### Root state

`/var/lib/omavless-netguard/armed-v1.json` is root-owned, mode `0600`, bounded,
atomically replaced and directory-fsynced. It contains only schema/policy
version, enrolled UID, generation, armed state and fixed-policy flags. It has
no profile ID, endpoint, hostname, URI, password, key or subscription URL.

Missing state means fresh/disarmed only after kernel table absence is verified.
It cannot prove that an interrupted arm left no rules: a stale proven-owned
table requires reconciliation, and foreign/unreadable facts require refusal.
The inactive [root-state foundation](../development/K1_ROOT_STATE.md) implements
only marker persistence, not a service or kernel ownership receipt.
Normal disarm retains a durable
closed-generation fence in the same record, even when no table remains. Future
arms require a strictly greater generation; exhaustion never wraps. Recovery
and upgrade must preserve this fence rather than silently recycling generations.
An existing malformed/newer document requires emergency handling: install the
most restrictive fixed policy only if the table is absent or proven-owned,
then report emergency protection and require explicit recovery. Foreign or
unreadable table facts refuse mutation and report recovery without claiming
protection. Neither case is assumed disconnected.

### Boot ordering

When K1 has been explicitly enabled, NetGuard is ordered before
`network-pre.target` and pulls that passive target in. If persistent state is
armed, it restores and verifies the nftables table before normal network
configuration proceeds. If disarmed, it verifies that no stale OmaVLESS table
exists and leaves other firewall state untouched.

The first K1 package must test this ordering against the supported Omarchy
NetworkManager unit graph; a document-level unit sketch is not boot evidence.

### Process/service failure

- Core failure: kernel rules remain; runtime reconnects while armed.
- Runtime failure: kernel rules and root state remain; user systemd restarts
  and reconciles desired/protection state.
- NetGuard process failure: nftables state remains in the kernel; the root
  service restarts and verifies it from persistent state.
- Shell/TUI/terminal failure: no protection or desired-state transition.
- Direct administrative service stop: does not disarm in `ExecStop`; rules
  remain until explicit disconnect or recovery.

### Disable, upgrade and removal

- Plugin disable/remove follows the T1 lifecycle contract: request explicit
  disconnect, verify core cleanup, then disarm. If runtime/helper communication
  fails, keep protection and report manual recovery.
- Runtime package upgrade preserves root state and kernel policy while the user
  process restarts/reconciles.
- NetGuard upgrade must understand the stored policy version before replacing
  the running helper. Unknown newer state fails closed.
- Package removal performs the fixed root recovery transaction first and aborts
  removal if its own table/state cannot be verified absent.
- The marketplace plugin never installs, enables or removes NetGuard.

### Emergency recovery

The root package provides one console command equivalent to:

```text
sudo omavless-netguard recover
```

It requires a terminal/admin decision, deletes only the proven-owned dedicated
OmaVLESS table and clears armed intent, retaining any known closed-generation
fence. It verifies table/armed-intent absence and prints bounded status.
If the generation cannot be trusted, renewed arm requires separately reviewed
enrollment/generation recovery; simply deleting invalid state must not silently
re-admit previously delayed requests.
It accepts no shell fragment, nft expression, path, table name, interface,
address or command. Recovery intentionally restores direct connectivity and is
therefore never invoked automatically by QML or the unprivileged runtime.

## 8. Alternatives considered

| Approach | Decision | Reason |
|---|---|---|
| nftables inside QML/backend | rejected | broad privilege in UI/runtime; dies with owner |
| CAP_NET_ADMIN on runtime | rejected | grants a large credential-processing daemon unnecessary host authority |
| sudo/pkexec for each transition | rejected | prompts/races in normal operation and encourages broad policy |
| NetworkManager dispatcher only | rejected | event gaps on forced loss and no durable desired-state owner |
| policy routing only | rejected | routes can be replaced and do not independently block IPv4/IPv6/DNS |
| one-shot arbitrary firewall helper | rejected | weak persistence/reconciliation and unsafe input surface |
| root NetGuard + fixed nft template | selected | kernel persistence, atomicity, bounded API and explicit recovery |

NetworkManager and systemd remain integration sources, not alternate policy
owners. nftables is the enforcement authority; routing and dispatcher signals
are supplemental observations.

## 9. K1 acceptance matrix

Every result records exact package/runtime/plugin commits and whether evidence
came from deterministic tests, Try Omarchy ARM64 or bare-metal Omarchy.

| Case | Required result | Environment |
|---|---|---|
| fixed API malformed/oversized/duplicate fields | rejected without state change | deterministic |
| arbitrary command/path/nft/interface input | schema cannot represent it | deterministic |
| arm/disarm crash points | no connected+disarmed state; recovery bounded | deterministic + Try Omarchy |
| atomic table ownership | only dedicated table changes | Try Omarchy |
| IPv4 direct probe while core dead | blocked | Try Omarchy + bare metal |
| IPv6 direct probe while core dead | blocked where IPv6 is available | bare metal required if VM lacks routed IPv6 |
| UDP/TCP/local-stub DNS probes | no direct resolver success | Try Omarchy + bare metal |
| Mihomo SIGKILL | desired connected, armed, no leak, bounded recovery | Try Omarchy |
| runtime SIGKILL | rules survive; restarted runtime reconciles | Try Omarchy |
| NetGuard SIGKILL | kernel rules survive; helper verifies on restart | Try Omarchy |
| shell/TUI/terminal killed | tunnel/protection unchanged | Try Omarchy |
| failed core restart | protected failure, not direct fallback | Try Omarchy |
| partial setup/TUN removed | direct probes remain blocked | Try Omarchy |
| new physical default route | no bypass | Try Omarchy |
| Wi-Fi drop/rejoin | protection survives real carrier change | bare metal required |
| Wi-Fi to Ethernet switch | new NIC creates no bypass | bare metal required |
| suspend/resume | rules and desired state reconcile without leak | bare metal required |
| reboot while armed | policy active before physical connectivity | bare metal required |
| explicit disconnect | core/TUN gone before verified disarm | Try Omarchy |
| stale/corrupt state | restrictive policy plus recovery status | deterministic + Try Omarchy |
| plugin disable | verified cleanup or protection retained with blocker | Try Omarchy |
| upgrade/rollback | no second owner, leak window or lost state | Try Omarchy |
| package removal | table/state removed or uninstall aborts | Try Omarchy |
| emergency recovery | only OmaVLESS table/state removed | Try Omarchy + bare-metal confidence pass |
| foreign firewall coexistence | no global flush; own table only | Try Omarchy + bare metal |

Physical Wi-Fi/Ethernet transitions, suspend/resume and early boot ordering are
hard bare-metal gates because Try Omarchy cannot reproduce the relevant NIC,
firmware and host sleep behavior. Ordinary parser/state-machine/helper crash
tests remain valid in Try Omarchy.

## 10. K1 implementation slices

K1 should remain reviewable:

1. fixed NetGuard protocol, state machine, rule-template renderer and
   deterministic crash-point tests without installation;
2. root service/package integration plus atomic nftables arm/disarm/status and
   console recovery, tested disconnected first;
3. runtime desired-state coordination and fixed Mihomo bypass mark;
4. exact-head Try Omarchy failure matrix;
5. mandatory bare-metal NIC/suspend/boot matrix before K1 is merge-ready.

No slice adds Routing protection, LAN exceptions, permanent Lockdown,
WireGuard/AWG, arbitrary firewall configuration or a generic privileged IPC.

The [2026-10-02 first-listener publication checkpoint](../testing/K1_FIRST_LISTENER_PUBLICATION_2026-10-02.md)
records an inactive bind-before-group-access candidate. It is not a packaged
helper or evidence that any kill switch is available.
The [fixed Emergency reply transcript candidate](../development/K1_EMERGENCY_REPLY_TRANSCRIPT.md)
adds bounded raw-byte parsing without a socket, effect, ownership proof or
installed caller; it does not make K1 available.
The [package-group identity follow-up](../testing/K1_PACKAGE_GROUP_IDENTITY_2026-10-02.md)
checks a proposed fixed local group name before publishing access; package
creation and real session membership are still separate gates.

## 11. Source basis

- nftables atomic transactions:
  <https://wiki.nftables.org/wiki-nftables/index.php/Atomic_rule_replacement>
- current nftables expressions, marks and cgroup-v2 socket matching:
  <https://netfilter.org/projects/nftables/manpage.html>
- NetworkManager dispatcher events and forced-disconnect limitation:
  <https://networkmanager.dev/docs/api/latest/NetworkManager-dispatcher.html>
- systemd `network-pre.target` firewall ordering:
  <https://wiki.freedesktop.org/www/Software/systemd/NetworkTarget/>
- current Arch nftables package and systemd unit contents:
  <https://archlinux.org/packages/extra/x86_64/nftables/>

The inspected Try Omarchy baseline had Arch ARM64 kernel 7.2, cgroup v2,
systemd 261, NetworkManager 1.58 and nftables 1.1.6. Those versions are research
context, not substitutes for the exact K1 package/runtime acceptance record.
