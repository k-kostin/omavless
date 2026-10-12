// SPDX-License-Identifier: MIT
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fixture, profileById, ProfileCache } from "../shell/data.js";
import { Metadata, MAX_ROWS, replacementFixture } from "../shell/metadata.js";
import { UpdateView } from "../shell/workload.js";

assert.deepEqual(replacementFixture, JSON.parse(readFileSync(new URL("../metadata-fixtures.json", import.meta.url), "utf8")));
const initial = Metadata.fixture();
assert.equal(Object.isFrozen(initial), true);
assert.equal(Object.isFrozen(initial.rows), true);
assert.equal(Object.isFrozen(initial.rows[0]), true);
assert.throws(() => new Metadata([{ ...initial.rows[0], name: "\u001b" }]), /validation/);
assert.equal(Metadata.validate(initial.rows, { rows: initial.rows, content: initial.content, validated: true }), null);
assert.equal(Metadata.validate([{ ...initial.rows[0], name: "\u001b", validated: true }], initial), null);
assert.equal(Metadata.validate([initial.rows[0], initial.rows[0]], initial), null);
assert.equal(Metadata.validate([{ ...initial.rows[0], name: "\u001b" }], initial), null);
assert.equal(initial.rows.length, MAX_ROWS);
const view = new UpdateView(initial);
view.attach(); view.burst(fixture.scenes[0], 1);
const cache = new ProfileCache();
let selected = "north";
assert.equal(cache.get("Renamed", false, "sample", view.metadata).length, 0);
assert.equal(view.replace(fixture.scenes[0], view.metadata.renamed(selected)), true);
assert.equal(view.metadata.selection(selected), selected);
assert.equal(profileById(selected, view.metadata).name, "Renamed · North · Example");
assert.equal(cache.get("Renamed", false, "sample", view.metadata).length, 1);
assert.equal(view.replace(fixture.scenes[0], view.metadata.removed(selected)), true);
selected = view.metadata.selection(selected);
assert.equal(selected, null);
assert.equal(profileById("north", view.metadata), null);
assert.equal(cache.get("", false, "sample", view.metadata).length, 4);
assert.equal(view.state.connected, "south");
assert.equal(view.replace(fixture.scenes[0], view.metadata.replacedCollection()), true);
assert.deepEqual(cache.get("", false, "sample", view.metadata).map((row) => row.id), ["replacement-a", "replacement-b"]);
assert.equal(profileById("south", view.metadata), null);
assert.equal(view.state.connected, "south"); // Missing metadata is not Disconnect.
assert.equal(view.metadata.selection("south"), null);
assert.equal(view.metadata.selection("local"), "local");
const retained = cache.get("", true, "all", view.metadata);
const metadata = view.metadata;
for (let count = 0; count < 100; count += 1) {
  view.burst(fixture.scenes[0]);
  assert.strictEqual(view.metadata, metadata);
  assert.strictEqual(cache.get("", true, "all", view.metadata), retained);
}
assert.equal(view.replace(fixture.scenes[0], view.metadata.rows), true);
assert.strictEqual(view.metadata, metadata); // A newer generation, identical content.
assert.strictEqual(cache.get("", true, "all", view.metadata), retained);
assert.equal(view.replace(fixture.scenes[0], initial.rows), true);
assert.equal(view.metadata.selection(selected), null); // Reset never restores inspection.
assert.equal(cache.get("", true, "all", view.metadata).length, MAX_ROWS);
assert.equal(view.replace(fixture.scenes[0], view.metadata.removed("generated-09999")), true);
assert.equal(view.metadata.selection("generated-09999"), null);
assert.equal(profileById("generated-09999", view.metadata), null);

// Invalid candidates never consume freshness or replace any prefix of a list.
for (const rows of [
  [...initial.rows, initial.rows[0]],
  [initial.rows[0], initial.rows[0]],
  [{ ...initial.rows[0], name: "x".repeat(513) }],
  [{ ...initial.rows[0], country: "\u001b[2J" }],
  [{ ...initial.rows[0], host: "actual.invalid" }],
]) assert.equal(view.replace(fixture.scenes[0], rows), false);
assert.equal(view.metadata.profile("generated-09999"), null);
assert.equal(view.metadata.rows.length, MAX_ROWS - 1);

const current = view.freshness.request();
const next = view.freshness.revision + 1;
const packet = { instance: current.instance, revision: next, phase: "connected", connected: "south" };
assert.equal(view.receive(current, packet, view.generation, initial.rows), false); // Same generation, changed rows.
assert.equal(view.receive(current, packet, view.generation + 1), false); // Future status without metadata.
assert.equal(view.receive(current, packet, 0, initial.rows), false);
assert.equal(view.receive(current, packet, view.generation + 1, initial.rows), true);
assert.equal(view.receive(current, { ...packet, revision: next - 1 }, view.generation + 2, []), false);
view.lost();
assert.equal(view.flush(), false);
assert.equal(view.metadataFresh, false);
assert.equal(view.state.connected, null);
assert.equal(view.metadata.profile("generated-09999"), null); // Queued reset did not restore it.
view.attach();
const fresh = view.freshness.request();
assert.equal(view.receive(fresh, { ...packet, revision: 1 }, 1), false);
assert.equal(view.receive(current, { ...packet, revision: next + 1000 }, 1, initial.rows), false);
assert.equal(view.receive(fresh, { ...packet, revision: 1 }, 1, view.metadata.rows), true);
assert.equal(view.flush(), true);
assert.equal(view.metadataFresh, true);
assert.equal(view.metadata.profile("generated-09999"), null);
assert.equal(view.state.connected, "south");
console.log("G1 metadata replacement: PASS (shared fixtures; whole replacement, selection loss, generation/loss gates, status cache reuse)");
