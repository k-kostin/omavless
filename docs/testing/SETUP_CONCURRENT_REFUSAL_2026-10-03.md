# Concurrent fresh-setup refusal contract

Source-only diagnostic/test correction forked from P4 Draft #571's executed
`d175b8a0da5c9ddde2c8c59c6d69211c980e1da6`, not main/RC. Its final dependency
base is #571 `14eeb6fc0207d4fb9c3facccd20d73e61fd1b614`. The Rust delta from
#558 `a8fa1711db5b2b9e69c9548078f199d1360ad46e` to the failed head is empty.
Neither a passing test nor this correction is whole-P4 or installed fresh-user
acceptance, runtime activation, an owner/main merge or publication permission.

## Retained negative evidence and attribution

[Test run 37098664387](https://github.com/k-kostin/omavless/actions/runs/37098664387)
failed at `fresh_setup_cli.rs:376`: the concurrency test required a lock-busy
message but received the exact fixed unsafe-path/permissions message. Nine
other CLI setup tests passed; this is retained as NONPASS, not waived as a flake.

The real unchanged executable was built at the exact failed source and frozen
outside Cargo without capabilities. In 128 actual two-process CLI cases there
were 128 successful children, 104 exact Busy refusals and 24 exact UnsafePath
refusals, with no other category. Every pair created exactly two canonical
files, preserved the complete fixed initial store/template and private modes,
and then completed a zero-create retry with unchanged member bytes and full
identity metadata. Ownership/runtime publication remained absent. Both children
were reaped before inspecting either result. The unmodified frozen integration
test then independently failed locally on its first invocation: zero passed,
one failed, exit101, the same line376/message mismatch. These are counterexamples
to the old assertion, not fresh-setup host acceptance.

The hardened constructor inherited from `660f8cd` deliberately refuses an
intervening lock creator. Two callers can both observe ENOENT; the first creates
the private lease, and the other's `O_CREAT|O_EXCL` open cannot reach flock.
The opener maps refusal to `UnsafeRuntimeDirectory`, which setup maps to
`UnsafePath`. Reopening, retrying or repairing that new inode is prohibited.
An ordinary already-existing held lease separately returns Busy.

A narrower deterministic private fixture pauses only at the existing
`PriorChecked` scheduling seam, creates and retains a **real competing
MigrationLock**, and checks the original constructor's exact refusal. A second
fixed exclusive open asserts actual kernel `EEXIST` against that retained
winner. Winner FD/path metadata, permissions, link count, empty bytes and lease
authority remain unchanged; no ownership state/runtime directory is published.
No fabricated errno, alternate production opener or permission repair is used.
This proves the local mechanism. With no syscall transcript from the CI runner,
attributing the CI event specifically to EEXIST remains an inference. Host
`strace` was unavailable; no tool installation or ptrace/security change occurred.

## Bounded correction and gates

Only a cfg(test) constructor fixture and the executable integration test change.
The latter collects both child outputs before assertions, admits only exact
exit2/empty-stdout Busy **or** UnsafePath messages, and retains/strengthens
created-count, full fixed store/template, private UID/mode/link, unchanged
device/inode/mtime/ctime/bytes on retry and no-owner checks. Generic errors and
partial success are not accepted. Existing unsafe-path and held-lock negative
tests remain unchanged. Production acquisition/validation/error mapping is
unchanged; the actual production executable remained byte-identical after edits:
SHA256 `63f5b0e11ca4439c3d1a3f11c4361c0f23d206fb8ab6f33261ab85b242304ca3`.

Pre-freeze checks passed: all ten CLI setup tests, the new deterministic fixture,
36 cutover-related tests, strict workspace/all-target Clippy, format/diff, and
313 source/frontend tests with two existing skips plus JS/QML/navigation gates.
The independent source review found no concrete defect. Final immutable-repeat
and cloud results belong to the exact Draft head and are recorded in its PR body;
predecessor CI failures are not relabeled. No VM, installed config, user/system
service, DNS, route, provider or settings operation was performed. All synthetic
roots/artifacts/logs remain private outside Git.
