# G1a synthetic interaction audit — 2026-10-01

This follows the [first VM trial](G1_SYNTHETIC_TRIAL_2026-10-01.md) on
`dev/g1-synthetic-interaction-audit`. It is exploratory UI evidence, not a
production GUI decision, installed OmaVLESS test or G1b daemon-binding gate.
Both trial processes still use invented `.example` profiles only. The installed
VPN, private data, routes and service were not accessed or changed.

The inspection action selects a synthetic row; scene buttons only select a
synthetic status. Neither action connects. The selected row remains an
independent fact from the connection. During the *switching* scene, the old
server may be shown as **previous, not verified now**, but never as a current
confirmed connection. Only the *connected* scene may mark a row Connected.
The *unverified* scene uses a warning/unknown treatment rather than the
failed/recovery danger treatment. These decisions apply to the direct Rust
and Shell trials, in English and Russian. They do not change product QML.

On the x86_64 Omarchy Dev VM, the Shell trial was rendered at 1110×1198
(1⅔ display scale). Before this audit the panel content touched its border;
12-pixel content padding gives search, list rows and details a visible inset.
The Shell view now owns an initial focus handle. Keyboard-only `Tab`, `Return`
walked from the initial view through *connected*, *connecting* and *switching*
scene controls. The switching capture showed a previous-server label and no
Connected row badge. The direct trial likewise rendered the switching scene
in English and Russian, and an initial focused search field. Captures remain
outside Git because they are transient acceptance artifacts.

The fixture parity check, Shell JavaScript syntax checks, direct Rust format
check and three direct Rust unit tests passed. `gpui-shell check` retains the
upstream hidden-view virtual-list panic described in the first VM trial; an
actual Shell render is evidence for this narrow interaction path, not a
replacement for the host check.

Still open: full keyboard and screen-reader matrix, IME, wheel-scroll
continuity, large-list update latency, live theme changes, short-window
layout, ARM64 and packaged install/remove. No G1a adoption decision or
daemon client is justified by this follow-up.

## Shell-only synthetic palette follow-up

At candidate `d28b4e0`, the Shell trial gained a complete-palette guard and
dark/light/malformed synthetic choices. On the same x86_64 Omarchy Dev VM,
the pinned Shell host rendered the initial dark window, then light and a
malformed-palette fallback to the complete dark default through keyboard
activation. The English and Russian labels were inspected. The selected
*North* row and separate confirmed *South* identity stayed distinct across
the theme swaps; malformed input did not leave a mixed light/dark window. A
follow-up render of the final candidate confirmed that the fallback note takes
no empty layout row in ordinary dark mode. These invented `.example` rows
contain no real profile metadata.

The Node fixture/palette assertions and Shell syntax checks passed. This is
an event-driven in-memory trial only: live Omarchy theme-file replacement,
broken/missing filesystem theme, direct Rust parity, narrow layout,
accessibility and packaging remain **NOT RUN**. The upstream hidden-check
panic still applies. No product VPN, installed plugin or host theme was
changed; temporary screenshots remain outside Git.

### Short-window correction on the same trial

At `b450b02`, the VM showed an actual layout defect at 400×700 logical pixels:
the Shell profile panel kept its 30rem minimum and was clipped to the right.
Putting flex sizing on a wrapper and allowing the panel/list internals to
shrink kept the collection controls, search, list rows and long safe labels
within the window. A 1110-pixel-wide recapture retained the two-column
layout; the 400-pixel EN and RU captures showed the single-column profile
panel without horizontal clipping. The selected/connected distinction
remained visible. Full keyboard/wheel reachability of the lower Details panel
was **NOT RUN** and remains a G1a gate; these captures do not establish it.

### Narrow scroll reachability follow-up

The subsequent Shell-only layout slice removes the viewport-height flex cap
from the wrapped Profiles/Details row. In the same isolated x86_64 VM, an
actual 400×700 logical-pixel window was scrolled with a virtual mouse wheel
to the bottom: the entire Details panel, including the read-only notice, was
visible. The selected *North* and confirmed *South* labels remained distinct
after scrolling. Returning to the top and resizing to 1110×1198 logical
pixels retained the two-column layout. These observations are from the
synthetic Shell trial only; screenshots remain outside Git. The temporary
mouse-input test daemon was stopped after the run. The guest received
`ydotool` as a development-only test utility; no OmaVLESS package, service,
private store or VPN state was changed.

