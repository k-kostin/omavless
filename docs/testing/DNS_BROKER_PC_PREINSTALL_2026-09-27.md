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

This demonstrates that the guest and its current outer-host egress can carry
at least some proxied HTTPS traffic. It does **not** prove why the tested
XHTTP/gRPC profiles failed or whether a successful WebSocket profile will pass
the managed-DNS Full VPN/TUN gate. The next attended gate should select a
repeat-passing WebSocket record, then require actual TUN-bound HTTPS and clean
release. The owner-visible `ready`/`settled` barrier remains mandatory before
each host effect. No release or #270/#132 closure follows from this screen.
