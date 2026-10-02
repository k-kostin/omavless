# P4 synthetic WireGuard loopback smoke design

Status: **implemented; scoped standard-WG transport smoke passed**. The
[exact evidence](P4_WG_LOOPBACK_SMOKE_2026-10-03.md) records artifact identities,
preparation actions, repeated positive/negative results and cleanup. This is one
standard-WG transport smoke on a newly authorized disposable VM, not product
activation or Full/Routing/Direct, AWG, autoconnect or real-provider acceptance.
The root owner must explicitly hand off that VM before any operation. Do not
run this procedure on the physical PC or an existing private acceptance guest.

For the implementation checkpoint, the owner explicitly transferred sole
Omarchy Dev VM use and authorized its stock `wireguard-tools` installation and
stock kernel module registration when preflight found them unavailable. This
is an explicit scope amendment, not an implicit fallback. The existing guest's
installed service/network state must remain preserved; only the peer fixture
uses a disposable namespace. The original no-install/no-module-load refusal
below remains the harness's behavior. Tool preparation is outside the harness
and must be recorded as a guest-wide action, including package hooks; it is not
part of an "all actions confined to namespace" claim. Do not unload potentially
shared stock modules after the test.

## Preconditions and isolation

Use an ordinary VM account and a new user/network namespace, with that account
mapped to namespace UID 0. Refuse if user namespaces are disabled, kernel WG
support is not already available, tools are missing or the tested core cannot
run without privileged file capabilities. Do not install packages, change
sysctls/security policy, load modules or obtain sudo as an implicit fallback.
The [Linux user-namespace contract](https://man7.org/linux/man-pages/man7/user_namespaces.7.html)
explains namespace-relative capabilities; actual VM policy remains a preflight
condition, not inferred success.

Before creating WG, the harness must prove its user and network namespace
identities differ from the guest initial namespaces and its network contains
only its own loopback, no default route or physical/veth interface. Create the
WG interface **inside this same isolated network namespace**; never create it
outside and move it inward. WG keeps its UDP socket in its birthplace network
namespace, as documented by [WireGuard](https://www.wireguard.com/netns/).
No interface movement, forwarding, firewall/DNS changes, guest service actions
or guest/host network mutation is permitted.

Use a fresh private `0700` home scratch root and create credential files `0600`
before writing. Generate two ephemeral WG keypairs and optional preshared key
through private pipes/files only. No key literals in shell arguments,
environment, Git, raw output or command tracing. `wg setconf` receives a file,
not credential arguments; inspect only narrowly selected handshake/transfer
facts and redact peer-key columns before publication. Never use `wg show dump`
or `showconf`, which release secrets; see the [official wg manual](https://git.zx2c4.com/wireguard-tools/tree/src/man/wg.8).

## Proposed transport arrangement

- Namespace peer: `wg-p4` with local `10.203.0.1/32`, UDP endpoint on namespace
  loopback, and client peer `AllowedIPs = 10.203.0.2/32`. Add only the narrow
  client return route inside the namespace; do not add a default route.
- A bounded local HTTP fixture binds `10.203.0.1` and returns a fixed synthetic
  response. It is not an Internet/DNS probe and contains no reusable secret.
- Candidate input: generated client native WG conf, local `10.203.0.2/32`,
  endpoint `127.0.0.1` at the peer port, and allowed target `10.203.0.1/32`.
  Pass native import -> private v4 store -> candidate config preparation. Also
  native-export/reimport and compare the resulting private runtime config.
- Core: a verified plain, non-file-capability copy of the exact tested binary,
  with TUN disabled, no provider/geodata/download/DNS options and only a private
  Unix controller plus namespace-loopback SOCKS listener. Use one fixed WG
  outbound/selector with no DIRECT fallback; verify the selector target through
  the authenticated private controller. This uses the documented
  [Mihomo WG outbound](https://wiki.metacubex.one/en/config/proxies/wg/), not a
  newly invented URI or custom profile YAML supplied by a client.
- After peer setup, launch core/client/fixture children with no-new-privileges
  and namespace capabilities dropped. Retain only an owned setup/supervision
  process for bounded teardown. All process references must be exact owned
  child handles/PIDs; no name-wide kill or existing-daemon adoption.

## Proposed evidence and refusals

First capture an exact core/version/digest, candidate commit and VM/kernel/tools
identity. Run fixed offline core validation, then bounded startup and a request
through the SOCKS listener to the WG-side HTTP address. PASS requires the fixed
response, a fresh server handshake and transfer advancement. Raw peer/config
output stays private; publish only fixed categories/counts. A wrong client key
must make the same request fail within its deadline: this detects an accidental
DIRECT/fallback success. Restore the correct ephemeral input and verify another
request, then stop owned children and prove namespace/scratch cleanup.

Failures/timeouts must stop only owned children, preserve bounded private
diagnostics and report the exact failing stage, never silently skip assertions.
The harness must fail closed if source/config/namespace identity changes,
ownership is ambiguous or output safety cannot be demonstrated. Snapshot the
guest initial network facts before/after to prove no outside-namespace change.
Scratch cleanup must target only the harness's validated explicit root.

IPv6 can be an additional scoped case only after the first case works, with
namespace-local addresses/routes and the same negative control. No kernel
AWG substitution is claimed. This smoke does not use `NativeLifecycleHost`,
whose accepted TUN/ownership invariants must remain intact, and does not prove
product restart, active replacement/deletion compensation, Full/Routing/Direct
policy, DNS/provider behavior, AUTO-1, V0 or compatibility with a live server.
Those gates and precise before-quiesce core/flavor admission remain mandatory.
