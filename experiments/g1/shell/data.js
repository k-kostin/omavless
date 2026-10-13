// SPDX-License-Identifier: MIT
// Intentionally mirrors ../fixtures.json. tests/fixture-parity.mjs enforces parity.
export const fixture = {
  schema: 1,
  profiles: [
    { id: "north", name: "North · Example", country: "Finland", host: "north.example", subscription: "Sample collection" },
    { id: "south", name: "South · Example", country: "Latvia", host: "south.example", subscription: "Sample collection" },
    { id: "east", name: "Восточный демонстрационный сервер с длинным названием", country: "Japan", host: "east.example", subscription: "Sample collection" },
    { id: "west", name: "West · Example", country: "France", host: "west.example", subscription: "Sample collection" },
    { id: "long-en", name: "A deliberately long English demonstration server name for layout testing", country: "Canada", host: "long.example", subscription: "Sample collection" },
    { id: "local", name: "Local test profile", country: "Local", host: "local.example", subscription: "" },
  ],
  collections: [
    { id: "all", name: "All" },
    { id: "sample", name: "Sample collection" },
    { id: "generated", name: "Generated samples" },
    { id: "local", name: "Local" },
  ],
  scenes: [
    { id: "connected", phase: "connected", selected: "north", connected: "south" },
    { id: "connecting", phase: "connecting", selected: "north", connected: null },
    { id: "switching", phase: "switching", selected: "east", connected: "south" },
    { id: "reconnecting", phase: "reconnecting", selected: "north", connected: null },
    { id: "unverified", phase: "unverified", selected: "west", connected: null },
    { id: "failed", phase: "failed", selected: "north", connected: null },
    { id: "recovery", phase: "recovery", selected: "north", connected: null },
    { id: "removed", phase: "disconnected", selected: "removed-profile", connected: null },
  ],
  large_list_count: 10000,
};

// A stale last-known server is useful context during a switch, but never a
// confirmed live connection or a green row marker.
export function connectionPresentation(scene) {
  if (scene.phase === "connected" && scene.connected) {
    return { kind: "confirmed", id: scene.connected };
  }
  if (scene.phase === "switching" && scene.connected) {
    return { kind: "previous", id: scene.connected };
  }
  return { kind: "none", id: null };
}

function generatedProfile(index) {
  const number = String(index).padStart(5, "0");
  return {
    id: `generated-${number}`,
    name: `Synthetic node ${number}`,
    country: "Example",
    host: `node-${number}.example`,
    subscription: "Generated samples",
  };
}

// Selection can outlive a filter or the large-list toggle. Resolve its
// details without rebuilding the entire 10k-row fixture on every render.
export function profileById(id, metadata = null) {
  if (metadata !== null) return metadata.profile(id);
  const sample = fixture.profiles.find((profile) => profile.id === id);
  if (sample) return sample;
  const match = /^generated-(\d{5})$/.exec(id ?? "");
  if (!match) return null;
  const index = Number(match[1]);
  return index < fixture.large_list_count ? generatedProfile(index) : null;
}

export function visibleProfiles(query, large = false, collection = "all", metadata = null) {
  const profiles = metadata === null ? [...fixture.profiles] : metadata.rows;
  if (large && metadata === null) {
    for (let index = 0; index < fixture.large_list_count; index += 1) {
      profiles.push(generatedProfile(index));
    }
  }
  const needle = query.toLocaleLowerCase();
  if (needle === "" && large && collection === "all") return profiles;
  return profiles.filter((profile) => {
    const inCollection = collection === "all"
      || (collection === "sample" && profile.subscription === "Sample collection")
      || (collection === "generated" && profile.subscription === "Generated samples")
      || (collection === "local" && profile.subscription === "");
    return (large || profile.subscription !== "Generated samples") && inCollection && [profile.name, profile.country, profile.host, profile.subscription]
      .some((value) => value.toLocaleLowerCase().includes(needle));
  });
}

// Only actual metadata content and filters invalidate this projection.
// Status samples/source epochs never enter its key.
export class ProfileCache {
  #fixtureRows = null;
  #projection = { key: null, query: null, large: null, collection: null, rows: null, terms: null,
    visible: [], ids: new Set(), positions: new Map() };
  get key() { return this.#projection.key; }
  get visible() { return this.#projection.visible; }
  get ids() { return this.#projection.ids; }
  get positions() { return this.#projection.positions; }

  isCurrent(query, large, collection, metadata = null) {
    const current = this.#projection;
    const rows = metadata === null ? this.#fixtureRows : metadata.rows;
    return rows !== null && current.rows === rows && current.query === query
      && current.large === large && current.collection === collection;
  }

  get(query, large, collection, metadata = null) {
    // One admitted immutable source and one projection, never a query history.
    // Keep visibleProfiles unchanged as the independent legacy parity oracle.
    const rows = metadata === null ? (this.#fixtureRows ??= visibleProfiles("", true)) : metadata.rows;
    const key = JSON.stringify([query, large, collection]);
    const current = this.#projection;
    if (key !== current.key || rows !== current.rows) {
      let terms = rows !== current.rows ? null : current.terms;
      const needle = query.toLocaleLowerCase();
      if (needle !== "" && terms === null) {
        terms = rows.map((profile) => [profile.name, profile.country, profile.host, profile.subscription]
          .map((value) => value.toLocaleLowerCase()));
      }
      const visible = needle === "" && large && collection === "all" ? rows : rows.filter((profile, index) => {
        const inCollection = collection === "all"
          || (collection === "sample" && profile.subscription === "Sample collection")
          || (collection === "generated" && profile.subscription === "Generated samples")
          || (collection === "local" && profile.subscription === "");
        return (large || profile.subscription !== "Generated samples") && inCollection
          && (needle === "" || terms[index].some((value) => value.includes(needle)));
      });
      const positions = new Map(visible.map((profile, index) => [profile.id, index]));
      const ids = new Set(positions.keys());
      // One publication after all preparation; an interrupted event cannot
      // leave a new row array paired with an old key/index. Old snapshots keep
      // their own unchanged arrays and maps for lazy count/key/range callbacks.
      this.#projection = { key, query, large, collection, rows, terms,
        visible: Object.freeze(visible), positions, ids };
    }
    return this.visible;
  }
}

// Use the index belonging to the same rendered projection, not a newer cache.
// Arrow events retain their original ordering; this does not debounce inputs.
export function nextIndexedProfile(visible, positions, currentId, direction) {
  if (visible.length === 0) return null;
  const current = positions.get(currentId);
  if (current === undefined) return visible[direction < 0 ? visible.length - 1 : 0].id;
  const next = Math.max(0, Math.min(visible.length - 1, current + direction));
  return visible[next].id;
}

// Keyboard position is navigation, not profile selection or a connection.
// A filtered-out position starts at the nearest edge in the key direction.
export function nextHighlightedProfile(visible, currentId, direction) {
  if (visible.length === 0) return null;
  const current = visible.findIndex((profile) => profile.id === currentId);
  if (current < 0) return visible[direction < 0 ? visible.length - 1 : 0].id;
  const next = Math.max(0, Math.min(visible.length - 1, current + direction));
  return visible[next].id;
}
