# PC x86_64 DNS broker pre-install evidence — September 27, 2026

This is a bounded pre-install checkpoint for Draft #295, not acceptance of
password-free DNS or of the 0.9.0 release. The source was
`e9073b574c7c2aec5fe5d9e0caeed60162b4e162` on
`dev/dns-transaction-foundation`. The outer PC's VPN, routes, DNS and installed
OmaVLESS were not changed. The separate Omarchy x86_64 VM was initially running
stock OmaVLESS 0.8.2, disconnected with startup off, no profile and no TUN.

## Exact local candidate

The review-only Mihomo patch from this source was applied once to upstream
`ab405bad5beeeac8b003bb01f60f134f6df54471`; the reviewed sing-tun patch
was applied to `b50ae28a1409c7bce8e96e6c6966cf57d8ace754` with a
scratch-only local module replacement. Go 1.27.0 built the x86_64 core with
`CGO_ENABLED=0`, `with_gvisor`, locked modules and `-trimpath`. Focused Go
config/listener tests passed. Rust release runtime and broker built with locked
dependencies from the exact repository source. These are locally built review
inputs, not signed/public artifacts or proof of upstream authenticity.

| Input | SHA-256 |
| --- | --- |
| Patched production-tag Mihomo | `dc0732957deed3382bdb69ae693d631e822d550c18107d504a326bd193e40626` |
| Rust DNS broker | `55f77cc57dde588f84c277d5b5877026b4afbcbf097970fe9892066c0a9bcf32` |
| Rust runtime | `7e3c9047530b4383d9da10ce44f9521ace40042a886dc5d847376d52b6a0ba7a` |

The experimental package staged and built without installing or enabling a
service: `omavless-dns-experimental-0.9.0rc1-1-x86_64.pkg.tar.zst`, SHA-256
`2fcc027173dd698b806ea5c27efc50840c33c8f2bb680a531e61e43997ad536f`.
The archive contains the two pinned binaries, fixed unit, ALPM guard/hook and
source receipt; it contains no enrollment, stock core override, user service or
private profile. Archive-extracted binary hashes match the table. A separate
unpublished normal RC package and frontend were assembled from the same source:
package SHA-256 `e99cc10bb26c98169279d248176ce1bfea33c24cd9ddc4a7945aa004f7290758`,
frontend SHA-256 `ba554db727744c2b2955270ba5908280914f0c3b35e8c1314c4af3d880fde6ad`.
The pair was copied into the isolated VM and its package hashes read back; neither
package was installed at this checkpoint. Build outputs remain outside Git.

## Isolated evidence and a corrected test assumption

The production-tag x86_64 core passed all eleven synthetic whole-core/channel
facts and thirteen DNS ownership facts. The standalone kernel TUN lease probe
passed all fifteen fixed facts. The packet probe initially showed
real TUN counters and correct marked/unmarked routes, but all four TCP/UDP echo
cases failed on both the PC and VM. A fresh network namespace on these systems
inherits `net.ipv4.conf.all.rp_filter=1`; Linux uses the effective maximum of
`all` and the interface setting, so disabling only the test TUN's filter did
not disable reverse-path filtering. Disabling `all.rp_filter` **inside the
already-verified disposable namespace only** made the unrestricted and
restricted TCP/UDP echo cases pass. The corrected source probe then passed all
ten facts on both the outer PC and VM using the exact production-tag core.
This changes no host-level sysctl. The earlier failure must not be treated as
either a core regression or a passing test.

The full Python/QML suite and Rust suite passed after the test correction;
targeted acceptance/packet tests reported 53 passed after the final dual-pin
runner check. The installed-owner DNS runner requires independent hashes for
both the core and the running root broker and has mock safety tests, but has
not run a real Connect or applied DNS.
The older isolated-home runner now refuses managed mode before private/host
access because current production login admission requires the installed owner.

## Still open

At the initial package checkpoint the VM had no private fixture. The owner then
provided a subscription for VM-only testing; its private stdin import succeeded
and produced 35 VLESS records without changing the disconnected state. No URL,
record identifiers or profile content is retained in Git. The powered-off VM
disk and NVRAM were copied and byte-verified to a private, outside-Git checkpoint
before any attempted package change. The exact public 0.8.2 x86_64 rollback
package was separately retained and matched its published SHA-256. No package
installation or VPN cycle followed this checkpoint. The human-attended installed
mode sequence, actual
resolved readback, same/cross-UID admission, crash/retention, ALPM
removal/upgrade refusal, clean removal and recovery remain outstanding as
specified by the [PC handoff](../development/RC_090_PC_CONTINUATION_2026-09-25.md).
Synthetic Ready/Released messages and package metadata are not substitutes for
those installed results. #270/#132 and Draft #295 stay open; RC, main, release
and marketplace remain unchanged.

## Installed continuation on the same disposable VM

The VM subsequently installed the exact local `0.9.0rc1-1` application and
experimental packages. Installed runtime, broker and patched-core hashes matched
the pre-install table. The stock `mihomo-bin` package remained installed,
subscription/profile counts remained 1/35, startup stayed off and no TUN was
present. The experimental package did not auto-start or enroll the root service.
An explicit protected UID-1000 enrollment and manual root-service start then
passed service READY, narrow socket ACL and zero-FD checks.

The first trial user-service start failed closed before VPN activation. Cause:
a test-only systemd drop-in selected the patched core, but the native login
owner correctly rejects any drop-in on its packaged runtime unit. Removing the
drop-in and selecting the core through the user manager's transient environment
restored a stable disconnected native runtime. No connected or DNS result is
inferred from that correction.

The installed acceptance preflight also found that an ordinary desktop UID
cannot `stat /proc/<root-broker-PID>/exe` on this host. The reviewed runner now
pins the broker package hash and unit, systemd's running ExecStart/PID, and the
root process's name, UID and cgroup without requesting elevated privileges for
observation. This does not claim an independent running-inode proof. With that
correction, the runner reached its required human `ready` barrier before its
first Connect. All live network-mode, crash, removal and recovery gates remain
unproven until separately attended execution.

The exact common `0.9.0-rc.1` frontend archive was then installed in the VM
through its reviewed frontend updater. Omarchy validated the installed plugin;
Quickshell remained alive and the disconnected panel rendered with the retained
subscription. This is a disconnected UI check, not a connected-state or DNS
claim. No private screenshot is committed.

An actual `pacman -R omavless-dns-experimental` attempt while the root service
was active with FDstore=0 was aborted by its installed ALPM PreTransaction hook.
The package, binary hashes, active service and disconnected/TUN-free runtime
were unchanged afterward. This proves the **active-service** removal refusal,
not the stronger retained-lease/quarantine removal gate or clean removal.

## Attended mode-gate diagnostic, not a positive result

