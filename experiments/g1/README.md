# G1a synthetic GUI comparison

Two isolated, read-only desktop prototypes implement the same fixture screen:
direct Rust/GPUI with `gpui-omarchy`, and GPUI Shell with bundled `omarchy-ui`.
They do **not** talk to the OmaVLESS daemon, read profiles, start a VPN or
change routes. All hosts are `.example`; every connection/status label is a
simulation. This is research material, not an installable OmaVLESS frontend.

The [dated VM result](../../docs/testing/G1_SYNTHETIC_TRIAL_2026-10-01.md)
records what actually ran and what remains unverified. The
[interaction follow-up](../../docs/testing/G1_INTERACTION_AUDIT_2026-10-01.md)
records the later synthetic focus and transient-state review. The
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
layout when the window resizes, without a script-render event. The wrapped
panels contribute to outer scrolling in short windows: a 400×700 VM wheel
test reached the complete Details panel. `gpui-shell check`
still panics while materializing the virtual list outside a rendered view in
the inspected upstream host; an actual debug/release launch and fixture test
are the applicable checks until that is fixed. Do not treat a launch as VPN or
platform acceptance.

The sample list can expand to 10,006 rows. Selection, search, collection,
language and simulated states are local UI actions only. There is deliberately
no Connect, Disconnect, Quit, daemon bridge or credential input.
In both synthetic candidates, `Tab` reaches the profile list, `Up`/`Down` move a
highlight without changing the inspected or confirmed profile, `Enter`
inspects the highlighted row, and `Escape` returns focus from the list. An
empty search displays a localized no-results message; pressing `Enter` there
does not erase the prior inspected profile. Generated sample details remain
resolvable after filtering or hiding the large list. These are synthetic
keyboard interactions, not an accessibility or production acceptance claim.

The direct Rust prototype additionally exposes named native accessibility
roles for status, search, the virtualized profile list and the read-only
Details summary. Its row labels keep inspection selection separate from the
synthetic confirmed connection. These semantics were probed through the VM's
accessibility bus; full screen-reader acceptance remains open. The Shell
prototype's native accessibility tree is still incomplete in the pinned
host. See the dated interaction audit for the exact comparison and limits.

Both trials offer the same three **synthetic** palette inputs: dark, light,
and a deliberately malformed palette. The last one must replace the *whole*
palette with the standalone dark default, never leave mixed old/new colors.
The direct Rust trial checks its copies against the Shell fixture in a test.
The controls do not modify Omarchy's real theme files. The direct toolkit
initialization may read the system theme before the trial explicitly applies
its synthetic default; it then stops following system changes. These controls
exercise in-process presentation continuity, not live theme-file watching.

In the direct Rust trial, a narrow window also supports `Page Down` and
`Page Up` for the outer Profiles/Details scroll area, including when search
or the profile list has keyboard focus. These keys move only the viewport;
they do not inspect a different profile or alter the simulated connection.

The pinned GPUI Shell host does not expose a script-controlled handle for
its ordinary outer scroll area. Its synthetic trial therefore offers an
explicit read-only Details page: the header button or `Page Down` opens it,
and the header button or `Page Up` returns to Profiles. This is page
navigation, **not** equivalent scroll behavior. The inspected profile and
separately confirmed synthetic connection remain unchanged. Mouse-wheel
scrolling of the original combined page remains available.

For deterministic visual review, the direct Rust trial accepts only
`--scene <synthetic-id>` (for example, `--scene switching`). An unknown ID
refuses without echoing it. This chooses fixture state before the window opens;
it is not a daemon state selector or a VPN operation.
