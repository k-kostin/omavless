// SPDX-License-Identifier: MIT
// Synthetic full-list replacements; this is not the product's 256-row capacity.
import { visibleProfiles } from "./data.js";

export const MAX_ROWS = 10006;
// Local immutable-data provenance only. No caller marker or wire field can
// skip validation; changed/cloned rows and duplicate IDs still pass admission.
const validatedRows = new WeakSet();
const validatedProfiles = new WeakSet();
const validatedMetadata = new WeakSet();
export const replacementFixture = {
  renamedPrefix: "Renamed · ",
  replacementProfiles: [
    { id: "replacement-a", name: "Replacement A · Example", country: "Example", host: "replacement-a.example", subscription: "Sample collection" },
    { id: "replacement-b", name: "Замена B · демонстрационный профиль", country: "Example", host: "replacement-b.example", subscription: "Sample collection" },
  ],
};

export function validId(id) {
  return typeof id === "string" && /^[a-zA-Z0-9_-]{1,64}$/.test(id);
}

function utf8Bytes(text) {
  try { return encodeURIComponent(text).replace(/%[0-9A-F]{2}/g, "x").length; }
  catch { return Infinity; } // Malformed surrogate, with no Web API dependency.
}

export class Metadata {
  #index;
  constructor(rows = [], content = 0) {
    if (!Array.isArray(rows) || !Number.isSafeInteger(content) || content < 0
      || (!validatedRows.has(rows) && (rows.length !== 0 || content !== 0))) {
      throw new Error("Synthetic metadata requires validation");
    }
    this.rows = Object.freeze(rows);
    this.content = content;
    this.#index = new Map(rows.map((row) => [row.id, row]));
    validatedRows.add(this.rows);
    validatedMetadata.add(this);
    Object.freeze(this);
  }

  static validate(rows, previous) {
    if (!validatedMetadata.has(previous)) return null;
    if (!Array.isArray(rows) || rows.length > MAX_ROWS) return null;
    if (rows === previous.rows && validatedRows.has(rows)) return previous;
    const ids = new Set();
    for (const row of rows) {
      if (row === null || typeof row !== "object" || Object.keys(row).length !== 5
        || !validId(row.id) || ids.has(row.id)) return null;
      if (!validatedProfiles.has(row) && ([row.name, row.country, row.host, row.subscription].some((text) =>
          typeof text !== "string" || [...text].length > 160
          || utf8Bytes(text) > 512
          || /[\u0000-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069\u200e\u200f\u061c]/.test(text))
        || row.name.trim() === "" || !row.host.endsWith(".example")
        || !["", "Sample collection", "Generated samples"].includes(row.subscription))) return null;
      ids.add(row.id);
    }
    const fields = ["id", "name", "country", "host", "subscription"];
    if (rows.length === previous.rows.length && rows.every((row, index) =>
      fields.every((field) => row[field] === previous.rows[index][field]))) return previous;
    if (!Number.isSafeInteger(previous.content + 1)) return null;
    const immutable = Object.freeze(rows.map((row) => {
      if (validatedProfiles.has(row)) return row;
      const profile = Object.freeze({ ...row });
      validatedProfiles.add(profile);
      return profile;
    }));
    validatedRows.add(immutable);
    return new Metadata(immutable, previous.content + 1);
  }

  static fixture() { return Metadata.validate(visibleProfiles("", true), new Metadata()); }
  profile(id) { return this.#index.get(id) ?? null; }
  selection(id) { return this.#index.has(id) ? id : null; }
  renamed(id) {
    if (!this.#index.has(id)) return null;
    return this.rows.map((row) => row.id === id && !row.name.startsWith(replacementFixture.renamedPrefix)
      ? { ...row, name: replacementFixture.renamedPrefix + row.name } : row);
  }
  removed(id) { return this.#index.has(id) ? this.rows.filter((row) => row.id !== id) : null; }
  replacedCollection() {
    return [...this.rows.filter((row) => row.subscription !== "Sample collection"),
      ...replacementFixture.replacementProfiles];
  }
}
