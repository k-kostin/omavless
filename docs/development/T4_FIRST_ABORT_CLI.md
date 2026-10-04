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

### Stopped-owner admission and limits

The original CLI checkpoint `18b99a91a2ae78525f4d6c8d9cd7311965fc4cc0`
was **incomplete**: a live, disconnected `RuntimeServer` could retain an older
unlinked lock inode while recovery acquired its replacement. The explicit
real-server regression failed on that implementation (exit 101). Its separate
full Rust gate also failed the existing no-JSON-in-help contract (23 CLI tests
passed, one failed; runtime library 1166 passed/38 ignored). These failures are
retained, not reclassified by later fixes. Help wording now says private input;
the existing help guard is unchanged.

The correction adds a CLI-private stopped-owner observer, not a shared backend
policy change. Any existing control-socket name refuses without unlinking it.
The held daemon lock is only one boundary: fixed read-only system/user-manager
queries must prove the canonical services inactive with zero MainPID/ControlPID.
The root manager's `user@UID.service` identifies the actual user manager; retain
that process's original executable/identity, bind it to the root-owned systemd
executable and require the recovery process's PID/user namespaces to match.
Namespace or manager identity drift refuses; nested/unverifiable ownership is
unsupported, not silently treated as an empty host.

Each admission also inventories every visible numeric PID using all four UIDs
from bounded procfs status, never proc-directory ownership alone. Same-UID
visibility also requires the original `/proc` FD's mount ID to identify an
unrestricted procfs mount: hidden/partial PID views and unknown mount options
refuse before service queries. Same-UID
processes retain original proc/executable descriptors, start time, command bytes
and identity across rereads and PID-set comparisons. Normal `daemon` argv and
known OmaVLESS executable/argv0/comm identities refuse, including renamed,
deleted or mixed-version daemons and daemons in another network namespace.
Only the original exact recovery invocation may exempt itself. Local kernel
Unix-listener observation supplements, never replaces, that process inventory.
It matches only the exact retained caller socket path and the fixed
`/run/user/UID/omavless/control.sock`; another UID's listener is not evidence of
this user's ownership. Encoded spaces are preserved, not split or normalized;
prefix/suffix matches are not treated as the same pathname. Cross-XDG and
cross-network-namespace same-UID daemon detection remains the full proc inventory.

The pre-scope-correction checkpoint `26e0372108752e947695a26b41c039241686b1eb`
passed ordinary source (562 tests/two skips plus JS/QML), fmt/strict Clippy and
full Rust (runtime 1180 passed/38 ignored; CLI 25 passed). This does not accept
its overly broad other-UID listener refusal or supply VM/product acceptance.
A separate actual cached-owner child regression exercises the corrected
inventory path with both filesystem names removed: its frozen HOME-private ELF
runs a real `RuntimeServer`, retains the old unlinked flock/listener, and the
same original-proc-FD inventory reader returns `KnownOwner` for that exact child.
The test changes only traversal order within the complete enumerated PID set,
validated for equality/count; production order stays sorted. No process subset,
fake proc tree or service-query response is used. This is positive identification
and refusal of a live owner, not a complete absence proof. Uncertainty preserves
the fixture without signals, reaps, retries or cleanup; only exact known child
completion permits deletion of that test's synthetic root.

Checkpoint `52595b13a67e3bb3a4e960eb280da35816642e1e` remains NONPASS:
ordinary source (563 tests/two skips plus JS/QML) and strict Clippy passed,
but full Rust stopped at the DNS journal singleton test's final reacquisition
(50 passed, one failed, one ignored in that suite). The log does not establish
its underlying errno or cause. Independent review also found that the cached
worker's ordinary stack-owned server could be dropped on protocol EOF/error.
The follow-up keeps the server in `ManuallyDrop` immediately after binding;
protocol error, EOF, malformed input or panic retains it and parks permanently.
Only explicit finish plus successfully written/flushed acknowledgment consumes
and drops it. Pure lifecycle controls test these failure branches without
launching intentionally parked children. Prior evidence is not reclassified.

The separate journal-test follow-up isolates the unchanged singleton lifecycle
assertions in one exact filtered test worker, with an original executable FD,
HOME-private retained artifacts and raw matching WNOWAIT/waitpid completion.
It does not add a production unlock or eventual-success retry. A failing final
reacquisition records its typed error and a separate diagnostic flock errno.
A controlled HOME-private fork test demonstrates that even a CLOEXEC descriptor
can retain the lock between fork and exec after the parent closes it; this
establishes a possible mechanism, not the cause of the original 525 failure.
Unknown worker completion retains artifacts without signals or automatic
cleanup. The added nix dependency is test-only and reuses the locked version.

Limits per observation are 4096 numeric PIDs, 64 KiB status/stat or query output,
128 KiB command line, 4 MiB Unix table, 16 MiB aggregate proc/query bytes and a
two-second deadline. Permission errors (including unrelated same-UID nondumpable
processes), empty/ambiguous argv, PID churn, exec/UID changes, malformed data or
unsupported namespace evidence permanently refuse that invocation. These are
bounded observations plus a retained cooperative lock, not an atomic global
process snapshot or hostile same-UID security boundary.

Observation queries clear inherited bus/loader environment overrides and use
only fixed units and buses. The trusted original systemctl ELF FD is executed
without a pathname fallback; only exact raw WNOWAIT/known-terminal waitpid
completion permits result use. Unknown, timeout, malformed or nonzero outcomes
cause no further query, signal, cleanup or automatic retry. Retained private
process bytes are zeroizing and never exposed in public errors.

An active canonical VM runtime must not be stopped just to obtain a positive
test. Positive installed acceptance needs a separately approved disposable real
UID/user-manager environment with its canonical units inactive, not a fake bus
or nested PID namespace. Creating that environment is not authorized here.

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
