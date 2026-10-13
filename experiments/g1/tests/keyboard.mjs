// SPDX-License-Identifier: MIT
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { plainKey, navigatePage } from "../shell/keyboard.js";
import { nextIndexedProfile } from "../shell/data.js";

const cases = JSON.parse(readFileSync(new URL("../keyboard-fixtures.json", import.meta.url), "utf8"));
const modifiers = ["shift", "control", "alt", "platform", "function"];
const event = (key, modifier = null) => ({
  key, keystroke: modifier === null ? key : `${modifier}-${key}`,
  modifiers: Object.fromEntries(modifiers.map((name) => [name, name === modifier])),
});
for (const item of cases) {
  for (const modifier of item.modified ? modifiers : [null]) {
    assert.equal(plainKey(event(item.key, modifier)), item.plain);
    // Even a malformed/plain chord spelling must not hide held modifiers.
    if (modifier) assert.equal(plainKey({ ...event(item.key, modifier), keystroke: item.key }), null);
  }
}
assert.equal(plainKey({ ...event("enter"), keystroke: "ctrl-enter" }), null);

// Exercise the actual exported handler used at the rendered application root.
// These doubles observe application effects, not native focus/rendering.
const run = (detailsOnly, input) => {
  const calls = [];
  const view = { detailsOnly, selected: null, highlighted: "replacement-b", query: "Renamed",
    confirmed: "south", panelFocus: { focus: () => calls.push("focus") } };
  const context = { notify: () => calls.push("notify"), stop_propagation: () => calls.push("stop") };
  const accepted = navigatePage(input, view, context);
  assert.equal(view.selected, null);
  assert.equal(view.highlighted, "replacement-b");
  assert.equal(view.query, "Renamed");
  assert.equal(view.confirmed, "south");
  return { accepted, detailsOnly: view.detailsOnly, calls };
};
assert.deepEqual(run(false, event("pagedown")), { accepted: true, detailsOnly: true, calls: ["focus", "notify", "stop"] });
assert.deepEqual(run(true, event("pageup")), { accepted: true, detailsOnly: false, calls: ["focus", "notify", "stop"] });
for (const key of ["pageup", "enter", "escape", "down", "up", "tab", "backspace", "delete", "home", "end", "a"]) {
  assert.deepEqual(run(false, event(key)), { accepted: false, detailsOnly: false, calls: [] });
}
for (const modifier of modifiers) {
  for (const key of ["pagedown", "pageup", "enter", "down", "up", "escape", "a", "v"]) {
    assert.deepEqual(run(false, event(key, modifier)), { accepted: false, detailsOnly: false, calls: [] });
    assert.deepEqual(run(true, event(key, modifier)), { accepted: false, detailsOnly: true, calls: [] });
  }
}
assert.deepEqual(run(true, event("pagedown")), { accepted: false, detailsOnly: true, calls: [] });

// Source wiring guard only: the native host must still prove focus propagation.
const source = readFileSync(new URL("../shell/main.js", import.meta.url), "utf8");
const root = source.slice(source.indexOf("return new AppShell()"));
assert.match(root, /\.build\(cx\)\s*\.on_key_down\(\(event, context\) => navigatePage\(event, this, context\)\)/);
assert.equal(source.match(/navigatePage\(event, this, context\)/g).length, 1);
assert.match(source, /const navigateList = \(event, context\) => \{\s*const key = plainKey\(event\)/);

// Execute the actual list callback with retained synthetic view/context doubles.
// This checks its effects, not a simulated GPUI event-dispatch implementation.
const start = source.indexOf("const navigateList = ") + "const navigateList = ".length;
const end = source.indexOf("\n    };", start) + "\n    }".length;
const bindList = new Function("visible", "positions", "nextIndexedProfile", "plainKey", `return (${source.slice(start, end)});`);
const listRun = (input, visible = [{ id: "north" }, { id: "south" }], highlighted = "north") => {
  const calls = [];
  const view = { highlighted, selected: "north", confirmed: "south",
    profileCache: { visible }, updates: { metadata: { selection: (id) => visible.some((row) => row.id === id) ? id : null } },
    listScroll: { scroll_to_item: (index) => calls.push(`scroll:${index}`) },
    panelFocus: { focus: () => calls.push("focus") } };
  const context = { notify: () => calls.push("notify"), stop_propagation: () => calls.push("stop") };
  const positions = new Map(visible.map((profile, index) => [profile.id, index]));
  bindList.call(view, visible, positions, nextIndexedProfile, plainKey)(input, context);
  assert.equal(view.confirmed, "south");
  return { selected: view.selected, highlighted: view.highlighted, calls };
};
assert.deepEqual(listRun(event("down")), { selected: "north", highlighted: "south", calls: ["scroll:1", "notify", "stop"] });
assert.deepEqual(listRun(event("enter"), undefined, "south"), { selected: "south", highlighted: "south", calls: ["notify", "stop"] });
assert.deepEqual(listRun(event("enter"), [], null), { selected: "north", highlighted: null, calls: [] });
assert.deepEqual(listRun(event("enter"), [{ id: "south" }], "removed"), { selected: "north", highlighted: "removed", calls: [] });
assert.deepEqual(listRun(event("escape")), { selected: "north", highlighted: "north", calls: ["focus", "stop"] });
for (const key of ["pagedown", "pageup", "tab", "home", "end", "backspace", "a"]) {
  assert.deepEqual(listRun(event(key)), { selected: "north", highlighted: "north", calls: [] });
}
for (const modifier of modifiers) for (const key of ["up", "down", "enter", "escape"]) {
  assert.deepEqual(listRun(event(key, modifier)), { selected: "north", highlighted: "north", calls: [] });
}
console.log("PASS: shared plain-key admission, actual Shell page-handler effects, root wiring (native focus UNRUN)");
