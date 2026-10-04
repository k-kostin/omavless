# Explicit first-cycle Abort CLI integration

Approved bounded implementation plan, based on #605
`7b4ba2ca27ac42e49f04f4dad48c8c9ee14c3682`. The actual five-case process-loss
evidence remains tied to `a2eeb423`; the later ordinary-test HOME correction
does not rerun or replace it. All previous NONPASS outcomes remain retained.

The proposed normal command is `omavless restore abort --confirm-rollback`.
It accepts one bounded private stdin document containing the archive path and
passphrase, with strict field/type/duplicate/trailing-input checks. Secrets do
not travel in argv, environment, output or ordinary semantic request logging.
Input storage is zeroized on drop; no claim covers every allocator/internal
parser copy or hostile same-user memory observation.

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
