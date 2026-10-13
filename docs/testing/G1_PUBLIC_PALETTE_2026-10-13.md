# G1 public palette continuity: native result

Status: **Direct scoped visual checks passed; embedding large-list sequence
failed; remaining embedding checks NOT RUN.** This is an agent-operated synthetic
development-VM result, not product GUI adoption or G1a/G1b completion.

## Exact selection

- Source: `46905bf55e60dab82e757b1498c5dfd9ea7b084b` (#754).
- Pinned host SDK: `b4c7cbdbbb57952c692c47ed13bbde9a06cdb7c5`.
- Direct lock SHA256: `8ea23f0c669d331a29ed2bf25aedf7d1e9ca8ff4104896aa76261f899b608c22`.
- Embedding lock SHA256: `cd076d3d14c986a05b3c3d48df0703725432d33c3115791ee5ac9f1877a836c1`.
- Direct ELF SHA256: `b2947ed36ac0dfac754a3d792ce3c69b981480205e80df8851fa37481804d208`.
- Embedding ELF SHA256: `1c8c82c0b682bc072f8ca6c9f318a240f6d37a1bdfcf100d8899f0f13ffef9e5`.
- Public app-tree SHA256: `d3cede8e0ec80f0364cc4fa86c4af0afd892ab3b62ce2e6c64ea159b51c429df`.
- Selected manifest SHA256: `9cfeda04e31c297515efa734de0de5a43ea257f061a036cb6cbe81b62684a8c1`.

Both locked/offline all-target checks and sequential linked builds exited 0.
These are **dev** artifacts, not release-size or performance samples. Each ELF
was copied outside its compiler target and independently hashed before native
use; no active executable or resource directory was overwritten.

Environment: x86_64 Omarchy development VM, Wayland, Hyprland 0.56.2 at
`efb50993780079460b0cbed1363e2166a2de1d9f`; 1890x2080 display, scale 1.6666666.
Owned GUI viewports were 1070x1030 and 400x700 logical pixels. Resizing a floating
window recentered it; geometry was read back and repositioned before using the
corresponding cropped evidence. A misplaced initial crop is retained privately,
not interpreted as application clipping.

## Isolation and observation

Each candidate had a separate fresh owned synthetic HOME, copied public assets,
the fixed research manifest, and four initially empty SDK XDG directories. The
SDK's scoped store/generated typings were permitted only in that fixture. No
real profiles, provider URLs, daemon, broker, services, networking, TUN or DNS
operations were used. Palettes and list rows were synthetic; connection labels
explicitly said synthetic/maket and exposed no VPN actions.

The selected VM had one operator under an explicit custody loan. Rendered guest
captures, not an input-sent marker, supplied visual evidence. An existing
non-G1 desktop notification was present before either launch and excluded from
window crops; it is not a result of this test. QMP screendump's `no surface`
did not mean a blank guest: guest-side capture showed the actual unlocked frame.
The installed idle command showed an existing stay-awake marker; its tooltip
offered the opposite action. No idle preference was changed.

## Direct result

The wide EN sequence exercised static dark/light, malformed all-default
fallback, explicit public sampling, relative theme-link replacement,
replacement of the current directory, missing/malformed/oversized input,
recovery to light, manual pause while files changed, and explicit resume.
An escaping relative link also produced all-default fallback. This is sampled
public-fixture continuity, **not a real theme watcher or hostile-writer proof**.

Across those inspected frames, the whole palette changed consistently: header,
text, borders, rows and Details. No mixed old-light islands were observed in
default fallback. Query `example`, generated-list mode with 10,006 research
rows, inspected North and confirmed South remained distinct and retained.
Keyboard navigation scrolled to generated rows 00006–00012. After palette
changes and the wide RU toggle, reacquiring list focus and one Down advanced
the retained highlight to 00013 rather than resetting to the selected North.
Pointer activation of palette controls may naturally change focused styling;
the earlier screenshots alone were not treated as proof of hidden focus state.

Wide RU controls fit. At 400x700, both EN/RU controls wrapped without horizontal
clipping; Page Down/Up reached Profiles and Details and returned to the header.
Live light-to-dark replacement while the RU Details view was scrolled preserved
the visible position and North/South distinction. Narrow EN missing-input
fallback, recovery while at Details, and narrow RU malformed fallback retained
the same query/list and remained navigable. These are inspected selected
transitions, not every operation in every locale or full platform accessibility.

The original Direct process closed through the compositor's graceful window
close, exited 0, and its process/window absence and unchanged ELF were checked.

## Embedding failure — do not backcredit previous acceptance

The same-source embedding host started and rendered its ordinary wide screen.
The next sequence was: enable generated rows, focus search, type `example`,
Tab to the list, and send 18 rapid Down events. The next observed frame showed
the host error banner **“interrupted”** and retained the last successful view.
There were no intermediate frames, so the first failing event is **UNKNOWN**:
the result is not assigned solely to typing, arrows, the large-list toggle or
palette polling. Public palette sampling had not yet been activated.

The failed frame, original log, exact assets and synthetic HOME are preserved
outside Git. The GUI process subsequently closed gracefully and exited 0;
**that exit does not turn the failed render sequence into PASS**. Its process
and window absence and unchanged ELF were independently checked. Further
embedding palette/RU/narrow rows were not run on that original.

Source diagnosis found changed queries filtering 10,006 rows and repeatedly
normalizing text from inside render. The pinned SDK enforces separate 50ms
render/layout and 500ms event/task budgets. A separate proposed successor moves
preparation into owning events and preserves immutable per-render projections;
it must not widen SDK budgets, drop rows/keys, change source authority or claim
native timing from operation-count tests. Source analysis identifies a real
cost-placement problem but does not retroactively identify the first failing
event or accept that successor. A new finite native replay is required.

After both originals closed, the VM's boot/display, existing stay-awake state
and public interface inventory matched the inherited baseline. Custody was
explicitly returned. Main, packages, release artifacts and the Marketplace were
unchanged. Native failures and missing rows remain visible in G1 research.
