# S1 fixed transaction writer checkpoint

Development candidate, 2026-10-02. App proxy remains unavailable. This combines
the independently developed per-field journal/quiescence stack (#406 at
`472b1992b1a7a09150860808b29814e6b78e1742`) with independent desktop readback
(#424 at `1316e2ed1ec211e9c68dd1ac0f22b497549c15f8`). It adds no production
host constructor, write helper, CLI/IPC capability, package or UI control.
The [private exact-source evidence](../testing/S1_FIXED_WRITER_PRIVATE_2026-10-03.md)
records the executed fixture gates and their limits.

## Executor contract

`app_proxy::transaction::Transaction` advances at most one of the existing 26
fixed fields. It obtains a complete independent observation, durably records
that field's intent, repeats the complete pre-effect comparison, executes the
typed field operation, drains the retained port and confirms a complete
independent readback. Returning from the setter alone cannot confirm a write.

The admission fence is set before calling host code, so panic/unwind leaves it
closed. A timed-out or lost reply can still commit; no next field or restoration
is admitted until that port proves its own requests settled on the same owner.
An unchanged persisted snapshot cannot substitute for that proof. Explicit
compensation drains before capturing current state, then uses the existing
reverse per-field plan. Foreign/default/lock edits preserve the journal and the
external value. Storage errors preserve existing journal poison/staging rules.

`FixedHost` is a trusted implementation contract, not a capability that an
unverified observer or IPC client can provide. Its reads must be independent
persisted observations; operations and drain must remain bounded and retain
their actual target/session/owner. The normal runtime does not construct it.

A fresh executor refuses a reopened journal. Same-owner journal reentry may
consume an existing executor, retain its original port, release/reacquire the
fixed private storage lock and compare exact durable bytes. It admits only
explicit drained compensation, never resumed application. An older valid record
with the same binding also refuses. An actual process crash loses the retained
port and remains fenced under the separate unclosed takeover contract; reading
stored instance/owner identifiers is not authority to recreate that port.

The journal also provides a read-only recovery review, checked against its exact
private record and absence of interrupted staging. Only phase, fixed pending
field, bounded counts and original/intended/mixed/foreign relationship leave
that projection. Before and after a dead writer's delayed commit, its decision
remains `RetainUnsettledEvidence`; matching either value never yields a restore
permit. Foreign values require `PreserveForeignEdits`. Only an unchanged released
original reports `RetainReleasedTombstone`, which still authorizes no deletion.

## Private dconf writer experiment

The real dconf port exists only inside `#[cfg(test)]` in the opt-in GIO crate.
It connects solely to its fixture-created bus and database. It addresses the
fixture dconf service's retained unique owner, uses one GDBus connection and
checks that owner before/after operations. Typed changes encode exactly one of
the 16 allowlisted desktop paths, preserving absent reset separately from a
present empty/equal-default override. No manager environment is read or changed.
Every complete read comes from a separate child with the same private profile.

The experiment depends on reviewed dconf 0.49.0 implementation details, not a
stable public writer protocol. In [the pinned writer](https://github.com/GNOME/dconf/blob/a6b86bd66d3b42d8cfeacc2cc71a4fc78abadb71/service/dconf-writer.c)
`Change` performs its synchronous commit before replying; an empty changeset
returns a tagged reply without changing settings. Same-connection FIFO and the
single dispatch context make that empty operation a barrier after earlier
requests, including requests whose client timed out. Monotonic tags and unchanged
owner are checked. A lost/changed service refuses the barrier. The private
[changeset representation](https://github.com/GNOME/dconf/blob/a6b86bd66d3b42d8cfeacc2cc71a4fc78abadb71/common/dconf-changeset.c)
is explicitly an internal serialization format, so it must not silently become
the installed adapter. This does not claim filesystem power-loss durability.

Six opt-in installed-dconf tests cover complete exact restoration, each of 17
partial-write reentry positions, delayed commits under a stopped owned service,
service loss, foreign edits and an actual writer process exit. The crash case
records durable intent, sends a request to the stopped service, loses its reply
and exits. A new port refuses its reopened journal while independent disk state
still matches the original. Resuming the owned service then commits the old
request after the writer has died; the journal remains pending. This is why
process exit and matching readback cannot be crash-recovery receipts.

## Remaining production prerequisites

The [admission chain](S1_ADMISSION_CHAIN.md) remains authoritative. Before an
installed enable/restore path, establish the trusted system/user-manager,
broker/launcher AUTH-writer lifetime and supported graphical-session scope;
the existing retained pidfd pins an endpoint creator, not necessarily its writer.
Prove target/schema/backend/profile continuity, supported activation semantics,
per-field fixed typed writes and bounded same-owner drain on the actual installed
API. Add owner/revision/listener readiness, TUN-disabled exclusive loopback core,
new-app UWSM consumption and the explicit restoration-conflict escape policy.
Cross-owner process-crash takeover and NixOS remain separate unclosed gates.

Existing observer `admit_writes` still always refuses. All fixtures are synthetic
private data; no desktop proxy, user-manager environment, real bus, private
profile, host service, VPN/TUN or route state is used. ARM64/VM acceptance and
production installed restoration are unrun; these tests prove the declared
private x86_64 fixture only.

The existing short socket fixture correction from #382 is reused verbatim at
source `702b0b2f61e4b5b7e56a1677bde7b865e7d165ce`. It changes only two test helpers,
preserving their private create-only directory implementation. Normal build
outputs and temporary compilation remain under the user's home directory.
