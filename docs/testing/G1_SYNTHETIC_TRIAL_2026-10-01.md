# G1a synthetic native trial — 2026-10-01

Scope: exploratory GUI research, **not** product acceptance, VPN testing, G1b
daemon-client authorization or a decision to replace QML/TUI. Exact code is the
`experiments/g1/` tree on the review branch linked from
[G1 research](../roadmap/GUI_RESEARCH.md). The current OmaVLESS runtime, service,
private profiles and desired tunnel state were not modified. Screenshots and
transient logs stay outside Git.

## Build and environment

- Host build: Arch Linux x86_64, Rust/Cargo 1.98.1. Direct app: GPUI Kit 0.7.0,
  [`gpui-omarchy` `da310c5c`](https://github.com/huacnlee/gpui-omarchy/tree/da310c5c817a09010cc31cc2f0928d8e2af086e9), locked standalone Cargo
  manifest, with the upstream `gpui-pre-macros` release-profile workaround at
  the root of that standalone workspace. Debug test/build and external release
  build/run succeeded.
- Shell host: [`gpui-kit` `b4c7cbdb`](https://github.com/longbridge/gpui-kit/tree/b4c7cbdbbb57952c692c47ed13bbde9a06cdb7c5),
  GPUI Shell 0.7.0 / QuickJS JIT in its default debug and release modes.
  `omarchy-ui` MIT source is an unmodified bundle from
  [`8def5429`](https://github.com/huacnlee/omarchy-ui/tree/8def54298bda03b8d481436e62b5885af4eed674), with notice beside the
  source. Both host modes launched and rendered on the VM. A fresh Shell-host
  package or cross-architecture build was not prepared.
- Native run: Omarchy Dev VM, Linux `7.2.5-3-omarchy`, Hyprland 0.56.2,
  Wayland/virtio virtual display (1890×2080, 1⅔ scaling), x86_64. Binaries
  were copied to this VM as transient trial artifacts. The VM's installed
  OmaVLESS service was left alone.
- Fixture: six invented `.example` profiles, four collections, eight state
  scenes, 10,000 additional generated rows. JS and JSON fixture parity and the
  Rust selected-A/connected-B fixture test passed. Both UIs keep visual
  selection separate from the synthetic confirmed connection.

No comparison budgets were preregistered. Figures below are exploratory raw
observations, not acceptance thresholds or product speed claims.

## Observations

| Check, one VM | Direct Rust | Shell with bundled UI |
| --- | --- | --- |
| Release executable bytes | 44,473,136 | 84,253,320 (general-purpose host, not one-app bundle) |
| Warm-cache process start to Hyprland window, 50 ms polling | 110 / 111 / 110 ms | 110 / 110 / 110 ms |
| Idle PSS after ~2½ min | ~49 MiB | ~75 MiB |
| 10,006-row PSS | ~52 MiB | ~82 MiB |
| Native debug/release window | rendered / rendered | rendered / rendered |
| Network namespace isolated after packaging assets | window opened | window opened |
| Search, Enter-to-inspect, collection filter, EN/RU, state scenes | observed | observed |
| 700-px floating narrow layout | one column after resize | one column **after a UI action** |

The memory figures are sampled proportional-set sizes, not a controlled
longitudinal benchmark. The 50-ms startup poll detects a compositor window,
not the first finished frame. Warm page cache, one VM and tiny fixtures limit
any performance conclusion. The virtual lists rendered a bounded visible
slice; the list-size switch added only a few MiB in these samples. Sustained
updates, latency distributions and scrolling continuity are not yet measured.

An initial Shell manifest used an exact Git dependency on `omarchy-ui`.
Despite the pin and populated cache, launching with networking unavailable
failed during the **host's** pre-execution Git fetch. One warm launch exceeded
5 seconds; two subsequent launches took ~750 and ~639 ms to window creation.
Bundling the exact MIT source removed that runtime fetch; three warm launches
then measured ~110 ms, and an offline launch opened a window. The manifest also
uses `storage: false`: an empty capabilities object would have enabled the
Shell host's default storage authority. No bridge, credentials or VPN action
were registered. This does not amount to a security audit of the JS/JIT host.

The Shell window's `viewport_size`/`bounds` stayed at the old 880 px immediately
after Hyprland resized it to 700 px. A later locale-button action triggered a
render, at which point both reported 700 px and the one-column layout appeared.
Direct Rust reacted to the narrow resize without that extra action. Therefore
**Shell responsive behavior is not accepted** as-is; a host/window invalidation
fix is preferable to a timer-based repaint workaround. `gpui-shell check`
also panics while materializing this virtual list outside an active rendered
view (`current_view`); actual debug and release launches worked. These are
specific upstream-host integration gaps, not evidence of VPN failure.

In `--watch`, changing a script reloaded the view and reset local scene and
selection. A daemon-bound future client would have to re-negotiate and
re-project state after reload, never infer a connection from retained UI
memory. The synthetic app itself has no daemon state to change.

## Remaining G1a checks and decision

- **Not verified:** native accessibility tree, IME, comprehensive keyboard
  focus/Escape recovery, clipboard, wheel-scroll continuity and update/scroll
  latency under load. Search Enter and pointer activation were observed, but
  they do not establish the full interaction matrix.
- **Not verified:** live Omarchy theme replacement, missing/broken theme,
  whole-palette fallback or contrast across light/dark. Direct Rust uses
  `gpui-omarchy`'s theme; the Shell fixture currently applies fixed Omarchy-like
  synthetic tokens. Neither visual impression proves theme acceptance.
- **Not verified:** ARM64, NixOS packaging, cold-start distribution on a fresh
  machine, installed update/remove, native sandbox/JIT policy and crash
  recovery. No production installer or optional GUI package was added.
- **Resource cost:** both have substantial build trees and GPUI dependencies;
  Shell adds a scripting engine, vendor review and extra capability/distribution
  questions. User launch should never require Cargo or a remote fetch.

Recommendation: keep G1 production **deferred** while T2/T3 remain the product
focus. If G1 resumes, direct Rust is the simpler baseline. Shell may still be
worth revisiting if its resize invalidation, hidden `check` panic, permission
audit and measured iteration benefit are addressed. Neither candidate has
passed G1a, so G1b read-only daemon binding and G1c mutating controls do not
start from this report. Cleanup/rollback is removing only the trial windows,
transient binaries and isolated build caches; the daemon and VPN are untouched.