This closes the specific lower-panel wheel reachability gap from the preceding
400-pixel capture. It does not establish keyboard-only scroll access,
screen-reader behavior, direct-Rust parity, live theme watching, large-list
performance or G1b daemon attachment.

### Shell keyboard list and empty-state follow-up

The next Shell-only slice adds a focused, keyboard-navigable *synthetic*
profile list. The action target is the highlighted row for inspection, never
the VPN connection. `Tab` enters the list; `Up`/`Down` move the outline and
scroll it into view without changing the selected profile or the separately
confirmed connection. `Enter` selects only the highlighted profile for
inspection; `Escape` returns focus to the panel. When a search has no matches,
the list displays an English/Russian no-results message; `Enter` in either
the empty list or the empty search retains the prior inspection selection.

On the x86_64 Omarchy Dev VM at 1110×1198 logical pixels, the pinned Shell
host rendered the English list and search-empty state. Keyboard navigation
reached a generated row in the 10,006-row fixture and scrolled it into view.
The initial trial exposed a real defect: the row became selected, but its
details said "No valid selection" because only the six static fixture rows
were resolved. The corrected candidate resolves generated IDs without a
10k-row rebuild; a second VM render showed the generated profile name and
source in Details, while the confirmed *South* remained unchanged. The
Russian empty-state copy and the no-op `Enter` on zero matches were rendered
in the VM. Node fixture assertions, Shell syntax checks and diff checks pass.
Captures stayed outside Git. The transient trial was stopped and its copied
application directory removed afterward; no product service, route or private
store was touched.

This establishes only the exercised Shell keyboard path. Native accessibility,
screen readers, IME, narrow-window keyboard reachability, direct Rust parity,
measured large-list latency/memory, live theme watching and platform/package
gates remain open. G1a is not complete and G1b daemon binding is not started.

### Direct Rust keyboard parity follow-up

The separate direct Rust/GPUI trial now implements the same synthetic list
contract: focusable virtualized list, bounded Up/Down highlight with
scroll-to-item, Enter for inspection only, Escape back to search, and a
localized no-match view. Search Enter with no match preserves the inspected
profile. Generated fixture rows resolve for Details without rebuilding the
full list during inspection. None of these handlers attaches to the daemon or
performs a VPN action.

On the x86_64 Omarchy Dev VM at 1110×1198 logical pixels, keyboard input
reached the direct Rust list. Down and Enter inspected another static row
while the separately confirmed *South* stayed unchanged. The 10,006-row
fixture scrolled via repeated Down; Enter inspected a generated row with its
correct name/source. English and Russian zero-match states rendered, and
Enter in the empty search/list left the prior inspection intact. The trial
used a transient user unit and invented `.example` fixtures only; screenshots
remain outside Git. Five direct Rust unit tests, including generated-ID and
navigation bounds, passed. This closes the exercised keyboard parity slice,
not screen-reader, IME, short-window reachability, performance, package or
G1b daemon-binding gates.

### Direct Rust synthetic palette parity follow-up — 2026-10-02

The direct Rust trial now exposes the same dark, light and intentionally
malformed palette inputs as the Shell trial. Its unit test compares the exact
synthetic source strings and verifies that malformed input selects the whole
standalone Tokyo Night default, not a mixture of palettes. The controls change
only the local trial presentation; the selected-for-inspection and synthetic
confirmed-connection identities remain separate.

A release-mode binary was copied temporarily to the x86_64 Omarchy Dev VM.
At 1110×1198 logical pixels, the dark, light and malformed-fallback buttons
were clicked and each resulting screen inspected. North remained selected for
inspection and South remained synthetically confirmed in all three palettes.
The window was then floated and resized to 400×700. The malformed fallback and
light palettes rendered with Russian labels; palette controls wrapped without
overlap, and wheel scrolling reached the entire Details panel. The user VPN,
daemon, private store, system theme and installed plugin were not modified.
Captures and the transient binary stay outside Git.

This closes only synthetic in-process palette parity and the exercised narrow
scroll path. Live theme-file watching/replacement, native accessibility, IME,
full keyboard access to all controls, latency budgets, cross-platform behavior,
optional package lifecycle and G1b daemon binding remain unverified.

### Direct Rust narrow keyboard reachability follow-up — 2026-10-02

