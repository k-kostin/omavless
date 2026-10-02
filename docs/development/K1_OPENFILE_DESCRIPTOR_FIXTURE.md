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

## Still required

Reviewed safe `NS_GET_NSTYPE`, `NS_GET_ID`, `SO_NETNS_COOKIE` wrappers; trusted
host-only system-manager/package launch; safe production descriptor adoption;
structural namespace-transition prohibition; retained socket/session poisoning;
full policy/creator and durable transaction gates remain unchanged. Existing
OmaVLESS user units are not promoted to root authority by this experiment.
