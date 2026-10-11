# Clipboard-safe input conformance

Production inputs are QML clients, not a second VPN owner. Test only synthetic
metadata in a separate Quickshell process; never run a hostile offer in the
installed Omarchy shell. `shell.qml` loads the actual controls and reports
structured QtTest counts. An exit code alone does not establish test success.

Prepare a private directory outside Git with committed `plugin/`, this
`shell.qml`, `Commons` and `Ui` links to the installed Omarchy modules, and
mode-0700 `runtime/` and `mock-tools/`. Copy the supplied `wl-paste` fixture into
`mock-tools/` and make only that staged copy executable. The runner sets PATH
to that directory alone, so discovery failure cannot fall back to the host
clipboard. The actual installed Rust helper is used for explicit Paste.

Run `OMAVLESS_FIELD_CLIPBOARD_MODE=normal node tests/safe-input/run-stage.mjs STAGE gui`,
then the same with `oversized` and `slow`. These cover typing, bounds, Enter,
Tab/Shift+Tab, binding preservation/reopen, atomic Undo/Redo, menu/keyboard
Paste and helper refusal without changing the field. Missing Quickshell/Qt is
NOT RUN, not PASS. These GUI tests are separate from ordinary cloud contracts.

Compile `clipboard-probe.cpp` as a test-only shared library in STAGE, using
the installed Qt6Gui/Qt6Core headers/libraries. It installs a 256-KiB synthetic
QMimeData offer **only under QT_QPA_PLATFORM=offscreen**. Its startup hook skips
the Quickshell supervisor; the controller closes the actual QML worker through
its ordinary private IPC, rather than terminating the GUI from the hook.

`run-stage.mjs STAGE baseline` must observe payload retrieval from an original
editable TextField. `passive`, `slow-passive` and `late-passive` must observe
zero retrieval from the production controls, including a subscription dialog
constructed after the offer exists. The source control separately proves the
offer can deliver data. This tests the native Qt clipboard-query boundary; it
is not a destructive unbounded Wayland/OOM test or a live VPN gate.

The TextArea document APIs do not expose an atomic replacement edit block to
QML. The shared control therefore maintains a bounded 32-state client-only
edit history, preserving bindings and one-step Paste Undo/Redo. Programmatic
seeds and closed fields clear that history; it never enters diagnostics/store.
The concealed URL is permanently readOnly Password text, not a transparent
secret TextArea overlay. Showing it explicitly enables plaintext editing.