The first GUI terminal was closed before its result was retained. Two subsequent
attended runs reached Connect and then separately authorized Disconnect and
mode restoration. Both ended `passed:false`; cleanup reported disconnected,
DNS released and original mode restored. The owner reported **no separate OS
DNS/route authorization dialogs** during these transitions. The initial runner
collapsed the failure into a generic code. A type-only checkpoint on the later
run identified `PermissionError` while observing the connected core, before
the TUN-bound HTTPS probe or any mode change. On this file-capability host,
ordinary-UID [`/proc/<core-PID>/exe`](https://man7.org/linux/man-pages/man5/proc_pid_exe.5.html)
inspection is not assured after an executable gains
[file capabilities](https://man7.org/linux/man-pages/man2/PR_SET_DUMPABLE.2const.html);
this is a
test-observation problem, not proof of either DNS success or product failure.

The runner now requires public process name, direct parent, UID, exact effective
capabilities, runtime cgroup and private-controller peer PID, while using the
running executable inode comparison only when procfs permits it. The installed
path and package hash remain separately pinned. This narrower projection does
**not** claim independent running-inode proof. Its unit tests passed; the
updated runner has **not** yet passed an installed Connect. The VM was no longer
running after these observations; cause of shutdown has not been established.
The last read-only observation before that showed a disconnected, recovery-free
runtime, no TUN/owned core and zero broker FD-store entries. Positive mode/DNS,
crash/quarantine, clean removal and recovery gates remain open.

## Post-reboot VM inspection and restored test staging

The guest's previous journal ends without a shutdown record; the reason QEMU
stopped is still unknown. The outer host had no matching QEMU core dump or OOM
record, and `qemu-img check` reported no disk error before the VM was restarted.
This does not establish why the process ended.

After the guest booted, the installed application, experimental Mihomo and broker
still matched the independently recorded SHA-256 values above. The patched core
retained only the reviewed network capabilities. The native runtime was active
without a service drop-in; its fresh observation was disconnected with manual
recovery false, startup disabled, 35 retained profiles and one subscription.
There was no TUN or running Mihomo. The root broker was initially inactive,
static and had zero stored descriptors.

The installed QML panel rendered the disconnected state and retained profile
group. Its Settings navigation and the `Open app` action worked; the installed
TUI rendered a disconnected, Routing-mode view and explicitly said that
Internet/DNS had not been tested. Closing the TUI did not change the native
connection observation. These private screenshots remain outside Git. No
Connect, mode change, subscription refresh or network check was performed.

The user-manager's test-only `OMAVLESS_MIHOMO` selection had been lost on reboot.
With startup still Off and no TUN, it was restored transiently and the user
runtime restarted. Fresh observation again showed disconnected, recovery false
and no TUN. A read-only root package guard passed with the broker stopped and
FD store empty. The already installed/enrolled broker was then started manually,
not enabled: `ActiveState=active`, `UnitFileState=static`,
`NFileDescriptorStore=0`. The native runtime remained disconnected and TUN-free.
The updated acceptance runner was copied into the guest's private test cache;
its guest Python compilation and the 19 local runner/authorization unit tests
passed. **The installed positive DNS/mode runner was not invoked.**

The installed broker socket was root-owned, mode 0660, with the expected named
UID ACL and no owning-group/other access. An enrolled-UID `SOCK_SEQPACKET`
connection succeeded; the same socket operation as `nobody` failed with
`EACCES`. Neither sent a descriptor or lease request. The broker remained active
with FD-store zero; the runtime remained disconnected, recovery-free and
TUN-free. This proves the x86_64 guest's socket admission boundary, not a DNS
transaction, a forged-descriptor refusal or an application-identity guarantee.

This leaves a prepared but disconnected VM, not a DNS-3 PASS. Every authorizing
Connect, mode change and cleanup still requires the separate human
`ready`/`settled` procedure. No runtime/package/core/frontend change was made on
the outer PC, and the development branch is not an RC/main or release decision.

## Installed live-gate result on the isolated VM

The installed runner from `ba1ae19d3977ba8bac839f528ea773b57467b0ce`
subsequently completed attended Connect and separately attended cleanup in the
VM. The owner explicitly authorized the agent to enter each `ready`/`settled`
word in the VM terminal; the agent inspected the screen between words. One
intermediate run had owner input as well and is not treated as independent agent
evidence. No separate OS authorization dialog was observed in these runs. This
is a negative live gate, **not** acceptance of password-free DNS or the RC.

Connect reported an owned, connected core in global mode, a live `Meta` TUN and
one stored broker descriptor. The guest resolved both a public test name and
the selected server while connected, and the local proxy listened. However,
the fixed TUN-bound HTTPS probe timed out. A separate local-proxy request also
failed (timeout or HTTP 502). Two subscription profiles using XHTTP and one
using WebSocket produced the same fixed `full_vpn_https_failed` classification.
Both checked XHTTP endpoints were TCP-reachable while disconnected; that alone
does not prove a valid VLESS/Reality session or identify the cause of the failure.
The guest's direct HTTPS probe returned 200 before and after the VPN cycles.
The outer PC concurrently had V2RayN's `singbox_tun` and a rule routing ordinary
IPv4 traffic into it. That is a live-test confounder for QEMU user-mode NAT,
not an established cause of the guest's failures. The outer tunnel was not
stopped, reconfigured or bypassed.

After each failure the runner's separately authorized cleanup reported
`disconnected:true`, `dns_released:true`, and `mode_restored:true`. A fresh
read-only guest check confirmed native actual/desired Disconnected, no manual
recovery, original Rule mode, startup off, no `Meta` device and broker
`NFileDescriptorStore=0`. The broker service remained available but held no
lease. No outer-PC VPN, route, DNS or installed package was changed. Private
profile data, server addresses, raw logs and screenshots remain outside Git.

The connected HTTPS prerequisite for the subsequent mode sequence did not
pass, so Rule/Direct/Global transitions, resolved-DNS readback, crash/quarantine,
retained-lease removal, recovery and clean removal have **not** passed installed
acceptance. Draft #295 must remain Draft; RC, main, release and marketplace
publication remain unchanged. The next diagnostic needs to distinguish remote
session failure from guest/core routing or config translation, using redacted
core transport evidence and a known-working fixture before repeating this gate.
The next comparison should also use an explicitly agreed host-egress condition
so nested tunnelling is not mistaken for a DNS-broker regression.

## Follow-up: screen the subscription before another TUN gate

The previous three failed connected profiles were not a representative network
sample. With the VM disconnected, the installed native `profiles.probe` operation
tested all 35 retained VLESS records through its isolated, no-TUN auxiliary
Mihomo and fixed public HTTPS targets. All 35 endpoints resolved; **four**
profiles completed HTTPS through their proxies (60–182 ms in this pass), all
from the WebSocket subset. The 23 XHTTP records did not pass this probe.
Two of the successful WebSocket profiles passed individual repeat probes
(154 ms and 289 ms). These are proxy checks, not TUN, route, managed-DNS or
mode-transition acceptance. A separately provided, historically usable gRPC
control profile resolved but did not complete this VM proxy check; it was
temporarily imported through private stdin and then removed. The original
35-record subscription and disconnected/Rule/startup-Off state were restored.

After the owner recommended the same gRPC control profile again, it was
privately re-imported into the disconnected VM and tested twice using the
installed, isolated no-TUN `profiles.probe` operation. Both operations
completed normally and returned `resolved:true`, `reachable:false`,
`latencyMs:-1`. This is a repeated guest-specific proxy-test failure, not
evidence that the credential is globally invalid. The single temporary
standalone record was then removed; all 35 subscription records remained.
Fresh observation remained Disconnected/Rule without recovery, owned core or
TUN, and the broker descriptor store remained empty.

This demonstrates that the guest and its current outer-host egress can carry
at least some proxied HTTPS traffic. It does **not** prove why the tested
XHTTP/gRPC profiles failed or whether a successful WebSocket profile will pass
the managed-DNS Full VPN/TUN gate. The next attended gate should select a
repeat-passing WebSocket record, then require actual TUN-bound HTTPS and clean
release. The owner-visible `ready`/`settled` barrier remains mandatory before
each host effect. No release or #270/#132 closure follows from this screen.

## Repeat-passing proxy profile: TUN HTTPS path still fails

Three subsequent attended installed runs selected the same WebSocket profile
that had passed the disconnected isolated HTTPS check twice. Each reached a
connected owned core in Global mode, a live `Meta` TUN and an active broker
lease. The initial fixed HTTPS request timed out through `Meta`. A second
runner tried the same three independent HTTPS endpoints as the built-in
profile check: all three timed out through `Meta`, while the first endpoint
returned HTTP success through the connected core's local mixed proxy. Thus a
working profile/core outbound does not establish a working full-tunnel path.

The third run added bounded route/counter diagnostics. Both a public IPv4
destination and a fixed fake-IP destination resolved to the `Meta` interface
in the guest route table; the guest had ten IPv4 rules while connected. TUN
counters moved during each timed-out request, and the same public HTTPS target
again succeeded through the local core proxy. An additional ordinary guest
HTTPS request without interface binding also timed out while connected. The
route and traffic observations rule out a trivial missing TUN route and a
`curl --interface`-only artifact. They do **not** prove whether the remaining
fault is in kernel return filtering, TUN packet processing, core DNS/fake-IP
handling or another guest-specific interaction. The earlier outer-host V2RayN
TUN remains a confounder, not a demonstrated cause; it was not changed.

A fourth separately attended run split the TUN counters: **both RX and TX**
moved for each of the three timed-out HTTPS requests. While connected, Linux
reported `all.rp_filter=1` and `Meta.rp_filter=2`, whose effective value is the
loose mode. This does not support a simple strict-reverse-filter explanation.
The same core-proxy HTTPS request passed, and the run again disconnected,
released the broker FD and restored Rule without recovery.

An initial follow-up diagnostic did not reach its HTTPS request because the
test tool's new controller-connection projection could not validate its HTTP
response. It reported `core_controller_response`, then separately attended
Disconnect and Rule restoration completed with broker FD store zero. This is
an acceptance-tool failure, not evidence that the runtime or network regressed.
The projection's endpoint and empty-list handling were corrected, and the
supplemental observation was made non-blocking for the primary HTTPS gate
before another attempt.

Each failed attempt used separately attended Disconnect and original-mode
restoration. Final fresh observation was Disconnected/Rule, startup Off,
`manualRecoveryRequired=false`, no owned core or TUN and broker FD store zero.
The tested runner has no positive TUN-bound HTTPS result on x86_64, so it did
not proceed to Rule/Direct/Global, crash or active-lease removal checks. The
next bounded diagnostic should use a fixed HTTPS destination with DNS bypassed
and inspect only aggregate TUN tracker presence from the verified private
controller during that request. The disconnected guest returned HTTP 200 from
the fixed destination before the next run. The follow-up below isolates the
VM firewall without changing host DNS or the outer PC. #270/#132 and 0.9.0 RC
remain open.

## Fixed-IP/TUN tracker diagnostic

An agent-attended VM-only run used the same repeat-passing WebSocket fixture and
the exact pinned experimental package pair above. This was diagnostic evidence,
not the policy's human-attended acceptance. One initial terminal attempt stopped
at the post-Connect acknowledgement because virtual keyboard input lost letters;
no later effect was issued by that invocation. Fresh inspection showed an owned
connection without recovery. An explicit Disconnect then returned the VM to
Disconnected/Rule, no TUN and a zero-entry broker FD store before the next run.

The corrected diagnostic completed with separately inspected Connect,
Disconnect and Rule restoration. During Connected/Global, both public-IP and
fake-IP guest routes pointed at `Meta`; ten IPv4 rules were present. A fixed-IP
HTTPS request with DNS bypassed timed out. The controller responded to repeated
private `/connections/` reads, but no `Tun` tracker was observed during that
request. `Meta` RX and TX counters increased. Three independent TUN-bound HTTPS
targets also timed out, while HTTPS through the *same connected core's* local
mixed proxy succeeded. This narrows the guest failure to the full-tunnel ingress
or handling path rather than a universally broken proxy server or DNS lookup.
The absence of a tracker does not alone prove where packets were lost.

The runner reported `full_vpn_https_failed`, then independently confirmed
`disconnected:true`, `dns_released:true`, and `mode_restored:true`. A fresh native
observation showed Disconnected/Rule, `manualRecoveryRequired=false`, no owned
core or TUN, and the broker FD store had zero entries. No outer-PC network
configuration was changed. No positive installed DNS/mode or 0.9.0 RC gate is
claimed from this failed run alone.

## VM firewall A/B and successful agent-attended diagnostic

The guest had active UFW with default-deny incoming and no `Meta` allowance.
To separate OmaVLESS, server and DNS behavior from guest filtering, a separate
private standalone Mihomo config used DIRECT only, disabled core DNS/system-DNS
changes and enabled TUN auto-route. With UFW unchanged, fixed-IP HTTPS through
the test TUN timed out under both system and mixed TUN stacks; the same request
without that core returned HTTP 200. A temporary **VM-only** inbound allowance
on the standalone TUN interface made both bound and unbound fixed-IP HTTPS
return HTTP 200. The standalone core was stopped and its allowance removed.
This A/B identifies UFW filtering of TUN ingress as the cause of this guest's
otherwise misleading route/counter symptoms. It does not implicate the outer
PC's V2RayN or the tested profile.

A temporary **VM-only** `ufw allow in on Meta` then allowed the installed,
pinned OmaVLESS pair to complete the diagnostic with the same repeat-passing
WebSocket fixture. Fixed-IP HTTPS succeeded through `Meta`; private core
tracker observation confirmed a `Tun` connection, and both `Meta` RX/TX
counters advanced. A second public HTTPS target in Full VPN mode succeeded
through the TUN. Mode transitions Global → Rule → Direct → Global, Disconnect
and restoration to the original Rule mode completed. The runner's final result
reported `disconnected:true`, `dns_released:true`, `mode_restored:true` and
`passed:true`. Fresh native observation showed Disconnected/Rule,
`manualRecoveryRequired=false`, no owned core or TUN, and broker FD store zero.
The temporary `Meta` allowance was removed, including its IPv6 counterpart;
neither test-interface name remains in UFW's saved user rules. No outer-PC
network setting or VPN process was changed.

This is **agent-attended VM diagnostic evidence**, not the policy's required
human-attended installed acceptance. The UFW exception was temporary and is
not an endorsed permanent installer change. Before RC promotion, define and
review a narrowly scoped, reversible firewall integration for UFW-enabled
guests, then complete the remaining human-attended DNS/mode/crash/upgrade and
removal gates on the exact candidate build. #270/#132 and 0.9.0 RC remain open.

## Narrow VM UFW follow-up

A second agent-attended VM-only A/B reduced the ingress exception to the
specific TUN peer and local addresses. The independent DIRECT-only test core
used `DiagTun` at `198.18.0.1/30`. An attempted exception with source
`198.18.0.1` still timed out: the guest's UFW log classified the blocked TUN
packet as source `198.18.0.2`, destination `198.18.0.1`. Replacing it with a
temporary inbound exception **on DiagTun, from 198.18.0.2 to 198.18.0.1**
made fixed-IP HTTPS return HTTP 200. The test unit and rule were removed.

The same narrowly addressed, temporary **Meta** exception allowed the pinned
installed pair to pass fixed-IP HTTPS with advancing TUN RX/TX, public HTTPS
in Full VPN with advancing TUN RX/TX, Global → Rule → Direct → Global mode
changes, Disconnect and restoration to Rule. The supplemental short-lived
tracker sample missed its connection in this repetition; it was not used as
the HTTPS verdict. The runner reported `passed:true`, `disconnected:true`,
`dns_released:true`, `mode_restored:true`. Fresh native observation reported
Disconnected/Rule, no recovery, no core or TUN and zero broker FD-store entries.
The exact Meta exception was removed; neither `Meta` nor `DiagTun` remains in
UFW's saved user rules. The outer PC and its VPN were untouched.

This demonstrates an address-scoped workaround for the tested IPv4 VM flow,
not a reviewed default firewall policy or proof for other TUN addresses,
protocols, IPv6 or hosts. Prefer an explicit administrator prerequisite and
read-only diagnostics for UFW-enabled installations in this RC; automatic
firewall mutation requires a separate security/lifecycle design. These
agent-attended checks do not replace the owner's human-attended gate before
main.

The installed DNS acceptance runner now emits a bounded, read-only UFW service
hint before Connect. If fixed-IP TUN HTTPS fails but the core's local proxy
works, it emits `ufw_ingress_possible_not_proven` only when that service was
observed active. This is a triage hint, **not** proof that UFW blocked the
packet: other firewall managers, policy rules and server failures remain
possible. The runner neither calls privileged `ufw status` nor changes or
removes any firewall rule. An administrator must review the effective ingress
policy separately; the temporary VM rules above are not installer defaults.

## Corrected core-death diagnostic in the PC VM

The installed-owner runner now has a separate opt-in `--core-crash` path. It
pins the installed runtime/core/broker identities, verifies the sole owned
core's parent, process group and start time, then signals only that process via
pidfd. Its bounded `/proc` group scan distinguishes the expected unreaped
zombie leader from live residual members; it does not reap or signal by name.
Every effect remains behind a separate terminal barrier. Pure tests cover
the parser, zombie/live distinction, target pinning and barrier.

One agent-attended VM run on the pinned installed pair passed: after core
SIGKILL, the original leader was dead but still pinned, no live member remained
in its group, fresh observation reported `ownedCoreRunning=false`, no TUN was
visible and the broker retained no DNS FD. `lastKnownActual` remained the
historical `connected` value until explicit Disconnect; the gate intentionally
does not present that cached field as fresh health evidence. Disconnect reaped
the child and restored Disconnected/Rule with no recovery, owned core, TUN or
broker FD. No firewall exception was needed for this crash case. This is
corrected **agent-attended diagnostic evidence**, not a retroactive PASS for
the earlier ARM runner nor the owner's formal acceptance before main.

The runner was subsequently tightened so Connect and other actions use the
runtime's authenticated private Unix socket instead of passing the selected
profile ID as a CLI argument. An additional post-Disconnect assertion checks
that the original zombie PID was actually reaped. Pure tests cover the private
request framing/peer check without launching an action process. The corrected
runner passed a second agent-attended VM cycle with the same core-death,
DNS/TUN-release and Disconnected/Rule outcomes. The VM was again left with
zero owned cores/TUNs and zero retained broker descriptors. This still awaits
owner-attended acceptance on the exact promotion candidate.

## Active-lease package-removal refusal in the PC VM

With the verified installed runtime and broker pair, an agent-only VM
diagnostic connected through private IPC and established one managed DNS lease:
the broker was active, systemd FD store held one descriptor, and fresh native
observation showed one live core/TUN. The exact `omavless-dns-experimental`
package was present and its broker binary matched the pinned digest. A normal
interactive `sudo pacman -R omavless-dns-experimental` reached the installed
ALPM PreTransaction guard and **failed before package removal**. No force flags
or disabled hooks were used. Package identity, binary digest, active connection
and retained FD count were unchanged by the refused transaction. A separate
Disconnect then released DNS/TUN, and the original disconnected Rule mode was
restored with no recovery flag or remaining core/TUN/FD.

The VM retained the exact package archive for rollback. This verifies actual
active-lease **Remove** refusal, distinct from the earlier ARM **Upgrade**
refusal. It does not verify quarantined-lease removal, arbitrary failure paths,
distribution or the owner's formal human-attended gate before main. No outer-PC
package or network state changed.

## Clean removal, reinstall and explicit stale-socket recovery

After the refusal test released its lease, the VM's broker was stopped in an
independently observed empty state. The installed guard passed with the unit
inactive/dead, FD store zero and private journal empty. Ordinary interactive
`pacman -R omavless-dns-experimental` succeeded without force flags or disabled
hooks; the application stayed installed, Disconnected/Rule and recovery-free.
The exact prebuilt experimental archive, SHA-256
`2fcc027173dd698b806ea5c27efc50840c33c8f2bb680a531e61e43997ad536f`,
was then reinstalled by ordinary interactive `pacman -U`. Package file checks
reported 17 files with none altered; broker/core hashes and core file
capabilities matched the earlier pinned evidence. No private profile changed.

Starting the reinstalled broker initially **failed closed**. `systemd` showed
FD store zero and no live broker, core, TUN or socket listener; the root-only
private journal was empty. The fixed `control.sock` filesystem node remained
from the prior SIGTERM because `RuntimeDirectoryPreserve=yes`; the new broker
correctly refused to bind over it. After independently proving this exact
empty state, the agent interactively unlinked only that identified inactive
socket node in the disposable VM and started the broker. The service then
returned active/READY with a new socket inode and FD store zero. This was
agent-only lifecycle diagnosis, not a general permission to clear unknown
state, and no outer-PC service was touched.

The source ALPM guard was then hardened to refuse replacement/removal while any
stale socket node remains, forcing this explicit empty-state inspection **before**
a transaction instead of surprising the next service start. The rebuilt and
installed validation of that source change is recorded below; this earlier
reinstall used the unchanged original archive.

## Root-broker crash, quarantine and reboot in the PC VM

With the reinstalled pinned pair and broker active/FDstore=0, an agent-only
VM diagnostic selected a private test profile through the authenticated Unix
control socket. Fresh observation confirmed one owned core, one `Meta` TUN and
one retained broker descriptor in managed Full VPN. This crash case did not
claim HTTPS; the VM's temporary firewall exception had already been removed.

The agent signalled **only the named broker unit's main process** with SIGKILL.
The unit became failed with MainPID=0 while FDstore remained **1**, its private
journal retained one entry, and the original `Meta` device stayed present.
The runtime's historical connected field was not treated as fresh proof of a
healthy connection. An explicit broker start failed without clearing the FD or
TUN. The installed package guard refused; an ordinary interactive
`pacman -R omavless-dns-experimental` was then rejected by the actual ALPM
PreTransaction hook. The package and both pinned binary hashes remained
unchanged. Neither `systemctl clean` nor journal/socket deletion was used to
manufacture an empty state.

The disposable VM was then rebooted as the coordinated epoch boundary. After
boot the broker unit was inactive/dead with FDstore=0, no `Meta`, no core and no
old socket; the experimental package was still installed. The native runtime
reported clean Disconnected with no manual recovery and startup Off. Its
persisted desired mode had remained Global from the crash setup, so the agent
restored the original Rule mode through the authenticated private control
socket after confirming disconnection. The VM ended Disconnected/Rule with
private profiles retained. This reproduces the ARM quarantine/reboot boundary
on x86_64, but is agent-attended RC diagnostic evidence, not the owner's formal
human-attended promotion gate or a solution for the earlier legacy #132 path.

## Rebuilt stale-socket guard package and installed lifecycle check

From exact source commit `779639477020fe6c721acd70846574f420273c27`, the
agent staged a new x86_64 experimental archive using the same pinned broker
and patched core binaries. Source receipt, fixed archive file list and extracted
guard matched their independently checked SHA-256 values. The new archive hash
is `a9b52f3dbb2cb43281d1d67e1daa6810df8c62bf58c575bba60a380172275b0b`;
its guard hash is
`686ccd5d1ff36e53aa79f2dc268230a06764cd7ba5d75b026a5b14c6132937f5`.
This is another local review artifact, not a public release asset.

After reboot had cleared the quarantined epoch, the VM showed the broker
inactive/dead, FDstore=0, no socket/journal or TUN. Normal interactive
`pacman -U` reinstalled the same-version new archive through the old package's
PreTransaction guard. Installed guard, broker and core digests matched the
staged receipt; the reviewed core capabilities were restored, and `pacman -Qkk`
reported 17 files with none altered. The package did not enable/start the
broker or connect the application.

The agent started the broker with FDstore=0, then stopped it cleanly. Systemd
reported inactive/dead, MainPID=0 and FDstore=0; the private journal was empty,
no `Meta` or live core/listener remained, but the identified filesystem socket
node persisted. The **new installed guard refused** this state. An actual
interactive same-package `pacman -U` was also rejected by its ALPM
PreTransaction hook, with no package change. After repeating the empty-state
checks, the agent interactively unlinked only that socket; the guard passed and
the broker started normally with a new listener and FDstore=0. The transient
test-core selection was restored to the VM's user manager after reboot and the
disconnected runtime restarted. Final fresh identity/observation checks passed:
installed pair pinned, native app Disconnected/Rule, startup Off, no recovery,
36 private profiles retained, no TUN and broker FDstore=0. This validates the
guard and explicit lifecycle on the VM; no default or automatic firewall/DNS
policy change was added.

## Installed managed Connect with broker unavailable

On the same pinned VM pair, the agent first sent the enrolled UID's broker
socket one correctly framed Acquire carrying `/dev/null` rather than a TUN FD.
The broker returned the fixed Rejected frame; FDstore stayed zero, no TUN
appeared and the native runtime remained cleanly disconnected. This is a
single installed kernel-admission negative, not exhaustive malformed-frame
coverage or a substituted real TUN.

For an end-to-end managed-readiness negative, the agent then stopped the
empty broker service while the native runtime was Disconnected/Rule, startup
Off, with no TUN or held FD. The installed native runtime's authenticated
private control socket was asked to Connect one private VLESS fixture in Full
VPN. After its bounded wait, the action was refused. Fresh observation and
snapshot reported actual Disconnected, desired Connected=false, original Rule
mode, no manual recovery and no TUN; broker FDstore remained zero. No false
connected or confirmed Global result was published. The private profile ID
was never placed in a process argument or output.

Normal SIGTERM left the identified inactive broker socket node. After
independently rechecking inactive/dead MainPID=0, empty journal/FD store, no
listener/core/TUN and the disconnected app, the agent interactively removed
only that node and restarted the broker. Final pinned runtime/broker identity
and Disconnected/Rule/startup-Off/FDstore-zero checks passed. This is an
agent-only installed negative for **broker unavailable**; it does not reproduce
stock-core polkit cancellation, in-flight partial resolved writes or the
owner's formal pre-main acceptance.

## Runtime cleanup budget and reinstalled application candidate

The first CI run after the guard change failed one unrelated auxiliary-core
cleanup test under shared-runner load. That test's old 600 ms group-drain budget
could fail while scanning `/proc` twice even when the synthetic child had
stopped. The bounded runtime stop/drain budgets were widened to 3/4 seconds,
without replacing whole-group proof with a PID-name check or treating timeout
as cleanup. Targeted local auxiliary tests, the complete Rust script and the
complete non-Rust/QML script passed. CI on the runtime-fix commit
`727682f` passed test, x86_64 package and ARM64 package jobs; the previous red
run is retained as the trigger, not relabelled green.

From later clean source commit `13883df7d18bb10f15b4c985ddde71af9c9a1099`,
the agent built an **unpublished** local x86_64 application candidate with
binary SHA-256
`0e86e2e49a25bf7d05021a89c7739ba3f651374428a3a1a9f32afefec11db831`.
The native package archive SHA-256 is
`64bb113d67ba538ff2bd65ec54872e4ed0c3c4ee806b4af9dbb345c80ba9923a`;
the paired frontend archive remains outside Git with SHA-256
`f81671a52d161982681c9935edd8f2635e72d920940d93a2ab7c0f06e8932e35`.
The frontend archive was assembled, **not installed** at this checkpoint.
The earlier x86_64 application rollback package remained separately retained
and byte-verified. No asset/tag/public package was published.

With the VM already Disconnected/Rule, startup Off, 36 profiles, one
subscription, no TUN and broker FDstore=0, its user runtime was stopped.
Ordinary interactive `pacman -U` reinstalled the same-version new application
package; no force flags or profile cleanup were used. Installed binary hash
matched the candidate record and `pacman -Qkk omavless` reported 17 files with
none altered. The native runtime restarted with the already selected pinned
experimental core. Fresh installed runtime/broker identity checks passed;
Disconnected/Rule, startup Off, all private records, TUN0, broker FDstore0 and
no manual recovery were preserved. This is an installed *disconnected*
checkpoint for the runtime fix, not a repeated VPN/DNS acceptance on new bytes.

## Connected diagnostic on the rebuilt application bytes

After the disconnected replacement, an agent-attended VM-only run of the exact
installed-owner runner exercised the new application binary SHA-256
`0e86e2e49a25bf7d05021a89c7739ba3f651374428a3a1a9f32afefec11db831`
with the same pinned experimental core and broker. The guest's UFW baseline
was inspected first. A temporary inbound `Meta` exception restricted to source
`198.18.0.2` and destination `198.18.0.1` was inserted for this IPv4 gate.

The runner passed preflight, managed Full VPN Connect, direct-IP HTTPS with
independent TUN tracker and RX/TX evidence, public HTTPS through TUN, Global →
Rule → Direct → Global transitions, Disconnect and restoration of the original
Rule mode. Its final fixed result was `passed=true`, `disconnected=true`,
`dns_released=true`, `mode_restored=true`. Fresh read-side checks found startup
Off, desired Disconnected/Rule, no manual recovery, 36 preserved profiles, no
`Meta` TUN, broker FDstore zero, and both application and experimental package
file checks reported zero altered files. The temporary UFW exception was then
deleted by exact rule and the original numbered rule list was read back without
any `Meta` entry. No outer-PC service or firewall was changed.

This verifies the rebuilt application's connected positive path in the
disposable x86_64 VM. At that checkpoint the paired frontend was not yet
installed. The cycle does **not** prove IPv6/UDP firewall coverage, legacy #132
cancellation, remaining in-flight negatives or the owner's formal human-attended
pre-main acceptance.

## Paired frontend installation in the PC VM

The same source assembly produced frontend archive SHA-256
`f81671a52d161982681c9935edd8f2635e72d920940d93a2ab7c0f06e8932e35`.
Its contents and installer were inspected; the VM copy matched that hash and
passed `omarchy plugin validate` before installation. The prior static plugin
tree was preserved in a private VM-only rollback directory. With the native
runtime disconnected, the archive's reviewed `install.sh` updated the existing
`kdk.omavless` plugin without starting a tunnel. A checksum-aware dry run then
found no differing installed content; only the two archive installer scripts
were absent by design from the installed plugin tree. The installed plugin
validated, one Quickshell process remained running, and read-side checks still
showed desired Disconnected/Rule, startup Off, 36 preserved profiles, TUN0 and
broker FDstore0. This pairs the local package/frontend bytes for further RC
testing; it is not a fresh-install or UI interaction acceptance claim.

## September 28 agent-run mode sequence on the installed x86_64 pair

The disposable PC VM retained application binary SHA-256
`0e86e2e49a25bf7d05021a89c7739ba3f651374428a3a1a9f32afefec11db831`,
broker SHA-256 `55f77cc57dde588f84c277d5b5877026b4afbcbf097970fe9892066c0a9bcf32`
and experimental core SHA-256
`dc0732957deed3382bdb69ae693d631e822d550c18107d504a326bd193e40626`.
The agent began from a fresh clean Disconnected/Rule observation, startup Off,
no managed TUN and broker FDstore zero. An existing private VLESS test profile
was selected inside the guest process; neither its identifier nor input was
passed as a process argument or included in this report.

Separate native socket actions and fresh installed-owner checks passed Full VPN
Connect, then Rule, Direct and Global mode changes. Each connected checkpoint
verified the requested mode, owned core/TUN and a held broker DNS lease through
the installed runner's existing typed resolved readback. A separate Disconnect
confirmed clean disconnected observation, no TUN and broker FDstore zero; a
final mode action restored the original Rule preference. The runtime and broker
services remained active, with no manual recovery. The outer PC's VPN and
firewall were untouched. This cycle did not install a VM firewall exception or
run a TUN-bound HTTPS probe; prior UFW findings still apply.

This was an **agent-run VM diagnostic**, not the repository's human-attended
authorization acceptance. No `ready`/`settled` prompt was forged or scripted.
It advances the x86_64 mode/release checkpoint for these installed bytes only;
it does not close the owner's pre-main acceptance, normal package distribution,
the remaining in-flight negatives, firewall integration or stock-core #132.

## September 28 source-paired experimental package checkpoint

The offline review builder on committed source
`d118f6e658f3832a0150ca16dac959ac88b95c3f` exported the exact pinned
Mihomo and sing-tun commits, applied the retained full patches, populated Go
vendor dependencies from the pinned module graph, then tested and built with
`with_gvisor`, `CGO_ENABLED=0` and no network access during the build. Its
private receipt names Go `1.27.0-X:nodwarf5` on linux/amd64. The resulting core
SHA-256 was `460a6a40b1094267de764f8df20dec3893873c50c2469a5c9835af3a3fbdd219`;
the broker SHA-256 remained
`55f77cc57dde588f84c277d5b5877026b4afbcbf097970fe9892066c0a9bcf32`.
The corresponding patched/vendored source archive SHA-256 was
`00a32b8ebd311f475d06c23908c767ad513a29f14e20cbb81b5e0d9fbab01823`.
These are review artifacts, not signed releases.

The stage contract rejected unmatched or additional receipt fields in unit
tests and accepted this real source/binary/license pair. `makepkg` in the VM
initially exposed automatic extraction of `corresponding-source.tar.xz` over
the `mihomo` binary name; `noextract` fixed the recipe. On the exact source
commit above, all twelve fixed source checks then passed and the archive built
without installation. Its SHA-256 was
`88266b76859e3fa327b56aa07aef6acb097afc8cc5737dafa4beea0d591fa29b`.
Archive inspection found the fixed binaries, unit, hook, receipt, source archive
and three license texts; no stock `/usr/bin/mihomo` replacement or enrollment.
The VM's pacman emitted missing *sync database* warnings, but the local package
transaction completed using already installed dependencies; this does not
establish a clean repository-backed install.

For the VM-only same-version replacement, the user runtime was first stopped
while disconnected. The broker was stopped with MainPID/FDstore zero, journal
absent and TUN absent. Its leftover root-owned socket was inspected and removed
by exact interactive path, then the existing package guard passed. Ordinary
`pacman -U` installed the source-paired experimental archive with the guard
active; no force flags or disabled hooks were used. Installed hashes matched
the receipt, the reviewed Mihomo file capabilities were present, and neither
service nor TUN was auto-started. The preexisting root enrollment and private
profiles were preserved. After explicit broker/user-runtime starts, a fresh
installed-owner preflight passed, followed by a real managed Full VPN connect
and disconnect using a guest-private profile identifier. Connected readback
confirmed the owned core/TUN and broker lease; release confirmed clean native
observation, no TUN and broker FDstore zero. The VM ended with both services
active and disconnected. No firewall exception or TUN-bound HTTPS test was run
on these new core bytes, and the outer PC's network state was untouched.

The same new core passed isolated broker-channel, DNS ownership and TCP/UDP
packet probes under disposable namespaces before installation. These are
agent-run diagnostics only. Normal distribution/enrollment, active/unknown
upgrade and more installed in-flight negatives, firewall policy, formal
owner-attended acceptance and the legacy default #132 remain open.

## September 28 persistent selection diagnostic in the PC VM

From clean source `ca33eb726969e4811ece547cc2384204d20e3df8`, an
unpublished x86_64 application binary SHA-256
`ef086d0f2e22c5495f35d2f688b76d53eb0baa6bd8db091db9fe17ae9cf9e164`
was assembled into a local native package SHA-256
`278f6e76ce7400ae82a2e16ba1c38898e118b29b18d64d0c26ac3acb55828892`.
The corresponding frontend archive was inspected but **not** installed; the
VM retained the previously installed frontend. Full local Rust/developer
checks and all three source CI jobs passed on the exact source head. With the
VM disconnected, the application package was installed by a normal same-version
local transaction. The source-paired DNS package, broker and private profiles
were not replaced; the physical PC was untouched.

The installed runtime first passed read-only identity, broker and clean-state
checks **without** a selection marker. A fixed-content private user marker was
then installed in the VM only while its runtime was stopped. After restart,
installed-owner checks passed managed Full VPN Connect and Disconnect on the
existing private fixture. Connected checks confirmed the owned core/TUN,
controller managed-DNS flags and broker lease/readback; release confirmed a
clean disconnected observation, no TUN and broker FDstore zero. No fixture ID,
credential or provider URL entered command arguments or this document.

A separate disconnected negative changed the marker to an invalid `0644`
mode: the runtime refused startup, entered its bounded failed restart state and
created no TUN or broker lease. After restoring `0600`, resetting only the
failed user unit and explicitly restarting it, the runtime was active and
disconnected with broker FDstore zero. The selection marker remains present
in this disposable VM. This negative exercised startup validation on the
installed `ca33eb7` bytes; the subsequent source-only `2fdef7f` addition
rechecks the marker before every new preparation and was unit-tested, not yet
installed. The user manager's older transient core override remains configured
for experimental acceptance, so the VM result does not independently prove
that normal enrollment/distribution no longer needs that override. No
TUN-bound HTTPS or owner-attended authorization gate was run on this package.

## September 28 explicit enrollment/selection package diagnostic

The next stacked development head,
`9f47fb26db1218a7248e61556e19928f77fed1a0`, added a fixed root-only
enrollment/revocation command, an explicitly enable-able system unit, and an
ordinary-user `dns-pair status|select` command. The full developer suite,
locked offline Rust suite, clippy and local package checks passed. This is
still opt-in development work: neither package installation nor the user
selector automatically enrolls, enables a service or repairs an existing
route template.

The source-paired experimental x86_64 package SHA-256 was
`d627a7d2091043ac2dffaa54f45a899dd83159331a15e44af52637aced4afd95`.
Its installed core SHA-256 was
`460a6a40b1094267de764f8df20dec3893873c50c2469a5c9835af3a3fbdd219`,
broker SHA-256
`4f86bb49fc9d18a5f6d155533eba46ede6f5348f8083ca2ba454d0dd93389cf0`,
and unit SHA-256
`a82b31226f09dc0b8aa9d247a2919f66a2b4d46070356e56c21cf2df2672524f`.
The native application package SHA-256 was
`c5417b45f6b51fcb5e4244cfe8ed7cc54b4f1689f6803ae788c22e9c7945333f`,
with installed binary SHA-256
`cd27f58567d3c1dacc1bd33b1dff25cad40510aea8edc159efd8110ea21faba1`.
The frontend archive was not installed. Both installed packages came from the
same clean source head through ordinary local pacman transactions, with the
experimental package guard active and no disabled hooks or force flags. The
guest's pacman emitted missing sync-database warnings; this was not a clean
repository-backed installation.

The VM's existing private root enrollment was retained. A non-root invocation
of the installed enrollment command was refused, and a duplicate root
enrollment was refused while the broker was inactive and the empty-state guard
passed. The service remained disabled and inactive immediately after package
installation. It was then explicitly enabled and started in the VM, with
`ActiveState=active`, `UnitFileState=enabled` and FD store zero. The previously
selected user marker remained `0600`; installed `dns-pair status` reported only
that local selection, and `select` refused to overwrite it. A prior disposable
debug build of the same selection code had successfully created a missing
marker with the user runtime stopped, then refused selection while the runtime
was active. Neither result establishes a complete normal-user onboarding or
template-repair flow.

On the exact installed pair, read-only identity and idle-broker preflight
passed. A private existing VLESS fixture passed managed Connect, owned
core/TUN and broker lease checks, Global → Rule → Direct → Global transitions,
Disconnect and clean DNS/TUN release. That fixture and two adjacent records did
not complete public HTTPS through the local proxy, so they were not used to
claim working internet. With OmaVLESS disconnected, the guest itself returned
HTTP 204 and 200 from two independent direct HTTPS targets. An isolated,
no-TUN native profile probe screened 37 private VLESS records and found six
reachable HTTPS proxies; it emitted only counts and ordinal indices. One of
those passing records then returned HTTP 204 through the connected core's local
proxy. With a temporary, narrowly addressed **VM-only** UFW allowance on
`Meta` from the TUN peer `198.18.0.2` to `198.18.0.1`, the same connection
returned HTTP 200 through a fixed-IP TUN request and HTTP 204 through a public
DNS/TUN request. Global → Rule → Direct → Global transitions retained broker
lease/readback; a post-transition TUN request again returned HTTP 204. Explicit
Disconnect restored Disconnected/Rule with no managed TUN and broker FD store
zero. The exact temporary UFW rule was then deleted and verified absent from
saved user rules. The physical PC's VPN and firewall were not changed.

A separate user-provided control VLESS profile was imported solely into the
VM's private store. It reached the managed Connected state, but the guest's
proxy and TUN HTTPS checks ended in TLS EOF for that particular remote. This
does not prove a parser defect or that the credential is globally invalid;
the six working private profiles and direct guest baseline localize this
observation to the selected remote path or its conversion. No URI, endpoint,
UUID, subscription URL, profile name or raw log is included here.

These are **agent-run VM diagnostics**, not the mandatory owner-attended
authorization/cancellation acceptance. The VM-specific UFW prerequisite still
needs a reviewed user-facing treatment; the ordinary setup/template flow and
remaining retained-state/upgrade negatives need completion before this can be
called an RC-ready DNS path. No main, release or marketplace state changed.

## September 28 bundled-template preparation diagnostic

Stacked source `d8dd0398889af3676a003e0b061a9269d8f183e0` adds an explicit
`dns-pair prepare-template` CLI step. It accepts only the byte-for-byte bundled
default and preserves a private create-only backup; custom and partly managed
YAML is never silently rewritten. Focused tests, the locked full Rust suite,
clippy with warnings denied and the developer/QML suite passed locally. A
same-source native application package SHA-256
`ca09cf84b3ef3ca645ce0b773ff6dc7d31b0c185f901451db855cf74722048cc`
was installed into the disconnected PC VM through ordinary local pacman. Its
binary SHA-256 was
`63ae0c5a46d2e9cd55ff523d270110f459cca861578622953e4b1823414870c5`.
The broker/core package and frontend were not replaced.

With the existing user runtime stopped, the installed command recognized its
already managed private template and returned `changed:false` without changing
its bytes; the normal runtime then restarted into a clean Disconnected state.
For a positive preparation check, a separate empty 0700 test home and state
root was created under the VM's private cache, using only the public bundled
default template. The installed command returned `changed:true`, produced the
exact two reviewed managed flags after the fixed `Meta` device line, retained
an unchanged 0600 backup and left no staging file. In the same isolated test
root, installed `dns-pair select` created a 0600 marker and `status` reported
selection. The original VM config and profiles were never replaced; its user
runtime was explicitly restarted and rechecked Disconnected with broker FD
store zero. No network, DNS, firewall or outer-PC state changed for this test.

This validates the fresh-default setup step only on the installed x86_64 VM
package. It does not make custom-template upgrades automatic, prove release
distribution, or substitute for the owner's formal authorization gate.

On the same installed package pair, an active-lease removal negative was also
repeated. An internally selected previously HTTPS-passing private VLESS fixture
established a managed Global connection with one broker-held descriptor. A
normal `pacman -R --noconfirm omavless-dns-experimental` transaction was refused
by the installed pre-transaction empty-state hook; no package was removed and
the owned connection/core/TUN/DNS lease remained confirmed. A separate explicit
Disconnect and Rule restoration released the TUN and broker descriptor with no
manual recovery. The VM's firewall and the physical PC were unchanged. This is
an agent-run regression check, not the owner-attended retained-state gate.

An isolated VM reboot then tested the explicit system-service enablement. The
guest's encrypted root volume required its normal console unlock before Linux
or SSH could start; no application startup claim was inferred during that
expected pre-boot pause. After unlock, the broker was active and enabled with
FD store zero, the user runtime was active but desired/actual remained
Disconnected, recovery was false, local pair selection persisted and no `Meta`
TUN or temporary UFW allowance existed. This is an agent-run boot-state check
after console unlock, not unattended encrypted-volume boot or a public release
acceptance.

The post-reboot acceptance helper initially refused before any network action:
it still required a test-only user-manager `OMAVLESS_MIHOMO` override, which
correctly disappeared on reboot. The helper now checks the exact private,
durable source-pair selector and rejects any conflicting override. With that
helper correction, the installed pinned pair connected in managed Global mode
using an internally selected private fixture. An agent-run, pidfd-targeted
crash of its verified owned core then left no live core group, TUN or broker
DNS lease. Explicit Disconnect and Rule restoration returned a clean observed
Disconnected state without recovery. This validates the post-reboot ownership
and release path on the installed VM pair; it is not a formal owner-attended
authorization result and does not establish public release readiness.

An additional VM-only negative checked the selected pair with its root broker
explicitly stopped while no VPN lease existed. An ordinary-user Connect was
rejected; observed runtime stayed Disconnected/Rule with desired connection
false, no `Meta` TUN and no recovery flag. Restarting the stopped broker first
failed closed on its preserved Unix socket, as the package contract predicts.
After checking inactive PID/FD store, the empty private journal, absent socket
listener and absent owned TUN, only that fixed stale socket was interactively
removed by the VM administrator. The service then started successfully and
reported active/enabled with FD store zero; durable pair selection and the
original disconnected runtime remained intact. This is a deliberate manual
recovery step for a stopped service, not unattended startup/upgrade proof.

## September 28 required-pair admission and combined UI diagnostic

The VM application package from DNS/UI source composition
`547dd288d50b66ab0c3c49f3603c79faeba8e2ff` installed the required-pair
runtime binary SHA-256
`8082279db7d3448ee8179c58a7b296efd8e66c1481f2ab97271f58f0b5fb5c3d`.
The frontend was then paired byte-for-byte with local DNS/UI composition
`24e8e7d6e09cacda287e9c6d5c4fefff85507307`; the latter only changed
frontend/tests after the application build. Developer suite and `qmllint` passed
on that exact combined source. The installed root broker and experimental core
remained the previously pinned VM pair; the outer PC was not changed.

With the VM Disconnected/Rule and no TUN, temporarily removing only its private
managed-pair selection made `dns-pair status` report `selected:false`. A widget
Connect was rejected before desired-state generation changed or a TUN appeared.
The combined frontend showed a neutral bounded verification state, then
Disconnected with the localized setup instruction, not a false red recovery
state or Disconnect action. The selection file was restored and rechecked.

A locally saved control profile then reached Connected/Rule and a live TUN but
failed isolated proxy HTTPS. This matches its earlier VM-specific probe result;
it was not counted as working Internet. With the VM Disconnected again, the
installed isolated, no-TUN profile check completed 37/37 records and found six
HTTPS-reachable subscription records. One of those completed HTTPS 204 through
the connected core's local proxy. With a temporary inbound UFW allowance limited
to interface `Meta`, source `198.18.0.2` and destination `198.18.0.1`, fixed-IP
TUN HTTPS returned 200 and public HTTPS returned 204. The exact rule was removed
immediately; readback had no `Meta` exception. This is the same narrow IPv4 VM
firewall diagnostic as above, not an installer default or IPv6/UDP proof.

While that working connection was active, removing the VM's selection again
caused widget mode-change and server-change attempts to refuse without stopping
the owned connection: actual remained Connected, mode Rule and desired generation
unchanged; the TUN and isolated proxy HTTPS 204 persisted. Explicit Disconnect
still succeeded, released the TUN and returned Disconnected/Rule. The selection
was restored and `selected:true` verified. The VM finished disconnected without
a temporary UFW rule. No credential, subscription URL, profile ID, raw log or
VM screenshot is included in this record.

These are agent-run isolated-VM diagnostics, **not** the repository's required
owner-attended authorization/cancellation acceptance or a release-readiness
claim. Reviewed normal delivery/enrollment and host firewall treatment remain
open, as do the retained-state/upgrade negative matrix and formal #132 decision.

## Combined Draft RC package checkpoint

The exact `dev/rc-090-composition-check` source
`d177e78b8f6bb1ca6111693bf92aa27e19bc961a` passed its developer/Rust,
QML lint and four GitHub test/package jobs, including native ARM64. The x86_64
application archive from that CI run had SHA-256
`fb63fc2dff49daa80f1f588851a06c6db188dd3133f2e1eece3159b1f03211ca`
and binary SHA-256
`0525bb0bd98391388a7bc3ae968e07f545032ab40c2574e0cfc246b4e41b4300`.
An offline same-source frontend archive was assembled from that inspected
binary/source pair, checked by `SHA256SUMS`, and had SHA-256
`4216643627fac881fa7d17658951bc67f9f8044f30a12173ddb2a2f1036ba416`.

The isolated PC VM was Disconnected/Rule with no `Meta` TUN and broker FD store
zero before replacement. Its user runtime was stopped; the CI application
package was reinstalled at the same RC version, and the offline frontend was
installed through its reviewed native-only installer. Readback matched the CI
binary and staged frontend files; plugin validation passed. The user runtime
restarted and reported Disconnected/Rule, with the previous 37 profiles and one
subscription preserved, no TUN and broker FD store zero. The widget reopened
without a loading or recovery error. The immediately preceding application
archive was retained outside Git for rollback; no outer-PC package, VPN or
firewall change was made.

A subsequent disconnected, no-TUN isolated profile check covered the same 37
available records and found six with successful HTTPS; it emitted only counts
and ordinal positions. Two earlier manually selected records had failed HTTPS
through the connected core's local proxy, while the VM's disconnected direct
HTTPS returned 200. One newly screened record then established a managed Rule
connection: local proxy and fixed-IP TUN HTTPS both returned 200. With a
temporary VM-only UFW allowance restricted to `Meta`, source `198.18.0.2` and
destination `198.18.0.1`, public TUN HTTPS also returned 200. Widget clicks
changed Rule → Global → Rule; observed mode matched each settled state and
public TUN HTTPS remained 200 in Global. Widget Disconnect returned
Disconnected/Rule, removed `Meta` and released the broker FD store to zero.
The temporary UFW allowance was deleted and its absence verified. No private
server identity, URL, credential, controller log or screenshot is recorded.

This is an exact-byte installed update, live HTTPS/mode and cleanup diagnostic.
It does not replace fresh marketplace installation, formal owner-attended
negatives or reviewed normal DNS-pair/firewall distribution. A subsequent
documentation-only evidence commit does not change the tested binary or
frontend identity.
