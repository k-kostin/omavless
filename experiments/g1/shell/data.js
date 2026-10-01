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
export function profileById(id) {
  const sample = fixture.profiles.find((profile) => profile.id === id);
  if (sample) return sample;
  const match = /^generated-(\d{5})$/.exec(id ?? "");
  if (!match) return null;
  const index = Number(match[1]);
  return index < fixture.large_list_count ? generatedProfile(index) : null;
}

export function visibleProfiles(query, large = false, collection = "all") {
  const profiles = [...fixture.profiles];
  if (large) {
    for (let index = 0; index < fixture.large_list_count; index += 1) {
      profiles.push(generatedProfile(index));
    }
  }
  const needle = query.toLocaleLowerCase();
  return profiles.filter((profile) => {
    const inCollection = collection === "all"
      || (collection === "sample" && profile.subscription === "Sample collection")
      || (collection === "generated" && profile.subscription === "Generated samples")
      || (collection === "local" && profile.subscription === "");
    return inCollection && [profile.name, profile.country, profile.host, profile.subscription]
      .some((value) => value.toLocaleLowerCase().includes(needle));
  });
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
