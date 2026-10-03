# T4 retained current-epoch resync candidate

This inactive continuation is stacked on `dev/t4-current-live-resync` (#542).
Implementation tree: `5a408919ce8629defe1b63f4c7a942556b49c427`, retained unchanged
by merge `1e96c20b5cf740bf71f32866e1939f2b257c5d83`. The parent policy followup
`3c9b5806193d13543c65c88baead9fcd7eac5277` is included with cherry-pick provenance.

## Scope

The ordinary login receipt parser and strict consumed identity predicate are
shared, but ordinary pending checks remain intact. Production proof capture
uses only the existing fixed `package()` and `manager_epoch()` checks, exact
current cutover paths, original receipt FD and metadata, and the existing lease.
Synthetic source injection exists only under `cfg(test)`. The proof has no
Clone, serialization, caller-supplied epoch or Boolean production permit.

Current-Off resync captures its original snapshot before manager queries and
consumes the fresh epoch proof once. It writes no receipt or record, removes no
fence, exposes no IPC/CLI and constructs no owner. The only successful result is
`ResynchronizedStillFenced`. Finite readback does not claim atomic protection
against a hostile same-UID writer.

## Source acceptance

At the implementation tree above:

- Focused `cargo test --locked -p omavless-runtime --lib epoch --quiet`:
  12 passed, 2 ignored. Includes receipt schema/identity/mode/link/FD replacement,
  package and manager injection, original-snapshot drift, every-checkpoint
  receipt/epoch/host drift and actual synthetic SIGKILL/re-entry for Commit/Abort.
- Strict runtime all-target clippy passed; formatting and diff checks passed.
- `tests/run.sh`: 316 Python tests passed, 2 skipped, and source/frontend checks
  passed using a private temporary directory outside Git.
- Initial full runtime run: 979 passed, 1 failed, 30 ignored. The unchanged
  controller fixture failed before connection because the long home TMPDIR
  produced a Unix socket path exceeding `SUN_LEN`; this is not an epoch failure.
  The exact fixture rerun passed with short home TMPDIR; the complete rerun then
  passed: **980 passed, 0 failed, 30 ignored** (424.57 seconds).

No VM or installed package-positive test is claimed by this checkpoint.

## Next product boundary

Same-manager evidence is deliberately insufficient to bypass `current()`,
startup receipt validation, transaction reconciliation, coordinator admission
or any other normal pending guard. A later typed same-lease Off-startup path must
carry the exact historical/current witness through every affected guard without
manufacturing compatibility pointers or lifecycle receipts. It additionally
needs installed package/manager acceptance, fixed rendered-core validation and
fresh host/provenance checks. Existing source-executable refusal is tested, not
substituted for positive installed acceptance.

Missing or new-manager receipts refuse. Normal login consumption itself rejects
pending private transactions, so reboot/new-login continuation needs a separate
packaged invocation-authorized, crash-safe fresh-receipt transition. This slice
does not guess that protocol or reinterpret an old receipt as current authority.
