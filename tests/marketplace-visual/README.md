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

The preparation command prints a fresh `/tmp/omavless-marketplace.XXXXXX`
directory, records its source commit/tree and stages only committed plugin
blobs. `backend.sh` and `plugin/setup-runtime.sh` are replaced with the included
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

The same prefix supports `inspect`, `scene subscription`, `scene settings`,
`capture PUBLIC-SLUG`, `result`, and `finish`. Wait for the scene to settle,
`inspect` to report disconnected and five demo profiles, and `result` to report
`captured`. Review each saved PNG yourself before copying to shareable assets.
`capture` grabs the real popup card, including its border, at native pixels.
It refuses unknown/connected states. No screenshots claim successful VPN access.

Use `finish` to close only the isolated instance, then check that the real
plugin/runtime state is unchanged. Keep raw scratch captures outside Git; only
reviewed credential-free selected assets belong under `docs/marketing/images/`.
Do not run Install or real VPN transitions to create marketing imagery.
