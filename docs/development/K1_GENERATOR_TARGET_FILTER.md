# K1 user-generator target fixture correction

This separate developer fixture continues Draft #593, without replacing its
first unsuccessful invocation or either read-only diagnostic. It has a new
unit name and create-only stage: `omavless-k1-generator-filter-fixture.service`
and `/run/omavless-k1-generator-filter-fixture`. It is never installed or called
by production. The probe instruction bytes are unchanged; it only attempts
`setns` on `/dev/null`, never a real namespace transition or network operation.

## Observed prerequisite

The metadata-only diagnostic at
`d0d02e78056d30363c3a2e154ef3e0038eedd10f` ran once in the dedicated x86_64
KVM and reported `escape_observed`, activation-root ordinal 14, same guest UID
and a regular target. Separate read-only checks confirmed the resolved target
belongs directly to the already-inventoried user `generator.late` root and both
source/target have the service suffix. Private filenames and target bytes are
not publication material. No activation contents were read by that diagnostic.
Its public receipt SHA256 is
`9a4d4e33627f55decd22142f8ac64458f804d493bf7317f39ab2e1b9097c790f`.
The original fixture unit, link, cgroup and probe were absent afterward, while
canonical PID 938 remained unchanged. This narrower inspection does not supply
the missing full preservation receipt for the original 0/2 invocation.

The old inventory listed all three user systemd generator directories but
rejected symlink targets within them. The new guard adds only those same three
exact directories to target scope. It does not allow `/run`, arbitrary user
runtime paths, new UID roots, private regular files or unreviewed activation
roots. Resolved outside targets, parent traversal escaping a declared root,
escaping link chains and missing targets still refuse. The full inventory and
before/after comparison remain required; a generator target is not host,
namespace, firewall or cleanup authority.

## Separate immutable generation

`generator_filter_guest_guard.py`, `generator_filter_vm_fixture.sh` and the two
`omavless-k1-generator-filter-*.service` fixtures are separate from the retained
old artifacts. The new guard pins the new runner; the runner pins both new unit
bytes and the original frozen probe. Credentials, empty capabilities,
NoNewPrivileges, effective-property checks, raw WNOWAIT/exact-reap evidence and
permanent uncertainty refusal are unchanged. No failure cleanup, stop, reset,
retry or unknown-PID signalling is added. Successful cleanup still requires
exact owned link/artifacts and quiescence.

Offline tests run the new shell against isolated mock tools, exercise known
wait receipts and every previous uncertainty case, and test actual private
symlinks under all three synthetic generator roots with escaping counterexamples.
The older source fixtures and failure evidence remain unchanged.

Root full diff/source review, exact sealed tests and hashes, create-only staging
and one exclusive VM lease are required before a new paired invocation.
Until that happens this correction is source-only, not observed filter or
canonical namespace-provenance acceptance. No main, installed package or host
network change is authorized by it.
