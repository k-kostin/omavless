# Bounded public theme-source seam (source stage)

Parent: frozen #746 `93cffe1d97131651e6ffbe8a65009d0203045d0a`.
This is G1a synthetic presentation research, not real Omarchy theme following,
G1b, a production theme loader or a toolkit/adoption decision. No VM/native
window/build of the embedding host was performed.

## Actual pinned API findings

- `gpui-omarchy da310c5c817a09010cc31cc2f0928d8e2af086e9` has capacity-one
  notify-based system following and refreshes watched directory identities after
  symlink/directory replacement. Configurable-home following/watcher are private.
  Public `from_current_dir` uses unbounded `read_to_string` for palette/name;
  explicit `Theme.apply` stops following. Do not use that loader to claim 4 KiB.
- Shell `b4c7cbdbbb57952c692c47ed13bbde9a06cdb7c5` exposes asynchronous,
  root-scoped `fs.readFile` with a 64-MiB ceiling, but no `fs.watch`. Generic
  script fs is excluded from the new extension API. Source `--watch` reconstructs the script object
  and loses local state; it is not theme continuity.
- The same host publicly supports `HostModule.async_function`, plain data
  exports, `export_module`, and `host::main_with_components`. Its cap-std
  directory grant is private, not an available embedding-reader API. The staged
  embedding source uses real public APIs, not a fork or new permission framework.
- The supported CLI entry has **inherited grants even with `storage:false`**:
  app/store read roots, store write root and `exit`. The manifest blocks
  `localStorage`, not those filesystem grants or creation of its SDK store.
  `set_capabilities` before this entry is replaced by `local_capabilities`.
  The public branded entry uses the same grant path; no CLI grant override was
  found. This proposal does not claim deny-all fs. Its inherited grants were
  approved only inside the fresh owned synthetic HOME/XDG/app fixture with
  exact manifest and sole trusted writer, not as a new product permission.

## Implemented source portion

Shared fixtures define <=4096-byte ASCII synthetic palette text: explicit
dark/light mode, nine required RGB roles and six optional RGB roles. Unknown,
duplicate, incomplete, malformed/control/UTF-8/RGBA or oversized input publishes
no prefix: a whole explicit G1 dark default is prepared. This is a strict fixture
grammar, not general TOML/ANSI/user-theme support. No theme name, font, path or
raw text crosses the prepared-data seam.

Both static renderer controls now use that seam. Direct's former broken fallback
used SDK Tokyo Night, whereas Shell used G1DARK; the successor aligns the whole
fallback to G1DARK. This changes the affected Direct palette case; old native
results are not assigned to this successor.

Sequences bounded to JavaScript-safe integers are research data, not runtime/
effect authority. Admission precedes the one latest slot; older/bounded/
inconsistent samples cannot replace an admitted value. Same-content samples
reuse colors without repaint; fallback-status changes still notify. No view,
metadata/cache, filter, selection, focus or scroll owner is reconstructed. The
actual Shell consumer is tested with toolkit-global color effects substituted;
native positioning is not proved.

`theme_fixture.rs` is a Linux-only source-stage reader of fixed
`current/theme/colors.toml`. It anchors each component to borrowed directory
FDs via the process's own `/proc/self/fd`, admits at most eight relative symlinks/
depth with 64 steps, refuses absolute/root-escaping links, uses Linux
O_NOFOLLOW/O_DIRECTORY/O_NONBLOCK, and reads only regular owned non-writable
files without group/world write permission through one FD with a 4-KiB+1 growth
cap. Parent directories are opened
fresh per sample after subtree replacement. The selected root itself stays one
captured scope, never guessed/adopted from a replacement path. Ordinary I/O has
no hard kernel deadline; off-UI/max-one inflight protects rendering/resource
count, not hung-filesystem recovery. Same-user external tampering is not an
atomic freshness/public-provenance guarantee; the fixture operator owns the tree.

Only new public HOME/.cache/ovt fixtures were sampled. Tests cover directory/
symlink replacement, missing/malformed/oversize/restored files, absolute/parent/
loop escape refusal, non-regular Unix-socket/FIFO refusal, latest slots and
unchanged metadata/browsing owners. The real Linux FIFO fixture has no writer:
the externally bounded standalone test verifies nonblocking refusal. This is a
small source/kernel control, not SDK/native GUI rendering evidence.

## Staged embedding source — NOT built or activated

