# Same-result manager executable-open diagnostic (source-only)

This narrow successor preserves the tested #645 `ea4bc252fe6c511835a44bcf3b3bfaf139c235f4`
and its immutable native/frozen receipts. The actual UID48048/v7 invocation
remains NONPASS and stopped. Its separately reviewed fixed-file observer recorded
the ten-phase BEFORE prefix ending at `manager_process`, with subordinate
`executable_open`. That is not an errno, cause, completion or CLI-admission proof.
No old scope query, retry, cleanup or process action is authorized here.

One separately owned HOST child demonstrated that the exact original-PID-directory
`openat("exe", O_PATH | O_CLOEXEC)` succeeds with dumpability enabled and returns
typed `EACCES` after only that child's dumpability is disabled. The child had an
explicit bounded byte protocol, normal exit zero, exact WNOWAIT zero observation
and a sole raw-zero reap. This establishes a denial counterexample, **not the
guest's errno or cause**. No primary-process credential/capability/dumpability
change or guest observation was part of the control.

The cfg(test)-only latch consumes the error of the **same existing nix open**
before its unchanged `Result<_, ()>` erasure. Only an active, unsealed diagnostic
whose exact major phase is `ManagerProcess` and subordinate BEFORE-step is
`ExecutableOpen` records it. No extra syscall, read, parse, retry, output or budget
charge is introduced. Ordinary production flags, validation, ownership and error
policy are unchanged. No ptrace capability/bypass, dumpability change, weaker
manager identity check or larger budget is proposed.

Only four literal categories exist: `eacces`, `eperm`, `enoent`, `other`; all other
typed errno values map to `other`. No number, raw message, path, PID, UID, profile,
network or account content is emitted. Recording an error permanently seals the
diagnostic against later operations; it cannot manufacture a success result.

The one bounded terminal write (at most 256 bytes) optionally adds a third line,
in this exact order:

```text
T4_STOPPED_FAILED_AT_V1 manager_process
T4_STOPPED_MANAGER_CAPTURE_BEFORE_V1 executable_open
T4_STOPPED_MANAGER_EXECUTABLE_OPEN_ERROR_V1 eacces
```

The final literal can be any one of the four enumerated categories. The third
line is legal only with that exact failure/BEFORE pair, never another phase,
another step, a success marker or a missing/reordered/truncated/duplicate line.
A future fixed-file observer must reject such malformed grammar rather than
export private bytes. This records a returned-error category, **not its cause**.
The existing 31-phase success output remains byte-for-byte unchanged.

Rust controls cover every category (including unknown errno mapped to `other`),
inactive/wrong-phase/wrong-step behavior, no later operation or overwrite after
failure, forced-success refusal, one write, short/throw/overcount terminal refusal
and permanent no-fallback/no-retry sealing. Source controls bind the latch to the
single original `openat`, compile-time elimination, exact flags and output order.

This initial diagnostic checkpoint is not a delivery graph or VM-ready build.
The inherited v7 delivery identities are historical only: a fresh separately
reviewed identity, exact source/native/build pins, full source/Rust gates, new
frozen originals and reviewed HOST/guest proposals are mandatory before ROOT's
sole authorized future invocation. No old frozen artifact is rebuilt in place,
and no old stopped scope may be reused. Main/RC/release are untouched.
