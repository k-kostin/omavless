# K1 OpenFile descriptor-match experiment

This developer-only standalone Rust example and fixed KVM runner investigate
one possible launch mechanism from the [namespace prerequisite](K1_NAMESPACE_API_PREREQUISITE.md).
They add no product API, service, dependency, socket, namespace syscall or nft
effect. `DescriptorMatch` is deliberately **not** `Canonical`, kernel network
namespace type/ID, namespace cookie or table ownership. K1 stays unavailable.

## Narrow observation

The standalone `k1_openfile_descriptor_fixture` accepts no arguments. An
explicit fixture opt-in and effective UID zero prevent accidental invocation;
neither authenticates host provenance. It ignores `LISTEN_PID`, `LISTEN_FDS`
and `LISTEN_FDNAMES` as authority. Controlled units supply exactly FD 3.

Before opening anything else, safe `File::open("/proc/self/fd/3")` retains that
descriptor's namespace object. Missing FD 3 fails rather than allowing an
earlier procfs opener to occupy its slot. The fixture requires nsfs filesystem
kind, checks procfs kind, and compares retained FD device/inode with a pinned
`/proc/thread-self/ns/net` descriptor. It reopens both fixed paths and checks
both original and reopened identities before returning `DescriptorMatch`.
No raw-FD ownership constructor, unsafe code or Python helper is used.

Nsfs is not network-namespace type proof. A PID namespace fails this experiment
because it differs from the expected current network namespace, not because
the code implements `NS_GET_NSTYPE`. Before/after sampling detects a changed
descriptor at the second check, not a replace-and-restore race. The standalone
single-threaded fixture contains no FD-3 replacement or namespace transition;
this is not a production lifetime guarantee. An arbitrary root launch with a
matching supplied descriptor can also pass; no authenticity claim follows.

## Fixed VM gate

Build only the developer example:

```sh
cargo build --locked -p omavless-netguard --example k1_openfile_descriptor_fixture
cargo test --locked -p omavless-netguard --example k1_openfile_descriptor_fixture
```

The explicit owner-authorized gate stages that exact executable as root-owned
mode 0700 `/run/omavless-k1-openfile-fixture/probe` inside the dedicated KVM VM.
Its parent directory is root-owned mode 0700. The test-only runner is
`crates/omavless-netguard/tests/support/openfile_vm_fixture.sh`, invoked as root
with only `OMAVLESS_K1_OPENFILE_VM=1`, no arguments. It refuses an existing
same-name unit or unexpected execution environment. Password authorization,
if needed, belongs only on interactive tty/stdin, never argv/env/files/logs.

Every case uses the same fixed transient root unit and these literal properties:
`Type=exec`, `User=root`, `RuntimeMaxSec=10s`, `TimeoutStopSec=2s`,
`PrivateUsers=no`, `PrivatePIDs=no`, `PrivateNetwork=no`,
`NoNewPrivileges=yes`, plus `systemd-run --wait --pipe --collect` and fixture
opt-in. Only the negative private-network case overrides `PrivateNetwork=yes`.

| Case | OpenFile path, always read-only | Expected |
| --- | --- | --- |
| match | `/proc/1/ns/net` | DescriptorMatch, exit 0 |
| missing | none | Refused, exit 2 |
| regular | `/usr/lib/os-release` | Refused, exit 2 |
| PID namespace | `/proc/1/ns/pid` | Refused, exit 2 |
| private network | `/proc/1/ns/net` | Refused, exit 2 |

The fd-name is always `k1-host-netns`; changing this environment-visible label
cannot create authority. The runner bounds each call and verifies transient
unit unloading after both positive and negative cases. It never starts/stops
an installed runtime or VPN service. Only the fixed owned transient unit may
be stopped on failure. The private-network negative creates only the service's
disposable namespace. Remove only the exact staged probe and its empty directory
after testing; no persistent unit or package is installed.

