// SPDX-License-Identifier: MIT
// Dedicated source-enabled trial only. Bare Shell client imports no extension.
import { ThemeFollow, POLL_MS } from "./theme-follow.js";
import { checkedTheme, DEFAULT_PALETTE, MAX_THEME_SEQUENCE } from "./theme-source.js";

export function checkedSample(reply) {
  if (!reply || typeof reply !== "object" || Object.keys(reply).length !== 3
    || !["sequence", "status", "palette"].every((key) => Object.hasOwn(reply, key))
    || !Number.isSafeInteger(reply.sequence) || reply.sequence < 1 || reply.sequence > MAX_THEME_SEQUENCE) return null;
  const prepared = checkedTheme({ status: reply.status, palette: reply.palette });
  return prepared ? Object.freeze({ sequence: reply.sequence, prepared }) : null;
}

export function installThemeSource(view, samplePalette) {
  view.themeFollow = new ThemeFollow();
  view.themeSampler = samplePalette;
  view.themePollRunning = false;
}

export function manualTheme(view) {
  view.themeFollow?.mode(false);
  // Do not cancel/drop a worker or free inflight here. Drain the outstanding
  // sample; its old-generation result is refused. The owning SDK cancels
  // continuation on view release, not the kernel read's Permit.
}

export function followTheme(view, cx) {
  if (!view.themeFollow || !view.themeFollow.mode(true)) return;
  view.themeChoice = "source";
  cx.notify();
  if (view.themePollRunning) return;
  view.themePollRunning = true;
  view.themeTask = cx.spawn(async (context) => {
    try {
      while (view.themeFollow.following) {
        const generation = view.themeFollow.begin();
        if (generation === null) break;
        let sample;
        try { sample = checkedSample(await view.themeSampler()); }
        catch { sample = null; } // Fixed fallback only; never expose raw errors.
        if (sample === null) {
          if (view.themeFollow.failed(generation)) {
            view.applyThemeSample(view.themeUpdates.sequence + 1,
              { status: "unavailable", palette: DEFAULT_PALETTE }, context);
          }
        } else if (view.themeFollow.complete(generation, sample.sequence)) {
          // Producer and UI clocks are separate: static/startup updates cannot
          // collide with source sequence or admit a cancelled prior generation.
          view.applyThemeSample(view.themeUpdates.sequence + 1, sample.prepared, context);
        }
        if (!view.themeFollow.following) break;
        await context.sleep(POLL_MS); // After completion, never an interval queue.
      }
    } finally { view.themePollRunning = false; }
  }, { owner: view });
}
