# Inert frontier fixture portability

The exact T3 source checkpoint `8ea8cdc2ed85093e4c822dbe28264e6e53c94f9a`
passed its local source gate, but GitHub run
[37246155021](https://github.com/k-kostin/omavless/actions/runs/37246155021)
failed with 34 errors before Rust checks. Preserve that failure; it is not VM
acceptance or evidence of a networking fault.

The inherited synthetic tests had three environment dependencies:

- Real temporary file ownership was compared against the transports' fixed VM
  UID/GID 1000. A cloud runner is not that VM account.
- Supervisor tests replaced the interpreter's shared `types.ModuleType`.
  Python 3.12's inspection/autospec code uses that object as an actual type and
  rejected the mock before the intended supervisor assertion.
- One inherited temporary-directory wrapper required `TMPDIR` to be present,
  although the source gate does not require that environment variable.

Only test setup changes. `tests/frontier_fixture_helpers.py` supplies a
module-local `os` facade for the synthetic tree/account. UID/GID observations
are explicitly modeled as the literal VM account; actual exclusive file IO,
bytes, device/inode, mode, link count, size and nanosecond timestamps remain
real. No `chown`, host account change or production UID relaxation occurs.
The supervisor's module-local `types` facade provides only its mocked module
factory; the interpreter's shared `types.ModuleType` remains a type. Absent
`TMPDIR` uses the normal test temporary-directory allocator.

Existing wrong mode, symlink, extra entry, short-write, exclusive-create,
unknown metadata, nonzero/boolean child and no-followup assertions remain.
Additional controls verify real FD facts/bytes, unpatched shared OS calls,
and rejection of a wrong modeled VM owner. These are synthetic controls,
not actual guest transfer or process evidence.

No transport/supervisor source, frozen source pin, manifest, native code,
package, service or product API changes. New exact-head local/cloud results
belong to the corrective PR; previous whole failures are not rewritten.
