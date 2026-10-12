// SPDX-License-Identifier: MIT
// Only plain research-navigation keys. Native widgets keep their own chords.
export function plainKey(event) {
  if (event.modifiers && Object.values(event.modifiers).some(Boolean)) return null;
  return event.keystroke === event.key ? event.key : null;
}

// Called in GPUI's bubble phase at the application frame, after child widgets.
// The root includes the header; a body-only listener misses header focus.
export function navigatePage(event, view, context) {
  const key = plainKey(event);
  if (key === "pagedown" && !view.detailsOnly) view.detailsOnly = true;
  else if (key === "pageup" && view.detailsOnly) view.detailsOnly = false;
  else return false;
  view.panelFocus.focus();
  context.notify();
  context.stop_propagation();
  return true;
}
