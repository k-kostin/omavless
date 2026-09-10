# Native profile file export frontend

This QML-only checkpoint restores explicit profile file export through the
existing Rust `profile export ID file` and `desktop export-file` boundaries.
It adds no daemon methods, privileged actions, store writes or tunnel work.
Python's `export_file` remains the reference for content; its existing Rust
differential corpus and atomic same-user `0600` helper remain unchanged.

The selected profile row has an Export action. The existing centered single-line
prompt is reused for an absolute destination, with an explicit credential and
overwrite warning before confirmation. The selected record, daemon instance
and revision are captured at prompt opening and checked on confirmation and
again before the writer receives input. The fixed helper receives destination
and bounded exported content via stdin, never argv or diagnostics. It rejects
unsafe existing destinations and writes atomically with mode `0600`.

Export reads/writers are disposable processes with a 15-second watchdog.
Intermediate buffers are cleared after stdin handoff and collectors destroyed
on completion. Failure messages use only local English/Russian catalog keys.
An export already handed to the atomic writer is an explicit external effect;
later UI closure does not pretend to retract a completed file.

Deterministic gates cover selected-record identity, path byte/control bounds,
literal shell syntax, private stdin transport, stale/deleted record refusal,
malformed/error replies, cleanup/watchdog declarations and launcher arity.

Installed acceptance remains required on the final integrated head:

- English/Russian prompt, keyboard/cancel and non-overlapping profile controls;
- cancel creates no file;
- explicit export to a private temporary path has exact content and mode `0600`;
- existing same-user regular destination replacement and symlink refusal;
- exported content/destination absent from argv, public output and logs;
- panel reopen reports result, and VPN ownership/lifecycle remains unchanged.

No private fixture/export is committed. This is frontend parity, not R6 or a
claim that Python can yet be removed from the complete product.
