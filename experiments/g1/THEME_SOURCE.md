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
  script fs remains excluded. Source `--watch` reconstructs the script object
  and loses local state; it is not theme continuity.
- The same host publicly supports `HostModule.async_function`, plain data
  exports, `export_module`, and `host::main_with_components`. Its cap-std
  directory grant is private, not an available embedding-reader API. The staged
  embedding source uses real public APIs, not a fork or new permission framework.

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

`theme_fixture.rs` is an unregistered Linux-only source-stage reader of fixed
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
loop escape refusal, non-regular Unix-socket refusal, latest slots and unchanged
metadata/browsing owners. FIFO behavior follows the nonblocking open + regular
file check, but an actual FIFO/native gate remains unrun.

## Staged embedding source — NOT built or activated

`shell-theme-host/` is a reviewable sketch with the existing pinned SDK as its
only framework dependency. No lockfile/dependency resolution or GPUI consumer
build ran. Its future all-targets gate must also declare the existing
`serde_json = "1"` test-only dependency used by the shared fixture tests; that
manifest change has not been resolved or approved as part of this sketch.
It requires a fresh owned synthetic process HOME `g1h-*` under
`.cache/ovt`, fixed public `.cache/ovt/g1-theme` root and copied `app` directory
with the exact existing denied-grants manifest. It refuses preexisting
`.config`/`.local` trees before initialization. It registers only no-argument
`samplePalette()`, max-one inflight, work off UI; replies have only validated
mode/colors/status/sequence. No script fs/process/network/storage/clipboard or
backend/core IPC is introduced. No raw path/error/palette text is returned.

Before native use: primary/independent permission/reader-delta review; exact
dependency/lock/consumer build; full fixture preflight; dedicated Shell consumer
binding (not an import breaking the bare host); Direct off-UI binding using the
same reader; explicit source-follow vs static-button policy/sequence ownership;
max-one polling/cancellation checks. No guessed public SDK API is supplied.

Only a separately authorized slot can establish file-source/poll/watch, whole
palette rendering, EN/RU wide/narrow filter/inspection/scroll continuity, native
memory/frame costs or platform behavior. No real user's Omarchy themes are read
or edited for this task.
