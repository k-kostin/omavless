# Opt-in DNS broker host candidate — not an installer

This directory is a reviewable service template and an attended acceptance
contract for issue #270. The production package does **not** install or enable
this service. Its presence in Git is not evidence that password-free DNS has
passed a host test. No script here runs sudo, modifies host DNS, installs a unit,
releases a retained TUN, or changes the installed VPN.

The owning contracts are [DNS authorization](../../docs/roadmap/DNS_AUTHORIZATION.md),
[TUN authority](../../docs/roadmap/DNS_TUN_AUTHORITY.md), and
[FD-store retention](../../docs/development/DNS_FDSTORE.md).

## Fixed host boundary

The candidate runs `/usr/lib/omavless/omavless-dns-broker --serve` as root, not
Mihomo, the application runtime or arbitrary client code as root. Its only
client operation vocabulary is the fixed DNS lease protocol. No interface
names, DNS addresses, shell commands, unit names or file paths are accepted
from a client. The application and core continue running as the enrolled user.

`CAP_NET_ADMIN` is retained because the Linux `TUNGETDEVNETNS` inspection ioctl
requires it. This is a real capability and must not be described as a
capability-free broker. The implementation has no netlink/route mutation API.
The unit stays in the host network and user namespaces. `PrivateDevices=yes`
does not require reopening `/dev/net/tun`: the core transfers an existing FD;
the broker inspects and holds it. This combination still needs the installed
host gate rather than inference from static unit validation.

Root admission requires the fixed system bus and notification socket, pinned
systemd/resolved owners, and service properties matching this exact unit:
MainPID is the broker, NotifyAccess is `main`, FD store capacity is one, and
both FD-store and runtime-directory preservation are `yes`.

### Enrollment and filesystem access

One administrator-approved installation transaction must create:

| Path | Required policy |
| --- | --- |
| `/usr/lib/omavless/omavless-dns-broker` | reviewed binary, root-owned, not writable by ordinary users |
| `/etc/omavless-dns` | root-owned protected directory, no group/other write |
| `/etc/omavless-dns/enrollment.json` | root-owned regular file, mode 0600, one hard link, at most 256 bytes |
| `/run/omavless-dns` | unit-created root:root 0711, preserved on stop |
| `/run/omavless-dns/private` | root:root 0700, preserved journal directory |
| `/run/omavless-dns/control.sock` | root-owned socket, narrow ACL described below |

Enrollment is exactly `{"schema":1,"uid":1000,"policy":"meta-ipv4-v1"}`,
where `1000` is an **illustrative** nonzero numeric UID, not an installation
default. The administrator must choose the actual local desktop account. Zero,
UID sentinel, unknown/duplicate fields and invalid or oversized files are
refused. Changing enrollment while a lease/journal/store exists is forbidden;
first reach a proven empty state or the explicit recovery boundary.

The socket starts at mode 0600. Before READY or accepting a request, the broker
sets and reads back one exact POSIX ACL: owner `rw`, enrolled UID `rw`, owning
group none, mask `rw`, other none. Its resulting mode is 0660 because mode group
bits represent the ACL mask; this does **not** grant the owning group access.
No supplementary group, relogin, default ACL, world access or runtime `setfacl`
command is needed. Peer credentials remain mandatory on every protocol frame.
Ordinary applications of the enrolled UID share this deliberately narrow
capability; this is not executable/app identity authentication.

The typed access helper uses no-follow xattrs on the fixed filesystem socket
node, protected root-owned ancestors, inode checks and exact readback. Setting
xattrs on the listener FD would target sockfs, not the filesystem node. An
unsupported ACL filesystem or any readback failure blocks startup; there is
no permissive fallback. Cross-UID denial needs an actual attended host test.

## Startup, stop and recovery

Startup must refuse any inherited/systemd-stored FD, persisted journal record,
staging record or unknown pre-existing lease/socket state. It must **not**
silently unlink a stale socket merely to start. Those states need inspection
and explicit recovery, not replay of a previous user's DNS request. Even an
apparently clean inherited FD is not a new lease. Closing the child's inherited
descriptor is not removal of the manager's retained descriptor.

`READY=1` is sent only after root admission, journal admission, verified empty
FD store, listener creation and exact ACL readback. It means broker availability,
not VPN readiness. Core readiness is a separate per-lease response following
kernel identity admission, retained-object verification, fixed DNS application
and readback. No watchdog is configured until the broker implements it.

