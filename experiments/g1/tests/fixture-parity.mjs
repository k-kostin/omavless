import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { connectionPresentation, fixture, visibleProfiles } from "../shell/data.js";

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
console.log("G1 synthetic fixture parity passed");