`OpenFile=` exists since systemd v253. Inspected host and Omarchy Dev KVM had
systemd `261.2-1-arch`; VM package `omarchy 4.0.4-1`. The v261
[service documentation](https://github.com/systemd/systemd/blob/v261/man/systemd.service.xml#L1369)
and [exec implementation](https://github.com/systemd/systemd/blob/v261/src/core/exec-invoke.c#L5259)
place descriptor collection before service sandbox setup. Positive descriptor
transfer plus private-network mismatch are the limited runtime gate; they are
not exhaustive validation of mount/PID/user namespace ordering or root-manager
provenance. Exact-head execution evidence and availability belong in the PR.

### Recorded isolated result, 2026-10-02

Code head `049e59cd8a35b7b3168e8a1d2b1643b208bff9d1` passed all five
cases in the dedicated x86_64 KVM Omarchy Dev VM: systemd `261.2-1-arch`,
Omarchy `4.0.4-1`. Root authorization was supplied only through interactive
tty/stdin. The exact binary SHA256 was
`36fe02473a34c4ef094bb6bdf2d7b15df30c60aeda745eab8ac81b99067fc728`;
runner SHA256 was
`f2f49123cbb0e3daf15162c1a8b7bce92ff08789192b97182f4ae3505a94b951`.
Both transferred hashes were verified before execution. Each fixed case
printed its PASS category, followed by `K1_OPENFILE_VM_PASS`.

The transient unit's final `LoadState` was `not-found`. Only the exact staged
probe, runner and empty `/run/omavless-k1-openfile-fixture` directory were
removed. This confirms controlled descriptor delivery/agreement and four
refusal cases; **DescriptorMatch remains different from Canonical**. It proves
neither general inherited-FD authenticity nor unavailable NS_GET_NSTYPE/ID,
socket-cookie or nft ownership checks. Subsequent documentation-only changes
do not expand the tested code's evidence.

## Still required

### Non-installed launch/package boundary candidate

`crates/omavless-netguard/tests/fixtures/omavless-k1-openfile-fixture.service`
records one fixed descriptor-inspection launch for review. It stays under test
fixtures, outside all installed unit search paths and package inputs. It has
no install section, socket/D-Bus activation, boot ordering, state/enrollment
directories or preparatory/cleanup commands. Its only executable is the existing
standalone probe at the fixed developer staging path. Empty capability sets
grant no NetGuard networking capability; this extra restriction has not been
exercised in a service launch.

The existing transient VM runner does **not** load this unit. Its earlier
exact-head VM evidence above remains unchanged and does not validate this new
configuration. No service is loaded, started, enabled or installed by this
candidate or its tests.

`tests/test_k1_launch_fixture_boundary.py`, included in `tests/run.sh`, checks
the complete fixed directive allowlist and executes the real Arch payload
stager against a temporary destination with synthetic executable bytes. The
positive complete file/directory inventory, byte/mode checks and symlink
rejection prove that normal payload staging excludes the fixture, probe,
NetGuard system service, activation links and hooks. This needs no root,
package manager, Cargo build or running service; it is payload evidence, not
an installed-package acceptance test. Python remains developer test tooling.

These checks establish configuration and package exclusion only. They do not
establish systemd execution, safe production FD adoption, namespace type/ID,
socket cookie, host-manager provenance, prevention of switch-and-return,
canonical authority or nft ownership. Private PID/user/mount/proc views and
substituted real namespace descriptors remain untested launch scenarios.
`DescriptorMatch` cannot be converted into `NamespaceObservation::Canonical`
or an `EffectPort`. K1 stays unavailable.

### Actual fixed-unit VM launch, still not production authority

The separate `tests/support/openfile_unit_vm_fixture.sh` runner closes one
specific gap above: it loads the exact non-installed fixture unit through a
temporary `/run/systemd/system/` symlink in the dedicated KVM VM, starts it,
waits for the fixed descriptor probe to exit successfully, then unloads the
unit. The runner requires root, an explicit VM-only opt-in, a previously absent
unit, a private root-owned staging directory, fixed probe/unit modes and the
exact unit-file SHA256. Before start, it also rejects effective systemd
drop-ins, extra dependencies or commands and an unexpected start executable;
verifying the fragment alone would not exclude these additions. It removes
only its own fixed symlink and leaves the
probe staging files for the caller to remove. It does not enable a service or
run the normal VPN/runtime, nftables, TUN or routing. An interrupted run
requires verifying the exact unit and link state before a retry; it must not
overwrite a pre-existing unit.

The 2026-10-02 VM test of this candidate used systemd
`261.2-1-arch` and Omarchy kernel `7.2.5-3-omarchy`. The probe SHA256 was
`36fe02473a34c4ef094bb6bdf2d7b15df30c60aeda745eab8ac81b99067fc728`;
the unit SHA256 was
`753c11ee3f6bbfb9f259097f6455ba9fb5d185045d94c6f6f2c937b496bca6ab`.
The revised runner SHA256 was
`ab113a8d5addcea7f7cb80e2241f5387679cf291867f29eb8914d7ecf5d2b1e6`.
The fixed unit returned `K1_OPENFILE_UNIT_VM_PASS`; afterward `LoadState` was
`not-found` and the `/run` unit link, private staging directory and user
transfer directory were absent. A second run with the benign, unit-specific
`omavless-k1-openfile-negative-dropin.conf` returned exit 2 before the probe
started; its temporary unit link and the drop-in were removed, and `LoadState`
again became `not-found`. This is evidence of that exact temporary unit
launch, not of an installed root NetGuard service or K1 readiness. Kernel
namespace type/ID, socket namespace cookie, canonical system-manager
provenance, switch-and-return prevention and nft ownership remain open.

### Production prerequisites

Reviewed safe `NS_GET_NSTYPE`, `NS_GET_ID`, `SO_NETNS_COOKIE` wrappers; trusted
host-only system-manager/package launch; safe production descriptor adoption;
structural namespace-transition prohibition; retained socket/session poisoning;
full policy/creator and durable transaction gates remain unchanged. Existing
OmaVLESS user units are not promoted to root authority by this experiment.
