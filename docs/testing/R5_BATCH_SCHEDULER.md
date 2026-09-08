# Native batch scheduler acceptance boundary

Continuation of PR #161 on main `5575776b365df9daa26587020ec1eaffd7a1d368`.
The original cloud handoff remains historical; all its prerequisites are merged.
This checkpoint accepts the inactive native implementation separately from the
future installed frontend/ownership transition. It does not claim R5 complete.

## Reference and ownership

Python's `subscription-refresh-all` command in `backend.main` remains the
installed production owner and oracle. Existing subscription domain/store
differential corpora preserve feed
identity and all-or-nothing replacement. The accepted offline refresh-all
reference is additionally compared by
`incremental_preparation_matches_the_accepted_atomic_reference`. Scheduler
timing deliberately differs from Python: start acknowledges admission, not
success. Clients must poll the instance-bound terminal state.

The only new native entry points are fixed `subscriptions.refresh_all`,
`operations.get`, `operations.cancel` and their semantic CLI mappings. There
is one worker, at most one active batch, four shared fetch permits, 128 retained
terminal records, a 25-second provider bound and 30-minute whole-job bound.
No QML/plugin/backend/package file is changed. No installed ownership marker,
private profile, service, TUN or firewall is changed by this checkpoint.
Python cannot be removed.

## Rebase and integration corrections

All four existing commits are preserved on current main. Range-diff of
`7352984..934bc8d` against `5575776..40f0de6` shows context-only changes for
the first implementation/format commits and exact matches for both later test
commits. Main's diagnostic, import/export, preset, custom-rule and route-check
methods remain alongside batch methods.

Three narrow corrections were required:

- The newly merged durable preset barrier now blocks batch admission, exact
  cached start replay and prepared completion before writes or clock reads.
- Shutdown no longer holds the scheduler admission mutex while a provider
  request drains. Already admitted unary callers get `daemon_restarting`
  without waiting for the provider timeout; shutdown still joins its worker.
- An ambiguous batch store I/O failure may occur after atomic rename. It now
  becomes `manual_recovery_required` and blocks the shared owner, not an
  ordinary failure with an unchanged revision and permission to retry. The
  owner does not overwrite unknown current bytes. Deterministic pre-write and
  post-replacement failures verify terminal state, unchanged revision, refused
  batch retry and refused ordinary profile mutations. This is an in-process
  refusal barrier, not a new persistent recovery journal.

## Local gate

Run locally on Try Omarchy ARM64:

```sh
OMAVLESS_TEST_MIHOMO=/usr/bin/mihomo ./tests/run-rust.sh
OMAVLESS_TEST_MIHOMO=/usr/bin/mihomo ./tests/run.sh
```

The focused `cargo test -p omavless-runtime batch --locked` gate includes actual
private Unix sockets plus canonical loopback HTTP through the production
subscription transport: one-commit success, replay, cancellation, HTTP failure,
ownership revocation, status responsiveness and exact unchanged store on failure.
The HTTP listener is a synthetic provider, never a Mihomo TCP controller.
No network-interface or privilege-sensitive behavior is part of this scheduler.

Additional regressions cover shared-pool saturation, disconnect fencing,
in-flight shutdown, failed spawn, panic recovery, stale supervisor tickets,
operation-ID collision, deadlines and cancellation/commit ordering. Spawn
failure injection exists only in test builds and accepts no client control.
Error projections never include synthetic feed URLs, profile data or raw
provider response bodies. Full corpus parity remains required, not only these
new Rust-specific scheduling assertions.

Installed provider/frontend start/progress/cancel and real ownership cutover
remain part of the later bridge gate, not a claim made from isolated fixtures.
The independent packaged TUN and readiness consolidation issues #178/#183
still block that activation. Bare-metal adds no mandatory scheduler-specific
gate because this slice has no physical NIC/suspend behavior.
