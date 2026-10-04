# S1 hypothetical client-layer revision protocol

Developer-only source proposal, October 4, 2026. This is not an available
systemd API, manager writer, production backend change or global App Proxy
authority. The existing observer still refuses write admission. No manager,
desktop setting, environment variable, application, network or VM is touched.

The [layer counterexample](S1_MANAGER_ENVIRONMENT_LAYERS.md) at Draft #570,
`bdad7fcff1a5af0771d7f3ab2233ba3a013940bf`, remains valid: effective environment
equality cannot recover original client absence or detect masked/ABA edits.
The [foundation](S1_PROXY_FOUNDATION.md) and
[host observation contract](S1_HOST_OBSERVATION.md) remain authoritative.
This proposal asks what *additional hypothetical authority* would remove that
particular ambiguity. It does not infer such authority from installed systemd,
Dump text, effective readback, a lock, a Boolean or a successful fake transcript.

## Required hypothetical primitive

One authenticated manager operation would have to return a coherent typed
snapshot of both transient and client layers for the ten fixed case-sensitive
proxy keys, with absence distinct from present empty. Its stamp contains an
owner identity, non-reused manager incarnation and non-wrapping revision.
Every relevant edit of either layer, including same-value writes and changes
away and back, must advance that revision. A conservative manager-wide revision
would also suffice but may reject unrelated concurrent changes.

Conditional mutation must compare that stamp and complete captured layers and
mutate one fixed client key atomically, returning the exact committed snapshot
and new stamp. Comparing on the client followed by an ordinary write does not
implement this primitive. Revision exhaustion refuses before mutation. The
model represents owner/incarnation as integers only to make drift injectable;
they are not an authentication mechanism or a proposed wire encoding.

Apply saves the original client layer, not the effective value. Restore compares
against the last acknowledged commit and reinstates original client presence
and bytes only when the entire condition still holds. Transient drift is a
conflict even when hidden by an active client override. A conflict leaves the
current values intact and seals this protocol instance; it does not authorize
foreign-state cleanup or manufacture Released.

The protocol seals before dispatch. Unknown completion before or after a fake
commit is permanently terminal: no subsequent snapshot, retry, restore, drain
or release through that instance. A lost reply after commit can leave the owned
override installed. Recovery would require separately specified authenticated
request identity/completion and durable ownership machinery; this slice does
not implement it. A new model instance is not authorized recovery.

## Executable scope

`app_proxy::transaction::tests::revision_cas` is reachable only under the
existing `cfg(test)` transaction module. It uses the existing fixed
`EnvironmentKey` and `EnvironmentValue` types, in-memory arrays and finite
event transcripts. Values and snapshots deliberately have no Debug output.
There is no journal, host adapter, D-Bus call, child process or filesystem use.
The fake operation is synchronous and indivisible by construction, so it cannot
prove any real asynchronous implementation supplies these guarantees.

Six deterministic tests cover all ten keys where applicable: original absent,
empty and nonempty restoration; owner/incarnation/revision drift before apply
and restore; masked transient edits; same-value and ABA edits in either layer;
lost completion before and after commits in either phase; and revision
exhaustion. Terminal controls compare the entire state and transcript after
three attempted follow-up steps, requiring no new model operation or mutation.

```sh
cargo test --locked -p omavless-runtime revision_cas
```

Passing these tests proves only the stated fake transcripts. Production
receipt decoding, source authentication, atomic manager support, arbitrary
foreign writers, multi-key transaction semantics, durability/crash takeover,
listener lifetime, supported new-application consumption and global App Proxy
acceptance remain unimplemented or independently gated. The existing GIO child
consumer evidence is neither rerun nor broadened by this proposal.

Source checks for this slice: six focused Rust tests PASS; runtime all-target
strict clippy, workspace formatting and diff checks PASS; `tests/run.sh` PASS
(326 Python tests, two existing opt-in skips, JS and QML contracts). Private
fixture scratch was outside Git ancestry. No runtime or VM check was requested
or performed. The Draft records the immutable source head for these checks.
