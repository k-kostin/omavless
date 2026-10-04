# K1 private lifecycle admission slice

Developer-only successor to #612, not a production caller or a lifecycle PASS.
The old #604/#609/#610 failures and #612 inert configured capture remain exact
historical evidence; none admits this new startable unit. No guest execution is
authorized by this document, a source gate, or the ignored native entries.

## Fixed candidate, no Start

The new literal unit and root stage are
`omavless-k1-private-lifecycle-admission.service` and
`/run/omavless-k1-private-lifecycle-admission`. The unit is intentionally startable,
but this slice contains no Start operation. The metadata helper only references,
reads and explicitly unreferences this exact unit on one manager connection.
The outer capture publishes its own link and, only after known complete success,
unlinks that link and reloads the manager. It never enables, starts, stops or
resets the fixture. An exclusive development-VM lease and trusted root/operator
are assumptions: unrelated privileged activation is outside this fixture proof.

`kernel_private_admission_fixture.rs` is a literal-only copy of the #604 ignored
fixture: only stage, environment flag and selector change. It composes the SAME
`FixtureCreator`, `LocalReadSession`, complete inventory and `LockedState`; no
second netlink writer, relaxed namespace proof, installed service or generic IPC
is introduced. The new unit points to that exact test, not `/usr/bin/false`.
The older module remains byte-identical. The admission helper invokes neither
writer nor namespace setup. See [the existing writer contract](K1_MANAGER_PRIVATE_LIFECYCLE.md)
for before-socket actual isolation, inherited negative witness, pending/closed
state, complete inventory, conditional deletion and indefinite uncertain-owner
retention. These runtime duties are **not** discharged by effective properties.

## Permission and configured facts

The native helper retains the #612 unique-owner/same-connection Version-before,
Ref, typed Unit/Service dictionaries, Dump, Version-after, Unref and single
post-Unref all-interface GetAll sequence. Its exact version is `261.2-1-arch`.
Missing, duplicate, malformed, differently typed or mismatched properties refuse.
The finite Unit/Service selection also checks the exact writer argv, environment,
read-only OpenFile witness, no additional Exec commands/drop-ins/environment
files/passed descriptors/mount overlays/joining paths, capabilities, namespaces,
NNP, deadlines, restart/kill behavior and never-started state. Requires accepts
only the two permutations of the same sysinit.target/system.slice pair.

Selected received variants are retained privately with their signatures and
values. The outer independently checks this finite typed dictionary, including
bool-versus-integer and empty-container types, before cleanup. Unknown unrelated
manager properties are not emitted. The configured Dump parser additionally
binds the exact new unit/fragment/unit hash, command, append paths, environment,
Open File and configured WatchdogSec zero. Pre-start runtime WatchdogUSec must
separately be UINT64_MAX: configured zero is not runtime-zero evidence.

The new Environment/Open File labels derive from primary systemd v261
[exec_context_dump](https://github.com/systemd/systemd/blob/v261/src/core/execute.c),
[service_dump](https://github.com/systemd/systemd/blob/v261/src/core/service.c), and
[open_file_to_string](https://github.com/systemd/systemd/blob/v261/src/shared/open-file.c).
Other finite dump labels retain the #612 observed grammar; an unfamiliar new
field refuses, never becomes a substring/normalization fallback. Synthetic
parser controls are not an observation of this new unit's actual dump.

## Remaining gate

The native checkpoint is `e15ee7bfbf206415460e9e09e4f8877fd67e6a73`;
the frozen test ELF SHA-256 is
`0f0563d22067f4c0a4ef0f7ed1e129f1a610361bd06937d25e622dadca1aaad0`.
The outer, query and create-only loader have distinct new files; the latter
requires trusted host-to-root source delivery, never execution of user-staged
loader code. Its fixed input directory contains four pinned artifacts plus the
unexecuted loader. No original fixture files were changed. Ordinary controls
exercise typed mismatches, first-error/no-next-call, every outer phase refusal,
unknown child, cleanup preconditions and exact source/pin/literal composition.

Full source/Rust gates and independent review are required before capture. The original
unit and ELF FDs must remain held/rechecked throughout the outer invocation.
Any unknown/nonzero/helper failure stops all subsequent queries and cleanup;
the native connection is retained on uncertainty, without compensating Unref.
Only a known capture may compare the complete same-invocation canonical baseline.
The exact old #609 retained link/fragment stays visible through the inherited
fixed-pair snapshot admission; there is no broad `/run` exception or baseline mask.

Even a successful admission capture is not permission to start this saved tuple
later. A future reviewed lifecycle observer must repeat effective/FD admission
in its own invocation, then the native writer must prove actual isolation before
its first socket and at every effect boundary. No canonical namespace authority,
ownership continuity, installed enforcement, full K1 or kill-switch acceptance
is claimed here.

Checkpoint `9c579b4` completed its full Rust gate successfully. Its source gate
ran 635 tests (two skipped) and refused one old #604 boundary assertion: the new
metadata-module name contained `manager_private` in `lib.rs`. The old assertion
was preserved; only the NEW metadata helper/parser modules were renamed to
`manager_lifecycle_admission_*`. The nested writer module and all older fixture
files were unchanged. This required a new frozen ELF and fresh artifact pins;
the earlier native freeze is retained as superseded source evidence.

The first new RPC negative test failed before exercising capture because its
synthetic temporary-directory label exceeded the existing helper bound. The
label was shortened; the bound and capture policies were not changed. Corrected
focused native controls passed (20 plus 3 permission controls, two guest-only
entries ignored); strict netguard clippy and formatting also passed. This is
source evidence only, not a new guest result.
