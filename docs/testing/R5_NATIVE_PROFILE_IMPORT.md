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