`Restart=no` intentionally makes a crash visible rather than entering a
recovery/refusal loop. `FileDescriptorStorePreserve=yes`, per-FD `FDPOLL=0` and
`RuntimeDirectoryPreserve=yes` preserve quarantine through stop, crash, upgrade
or failed restart. The unit has no cleanup hook. `systemctl stop` and a successful
new `GetAll` are **not** proof that old resolved method calls have settled.

Clean release must prove the original resolver owner/boundary, revert/read back
the reserved link's baseline, durably record cleanup, remove and verify the FD
store empty, and only then clear the journal and release the original object.
Any unknown outcome preserves quarantine; it must never be promoted by retrying
an inconclusive readback. Stopping/disabling the plugin is not a shortcut.

Until an independently tested administrative recovery procedure exists, an
unknown old-write state uses an **explicit coordinated reboot** as the bounded
recovery boundary. Before that reboot, do not remove the journal, use
`systemctl clean --what=fdstore`, unload/remove the broker unit, replace enrollment,
or close the last retained TUN. Reboot ends the old bus/core/link epoch; it does
not authorize a future unattended reconnect. This candidate is not a kill switch.

Package upgrades/removal must refuse or defer while any lease, journal, stored
FD or unknown outcome remains. Do not remove the unit before retained state is
resolved. The [experimental package fixture](package/README.md) implements a
read-only ALPM PreTransaction refusal guard; installed upgrade/removal acceptance
is still pending. The unit intentionally has no enable/boot target.

## Resource policy

The accepted typed D-Bus response is capped at 16 KiB, but zbus may allocate a
larger frame up to its upstream 128 MiB wire ceiling before rejecting it.
MemoryHigh=192M/MemoryMax=256M and no swap bound host impact, not every theoretical
combination of queued frames. Resource exhaustion can kill the broker and must
therefore exercise the same retained-object quarantine path. A 32-task and
128-descriptor cap is provisional until actual aarch64/x86-64 host acceptance.
Do not claim these settings prove resource acceptance solely from unit parsing.

## Later attended host acceptance (one explicit installation consent)

1. Build and identify the exact broker and reviewed core-adapter pair. Preserve
   the current runtime/package and a usable rollback route. Use a visible owner
   terminal for administrator consent; never a background password collector.
2. Require disconnected/settled starting state and no unknown old broker state.
   Show the chosen numeric account, exact fixed paths, fixed privileges and
   package identities **before** confirming enrollment. Do not copy private
   VPN data into enrollment or command arguments.
3. Install only the explicit candidate binary/unit/enrollment; recheck ownership
   and modes. Start the service manually, without enabling boot autostart. There
   is deliberately no unattended installation script in this directory yet.
4. Verify RootContext service properties and READY, exact ACL, enrolled-user
   success and another ordinary user's refusal. Verify that no broad polkit
   rule or extra privileged application/runtime appeared.
5. Use the reviewed opt-in core adapter for one real connect/disconnect. Check
   exactly one expected TUN, original namespace, reserved DNS baseline, FD store
   identity before writes, actual resolver state/readback, and no repeated DNS
   prompts after enrollment. Verify ordinary networking and no physical-link
   changes. Keep the previous default path available until this passes.
6. Prove negative/partial/unknown outcomes and broker/core crash retention with
   bounded probes and explicit recovery. No unknown state may recycle an index
   or be cleared as an apparent normal disconnect. Verify stop/restart/upgrade
   and removal refusal while retained state exists.
7. Complete clean release and verify empty store/journal before any removal or
   rollback. If this cannot be proven, stop testing and use the documented
   administrative recovery boundary; do not improvise destructive cleanup.

Static tests and disposable private sockets cover configuration/ACL mechanics,
not these real DNS acceptance gates. No production password-free claim is made
until the exact installed pair passes them.

### Opt-in service acceptance runner

After separately attended installation, enrollment and startup, use
`tests/native_service_acceptance.py --run --binary /absolute/candidate/omavless
--private-vless-store /absolute/private/store.json --mode global
--experimental-dns-broker-core-sha REVIEWED_SHA256` in a visible terminal.
The existing per-effect human authorization barriers remain mandatory.

This opt-in accepts only the root-protected installed candidate at
`/usr/lib/omavless-dns-experimental/mihomo`, with an exact SHA-256 pin. It uses
the fixed `Meta` link and canonical broker flags, verifies actual-child controller
readiness, manager retention count and resolved readback, and runs bounded HTTPS
bound to the TUN. It requires an independently settled disconnected baseline;
it does not stop the user's existing runtime or select another private profile.
Restore that separately preserved original state after the isolated gate.
Failure/unknown retention requires the recovery contract above, not journal
deletion. A successful runner alone cannot establish that no password dialogs
appeared: that observation and root-service/crash/removal gates remain separate.