User goal: reach the lower read-only Details panel without a mouse in a short
window. `Page Down`/`Page Up` now move only the outer narrow-window scroll
area; profile inspection still requires explicit `Enter`, and the confirmed
synthetic connection remains independent. The scroll region itself is a Tab
stop with a focus border. Wide layout remains a two-column panel without
postresize scroll-key interception.

The corrected source is `8f754a138cae56cb9e15f4c18bb56cc0098ec974`;
its release binary was `f7935a87f548253c9f0933812930cfa9f683d5b7dad038bf164bb486b04b96b1`.
Both the baseline and corrected direct-Rust synthetic binaries were built in
release mode; each SHA-256 matched after temporary transfer to the x86_64
Omarchy Dev VM. At 400×700 logical pixels, the baseline left Details below
the viewport after `Page Down`; the corrected candidate reached the entire
Details panel after two keypresses, and `Page Up` returned toward Profiles.
From the keyboard-focused list, Down/Down/Enter inspected the long invented
East row; Details continued to show a separately confirmed South. At
1100×1198 logical pixels, the same binary retained the two-column layout and
independent identities. Eight direct Rust unit tests and its release build
passed. Captures remain private and outside Git. This is native rendered GUI
evidence for one direct-Rust interaction, not Shell parity, accessibility,
daemon binding, package acceptance or VPN health.

### Native accessibility comparison follow-up — 2026-10-02

The G1a comparison found a concrete difference between the synthetic hosts.
With the VM accessibility bus enabled, the pinned Shell trial registered an
application but exposed unnamed buttons and search, with no useful status,
profile or Details semantics. In its 400×700 logical-pixel window, `Page Down`
also did not reach the offscreen Details panel, including when the scene
control had focus. Shell mouse-wheel reachability remains as documented above;
this is not a general rejection of GPUI Shell.

The direct Rust trial now gives the status, search, list, visible rows and
Details explicit native accessibility roles and names. Row names distinguish
*selected for inspection* from *confirmed connection*, in English and Russian.
An accessibility action on an East row changed the inspected Details while
the separately confirmed South remained unchanged. In the switching scene,
South became *previous, not verified now* rather than Connected. The failed
scene exposed an alert instead of presenting a routine transition as an error.
The search remains only a local fixture filter; no native action can connect.

The tested direct release binary SHA-256 was
`03534d5bb2ebb5bf8a2e5caefa846e5579ffea4fa6e30be60dcaf44b240c158a`.
At 400×700 logical pixels the amended frame retained the single-column
layout. Expanding to 10,006 synthetic rows left only eight visible options
in the native accessibility tree; process PSS rose from 51,962 to 55,343 KiB
in this one VM run. This is a bounded observation, **not** a latency or memory
budget pass. The trial used invented `.example` rows, temporary binaries and
a transient user unit; no OmaVLESS runtime, profiles, routes or VPN were touched.
Screenshots and accessibility probes remain outside Git. The VM accessibility
bus was restored to its prior disabled state after the probe.

Still **NOT RUN**: real screen-reader navigation/announcements, IME
composition, live Omarchy theme-file changes, measured interaction latency,
ARM64, optional package lifecycle and any read-only daemon binding. G1a is
still an experiment; G1b has not started.

### Shell narrow keyboard Details fallback — 2026-10-02

The comparison above found that a keyboard-only visitor could not reach the
offscreen Details panel in the pinned Shell host's 400×700 logical-pixel
window. This host exposes a handle for virtual-list scrolling but not for
the ordinary outer scroll area through its script API. The Shell trial now
provides an explicit read-only Details page, opened with the header button
or `Page Down` and closed with the header button or `Page Up`. It is a page
transition, not a claim that the outer scroll problem was fixed. The
original combined page retains mouse-wheel access to its lower Details.

The x86_64 Omarchy Dev VM rendered both states at 400×700. `Page Down`
displayed the complete details card; `Page Up` returned to Profiles. North
remained selected for inspection while South stayed the separate synthetic
confirmed connection on the Details page and after returning. The trial
used only invented `.example` fixture rows; screenshots remain outside Git.
This closes the narrow *keyboard reachability* gap via an alternative view,
not full keyboard, accessibility, screen-reader, IME, latency or package
acceptance. GPUI Shell's native accessibility tree remains incomplete and
G1a is not accepted for production.
