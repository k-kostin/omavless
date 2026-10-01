import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { connectionPresentation, fixture, visibleProfiles } from "../shell/data.js";
import { BROKEN, DARK, LIGHT, completePalette, resolvedPalette } from "../shell/palette.js";

const expected = JSON.parse(readFileSync(new URL("../fixtures.json", import.meta.url), "utf8"));
assert.deepEqual(fixture, expected);
assert.notEqual(fixture.scenes[0].selected, fixture.scenes[0].connected);
assert.equal(fixture.scenes.find((scene) => scene.id === "removed").selected, "removed-profile");
assert.equal(visibleProfiles("south").length, 1);
assert.equal(visibleProfiles("north", true).length, 1);
assert.equal(visibleProfiles("synthetic node", true).length, fixture.large_list_count);
assert.equal(visibleProfiles("", true, "sample").length, 5);
assert.equal(visibleProfiles("", true, "generated").length, fixture.large_list_count);
assert.equal(visibleProfiles("", true, "local").length, 1);
assert.equal(visibleProfiles("south", false, "local").length, 0);
assert.deepEqual(connectionPresentation(fixture.scenes[0]), { kind: "confirmed", id: "south" });
assert.deepEqual(connectionPresentation(fixture.scenes[2]), { kind: "previous", id: "south" });
for (const scene of fixture.scenes.filter((scene) => !["connected", "switching"].includes(scene.phase))) {
  assert.deepEqual(connectionPresentation(scene), { kind: "none", id: null });
}
assert.equal(completePalette(DARK), true);
assert.equal(completePalette(LIGHT), true);
assert.equal(completePalette(BROKEN), false);
assert.deepEqual(resolvedPalette(BROKEN), { source: DARK, fallback: true });
assert.deepEqual(resolvedPalette(LIGHT), { source: LIGHT, fallback: false });
assert.equal(completePalette(DARK.replace('accent = "#7aa2f7"', 'accent = "oops"')), false);
assert.equal(completePalette(DARK.replace('mode = "dark"', 'mode = "night"')), false);
assert.equal(completePalette(`${DARK}\nforeground = "#ffffff"`), false);
assert.equal(completePalette(DARK.replace('foreground = "#c0caf5"', '')), false);
console.log("G1 synthetic fixture parity passed");
