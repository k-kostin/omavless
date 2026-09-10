# R5 isolated login config-validation adapter

This unregistered Rust library is a prerequisite for the read-only login host,
not login activation. No CLI/IPC registration, unit, startup preference change,
installed plugin change or second lifecycle owner is added. Existing production
startup validation remains unchanged; Python remains the reference/rollback.

## Exact input and resource boundary

`ValidationSnapshot::capture` consumes the caller's already validated store,
desired state and template snapshots. It accepts only exact checked-in RU/CN/IR
bundles, including their canonical mode-line variants. Custom templates, even a
small textual change, refuse. Recognition is not a general YAML parser.

Every resource path from the trusted bundle is mandatory: 23 RU, 5 CN and 7 IR
MRS files. The private data directory and owned non-writable-by-other-users
ruleset directory are checked. The ruleset directory is held open; final files
are opened without following symlinks, bounded and checked before/after reading.
Each file is at most 8 MiB; total resources at most 64 MiB and count at most 32.
Missing, empty, unsafe or oversized resources refuse, never trigger fetching.
Resources are copied once into an immutable private memory snapshot. No store or
template file is reread. The canonical domain renderer is unchanged: provider
URLs, rule references and custom rules are not stripped to manufacture success.
Only the disposable private controller location is supplied as `/work/mihomo.sock`.

The current resource contract deliberately excludes dynamic/custom paths,
external providers and implicit geodata inputs not present in those exact
bundles. Extending it requires explicit resource coverage and tests, not a
permissive YAML blacklist. Parser success is not provider freshness, loaded rule
counts, DNS correctness, service permission, TUN readiness or live connectivity.

## Fixed sandbox

Execution accepts only a bounded (128 MiB) static little-endian ELF64 core
without a dynamic interpreter. The selected file must be regular, owned by root
or the current user and not group/world writable. Its bytes are snapshotted;
file capabilities are not copied to the executable scratch file. Dynamic core
packages and Nix loader paths require a separately reviewed adapter.

The only launcher is `/usr/bin/bwrap`, with mandatory user/network/PID and other
namespace isolation, further user namespaces disabled, all capabilities dropped,
new session, die-with-parent and cleared environment. There is no optional
namespace fallback, inherited host home, `/usr` tree, network namespace sharing,
shell command, downloader or privileged helper. If bwrap/user namespaces are
unavailable the validation refuses. No dependency is installed automatically.

Config/core/resources are written exclusively beneath a new `0700` scratch
directory under the caller's private runtime directory and bound read-only.
The exact resource filenames are mounted at `/work/ruleset`, preserving relative
references. Only disposable `/work` tmpfs (16 MiB) and sandbox pseudo-filesystems
are writable; the sandbox root is remounted read-only. The fixed core arguments
are `-t -d /work -f /input/config.yaml`. Stdin and both outputs are discarded.
A three-second execution deadline kills the sandbox, whose PID namespace and
die-with-parent contract cover descendants. Input allocation and scratch space
are bounded; this is not a separate CPU/RSS cgroup budget.

Prelaunch verification checks every staged file/directory against its held inode.
Cleanup holds every created file/directory inode and removes only exact matching
objects, never recursively deletes a possibly replaced directory. Replacement,
unexpected entries or uncertain cleanup returns `Cleanup`; it is not success.
Failed partial initialization cleans a proven empty owned directory; otherwise
it preserves the uncertain tree and returns `Cleanup`, never recursively deletes.
Private payloads have no Debug/serialization API and failures are fixed enums.
The caller still owns same-user concurrency/lock fences; this is not protection
against an actively malicious same-UID process modifying its own private files.

## Acceptance and remaining integration

Deterministic tests cover exact bundled manifests/modes, resource bounds and
symlink/permission refusal, one-snapshot canonical rendering in all three modes,
static ELF restrictions, fixed sandbox argument policy, timeout, error safety and
replacement-preserving cleanup. Synthetic core configurations contain no real
VPN credential or private endpoint.

The opt-in `installed_core_isolated_offline_snapshot_optin` uses
`OMAVLESS_TEST_MIHOMO=/usr/bin/mihomo`: resource-free offline `-t` must succeed,
and the established GEOSITE download counterexample must fail without reaching
a controlled host-loopback listener. Both cases leave scratch empty. This is
installed-core namespace evidence, not validation of existing private caches or
provider interoperability. Normal tests without opt-in do not prove sandbox
availability. Both installed cases deliberately construct test-only minimal
snapshots rather than using bundle capture: GEOSITE is unsupported by the public
capture boundary. These cases prove execution isolation, not installed acceptance
of complete resource-bearing bundles; that remains a separate follow-up gate.

Try Omarchy ARM64, 2026-09-10: 492 runtime unit tests passed; all ten focused
adapter tests passed again with installed Mihomo 1.19.30 and bubblewrap 0.12.0.
The synthetic VLESS canonical configuration succeeded offline; the GEOSITE
download case was rejected without any host-loopback request. Both cleaned their
scratch trees. The first installed test caught an executable descriptor retained
writable (`ETXTBSY`); execution now retains a verified read-only inode anchor.
No installed configuration, private profile, service, route, DNS or TUN changed.
The PR records the exact tested commit and additional static checks.

Before production login activation, compose this with strict empty-host and
permission checks under owner/migration locks, trusted once-per-manager trigger,
receipt ordering, input revalidation and legacy enablement conversion. Bind the
selected executable identity to the future service's core contract. Missing cache
provisioning must remain an explicit user-visible blocker or separately reviewed
preparation flow. Never claim saved startup preferences enable login. R5/R6 and
the startup activation gates remain open.
