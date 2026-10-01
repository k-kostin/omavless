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

export function visibleProfiles(query, large = false, collection = "all") {
  const profiles = [...fixture.profiles];
  if (large) {
    for (let index = 0; index < fixture.large_list_count; index += 1) {
      const number = String(index).padStart(5, "0");
      profiles.push({
        id: `generated-${number}`,
        name: `Synthetic node ${number}`,
        country: "Example",
        host: `node-${number}.example`,
        subscription: "Generated samples",
      });
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
