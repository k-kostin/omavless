# Native new-profile import

This bounded successor to #211 restores clipboard/file acquisition, private
profile preview and explicit **new** profile confirmation in the native QML
surface. It does not restore replacement/editor or subscription management.

The existing Python unified import is the semantic/UI reference. Rust already
owns `imports.classify`, desktop acquisition and `profiles.import`; this change
composes those boundaries through the installed frontend. Python remains
oracle/rollback, never a native fallback; R5/R6 and full interface parity remain
open. No VPN lifecycle algorithm or privileged action is introduced.

## Security and intentional differences

- The fixed launcher runs only native clipboard-read, pick-import, import
  preview and profile-import commands. It never accepts a generic method or
  shell command. Picker output is **contents**, never a path passed as a URI.
- Both sources use the same classifier. Native v1 input is capped at 32 KiB
  UTF-8 bytes, narrower than the desktop helper's 64 KiB limit; oversized or
  malformed input is rejected, not truncated.
- Acquired input is retained privately for preview/confirmation; no file is
  reread. Name plus original input travels through bounded stdin, not argv.
  Success metadata has no reusable credentials. Preview is private UI metadata,
  not a shareable diagnostic; all dynamic values use plain-text sinks.
- Confirmation is fenced by instance/revision/operation and uses the existing
  canonical owner/store transaction. Exact retry retains the same input and
  operation; there is no optimistic addition or second lifecycle owner.
- Existing names are rejected, not implicitly replaced. A recognized
  subscription produces an explicit unavailable/duplicate notice, not a legacy
  mutation or remote fetch. Existing supported protocol allowlists remain.
- Cancel/late-source/late-preview/changed-state cases cannot silently import.
  Fixed public errors never display raw helper/parser stderr.

## Required acceptance

- Full Rust, focused parser/CLI/socket replay/duplicate/privacy tests; existing
  canonical profile import differential corpus; Python/QML/launcher/catalog.
- Executed QML JavaScript: byte bounds, typed preview, same-input confirmation,
  cancellation, stale state, unknown-result exact retry, duplicate/no replacement,
  subscription refusal and public-error privacy.
- Exact candidate on Try Omarchy ARM64: English/Russian clipboard/file preview,
  cancel, duplicate rejection, explicit confirmation using isolated synthetic
  records; no real profile deletion or redundant private subscription creation.
- Actual chooser and clipboard acquisition, installed binary/QML identity,
  no Python fallback, unchanged connection/disconnection and final clean runtime.

Host acceptance is pending until recorded here. Synthetic data and screenshots
remain outside Git; no unavailable protocol interoperability is inferred.

## Local checkpoint — 2026-09-09

Exact code candidate: `e60337fb933fcf789808868d81d88332a03719dc`, based directly
on main `d90bd1605030710251f49fbd7e11194ddda6ad60`.

- Full Rust: 751 passed, four existing ignored; format, clippy and parity pass.
- Python: 338 run, 337 passed, one root-only skip with
  `OMAVLESS_TEST_MIHOMO=/usr/bin/mihomo`; QML/catalog/launcher pass.
- Eight executed native-import JavaScript tests, 15 native action tests,
  six snapshot tests, 16 launcher tests pass.
- Both installed-Mihomo Rust opt-ins pass with synthetic no-TUN configuration.
- Exact-head CI run `34376796510` passes.
- Same compiled executable and production QML exercised on Try Omarchy ARM64
  against a separate synthetic native owner/store. Helper acquisition used
  fixed synthetic `wl-paste`/`zenity` executables: this proves composition,
  **not** real clipboard/chooser interaction. File content was read by the
  actual native file helper. No real store or system clipboard was changed.
- EN/RU clipboard/file previews and cancel pass. Actual QML confirmation added
  one record (15 to 16); subsequent duplicate previews disabled confirmation in
  both locales. Final synthetic state had no pending/unknown action and no
  connection. Captures inspected locally showed localized titles/hints/buttons,
  no overlap and no background scrollbar under the modal. A focus-interrupted
  run was discarded and repeated uninterrupted.
- The synthetic UI/daemon were stopped after acceptance. Installed candidate,
  genuine chooser/clipboard interaction and connection regression remain pending.

Two invocation mistakes were corrected before counting evidence: the Python
opt-in requires an absolute core path, not `1`; the Rust opt-ins are selected
by that environment variable, not `--ignored`. Only corrected executed results
above are counted. No code workaround was made for either invocation error.

The later frontend IPC routing adjustment preserves all compiled Rust code;
installed QML head `2f7c4775f46d67efea317649836aceb0c6cd78c4` matches the checkout.
Package `0.0.0.r367.ge60337fb933f-1`, `/usr/bin/omavless` and the running daemon
share SHA256 `22ff534cf7736eb897e1f4e08b42b5ba5f2a7e81cc63d56fe55e6914915d1ea1`.
Plugin enabled, native facts coherent, disconnected with zero Mihomo/TUN.
Installed native helper discovery reports clipboard/picker/QR available and
zenity selected. Actual human chooser/clipboard and connection smoke remain
pending, not inferred from capabilities or synthetic tests.

CI run `34377693501` failed in the pre-existing editor fixture: expected
`Cancelled`, received `Unavailable`. It is not an import assertion failure.
The editor passes empty stdin, excluding the proposed seed-pipe EPIPE race.
The generic error does not establish the failing OS stage; transient executable
publication is a hypothesis, not a proven root cause. Test helpers now use
staged rename and the same bounded publication interval as existing core
fixtures. Production process/cancellation behavior and exact Cancelled/private
cleanup assertions are unchanged. A new green full gate is required.
