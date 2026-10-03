# K1 namespace-transition filter experiment

Source-only candidate; no service launch or VM execution yet. This advances the
structural transition-prevention prerequisite in
[namespace provenance](K1_NAMESPACE_API_PREREQUISITE.md), without granting
canonical host, socket, table or effect authority. No production caller,
dependency change, root installation or network change.

The developer example `k1_namespace_filter_fixture` attempts only safe nix
`setns` on a retained, verified `/dev/null` character-device descriptor with
zero flags. It cannot enter a namespace even when filtering is absent. The
fixed non-installed unit adds `RestrictNamespaces=yes`, retaining empty
capabilities, no new privileges and a bounded lifetime. Neither fixture is a
package input. The original OpenFile fixture and its evidence are unchanged.

The two required execution cases have different expected errors: ordinary
unfiltered invalid-descriptor interpretation is EINVAL; the namespace filter
must return EPERM before descriptor interpretation. This avoids a vacuous
valid-namespace EPERM caused by the already empty capability set. Success,
EBADF, ENOSYS and mismatched results refuse. Opt-in and root UID only prevent
accidental execution; neither authenticates the launch.

Primary source: [systemd v261 seccomp implementation](https://github.com/systemd/systemd/blob/v261/src/shared/seccomp-util.c)
blocks the entire setns syscall when all namespace types are prohibited. Its
unshare/clone flag filtering and clone3 refusal are distinct implementation
paths: this probe does not dynamically exercise them or prove all-architecture
filter installation. A pair of observed errno values alone is not canonical
host provenance or full structural-transition acceptance.

[Linux v6.18 setns](https://github.com/torvalds/linux/blob/v6.18/kernel/nsproxy.c)
rejects a descriptor that is neither a namespace nor a pidfd with EINVAL before
namespace preparation/validation. This is the independent source basis for
the control case, not an assumption that capability refusal always comes last
for every possible descriptor. Installed-kernel execution remains unrun.

The source-only `tests/support/namespace_filter_vm_fixture.sh` uses the same
fixed transient unit name sequentially for two hash-pinned unit files. It
requires identical credentials, empty capabilities, no added syscall filter,
no private user/PID/network namespace and no effective drop-ins or extra
commands/dependencies. Only expectation and RestrictNamespaces differ. Failed
or unknown effective-property representations refuse, never imply PASS.
It removes only its owned unit link; staged files remain for outer cleanup.

The reviewed-source continuation removes all failure-path stop/reset/unlink
behavior. Any failed start, unknown property/query, replaced artifact/link,
nonzero manager PID or populated cgroup retains the fixture and returns
NONPASS. Successful unlink requires inactive/MainPID 0/ControlPID 0, an absent
or unpopulated fixed cgroup, and unchanged link/artifacts. Missing properties
cannot satisfy an expected-empty check. Both cases recheck exact ELF hash,
root ownership, mode, one hard link and absent file capabilities, plus unit
bytes/ownership/mode. Effective inherited environment files/variables,
supplementary groups, alternate roots and namespace-joining paths must be empty.

`namespace_filter_guest_guard.py` is the separate root-reviewed outer candidate.
It pins runner/probe bytes and compares complete IPv4/IPv6 address/routes/rules
(only exact address lifetime countdowns may decrease), the canonical runtime
PID/start/executable, cores/TUNs/resolver, private-file hashes, installed package
and executable identities, activation files and supported symlink targets,
runtime dependency graph, system manager environment and host/systemd identity.
Private snapshots are never printed. Unknown snapshots refuse; no policy is
changed to obtain a result. A raw waitid/WNOWAIT plus exact waitpid receipt is
required for child completion; timeout or uncertainty never signals a process,
retries a failed wait query or removes artifacts. Result creation is exclusive
and private; exact control/filtered receipts and preserved state are both
required. This candidate is still unexecuted and awaits independent root review.

Offline tests execute the actual shell body against isolated fake tools to
check both success cases, first/second start failure, replaced link/ELF,
populated cgroup, live PID, unknown query/property/filter, inherited environment,
root/group/namespace drift, capabilities and hard links. They also test the
outer comparison and one-shot wait/refusal paths. These are mock controls,
not actual systemd filter or installed preservation evidence.

Root review caught that plain Popen destruction could implicitly poll after an
uncertain explicit wait. The corrected guard uses query-free OwnedProcess
destruction/internal polling and permanently retains uncertainty before any
later spawn, public-read command or snapshot. Public-read commands use private
anonymous files and the same bounded exact-reap path, without subprocess.run
timeout cleanup. Queued later-success, destructor, cancellation, timeout and
unknown-final-reap tests must pass; none retroactively validates the original
source-only guard. The ELF and its instruction bytes are unchanged.

Before VM execution, independently freeze/hash the exact binary and runner,
review effective host/container/systemd identity and preservation guards, and
check that both complete case receipts refer to those same artifacts. An
exclusive VM lease and root review remain required. Do not execute either
mode on the primary PC. No executed systemd/filter acceptance is claimed.

## First guarded invocation — October 4

Source `191c67ec0b1da53c4375af67056fe5d3ddabcad0` passed 527 source tests
(two skips), JavaScript/QML/navigation and the focused mock controls. The
unchanged probe SHA256 is
`b7dc81b89045c591efd375765ddf4fc4792afedf86cbc73c18fd94d1227d7332`;
runner `f3e064489cab6e66346f8d419cd8e8200ba50db019cd7b3e6af141df163e6e2d`;
guard `c4a688875037f1d990ee93f0018108ab82bceb211090f61f77c64b83a4c7cc40`.
A separately reviewed retained-byte loader verified all five exact artifacts
before exclusively creating the root stage. One owner-authorized dedicated
x86_64 KVM invocation then returned exit 2 and the fixed guarded NONPASS marker.

The root stage contains only the five original artifacts: no runner.log or
result.json was created. Thus neither control nor filtered case ran (0/2);
this is preflight refusal, not an observed EINVAL/EPERM or filter failure.
The generic marker did not preserve the exact internal reason. Subsequent
read-only checks found the fixed broker path and successful root-to-user
service query, but those do not establish which earlier predicate failed.

Independent post-failure inspection found no unit link, cgroup or matching
probe/runner/guard process; the fixed unit was not-found/inactive with both
manager PIDs zero. The canonical runtime was active/running at PID 938 and
its executable matched installed omavless; both native packages were
0.9.7rc1-1. No complete before/after preservation receipt was produced, so
complete preservation is unproven, not PASS. Source/root stages were retained;
no stop/reset/unlink/retry occurred and the exclusive VM lease was returned.

The privately retained root-stage archive SHA256 is
`ed2e28d900efcdef59971949e0e72e1fc6ecffd7fe2e822fe91e59831bedd8c7`.
All five members, root ownership/modes and original hashes were independently
verified. Safe phase/category diagnostics are the next source-only step; any
new invocation needs its own reviewed artifacts and exclusive lease.

## Source-only snapshot diagnostic continuation

`namespace_filter_snapshot_diagnostic.py` loads only the exact original guard
bytes above, verified before compilation under a non-main module name. It
calls only original fixed artifact preflight and `snapshot`, never original
main or the runner. An additional exact read-command allowlist rejects unit
start/stop/reload, shell commands and unknown argv before dispatch. Original
bounded OwnedProcess capture, refusal predicates and permanent uncertainty
latch remain intact. A second diagnostic cannot revive an uncertain instance.

The original five root-stage files remain unchanged. A separate create-only
diagnostic receipt directory is required; the only output is a bounded schema,
original hash, completion boolean, finite public phase/category and source
line from the pinned guard. Exception messages, traceback locals, filesystem
path values, command output, snapshot contents and private hashes are omitted.
Actual activation-symlink counterexamples exercise the original refusal and
verify its public category; mutating-command, source-tamper, private-message
and unknown-latch tests cover the diagnostic boundary. This is not a retry or
proof of the first failure's cause. Root review and a separate read-only VM
lease are required before running it.