`shell-theme-host/` is a reviewable sketch with the existing pinned SDK as its
only framework dependency and existing `serde_json = "1"` as a test-only
dependency. No lockfile/dependency resolution or GPUI consumer build ran.
It requires a fresh owned synthetic process HOME `g1h-*` under
`.cache/ovt`, explicit `G1_PUBLIC_THEME_SOURCE=1`, fixed public
`.cache/ovt/g1-theme` root and copied `app` directory. The exact
`shell/theme-manifest.json` keeps the same identity/storage:false and selects
only the dedicated `theme-entry.js`; the bare-host manifest stays unchanged.
Before SDK initialization it
rejects symlink/parent escape in selected HOME and requires exact synthetic HOME
children for `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `XDG_STATE_HOME`,
and singleton `XDG_DATA_DIRS`/`XDG_CONFIG_DIRS`. Missing/inherited/outside/list
overrides refuse; no environment variables are mutated. The four selected SDK
roots must be owned mode-0700 directories with no preexisting entries, captured
with held FDs. The public `bundle_id_for_path(app)` must yield a bounded safe
identity; the pinned SDK store then resolves to new descendants under captured
`sdk-data/gpui-shell/apps` before its `create_dir_all` can run.

It refuses preexisting `.config`/`.local` trees before initialization. Actual
`XDG_RUNTIME_DIR`/display transport remains the OS session, not a store grant.
The fixture writer must not replace selected roots during the path-based SDK
run: held FDs do not turn SDK I/O into a hostile same-UID sandbox. The standalone
driver tests exact XDG/bundle identity admission plus empty/symlink root refusal
without initializing the SDK or using an actual process environment.

The extension itself registers only no-argument
`samplePalette()`, max-one inflight, work off UI; replies have only validated
mode/colors/status/sequence. It adds no arbitrary path or fs/process/network/
storage/clipboard API and no backend/core IPC. The CLI's inherited scoped fs and
exit grants remain as enumerated above, approved only for this owned public
fixture. Source re-review cleared the boundary, not runtime/consumer/native
use. No raw
path/error/palette text is returned by `samplePalette()`.

Before native use: primary/independent permission/reader-delta review; exact
dependency/lock/consumer build; full fixture preflight; dedicated Shell consumer
and Direct binding review/typecheck; max-one platform polling/cancellation
checks. No guessed public SDK API is supplied.

## Consumer source binding — NOT built or activated

The dedicated Shell entry installs the extension only in the source-enabled
trial. Direct selects the same fixed public preflight before SDK initialization;
an opt-in with invalid scope refuses before starting a window. No extension
import/control is advertised by the bare Shell client. Source-enabled controls
are bilingual **Read public palette** and explicit polling/manual state. They
mean fixture reading only, not real theme following or a VPN mutation. Existing
static buttons switch to Manual; starting source reading clears the static
choice cue. Missing/malformed/oversize/unreadable data gets whole default, not
mixed retained colors.

Both consumers use separate monotonic producer and UI delivery clocks. Polling
is one sequential request followed by a 500-ms delay, never an interval queue.
An opaque per-read ticket plus generation refuses wrong requests, duplicates
from the same generation and late pre-Manual results. Manual invalidates delivery
but never releases an active read reservation: the worker drains, then polling
stops (or an explicitly resumed generation continues). The native Permit is
owned inside background work until the synchronous read returns. View release
uses actual weak-entity/SDK task ownership; it is not kernel-read interruption
or a hard I/O deadline. Failed extension replies stop in Manual/whole-default
without silent retries. No scene/metadata/cache/search/highlight/selection/
connected/focus/scroll owner is recreated.

Small controls execute the actual Shell loop and view consumer with scheduling/
toolkit color effects substituted; they cover Manual during read, resume without
overlap, stale delivery, separate clocks and bad-reply/no-retry fallback. The
standalone Rust driver covers the corresponding ticket policy and actual fixed
public preflight. Source/kernel FIFO control is separate. Actual client SDK
typecheck/build/registration, GUI teardown and EN/RU wide/narrow reachability
remain UNRUN and require immutable-delta re-review.

## Dependency preflight and build plan

The initial `cargo generate-lockfile --offline` preflight refused: shared Cargo
had no checkout for the exact public SDK b4c7 Git dependency. No lockfile or
compile was produced. The separate b4c7 clone is clean and its lock retained;
quickjs965/605 and llrt7b95 source pins already exist in shared Cargo cache.
This is cache preparation, not a fundamental dependency blocker or permission
to upgrade. The next agreed recipe uses an isolated HOME-backed Cargo cache for
exact pinned public source fetch, reuses retained registry/lock data (no heavy
target copying) and compares every SDK/git revision and overlapping crate
version. No fetch has run; significant graph changes must be reviewed first.
Typecheck exact locked clients sequentially in separate owned G1 targets,
never overwrite frozen #743/#746/native-wire executables or use the main
agent maintenance target. Only after source/typecheck review may executable/
assets copies be frozen, hashed and proposed for a separately authorized
registered-host/native slot. No command in this plan was executed.

Only a separately authorized slot can establish file-source/poll/watch, whole
palette rendering, EN/RU wide/narrow filter/inspection/scroll continuity, native
memory/frame costs or platform behavior. No real user's Omarchy themes are read
or edited for this task.
