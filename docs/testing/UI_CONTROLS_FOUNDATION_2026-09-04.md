# First shared controls pass

Base: main `2f9f6bcdbafdfa6b34da50160e7a739618ab814b`.
This implements the beginning of P-U1, not visual acceptance of the whole panel.

## Implemented

- `OmaNavigationButton` wraps the installed shell's `PanelActionButton`, without
  copying it. Back, subscription refresh, Settings and hero QR use a shared
  minimum 32 scaled-pixel hit area, icon size, border and accent hover treatment.
- Inline row actions retain their existing quieter role; text buttons and the
  mode selector are not redesigned in this first pass.
- All eight vertical scrollbars use `OmaScrollBar`: rectangular thumb, theme
  foreground/accent, constant 12 scaled-pixel hit lane, no platform rounding.
  The minimum thumb length scales with the theme rather than the document size.
- Startup (both nested lists) and onboarding now reserve the same 16 scaled-pixel
  gutter as the other scrolling pages. Content does not move when a bar appears.
- Existing modal policies, callbacks, enabled guards and keyboard routing remain.

The shell control contract was inspected at omacom/omarchy
`493067741e081c3b09082da6bfd51e99ec24ef00`, `shell/Ui/PanelActionButton.qml`
and `shell/Ui/Button.qml`. These differ in hover-border behavior; this change
uses the existing `bordered` property rather than accessing private internals.
The owner's installed shell revision must still be checked.

## Cloud checks and limits

270 Python tests: 267 passed, 3 expected opt-in/environment skips. QML/i18n
contracts passed. No Quickshell, actual Omarchy imports or rendered screenshot
is available in this environment. Static checks do not validate appearance.
No runtime/backend change, new translatable text, manifest bump or merge.
The independent search fix #158 is not included in this branch.

## First host review

1. Inspect light/dark and the usual console theme at normal and enlarged scale.
2. Compare Back, refresh, Settings and QR hit areas and idle/hover/focus states.
   Check disabled actions cannot execute and repeated navigation returns focus.
3. Check all scrolling pages, both Startup scroll regions and onboarding with
   short and long content: no overlap, no horizontal scroll, draggable thumb,
   legible final row, stable content width, mouse wheel and keyboard navigation.
4. Open a modal above the panel: underlying bars stay hidden and inactive.
5. Capture before/after screenshots on the same host. Assess density and whether
   32px navigation targets fit the hero. Tune this shared component before
   migrating import, text and row action roles. Do not declare P-U1 finished.
