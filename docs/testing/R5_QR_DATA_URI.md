# Native in-memory QR display

This independent client-only helper restores QML QR display without a
temporary PNG file or shell/base64 pipeline. Fixed `desktop qr-data-uri` takes
private text only on stdin, reuses existing `qr_png` and encodes its bounded PNG
as canonical `data:image/png;base64,...`. Existing binary `desktop qr` is unchanged.
The helper itself has no daemon method, store/ownership read, process owner
change, network operation or privileged path. QML obtains explicit private
export data from the existing canonical `profile export ID qr` read before
passing it to the helper. Record IDs address the selected profile; reusable
URI/credentials never enter argv. Subscription-managed profiles may be exported
under the existing canonical policy; no profile or subscription mutation occurs.

The existing Python QR output and native `qr_png` semantics are the reference.
Decoded PNG must equal the binary helper output. The existing centred QR window
retains its credential warning and is fed a separately bounded data URI, never
a data URI disguised as a file path. Image caching stays disabled; closing
clears the source and rejects unwanted late completions. Captured revision and
instance are checked against the frontend snapshot; the export response itself
does not add a new instance field or promise an atomic cross-process read.
Python remains the UI oracle/rollback; this is not R5/R6 or complete UI parity.
The runtime adds a direct dependency edge to the
already locked base64 0.22 crate; no new version/source is introduced.

Bounds: 64 KiB UTF-8 input without NUL; existing ten-second/4 MiB PNG encoder
budget and signature check; 5,592,430-byte maximum encoded output. Errors are
fixed and never echo input/helper stderr; successful output is private, not
diagnostic. Encoder exit 1 remains failure, not interactive cancellation.

Required gates: full Rust/fmt/clippy/parity, focused helper/CLI byte-equivalence,
max-bound and invalid-output/privacy tests, QML parser/state/launcher tests and
installed qrencode smoke using synthetic input. Exact QML must render the
synthetic image in EN/RU and exercise dismissal, stale completion, missing
dependency/failure, profile selection and panel reopen. No private profile URI
or QR screenshot is published, and no VPN connection is required for this
read-only desktop boundary.

## Local checkpoint

Try Omarchy ARM64: full Rust suite 750 passed, four existing ignored;
fmt/clippy and parity smoke passed. Native QR JavaScript 8 passed, existing
native actions 15 passed, launcher 17 passed and QML contracts passed.

An isolated Quickshell instance with synthetic store and the candidate binary
rendered the actual qrencode PNG in EN and RU. Both captures were visually
reviewed: image, translated credential warning and Close button were visible
without overlap. Dismissal cleared image/input and both dynamic processes were
destroyed. No real private profile or installed plugin was changed by this gate.
This preliminary working-tree check must be repeated or hash-verified against
the committed candidate. Keyboard/error-state and final integration acceptance
remain pending; this checkpoint is not merge acceptance.
