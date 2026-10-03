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

## Bounded developer evidence

Actual execution belongs to code `079bf341f03128f1173c4fec29c3c79db3bba794`,
not to the later documentation head. Two independently invoked disposable-VM
gates each measured the success and denial cases in fresh namespaces. Both
strict external preservation guards passed. This closes only this fixture's
actual-binary/mock-host compatibility gap, not whole T3/T4 acceptance.

| Actual code | Independent invocation | Measured cases | Terminal result |
| --- | --- | --- | --- |
| `832a0be` | 1 | 0/2 | NONPASS: wrong fixture enrollment policy; before broker readiness |
| `2cb1965` | 1 | 0/2 | NONPASS: broker ready, core config permission refusal; before TUN/DNS effects |
| `079bf34` | 1 | 2/2 | PASS: success and attributable DNS denial |
| `079bf34` | 2 | 2/2 | PASS: independent success and attributable DNS denial |

The old counterexamples remain retained, not relabeled as later-head results.
The release-policy correction changed only the fixture to the immutable
release-package broker's exact policy. The mode correction explicitly applies
0755/readback to fixed private-root traversal directories and 0644/readback to
four inert synthetic account/NSS files. Umask remains 077; private state/TMP/core
HOME and enrollment remain 0700/0600. No broker/core/helper rebuild or production
admission/policy relaxation occurred between these invocations.

Both accepted success receipts include actual broker/DNS readiness, two exact
loopback stream exchanges, wrong-token nonmutation, selected close with the
other stream/tracker preserved, active DNS lease afterward, then separate owned
core shutdown and verified DNS/store/journal/TUN release. Both denial receipts
include the actual recorded DNS-write attempt and revert, bounded observed
nonready (not unknown/unavailable), no close dispatch and verified release.
Real TUN descriptors, packet credentials, ACL and MainPID checks were used;
systemd/resolved themselves were mocked. Transport bytes used SOCKS/HTTP CONNECT,
not TUN traffic-egress. Normal activation and installed attestation remain false.

Every invocation preserved all eight canonical categories and every non-timer
IPv4/IPv6 address/route/rule field. Only numeric nonincreasing address lifetime
countdowns were allowed; no increase, ordering difference or comparator waiver
was accepted. The canonical running service, its executable, namespace, private
settings, core/TUN inventory and resolver state remained unchanged.

Exact accepted frozen identities (SHA256):

- launcher: `aeaf2bd7edaf9ac4bfa370906cdbad18d1de89917de4bf56966b8cbe038fc556`;
- external strict guard: `5d18e63ef93e5b0907033015f6d7cb55639c296df4a3d6b4f2cfcf6f5574cdb5`;
- unchanged mock host ELF: `d0fac9a0c6ef95b84a357d8ac485bbdcd0af59590cb0fd25f3eb2d7a363cf2a8`;
- mock host source: `bf14dd3edecb9a6ae980cc8b51bcead6cfed2109f7c70899152a8c49358563eb`;
- object-only actual-code source archive: `c414921d346dc5d88e6e790bb36362e1d2f291a75a1c8970e963b29b4e017b9d`;
- original consumed manifest: `39fa6ae1e39b47e511e7a794cadff3322df9b445f04902dcc0c0d1bcd013b532`;
- original core: `3b1da75d3c9fd8440216f9c256c6c59da812faae88debc936f3c72fef9724544`;
- original broker: `6126e5b159eb7996cbf8ac6bdb212be3d7b4b12b1809e09e74c19dbc1394001e`.

The mock ELF is a distinct c4-base-plus-fixture build, not an exact c4 broker
build claim. The consumed complete developer bundle remains the immutable
`8c038e76c8407eebd7afdd6e0389fc2bbc28cab9` export, with its original unexecuted/
uninstalled authority fields unchanged. New compatibility receipts are separate.

Private accepted archives have SHA256
`288bd297d8c28b3556fd4145263ebc2bc65705f5be11b28ad76971ff62dd0042` and
`4d03c0945362e8846c8e54363211a05c5a38c4fb72d51fadfb3880752620947c`.
Each archive was compared against its retained private host copy; all 17 raw
log/projection/result file hashes matched guest and host. Only the two accepted
owned guest staging directories were recoverably moved to Trash after archive,
process-absence and empty-root checks. Prior NONPASS stages remain untouched.
The exclusive VM lease was explicitly released.

Actual-code checks: 15 pure guards PASS (also from frozen object export), full
source suite 379 PASS with 2 skipped, JS/QML checks PASS, diff/syntax checks PASS.
The unchanged fixture build used locked/offline c4 dependencies; build and
clippy `-D warnings` passed. The initial compile typo failure stays retained.
Code-head CI is [terminal SUCCESS](https://github.com/k-kostin/omavless/actions/runs/37113485454).
Later report-head source/CI checks do not imply new VM execution. Package/root
attestation, real systemd/resolved integration, attended authorization, normal
policy adoption and TUN traffic/provider acceptance remain separate gates.
