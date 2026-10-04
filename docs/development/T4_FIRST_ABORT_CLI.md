# Explicit first-cycle Abort CLI integration

Approved bounded implementation plan, based on #605
`7b4ba2ca27ac42e49f04f4dad48c8c9ee14c3682`. The actual five-case process-loss
evidence remains tied to `a2eeb423`; the later ordinary-test HOME correction
does not rerun or replace it. All previous NONPASS outcomes remain retained.

The normal command under development is `omavless restore abort --confirm-rollback`.
It accepts one bounded private stdin document containing the archive path and
passphrase, with strict field/type/duplicate/trailing-input checks. Secrets do
not travel in argv, environment, output or ordinary semantic request logging.
Input storage is zeroized on drop; no claim covers every allocator/internal
parser copy or hostile same-user memory observation.

The v1 private stdin object has exactly `schema` (integer `1`), `archive`
(absolute path string, at most 4096 bytes), and `passphrase` (12–1024 UTF-8
bytes). The entire input is at most 32768 bytes; duplicate/unknown fields,
wrong types, a second JSON value, parent traversal and NUL paths refuse.
Prepare this input through a private channel: do not put passphrases in shell
arguments, command history, environment, shareable examples or logs. Only fixed
public errors and the still-fenced success message are printed.

The existing singleton file must be empty, single-link, caller-owned `0600`
inside the proven original private directory. Admission checks the original
held descriptor and pathname; any failed check poisons that invocation rather
than accepting a later restored pathname. The ordinary daemon lock writer is
unchanged. The checked recovery constructor remains module-private; there is
no caller-supplied host/path override or generic IPC method.

This is explicit recovery while the runtime is stopped, not a new dispatcher
or an alternative live owner. Before the migration lease, retain an existing
exclusive daemon singleton lock and its private directory. Missing, unsafe,
busy or replaced lock/directory refuses; no create, chmod, socket removal,
service stop/start, ordinary owner construction or hidden retry is allowed.
Repeat this retained boundary through the actual lower recovery gates and final
return. The existing daemon lock writer is not weakened or silently reused as
an existing-only recovery reader.

The sole effect path calls the reviewed fixed-current first-Abort composition:
fresh archive authentication, exact current paths, original-to-this-invocation
stage/live/slot identities, same migration lease, explicit Off and actual
observation-only host checks. Commit, empty/torn terminal, unrelated fences,
unsafe/missing sources and uncertain host state continue to refuse. Existing
Abort is reverified, not recreated. No fence retirement, historical startup
exception, generation rollover or automatic authority restoration is added.

Successful output means only **OLD restored; recovery fence remains**. Normal
startup stays blocked. The command does not enable first Restore/Commit, promise
usable normal ownership, implement a complete backup UI or close product T4.

Required gates: strict private input/argv and public-output controls, real
existing-lock contention/replacement and missing-path refusal, actual caller
recovery and retained early/late gates, existing Abort versus Commit/empty
terminal refusal, interruption evidence and ordinary startup fencing. Run
source/Rust/fmt/strict-Clippy gates on the exact implementation. A new reviewed
CLI-binary VM fixture requires separate authorization/lease; no VM execution or
installed acceptance is authorized by this plan. Main/RC/release remain untouched.
