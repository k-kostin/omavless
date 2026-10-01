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
