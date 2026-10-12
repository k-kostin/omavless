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
[bounded VM resource sample](../../docs/testing/G1_RESOURCE_SAMPLING_2026-10-02.md)
records five warm-start measurements per candidate and a separate 10,006-row
check. Its [measurement helper](tests/measure-vm.sh) refuses to run outside
an explicitly selected VM and measures only the trial process it starts. The
[G1 contract](../../docs/roadmap/GUI_RESEARCH.md) remains the decision authority.

## Reproduce

From the repository root, with the local Rust toolchain and graphics libraries:

```sh
CARGO_TARGET_DIR="$HOME/.cache/omavless-g1-target" cargo test --manifest-path experiments/g1/direct/Cargo.toml --locked
CARGO_TARGET_DIR="$HOME/.cache/omavless-g1-target" cargo build --release --manifest-path experiments/g1/direct/Cargo.toml --locked
CARGO_TARGET_DIR="$HOME/.cache/omavless-g1-target" cargo run --release --manifest-path experiments/g1/direct/Cargo.toml --locked
node experiments/g1/tests/fixture-parity.mjs
node experiments/g1/tests/freshness.mjs
node experiments/g1/tests/workload.mjs
node experiments/g1/tests/metadata.mjs
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

### Synthetic freshness boundary

The direct Rust and Shell trials include separate freshness
components (`direct/src/freshness.rs` and `shell/freshness.js`). Their user goal
is to keep a future read-only status view honest if a response arrives after
the client loses contact with its source or that source changes instance.
The primary action in these components is a simulated status read; its target
is the instance named by a locally issued request ticket. The visible result
is a confirmed connection only after a newer accepted snapshot says
`connected`. A navigation highlight or inspected profile never changes this
fact. These components now drive the synthetic rendered status through the
bounded update adapters described below; inspection remains separate.

The model clears confirmation immediately on simulated transport loss or new
hello, rejects a late response from a prior client epoch, and accepts a lower
revision only after an explicit new-instance attachment. Recovery and other
non-connected phases clear confirmation even if a stale ID is carried along.
It has no socket, wire decoder or daemon API; both
implementations use invented instance IDs and profile names. This is a G1a
architecture probe, not G1b read-only client acceptance. Exact semantic IPC
fields and lifecycle behavior remain owned by the established control-plane
contract. Earlier #528 evidence applies to its test-only wiring. The successor's
native rendering, EN/RU wrapping and interaction checks remain **NOT RUN** until
the exact new candidate is exercised by the designated VM operator.

### Finite synthetic update workload

User goal: continue inspecting a large list while source status changes, and
see an honest unknown state if that source is lost. **Simulate 1,000 updates**
targets the current invented source and scene; **Lose sample source** invalidates
that source; **Read fresh sample** attaches a new client epoch and reads one new
snapshot. These are local fixture controls, never VPN operations. They preserve
the inspection selection, highlight, search, collection and existing list scroll
handle. Existing scene buttons deliberately choose a new fixture and its initial
inspection selection, as before. The sample number makes the burst's newest
accepted update visible in both candidates.

Admission occurs before replacing a single pending snapshot. Old revisions,
wrong instances and old client epochs cannot displace an admitted update. Each
finite burst admits at most 1,000 updates and flushes once; loss clears pending
work and confirmed identity immediately. Burst/loss controls are unavailable
while detached. Reattachment alone clears confirmation; only a fresh accepted
connected snapshot restores it. Recovery and switching retain their existing
unconfirmed/previous-server presentation.

Status updates do not invalidate metadata. Both apps use tested profile-cache
helpers keyed by search, list size, collection and actual metadata content.
Inspection Enter reuses that cache; highlight membership checks use a retained ID
set instead of scanning all rows on each repaint. Direct Rust also retains its
row geometry until metadata or rem size changes. The tests exercise 100,000
updates with the same 10,006-row projection, empty filtering, generated-profile
inspection lookup, queued loss, stale replay and every shared fixture scene.

This is a finite synchronous synthetic producer, **not** a sustained background
stream, timed animation, traffic-history buffer or real changing metadata feed.
It establishes a replacement-slot bound, not transport backpressure or native
frame latency. The synthetic metadata successor below tests removed/renamed
rows; daemon/client wiring is still G1b.
Native wide/narrow EN/RU inspection, stable scroll/focus during bursts and source
loss, and latency/memory sampling under the declared workload remain pending.

### Whole synthetic metadata replacement

The successor to #741 adds the same executable scenario to both trials:
**Rename inspected sample**, **Remove inspected sample**, **Replace sample
collection**, and **Restore fixture list**. Their targets are an inspected
invented ID or the fixed Sample collection. These replace local fixture metadata;
they are not profile/store/daemon mutations. The shared
[`metadata-fixtures.json`](metadata-fixtures.json) supplies names and replacement
rows for both implementations. Fixture names remain unchanged across locales.

The initial full metadata snapshot contains **at most 10,006 research rows**.
This is deliberately not the native product's 256-profile capacity. The generated
list switch hides/shows those rows; it never regenerates a deleted profile. Full
replacement admission rejects oversized lists, duplicate/invalid IDs, oversized
text, controls/directional markers, non-`.example` hosts and unsupported fixture
collections before publishing any prefix. This is a fixed synthetic data model,
not a general patch protocol or a parser for real profile/provider inputs.

Each accepted status sample binds to the admitted metadata generation. Changed
rows require a newer generation and a newer status revision on the current
ticket. Status without corresponding metadata, older generations, changed data
at the same generation, wrong instance/epoch and regressing revisions refuse
before replacing the latest slot. Generation/revision use the JavaScript-safe
integer bound in this research model; they are data, never runtime capability or
effect authority. Same-content metadata at a newer generation retains the exact
content/cache identity. Actual name/membership/order changes invalidate the cache.

Replacement preserves search, collection, locale, palette and native scroll
handles. Inspection is reconciled by stable ID: renamed/filtered targets remain
selected; missing targets become explicit no selection, never another row.
Restoring the baseline does not restore a previously removed inspection target.
Keyboard highlight remains navigation and may move to an available row without
inspecting it. A current synthetic connected ID may survive removed metadata;
Details then explicitly says **not in current sample**, rather than inventing
Disconnected or substituting a different profile. Native viewport clamping after
membership/size changes is a separate pending renderer check.

Source loss clears queued metadata and connection confirmation, retains only
cached browsing, and visibly labels that list as cached/unverified. A new epoch
needs an explicit fresh full metadata snapshot before confirming any status;
the fresh-read control simulates that delivery. A late old-ticket reset cannot
restore removed rows or a connection. Whole metadata/status publication is one
flush; status-only bursts continue reusing both the actual metadata and filtered
projection in the 100,000-update tests.

Source scenario and fault checks do not establish rendering: exact-head wide and
400×700 EN/RU control wrapping, selected/connected distinction, cached-list label,
empty filtering, pointer/keyboard reachability and scroll continuity remain
**NOT RUN** until the sole designated VM operator tests this successor. Earlier
#741/c8e13578 source/build evidence and its frozen binary/full Shell resources
remain separate; its pending native card is not overwritten by this successor.
No G1a closure, G1b daemon binding, production adoption or native latency claim.

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
