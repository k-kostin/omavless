# T2a: opt-in read-only terminal client

This development checkpoint is **not the T2 MVP or a packaged release**. The
installed 0.8.2 application/plugin, marketplace snapshot and runtime ownership
remain unchanged. T2's owning product contract is
[TUI_APP.md](../roadmap/TUI_APP.md).

## User task and fixed interaction boundary

See the runtime's current local connection state, identify the active profile,
browse/filter alternatives, and close the terminal without changing the VPN.

- Header: connection observation and connected profile, independent of filtering.
- List: profiles, subscription source, favorite marker and connected/missing badge.
- Selection: `>` plus a separately labelled **Selected for browsing** footer.
  Moving selection, Enter, search, refresh and help cannot connect anything.
- Footer: navigation/search/refresh/help and **Close UI — VPN stays unchanged**.
  Long names cannot push the exit hint out of the footer.
- `/` enters name search; Enter/Esc finishes search; a further Esc clears it.
  Arrows/j/k and Home/End browse. `?` opens help; Esc closes help. `q` closes
  outside text entry; Ctrl+C closes from any view. No mouse is required.
- Minimum full layout: 50 columns by 14 rows. Smaller terminals show a resize
  message and remain closable. Provider names are plain text, never translated.

The status describes **local runtime observation, not Internet/DNS reachability
or fail-closed protection**. Cached metadata alone cannot produce Connected.
Connected requires coherent fresh metadata/observation, an identified profile,
owned running core, matching authenticated configuration and one managed TUN.
Failures, recovery, stale or mixed snapshots never become Disconnected/Connected
by default. The mode shown alongside status is desired configuration, not an
independent route-verification claim.

## Build/run without installing or restarting anything

```bash
cargo build --locked -p omavless-runtime --features tui
./target/debug/omavless tui
# Process-local locale; does not modify OS or plugin settings:
OMAVLESS_LOCALE=ru ./target/debug/omavless tui
```

If `CARGO_TARGET_DIR` is set, use its `debug/omavless` path instead. This is the
same primary binary/command contract as the roadmap, not a second VPN owner.
The default runtime build does **not** enable `tui`; Arch packaging and release
assets are untouched. The TUI library has no runtime/store dependency and only
receives a typed read callback from the CLI. Default runtime dependency-tree
checks must continue to exclude Ratatui.

No supported `Open app` button is added to QML yet. Do not replace the installed
binary/frontend merely to inspect this candidate. No automatic service startup,
activation, package installation or privileged prompt is part of this slice.

## IPC and freshness

Only four fixed methods are reachable from the TUI: `system.hello`,
`capabilities.get`, `ui.snapshot`, `runtime.observation`. The adapter reuses the
existing runtime client's private-directory/socket mode and ownership checks,
peer credentials, bounded v1 framing, response-ID validation and five-second
response deadline. There is no arbitrary method/parameter or controller API.

The client negotiates v1 and activated ownership, checks required methods, then
requires equal instance, revision, desired generation/mode/connection and actual
state across both projections. Same-instance revision regression is rejected;
a new runtime instance resets row selection. It accepts at most 256 profiles,
64 subscriptions, 80-scalar/320-byte names and 64-byte record identifiers.
Projection additions unrelated to this view are ignored, not exposed as raw JSON.

One worker performs the four reads serially. Capacity-one channels and one
pending refresh prevent unbounded jobs. Refresh interval is three seconds;
freshness expires six seconds after a read cycle **starts**, not when it finishes.
The input loop remains responsive during a slow read. Close drops the channels;
it does not wait for the read or send any shutdown/disconnect operation.
Terminal event reading is also off the UI thread with a bounded 16-event channel;
real terminal teardown cannot trap the signal/exit loop in a backend poll.
Signal handling and an RAII guard restore terminal mode on normal/error exits.
SIGKILL cannot run cleanup, but still cannot mutate the VPN.

## Presentation/privacy and localization trial

- Ratatui 0.30.2 with the Crossterm backend; other optional backends/default
  features disabled. See [Ratatui feature guidance](https://ratatui.rs/installation/feature-flags/).
- No arbitrary terminal escape, clipboard OSC, shell, file export or log path.
  Provider names are bounded; C0/C1 controls and directional override/isolate
  characters are replaced before rendering. Errors are fixed public categories,
  not raw remote messages. Private model types deliberately lack Debug/Serialize.
- This is a **private same-user UI**, not a shareable report. Names are visible
  intentionally. Live screen captures must not be published; record booleans,
  counts and sanitized classifications instead.
- A 30-key EN/RU compiled catalog is a bounded T2 trial. Shared keys are tested
  against QML's canonical translations. Unknown locale falls back to English;
  missing key shows fixed English fallback without echoing the unknown key.
  Locale is process-local, selected by OMAVLESS_LOCALE, LC_ALL, LC_MESSAGES, LANG.
- The [rust-i18n candidate](https://github.com/longbridge/rust-i18n) was evaluated
  as required by [I18N.md](../roadmap/I18N.md#rust-client-research-reference-t2).
  This slice has no interpolation/plural needs; it does not adopt that dependency
  or claim the eventual shared-catalog choice settled. Before expanding TUI
  translations, consolidate shared catalogs and trial bounded named arguments,
  plural/number formatting and provenance without global mutable locale.
- Default terminal colors only. Omarchy theme-following, theme replacement,
  light/dark testing and standalone theme configuration remain explicit T2 work.

## Repeatable development checks

```bash
bash tests/run-rust.sh
bash tests/run.sh
cargo build --locked -p omavless-tui --example fixture_preview
OMAVLESS_LOCALE=ru ./target/debug/examples/fixture_preview
```

The **test-only** example supplies synthetic metadata and never links the runtime
or accesses its socket/store. Optional example arguments `empty`, `unavailable`,
`recovery`, `slow` exercise presentation/error handling. Its Connected fixture is
not a live VPN result or a marketing screenshot. Do not add this fixture mode
to the production CLI.

Conformance tests cover read allowlisting, negotiation, coherent snapshots,
bounds, no false connection from selection/cache, stale/error/restart behavior,
keyboard semantics, EN/RU fallback, terminal-injection sanitization and narrow
rendering. Test-only PTYs cover q/Ctrl+C/SIGTERM/SIGHUP, slow-read exit and terminal
restoration. `tests/run-rust.sh` checks the optional runtime adapter as well as
the default workspace. The Python PTY driver is developer tooling, not a runtime
dependency.

Live acceptance should attach to an already running runtime, verify the rendered
active identity privately, close/reopen the client, and compare before/after
desired/actual state, revision and owner/core/TUN identity. Do not disconnect an
owner-requested healthy VPN to satisfy a generic UI-test cleanup rule.

## Still out of scope

Connect/disconnect/mode mutations, confirmation/revision-admission UX, grouped
subscription navigation/refresh, diagnostics/traffic/probes, theme adapter,
launcher/focus integration, default packaging and release, runtime restart under
a real tunnel, x86_64 installed TUI acceptance. These are follow-up slices, not
features implied by a read-only preview or a green test suite.
