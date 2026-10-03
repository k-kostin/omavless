# Actual resolved, actual exported binaries, modeled manager

This is a development-only follow-up to the [mock-host gate](../composed_dns_binary/README.md).
The normal runtime, installed package receipt, enrollment and policy remain unchanged.
The immutable developer bundle, broker and core keep their original hashes.
Only resolve1 becomes an actual systemd-resolved daemon. The fixed systemd1
MainPID/notification/descriptor-store authority remains explicitly modeled.
No real systemd PID1, service activation or installed retention/restart is claimed.

## Inventory and authority

The separate inventory-only disposable-VM lease read public packaged ELF bytes,
their linked dependencies, existing subordinate maps and package versions.
It performed no daemon/core/namespace execution, provisioning or guest writes.
The first dependency parser failed on a truncated path; a corrected read-only
invocation verified all eight canonical preservation categories and every IPv4/
IPv6 address/route/rule field, allowing only decreasing numeric address lifetimes.
Canonical runtime PID 86349 remained unchanged. The lease was returned.
`guest-inventory.json` contains only public installed package identities and maps.
Its dependency list is the loader-linked closure, not a claim about optional
dlopen modules. Execution must additionally record and verify loaded ELF mappings.
This is unsigned fixed-binary provenance, never release/package attestation.

The corresponding upstream source reference is systemd v261.2, commit
`4925d9f07fc697efccd98a93046ff535b8832445`, not floating v261:
[service capabilities](https://github.com/systemd/systemd/blob/4925d9f07fc697efccd98a93046ff535b8832445/units/systemd-resolved.service.in),
[nonroot startup](https://github.com/systemd/systemd/blob/4925d9f07fc697efccd98a93046ff535b8832445/src/resolve/resolved.c), and
[configuration precedence](https://github.com/systemd/systemd/blob/4925d9f07fc697efccd98a93046ff535b8832445/src/resolve/resolved-conf.c).
This identifies reviewed behavior, not a reproducible-build claim about Arch's
packaged ELF. Kernel nameserver/domain arguments override local configuration;
the launcher therefore privately masks /proc/cmdline with an empty read-only file.

The proposed exact inner-to-outer maps are `0:1000:1`, `974:100001:1`,
`1000:100000:1`, for both UIDs and GIDs, with setgroups=allow. Existing guest
subuid/subgid grants are 100000:65536; missing/mismatched grants refuse before
namespace creation. No account, group, subordinate-map or capability provisioning.

## Containment to review before execution

Every case requires new user/network/mount/PID/UTS namespaces and a namespace-owned
tmpfs root. Only lo exists initially. Mount propagation is private; /usr is
read-only nosuid/nodev, /proc belongs to the new PID namespace, and only fixed
null/urandom/TUN devices are exposed. No parent HOME, /run, bus, sysfs, cgroup,
host /etc, activation files or directory descriptors cross the boundary.
An external strict whole-invocation canonical guard is mandatory. No physical
PC or canonical guest network/service effects are authorized.

Broker, core and modeled manager keep exactly namespaced CAP_NET_ADMIN and
NoNewPrivs. Only nonroot UID/GID974 resolved receives the upstream unit's exact
SETPCAP/NET_BIND_SERVICE/NET_RAW set (0x2500); no capability is transferred to
broker/core. Each child verifies all five capability sets and UID/GID before exec.
Private NSS maps systemd-resolve to974; its own runtime directory is created
before launching directly as nonroot, avoiding its root privilege-drop path.

The private system bus listens only at the literal private
/run/dbus/system_bus_socket. EXTERNAL authentication remains required. No
activation directories or includes exist. Only root can own systemd1, only974
can own resolve1; core1000 can own neither. Resolve1 name ownership and observed
kernel process credentials are tied to the retained actual resolver child.
All helper/observer connections explicitly use this address, never ambient
DBUS variables, host system/session discovery or resolvectl.

Private resolved configuration disables stub listeners, built-in fallback DNS,
LLMNR, mDNS, DNSSEC and DNS-over-TLS. The last two are explicitly this fixture's
compatible policy, not a change to or validation of a user's inherited policy.
No private resolv.conf or kernel command line supplies upstream DNS. No networkd, logind or polkit
daemon is launched. No peer, external route or uplink exists. The core's fixed
loopback nameserver is inert. Settings/drop-ins under /usr must be masked or
independently pinned; user/host configuration must not be inherited.

## Exact evidence requirements

Use the actual core-created Meta and actual transferred/retained TUN descriptor.
Do not replace kernel provenance with an interface name or precreated dummy TUN.
Success observes real typed bus calls/replies and exact effective properties,
two loopback streams, wrong-token refusal, selected close, unaffected other
stream and still-active DNS lease. The modeled manager verifies real empty DNS
and unchanged non-DNS settings while its held TUN still exists, before accepting
FDSTOREREMOVE. Only then may journal/store clear and interface disappear.
Namespace destruction is containment, not resolver restoration.

A distinct private-bus SetLinkDNS denial must observe an actual attempted call,
settled AccessDenied, no DNS-ready and verified subsequent reset/cleanup; it is
bus-policy denial, not polkit behavior. Revert denial must preserve pending/
recovery state and the held descriptor, never fabricate cleanup. Resolver owner
loss must refuse further authority; absence or a successor name owner cannot
clear uncertainty. An independently configured unrelated private link must
remain unchanged. No generic HTTP exit status or timeout constitutes PASS.

Before releasing the broker, the observer seeds the unrelated link through
typed DNSEx/domain/default-route calls and retains its exact nonempty readback.
Active managed-link checks compare all non-DNS properties with the independently
captured baseline. At the end the broker is reaped before an authenticated
ordered monitor marker freezes the event history; unresolved calls or premature
monitor exit refuse finalization. Loaded-object checks bind each mapped device
and inode to its opened ELF, not merely its current pathname and hash.

Every child (including fixed ip/mount utilities) uses nonreaping wait observation
and exact raw final status. Before the fresh proc mount becomes `/proc`, bootstrap
utilities prove direct-child status only, never group absence; uncertainty stops
bootstrap and leaves only the separate namespace containment boundary. After
chroot, group inventory requires procfs PID topology matching the caller before
every scan. The first unknown wait or reap permanently prevents
further child queries, signals, reaping and cases in that process. No Popen
ECHILD-to-success fallback or cleanup retry is used. Cancellation must revalidate
the unreaped anchor before signaling. An unknown result preserves staging and
requires independent containment review; it is not a cleanup receipt.

Execution additionally requires exact `--launcher-sha`, `--host-source-sha` and
`--observer-source-sha` pins beside the frozen helper ELF pin. Validated source
bytes are staged before namespace execution. Per-case receipts retain synthetic
configuration hashes and the finalized observer receipt digest. An independently
reviewed external four-case canonical guard is still required. `vm-guard.sh`
adapts the earlier eight-category/full-IPv4/IPv6 guard to these exact source and
ELF pins; comparator counterexamples run without executing it. The earlier
two-case mock-host guard cannot be reused unchanged.

Each receipt must separately identify actual resolver and modeled manager,
exact source/ELF/config/monitor hashes, measured case count, process quiescence
and strict canonical preservation. Unexpected monitor loss, unmatched/late
reply, missing baseline, unknown child state or cleanup uncertainty is NONPASS.
No blind retry or automatic takeover/removal of old fixture directories.

## Current state

Inventory, launcher and observer source are implemented. Pure/source checks and
helper compilation do not execute namespaces, resolved, broker or core. Actual
daemon execution remains pending. Root must review the complete new code and
frozen containment artifacts before granting a separate execution lease.
