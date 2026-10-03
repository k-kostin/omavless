# Actual composed binaries against a mock host

Opt-in development gate on exact #579 source `3aa5848`, consuming the immutable
bundle actually exported by `8c038e7`. Source attribution is not a new export
claim. This does not change or execute the normal runtime, register a close
permit, modify a package receipt or install/enroll/start an actual system unit.

The actual frozen c4 `release-package` broker executes `--serve`, and the actual
three-patch core runs as a different namespace UID. RootContext, the channel's
real per-packet kernel credentials, exact ACL, actual kernel TUN admission,
broker transaction/journal and core conditional-close code remain unchanged.
`host.rs` is an **external test-only mock systemd/resolved authority**, not a real
manager or host resolver. Its store owns the received SCM_RIGHTS descriptor and
derives metadata from it; notification credentials must match the actual broker.
Its fixed mock unit MainPID is the actual supervisor-spawned broker child, never
a value supplied by the core. Mocked properties/effects do not prove installed
systemd/resolved compatibility or root-issued package authority.

Build `host.rs` only as an additional example in a **private object export** of
the exact c4 DNS workspace, using its existing locked/offline dependencies. Keep
the fixture source hash and export source separately from the c4 base. Never
rebuild, modify or relabel the frozen actual broker. No dependency or normal
workspace/member/API change is needed. Cargo target and TMP are under HOME;
freeze the fixture ELF outside Cargo before execution or dedicated target clean.

`probe.py --run --ack-disposable-vm --bundle ABSOLUTE_BUNDLE --fixture
ABSOLUTE_FROZEN_FIXTURE --fixture-sha EXACT_SHA --scratch ABSOLUTE_PRIVATE_PARENT`
is permitted only after full source/launcher review and an exclusive disposable
VM lease. It refuses absent explicit opt-in, wrong UID, wrong immutable manifest
or binary hashes, unsafe ancestry, symlinks/FIFOs, existing output and wrong maps.
Normal tests exercise only these pure guards, not unshare/TUN/network/core.

Each of two cases gets fresh rootless user/network/mount/PID namespaces. Maps are
inner0→outer1000 and inner1000→outer100000 (one UID/GID each), using pre-existing
subordinate IDs; setgroups must actually be `allow`. A namespace-owned tmpfs root
is required because access admission checks ownership of `/` as well as etc/run.
Only read-only nosuid `/usr` and the three staged executables are exposed, plus
the fixed null/urandom/TUN devices and a fresh namespace proc mount. Parent home,
private settings, run/DBus/systemd/controller paths and sysfs are not exposed.
Inner root remains confined to this user namespace, not guest-initial root.

The actual broker, mock host and inner1000 core execute with exactly namespaced
CAP_NET_ADMIN (effective/permitted/inheritable/bounding/ambient), verified after
setpriv and before exec, with NoNewPrivs. Broker needs it for TUNGETDEVNETNS; core
must create its own Meta, because the exact DNS policy forbids inherited FD mode.
No file capabilities, sudoers, package/module installation, DNS/default route,
sysctl or canonical host service operations are used. Configuration and all
traffic are fixed private loopback/TUN fixtures, never external providers/DNS.

Success proves actual broker readiness, actual DNS lease/core configured-ready,
two real SOCKS/HTTP-CONNECT loopback streams (not TUN traffic), wrong-token refusal, selected conditional close and
unselected exact bytes. The DNS lease must stay active after connection close.
Only subsequent owned core shutdown releases DNS, verifies store/journal empty
and destroys Meta. Denial requires an actual recorded SetLinkDNS attempt plus
settled denial/cleanup and absence of DNS-ready; no connection close is sent.
Startup failure, timeout, unknown state or cleanup uncertainty is NONPASS.

Raw child diagnostics, partial classifications and final JSON remain private in
new own staging. There is no automatic cleanup/takeover of old output; namespace
teardown is containment, not evidence of DNS recovery. An independent external
strict canonical baseline is required around every invocation. Count exactly
measured cases; never promote exit0/partial results to whole-feature acceptance.
No actual execution result is claimed by the source contract alone.
