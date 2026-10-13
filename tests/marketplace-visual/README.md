# Isolated marketplace screenshot harness

This renders **unchanged committed product QML**, with invented display metadata
and a disconnected-only read fixture. It is not a connected/live/provider test.
No real profile, URI, endpoint, subscription URL, key or QR exists in the fixture.
All unlisted backend commands fail silently, including mutations, clipboard
imports, credential exports and runtime cleanup. Presence facts for the normal
ready screen are simulated, not package-installation evidence.

Run the deterministic fixture/parser/refusal test first:

```sh
node tests/test-marketplace-assets.js
bash tests/marketplace-visual/prepare.sh /absolute/reviewed/source
```

The preparation command prints a fresh `~/.cache/ovt/omavless-marketplace.XXXXXX`
directory under checked same-UID non-writable, non-symlink parents (the `ovt`
namespace must be mode0700); it never repairs an unsafe existing parent.
It records its source commit/tree and stages only committed plugin
blobs plus the adjacent public `manifest.json` used by the Settings credit.
Dirty plugin/manifest changes refuse staging. `backend.sh` and
`plugin/setup-runtime.sh` are replaced with the included
read fixtures; **no QML or presentation JS is modified**. Node is a developer
fixture generator, not a plugin dependency. Nothing under tests is shipped.

## Desktop launch

Read the local Omarchy capture/UI instructions. Use the current normal theme
and native screen scale; never change the real plugin's locale/store/runtime.
On this VM the user runtime is `/run/user/1000`, display `wayland-1`.
Replace `CAPTURE` below with the exact freshly created directory. Do not run the
fixture outside the isolation boundary just to avoid configuring its paths.

```sh
bwrap --die-with-parent --unshare-net --unshare-pid \
  --ro-bind / / --proc /proc --tmpfs /home/kdk --tmpfs /run/user/1000 \
  --bind CAPTURE CAPTURE --bind CAPTURE/runtime /run/user/1000 \
  --ro-bind /run/user/1000/wayland-1 /run/user/1000/wayland-1 \
  --ro-bind /home/kdk/.local/state/omarchy/current/theme /home/kdk/.local/state/omarchy/current/theme \
  --setenv OMAVLESS_CAPTURE_DIR CAPTURE --unsetenv DBUS_SESSION_BUS_ADDRESS \
  qs -p CAPTURE
```

The real home and user-runtime sockets are hidden; only the theme and display
are exposed. Network is unshared, system files read-only. No auth prompt, store
access or service operation is part of this procedure. Wayland access is for
rendering, not a claim that arbitrary untrusted GUI code would be sandbox-safe.

Call only this instance, through its own runtime directory:

```sh
bwrap --ro-bind / / --bind CAPTURE/runtime /run/user/1000 \
  qs ipc -p CAPTURE call marketplaceReview scene main
```

The same prefix supports `inspect`, `scene subscription`, `scene settings`, `scene setup`,
`capture PUBLIC-SLUG`, `result`, and `finish`. Wait for the scene to settle,
`inspect` to report disconnected and five demo profiles, and `result` to report
`captured`. Review each saved PNG yourself before copying to shareable assets.
`capture` grabs the real popup card, including its border, at native pixels.
It refuses unknown/connected states. No screenshots claim successful VPN access.

Use `finish` to close only the isolated instance, then check that the real
plugin/runtime state is unchanged. Keep raw scratch captures outside Git; only
reviewed credential-free selected assets belong under `docs/marketing/images/`.
Do not run Install or real VPN transitions to create marketing imagery.

`scene setup` projects missing-component facts through the unchanged production
Setup page. If it reports `not-ready`, wait for the existing presence probe to
finish before choosing the scene again. Check `inspect.bootstrap === true`
before capture. This is a setup illustration, not clean-install acceptance.

## Terminal application screenshot

`node tests/marketplace-visual/tui.mjs CAPTURE` binds a private Unix socket only
under that freshly staged directory. It serves four fixed read responses using
the same credential-free metadata; every effect-bearing command refuses with a
fixed error. Frames, concurrent clients and lifetime are bounded. No real store,
service, controller, provider, network or executable dispatch is present.

Launch the installed `omavless tui` in a terminal through the same home/runtime/
network isolation above, replacing `qs -p CAPTURE` with `/usr/bin/omavless tui`.
Keep the renderer's ordinary font/theme; resize only this screenshot window.
Capture its actual window after resize/redraw settles, with an empty desktop
behind transparent terminal surfaces. Inspect every saved frame: reject clipped
borders or background windows. Close the client with `q`, then stop the one
verified fixture process with SIGTERM; never signal the real daemon or Codex.
Neither advertised controls nor these pictures establish working mutations or
VPN health. The README caption identifies example profiles/setup states.

## Development UI state review

The same isolated instance supports `reviewState LOCALE KIND WIDTH`, where
locale is `en`/`ru`, width is 360/460 logical pixels and kind is `normal`,
`empty`, `unavailable`, `missing`, `subscription-empty` or `search`.
It stops only this synthetic product's polling timers and projects bounded
fixture states through unchanged production QML. `reviewInspect` exposes
only state/row counts, expansion and hint geometry/focus booleans.
`reviewFocus`, `reviewToggle` and `reviewClearSearch` operate this isolated
panel's presentation, never runtime mutations. `reviewCapture PUBLIC-SLUG`
also permits the explicit unavailable state; the ordinary marketing `capture`
still requires Disconnected. These captures are development evidence only.

`reviewSettings LOCALE WIDTH` opens Settings and scrolls its existing Quit row
and adjacent passive version/credit into view without activating that action.
`inspect` also reports this public manifest-derived credit, never runtime or
package identity. Use it to check the footer in both languages and widths.
After layout settles, `reviewSettingsEnd` repeats the production scroll-to-Quit
path without a click; real Shift+Tab navigation remains a separate review.

Inspect EN/RU empty/unavailable/missing-subscription guidance, focus and actual
pointer hover at both widths. During `search`, the matching subscription must
be expanded, toggling must preserve the stored false preference, and clearing
search must restore collapsed. Review the saved PNGs yourself. Real installed
runtime/package behavior remains a separate gate.

`reviewImport LOCALE WIDTH` opens the production direct-entry prompt without
reading the clipboard or supplying credentials. Inspect EN/RU at 360/460 pixels;
`reviewCancelImport` exercises its Cancel signal before capturing the adjacent
main screen. Actual typing/Paste/keyboard and private-field clearing are covered
by `tests/safe-input/`; these captures do not claim a saved import or live VPN.

`reviewBundle LOCALE WIDTH EMPTY` opens the production count-only local-bundle
confirmation with synthetic counts; `EMPTY=true` exercises the disabled Add
state. This remains rendering evidence, not a successful persistent import.

`reviewSubscriptions LOCALE KIND WIDTH` projects `loading`, `unavailable`,
`empty` or `populated` through the production Subscriptions page, at 360/460
pixels in EN/RU. Only the isolated read fixture's snapshot/failure flag changes;
polling stays stopped and every runtime mutation still refuses. Inspect the
loading/unavailable/confirmed-empty wording, the existing Refresh action and a
populated adjacent state. This is rendering evidence, not a daemon read or
subscription fetch; use `reviewCapture` and finish only this isolated instance.
