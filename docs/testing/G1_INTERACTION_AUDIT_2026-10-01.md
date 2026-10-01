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
