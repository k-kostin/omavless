# T2e: Omarchy palette integration

Development checkpoint, not a stable release or completion of T2. Builds on
[read-only inspection](T2_INSPECTION.md); runtime ownership is unchanged.

The terminal client reads the installed Omarchy palette from
`$HOME/.local/state/omarchy/current/theme/colors.toml`. A separate capacity-one
worker samples every two seconds, re-resolving the replaceable theme-directory
link. It never writes desktop configuration or blocks keyboard/IPC handling.

Only four fixed RGB keys are consumed: foreground, background, accent and
selection. Input is bounded to 16 KiB, valid UTF-8 and quoted six-digit RGB;
missing, duplicate or malformed required keys select the complete fallback
palette. A final-file symlink, special file or FIFO is refused with nonblocking
open. There is no shell/TOML evaluation, arbitrary theme path or private error
output. The existing locked nix dependency supplies open flags.

Normal cells and selected rows use the palette. Accent is reserved for later
presentation refinement. Missing/unsupported themes use white on black with a
distinct selection; terminal color mappings and NO_COLOR remain respected.
Reload changes no selected profile, search, scroll, desired state or runtime
revision. Terminal closure still leaves the runtime untouched.

## Validation

- Four deterministic theme tests: dark/light/fallback, malformed bounds,
  atomic theme-link replacement, unsafe file targets and presentation-only
  reload invariants.
- Local full Rust validation: 1,027 passed, 11 explicitly ignored; 10 PTY
  checks, formatting, Clippy, feature build/check and parity passed.
- Four actual Foot captures inspected: English/Russian, light/dark palettes,
  visible selected row and fixed footer. Synthetic data only, outside Git.
  Capture subprocesses remove inherited NO_COLOR; product behavior does not.
- The test-only theme preview never accesses the daemon or private store.
- Combined live runtime inspection is recorded separately; this checkpoint
  does not claim remote subscription refresh, probes or Open app packaging.

No installed package replacement, OS theme change, main update or publication
is part of this change.
