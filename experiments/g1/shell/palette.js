// @ts-check
// SPDX-License-Identifier: MIT
// Synthetic palette inputs only. This module does not read the user's theme.

export const DARK = `
mode = "dark"
background = "#1a1b26"
foreground = "#c0caf5"
accent = "#7aa2f7"
red = "#f7768e"
green = "#9ece6a"
yellow = "#e0af68"
blue = "#7aa2f7"
magenta = "#bb9af7"
cyan = "#7dcfff"
`;

export const LIGHT = `
mode = "light"
background = "#f4f2ee"
foreground = "#25252b"
accent = "#6948a5"
red = "#af3446"
green = "#267746"
yellow = "#8c5e16"
blue = "#4164a8"
magenta = "#784b98"
cyan = "#236e7c"
`;

export const BROKEN = `
mode = "light"
background = "#f4f2ee"
foreground = "not-a-color"
accent = "#6948a5"
`;

const REQUIRED_COLORS = ["background", "foreground", "accent", "red", "green", "yellow", "blue", "magenta", "cyan"];
const OPTIONAL_COLORS = ["selection", "lighter_background", "dark_background", "light_foreground", "bright_foreground", "dark_foreground"];
const HEX_COLOR = /^#[0-9a-fA-F]{6}(?:[0-9a-fA-F]{2})?$/;

/**
 * A theme change is all-or-default. No mixed palette may be installed after a
 * malformed replacement. Production theme file watching remains out of G1a.
 * @param {string} source
 * @returns {boolean}
 */
export function completePalette(source) {
  /** @type {Record<string, string>} */
  const values = {};
  for (const line of source.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const match = trimmed.match(/^([a-z_]+)\s*=\s*"([^"]*)"$/);
    if (!match || Object.hasOwn(values, match[1])) return false;
    values[match[1]] = match[2];
  }
  if (values.mode !== "dark" && values.mode !== "light") return false;
  return REQUIRED_COLORS.every((key) => HEX_COLOR.test(values[key] ?? ""))
    && OPTIONAL_COLORS.every((key) => values[key] === undefined || HEX_COLOR.test(values[key]));
}

/** @param {string} source */
export function resolvedPalette(source) {
  return completePalette(source) ? { source, fallback: false } : { source: DARK, fallback: true };
}
