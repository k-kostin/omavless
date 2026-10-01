# G1a synthetic GUI comparison

Two isolated, read-only desktop prototypes implement the same fixture screen:
direct Rust/GPUI with `gpui-omarchy`, and GPUI Shell with bundled `omarchy-ui`.
They do **not** talk to the OmaVLESS daemon, read profiles, start a VPN or
change routes. All hosts are `.example`; every connection/status label is a
simulation. This is research material, not an installable OmaVLESS frontend.

The [dated VM result](../../docs/testing/G1_SYNTHETIC_TRIAL_2026-10-01.md)
records what actually ran and what remains unverified. The
[G1 contract](../../docs/roadmap/GUI_RESEARCH.md) remains the decision authority.

## Reproduce

From the repository root, with the local Rust toolchain and graphics libraries:

```sh
CARGO_TARGET_DIR="$HOME/.cache/omavless-g1-target" cargo test --manifest-path experiments/g1/direct/Cargo.toml --locked
CARGO_TARGET_DIR="$HOME/.cache/omavless-g1-target" cargo build --release --manifest-path experiments/g1/direct/Cargo.toml --locked
CARGO_TARGET_DIR="$HOME/.cache/omavless-g1-target" cargo run --release --manifest-path experiments/g1/direct/Cargo.toml --locked
node experiments/g1/tests/fixture-parity.mjs
```

The GPUI Shell runtime is a separate, upstream, non-published binary; the
experiment does not add it to the product package. Build the inspected
[`gpui-kit` revision](https://github.com/longbridge/gpui-kit/tree/b4c7cbdbbb57952c692c47ed13bbde9a06cdb7c5)
(`b4c7cbdbbb57952c692c47ed13bbde9a06cdb7c5`), then run its `gpui-shell`
binary with `experiments/g1/shell` as the application directory. For example,
inside an independently cloned checkout of that exact revision:

```sh
CARGO_TARGET_DIR="$HOME/.cache/omavless-g1-shell-target" cargo build --release -p gpui-shell --locked
"$HOME/.cache/omavless-g1-shell-target/release/gpui-shell" /absolute/path/to/omavless/experiments/g1/shell
```

Use `--watch` only for development; hot reload resets this prototype's
in-memory scene/selection. The Shell manifest explicitly disables storage and
grants no script network, process, filesystem or clipboard authority. Its
`omarchy-ui` source is bundled, pinned and licensed under
[`vendor/omarchy-ui/`](shell/vendor/omarchy-ui/UPSTREAM.md); the normal launch
must not fetch it. The profile/details panels now reflow through native flex
layout when the window resizes, without a script-render event. `gpui-shell check`
still panics while materializing the virtual list outside a rendered view in
the inspected upstream host; an actual debug/release launch and fixture test
are the applicable checks until that is fixed. Do not treat a launch as VPN or
platform acceptance.

The sample list can expand to 10,006 rows. Selection, search, collection,
language and simulated states are local UI actions only. There is deliberately
no Connect, Disconnect, Quit, daemon bridge or credential input.
