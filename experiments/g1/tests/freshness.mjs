import assert from "node:assert/strict";
import { Freshness } from "../shell/freshness.js";

const view = new Freshness();
view.attach("synthetic-a");
const first = view.request();
assert.ok(first);
assert.equal(view.receive(first, { instance: "synthetic-a", revision: 8, phase: "connected", connected: "south" }), true);
assert.equal(view.confirmed, "south");

const later = view.request();
assert.ok(later);
assert.equal(view.receive(later, { instance: "synthetic-a", revision: 9, phase: "recovery", connected: "south" }), true);
assert.equal(view.confirmed, null);
assert.equal(view.receive(first, { instance: "synthetic-a", revision: 8, phase: "connected", connected: "south" }), false);
assert.equal(view.revision, 9);

view.lost();
assert.equal(view.confirmed, null);
assert.equal(view.request(), null);
assert.equal(view.receive(later, { instance: "synthetic-a", revision: 10, phase: "connected", connected: "south" }), false);

view.attach("synthetic-b");
const current = view.request();
assert.ok(current);
assert.equal(view.receive(current, { instance: "synthetic-a", revision: 11, phase: "connected", connected: "south" }), false);
assert.equal(view.receive(current, { instance: "synthetic-b", revision: 1, phase: "connected", connected: "north" }), true);
assert.equal(view.confirmed, "north");
assert.equal(view.receive(current, { instance: "synthetic-b", revision: 1, phase: "connected", connected: "south" }), false);
assert.equal(view.confirmed, "north");
assert.equal(view.receive(current, { instance: "synthetic-b", revision: Number.NaN, phase: "connected", connected: "south" }), false);
assert.equal(view.confirmed, "north");

console.log("G1 synthetic freshness sequence passed");
