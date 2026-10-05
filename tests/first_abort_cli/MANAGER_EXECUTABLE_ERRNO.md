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

This diagnostic checkpoint is not a VM-ready delivery. The fresh source proposal
uses UID/GID48049, account `ov-t4-abort-v6`, HOME `/home/ov-t4-abort-v6`, runtime
`/run/user/48049`, delivery `/home/kdk_vm/.cache/t4-first-abort-cli-delivery-v8`,
root stage `/run/ov-t4-cli-guard-v8` and receipt schema v8. Absence is not asserted.
Native source checkpoint is `bf650713ed6daa066b38bbe8c1ed608f8a36534c`;
both the guard and trusted loader admit only that native pin. Their later source
checkpoint must be source-equivalent for Rust/Cargo, not relabeled as that build.
The inherited v7 receipts are historical only. No old frozen artifact is rebuilt
in place, and no old stopped scope may be reused. Main/RC/release are untouched.

## Exact native build and HOST freeze, October 5, 2026

Actual build/source head is `f17cb06cf742b8d40ce0278a15013ce1479665e0`, distinct
from the native checkpoint above. Its complete Rust/Cargo diff from bf650713 is
empty. The full source gate passed 639 tests/two skips plus JS/QML/frontend.
The corrected full Rust gate returned known exit0 (`603069`): runtime library
1193 passed/44 ignored/one filtered, including all three new errno controls;
workspace, strict Clippy/TUI and parity gates completed. Earlier full attempts
`a7cfa1` and `4ba4a5` remain NONPASS: fixture umask and HOME containment respectively.
The successful HOST environment used a separate private HOME containing the
exclusive new Cargo target and an independent short private TMPDIR; full-test
child umask022 and parent capture umask077. Production predicates were not relaxed.

The separate explicit locked/offline helper `--no-run` build returned exit0
`89af78`, Cargo `fresh:true`, actual original755/single-link, 278617560 bytes:
`137efa7ee75c634497989f8a9b389a81fbffac098d0cde2e15d91fd7af16d4ce`.
The separate explicit locked/offline normal-CLI build returned exit0 `5e7d1c`,
Cargo `fresh:false`, actual original700/two-links, 98974608 bytes:
`f6507d3ccf38a776cdeea2901d911938c14e63727f7e83dfabe9801694b64da9`.
Its one fixed Cargo deps alias was admitted only as build provenance, not execution
authority. Neither command executed an ELF or an ignored VM fixture.

After FULL ROOT and independent source/provenance review and 23 controls each,
ROOT alone invoked the fixed HOST freeze once. It returned known exit0 `86dddb`.
Only the exact held new CLI/fixed alias normalized700→755; helper755 was unchanged.
Original inode, size and hash were checked, with actual before/after metadata
recorded honestly. New frozen originals outside Cargo are500/single-link and
retain the two hashes/sizes above. The immutable v4 freeze receipt (1767 bytes,
600/single-link) has SHA256
`854ed5d8d25201b9869ba17337ab4c22e01699d1fa05e7b89a1e23cb487dbac6`.

This later evidence/delivery-source checkpoint is not relabeled as the actual
build head. A fresh create-only HOST seal and complete reviewed guest graph,
including exact frozen/source/data pins and fresh identity preflight, remain
required before ROOT's sole future VM invocation. No v8 guest action, CLI
admission, errno/cause observation or product acceptance is supplied by the
HOST build/freeze. All earlier stopped scopes and original generations remain.
