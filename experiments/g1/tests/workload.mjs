// SPDX-License-Identifier: MIT
import assert from "node:assert/strict";
import { fixture, profileById, ProfileCache } from "../shell/data.js";
import { UpdateView } from "../shell/workload.js";

const view = new UpdateView();
view.attach();
const ticket = view.freshness.request();
for (let revision = 1; revision <= 1000; revision += 1) {
  assert.equal(view.receive(ticket, { instance: ticket.instance, revision,
    phase: "connected", connected: "south" }), true);
  assert.equal(view.pending.sample, revision);
}
assert.equal(view.state.sample, 0);
assert.equal(view.receive(ticket, { instance: "wrong-source", revision: 2000,
  phase: "connected", connected: "north" }), false);
assert.equal(view.burst({ phase: "connected", connected: "north" }, 1001), false);
assert.equal(view.burst({ phase: "connected", connected: "north" }, 0), false);
assert.equal(view.receive(ticket, { instance: ticket.instance, revision: 999,
  phase: "connected", connected: "north" }), false);
assert.equal(view.flush(), true);
assert.deepEqual(view.state, { phase: "connected", connected: "south", sample: 1000 });
assert.equal(view.flush(), false);
assert.equal(view.burst({ phase: "recovery", connected: "south" }), true);
assert.equal(view.state.connected, null);
assert.equal(view.receive(ticket, { instance: ticket.instance, revision: 2001,
  phase: "connected", connected: "south" }), true);
view.lost();
assert.equal(view.flush(), false);
assert.deepEqual(view.state, { phase: "unverified", connected: null, sample: 0 });
assert.equal(view.available(), false);
assert.equal(view.burst(fixture.scenes[0]), false);
view.attach();
assert.equal(view.receive(ticket, { instance: ticket.instance, revision: 9000,
  phase: "connected", connected: "south" }), false);
assert.equal(view.burst({ phase: "connected", connected: "north" }, 1), true);
assert.deepEqual(view.state, { phase: "connected", connected: "north", sample: 1 });
assert.equal(view.burst({ phase: "unknown-phase", connected: "north" }, 1), true);
assert.deepEqual(view.state, { phase: "unverified", connected: null, sample: 2 });

// The shared fixture remains a separate metadata input. Status ingestion has
// no list/selection argument, so bursts cannot replace an inspected record.
const cache = new ProfileCache();
const visible = cache.get("", true, "all");
assert.equal(visible.length, 10006);
const selected = profileById("generated-09999");
for (let burst = 0; burst < 100; burst += 1) {
  view.burst(fixture.scenes[0]);
  assert.strictEqual(cache.get("", true, "all"), visible);
  assert.equal(cache.ids.has(selected.id), true);
}
assert.equal(view.state.sample, 100002);
assert.equal(profileById(selected.id).id, selected.id);
assert.equal(cache.get("no-matches", true, "all").length, 0);
assert.equal(profileById(selected.id).id, selected.id);
assert.equal(cache.get("", false, "all").length, 6);
assert.equal(cache.get("", true, "generated").length, 10000);
for (const scene of fixture.scenes) {
  view.attach();
  assert.equal(view.available(), true);
  assert.equal(view.burst(scene, 1), true);
  assert.equal(view.state.phase, scene.phase);
  assert.equal(view.state.connected, ["connected", "switching"].includes(scene.phase) ? scene.connected : null);
}
console.log("G1 synthetic update workload: PASS (100,000 updates; one pending slot; lost/stale gates)");
