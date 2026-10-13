// SPDX-License-Identifier: MIT
// Bounded synthetic color data only. NO filesystem, watcher or host grants.
export const MAX_THEME_BYTES = 4096;
export const MAX_THEME_SEQUENCE = Number.MAX_SAFE_INTEGER;
export const REQUIRED = Object.freeze(["background", "foreground", "accent", "red", "green", "yellow", "blue", "magenta", "cyan"]);
export const OPTIONAL = Object.freeze(["selection", "lighter_background", "dark_background", "light_foreground", "bright_foreground", "dark_foreground"]);
export const DEFAULT_PALETTE = Object.freeze({ mode: "dark", background: "#1a1b26", foreground: "#c0caf5", accent: "#7aa2f7", red: "#f7768e", green: "#9ece6a", yellow: "#e0af68", blue: "#7aa2f7", magenta: "#bb9af7", cyan: "#7dcfff" });
const fallback = (status) => Object.freeze({ status, palette: DEFAULT_PALETTE });
const statuses = ["good", "missing", "malformed", "oversize", "unavailable"];

function utf8Size(source) {
  let bytes = 0;
  for (const char of source) {
    const code = char.codePointAt(0);
    if (code >= 0xd800 && code <= 0xdfff) return null;
    bytes += code <= 0x7f ? 1 : code <= 0x7ff ? 2 : code <= 0xffff ? 3 : 4;
    if (bytes > MAX_THEME_BYTES) return bytes;
  }
  return bytes;
}

function checkedPalette(palette) {
  if (!palette || typeof palette !== "object" || Array.isArray(palette)) return null;
  const keys = Object.keys(palette);
  if (keys.some((key) => !["mode", ...REQUIRED, ...OPTIONAL].includes(key))) return null;
  if (!Object.hasOwn(palette, "mode") || !["dark", "light"].includes(palette.mode)) return null;
  const result = { mode: palette.mode };
  for (const key of [...REQUIRED, ...OPTIONAL]) {
    if (!Object.hasOwn(palette, key)) {
      if (REQUIRED.includes(key)) return null;
      continue;
    }
    if (typeof palette[key] !== "string" || !/^#[0-9a-fA-F]{6}$/.test(palette[key])) return null;
    result[key] = palette[key].toLowerCase();
  }
  return Object.freeze(result);
}

// A deliberately strict research fixture grammar, not a general TOML loader.
export function prepareTheme(source) {
  if (source === null) return fallback("missing");
  if (typeof source !== "string") return fallback("malformed");
  if (source.length > MAX_THEME_BYTES) return fallback("oversize");
  const bytes = utf8Size(source);
  if (bytes === null) return fallback("malformed");
  if (bytes > MAX_THEME_BYTES) return fallback("oversize");
  if (!/^[\x09\x0a\x0d\x20-\x7e]*$/.test(source)) return fallback("malformed");
  const values = {};
  for (const line of source.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const match = /^([a-z_]+)\s*=\s*"([^"]*)"$/.exec(trimmed);
    if (!match || Object.hasOwn(values, match[1])) return fallback("malformed");
    values[match[1]] = match[2];
  }
  const palette = checkedPalette(values);
  return palette ? Object.freeze({ status: "good", palette }) : fallback("malformed");
}

export function paletteText(palette) {
  return ["mode", ...REQUIRED, ...OPTIONAL].filter((key) => Object.hasOwn(palette, key))
    .map((key) => `${key} = "${palette[key]}"`).join("\n") + "\n";
}

const samePalette = (a, b) => paletteText(a) === paletteText(b);

export class ThemeUpdates {
  constructor() {
    this.sequence = 0;
    this.pending = null;
    this.current = Object.freeze({ sequence: 0, ...fallback("unavailable") });
  }
  receive(sequence, prepared) {
    // Admission BEFORE coalescing. A late malformed record cannot displace good.
    if (!Number.isSafeInteger(sequence) || sequence <= this.sequence || sequence > MAX_THEME_SEQUENCE) return false;
    if (!prepared || typeof prepared !== "object" || Object.keys(prepared).length !== 2
      || !Object.hasOwn(prepared, "status") || !Object.hasOwn(prepared, "palette")
      || !statuses.includes(prepared.status)) return false;
    let palette = checkedPalette(prepared.palette);
    if (!palette || (prepared.status !== "good" && !samePalette(palette, DEFAULT_PALETTE))) return false;
    if (prepared.status !== "good") palette = DEFAULT_PALETTE;
    else if (samePalette(palette, this.current.palette)) palette = this.current.palette;
    this.sequence = sequence;
    this.pending = Object.freeze({ sequence, status: prepared.status, palette });
    return true;
  }
  flush() {
    if (this.pending === null) return null;
    const sample = this.pending;
    const paletteChanged = !samePalette(sample.palette, this.current.palette);
    const statusChanged = sample.status !== this.current.status;
    this.current = sample;
    this.pending = null;
    return { sample, paletteChanged, statusChanged };
  }
}
