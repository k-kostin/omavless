// SPDX-License-Identifier: MIT
// Actual application callbacks with finite toolkit-description doubles.
// Counts prove ownership/cost relocation, NOT GPUI dispatch or native latency.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import * as data from "../shell/data.js";
import { Metadata } from "../shell/metadata.js";
import { UpdateView } from "../shell/workload.js";
import { plainKey, navigatePage } from "../shell/keyboard.js";
import { DARK, LIGHT, BROKEN } from "../shell/palette.js";
import { prepareTheme, paletteText, ThemeUpdates } from "../shell/theme-source.js";
import { manualTheme, followTheme } from "../shell/theme-client.js";

const source = readFileSync(new URL("../shell/main.js", import.meta.url), "utf8");
const script = source.replace(/^import[\s\S]*?;\n/gm, "").replace("export default class", "return class");
let phase = "idle", view, buttons, keys, labels, virtual, notices = 0;
const counts = { init: 0, event: 0, render: 0, navigation: 0 };
const lower = String.prototype.toLocaleLowerCase;
const get = data.ProfileCache.prototype.get;
const findIndex = Array.prototype.findIndex;
const filter = Array.prototype.filter;
const map = Array.prototype.map;
const some = Array.prototype.some;
const byPhase = (name, callback) => {
  const previous = phase;
  phase = name;
  try { return callback(); } finally { phase = previous; }
};
String.prototype.toLocaleLowerCase = function(...args) {
  if (Object.hasOwn(counts, phase)) counts[phase]++;
  return lower.apply(this, args);
};
data.ProfileCache.prototype.get = function(...args) {
  assert.notEqual(phase, "render", "render must not even request a projection rebuild");
  assert.notEqual(phase, "navigation", "arrows/inspection reuse their snapshot projection");
  return get.apply(this, args);
};
Array.prototype.findIndex = function(...args) {
  assert.notEqual(phase, "navigation", "indexed list navigation must not scan rows");
  return findIndex.apply(this, args);
};
Array.prototype.filter = function(...args) {
  assert.notEqual(phase, "render", "render must not filter a full row collection");
  assert.notEqual(phase, "navigation", "navigation must not filter rows");
  return filter.apply(this, args);
};
Array.prototype.map = function(...args) {
  if (this.length >= 10000) {
    assert.notEqual(phase, "render", "render must not map the full research list");
    assert.notEqual(phase, "navigation", "navigation must not map the full research list");
  }
  return map.apply(this, args);
};
Array.prototype.some = function(...args) {
  if (this.length >= 10000) {
    assert.notEqual(phase, "render", "render must not scan the full research list");
    assert.notEqual(phase, "navigation", "navigation must not scan the full research list");
  }
  return some.apply(this, args);
};

// Only API-description operations are doubled. The real G1Trial methods,
// metadata admission, projection, keys, freshness and palette policy execute.
class Element {
  constructor(id) {
    const target = { id };
    let proxy;
    proxy = new Proxy(target, { get: (_target, name) => {
      if (name === "then") return undefined;
      if (name === "when") return (condition, callback) => { if (condition) callback(proxy); return proxy; };
      if (name === "onClick") return (callback) => { buttons.set(id, callback); return proxy; };
      if (name === "on_key_down") return (callback) => { keys.push(callback); return proxy; };
      if (name === "on_item_click") return (callback) => { virtual.click = callback; return proxy; };
      if (name === "label") return (text) => { labels.push(text); return proxy; };
      return () => proxy;
    } });
    return proxy;
  }
}
class Text extends Element { constructor(text) { super(); labels.push(text); } }
const flex = () => new Element();
const input = { new: () => ({ text: "", events: new Map(),
  value() { return this.text; }, on(name, handler) { this.events.set(name, handler); } }) };
const context = {
  theme: () => ({ colors: { ring: "ring" } }),
  focus_handle: () => ({ focused: false, focus() { this.focused = true; }, is_focused() { return this.focused; } }),
  notify: () => notices++, stop_propagation: () => {},
};
const dependencies = {
  View: class {}, InputState: input, Scrollbar: { vertical: flex },
  VirtualListScrollHandle: { new: () => ({ calls: [], scroll_to_item(index, alignment) { this.calls.push([index, alignment]); } }) },
  h_flex: flex, v_flex: flex,
  v_virtual_list: (_id, count, _height, getKey, range) => {
    virtual = { count, getKey, range }; return flex();
  }, set_theme: () => {}, AppShell: Element, Badge: Element, Button: Element,
  Label: Text, ListRow: Element, MutedText: Text, Panel: Element,
  TextField: Element, Title: Text, TitleBar: Element,
  applyOmarchyRoles: () => {}, applyOmarchyStyle: () => ({}), omarchyTheme: () => ({}),
  ...data, DARK, LIGHT, BROKEN, UpdateView, Metadata, plainKey, navigatePage,
  prepareTheme, paletteText, ThemeUpdates, manualTheme, followTheme,
};
const Trial = new Function(...Object.keys(dependencies), script)(...Object.values(dependencies));
const resetDescriptions = () => { buttons = new Map(); keys = []; labels = []; virtual = null; };
const render = () => { resetDescriptions(); byPhase("render", () => view.render(context)); };
const click = (id) => {
  assert.equal(buttons.has(id), true, `actual rendered handler ${id} missing`);
  byPhase("event", () => buttons.get(id)({}, context)); render();
};
const search = (text) => {
  view.search.text = text;
  byPhase("event", () => view.search.events.get("change")({}, context)); render();
};
const key = (value, modifier = null) => ({ key: value,
  keystroke: modifier ? `${modifier}-${value}` : value,
  modifiers: { shift: modifier === "shift", control: modifier === "control", alt: false, platform: false, function: false } });
const listKey = (value, modifier = null) => byPhase("navigation", () => keys[0](key(value, modifier), context));
const parity = () => {
  const rows = data.visibleProfiles(view.search.value(), view.large, view.collection, view.updates.metadata);
  assert.deepEqual(view.profileCache.visible, rows);
  assert.equal(view.profileCache.positions.size, rows.length);
  rows.forEach((row, index) => assert.equal(view.profileCache.positions.get(row.id), index));
};

try {
  // Validated independent snapshots may have equal content counters. Their
  // identities must not alias, and fields must never form cross-boundary hits.
  const first = Metadata.validate([data.fixture.profiles[0]], new Metadata());
  const second = Metadata.validate([data.fixture.profiles[1]], new Metadata());
  assert.equal(first.content, second.content);
  const isolated = new data.ProfileCache();
  assert.equal(isolated.get("north", true, "all", first).length, 1);
  assert.equal(isolated.isCurrent("north", true, "all", first), true);
  assert.equal(isolated.isCurrent("north", true, "all", second), false);
  const oldRows = isolated.visible, oldPositions = isolated.positions;
  assert.equal(Object.isFrozen(oldRows), true);
  assert.equal(isolated.get("north", true, "all", second).length, 0);
  assert.equal(oldRows[0].id, "north"); assert.equal(oldPositions.get("north"), 0);
  assert.equal(isolated.get("ExampleFinland", true, "all", first).length, 0);
  // Simulate interruption during new terms preparation, not a native budget.
  const beforeRows = isolated.visible, beforePositions = isolated.positions, beforeKey = isolated.key;
  String.prototype.toLocaleLowerCase = function(...args) {
    if (String(this) === data.fixture.profiles[1].name) throw new Error("synthetic interrupted preparation");
    return lower.apply(this, args);
  };
  assert.throws(() => isolated.get("south", true, "all", second), /synthetic interrupted preparation/);
  assert.strictEqual(isolated.visible, beforeRows); assert.strictEqual(isolated.positions, beforePositions);
  assert.equal(isolated.key, beforeKey);
  assert.equal(isolated.isCurrent("south", true, "all", second), false);
  String.prototype.toLocaleLowerCase = function(...args) {
    if (Object.hasOwn(counts, phase)) counts[phase]++;
    return lower.apply(this, args);
  };
  const unicode = Metadata.validate([{ ...data.fixture.profiles[0], name: "İ Σ ß Восточный", country: "Japan" }], new Metadata());
  for (const query of ["İ", "i\u0307", "Σ", "σ", "ß", "Восточный", "ВОСТОЧНЫЙ", "ExampleFinland"]) {
    assert.deepEqual(isolated.get(query, true, "all", unicode), data.visibleProfiles(query, true, "all", unicode));
  }
  for (const locale of ["en", "ru"]) {
    resetDescriptions(); view = new Trial();
    byPhase("init", () => view.init({}, context)); view.locale = locale; render(); parity();
    const owners = [view.search, view.listScroll, view.panelFocus, view.listFocus, view.profileCache];
    click("large-list"); assert.equal(virtual.count, 10006); parity();
    const before = counts.event;
    for (const prefix of ["e", "ex", "exa", "exam", "examp", "exampl", "example"]) { search(prefix); parity(); }
    assert.equal(counts.event - before, 40031); // 4 fields once + 7 needles.
    assert.equal(view.profileCache.visible.length, 10006);
    assert.equal(view.selected, "north"); assert.equal(view.updates.state.connected, "south");
    // One captured handler receives the complete original 18-key order.
    view.listFocus.focus(); render();
    const captured = keys[0], projection = view.profileCache.visible;
    let expected = view.highlighted;
    for (let index = 0; index < 18; index++) {
      expected = data.nextHighlightedProfile(projection, expected, 1);
      byPhase("navigation", () => captured(key("down"), context));
      assert.equal(view.highlighted, expected);
      assert.deepEqual(view.listScroll.calls.at(-1), [index + 1, "center"]);
      assert.equal(view.selected, "north");
    }
    assert.equal(view.highlighted, "generated-00012"); render();
    listKey("enter"); assert.equal(view.selected, "generated-00012"); render();
    assert.equal(labels.includes("Synthetic node 00012"), true);
    const selected = view.selected, highlighted = view.highlighted;
    for (const modifier of ["shift", "control"]) for (const action of ["up", "down", "enter"]) listKey(action, modifier);
    assert.equal(view.selected, selected); assert.equal(view.highlighted, highlighted);
    const noRecompute = counts.event;
    const cachedRows = view.profileCache.visible, cachedPositions = view.profileCache.positions;
    click("locale"); click("locale"); click("theme-light"); click("theme-broken");
    assert.equal(view.selected, selected); assert.equal(view.highlighted, highlighted);
    click("update-burst"); parity(); assert.equal(view.selected, selected);
    const retainedRows = view.profileCache.visible, retainedPositions = view.profileCache.positions;
    click("source-loss");
    assert.equal(view.updates.metadataFresh, false); assert.equal(view.updates.state.connected, null);
    assert.strictEqual(view.profileCache.visible, retainedRows);
    click("source-attach"); assert.equal(view.updates.state.connected, "south");
    assert.strictEqual(view.profileCache.positions, retainedPositions);
    assert.strictEqual(view.profileCache.visible, cachedRows);
    assert.strictEqual(view.profileCache.positions, cachedPositions);
    assert.equal(counts.event, noRecompute, "locale/theme/status/source epoch reuse terms/projection");
    click("metadata-rename"); search("Renamed"); parity();
    assert.equal(view.profileCache.visible.length, 1); assert.equal(view.selected, selected);
    const staleClick = virtual.click;
    click("metadata-remove"); parity(); assert.equal(view.selected, null);
    byPhase("event", () => staleClick(selected, context));
    assert.equal(view.selected, null, "late pointer callback must not resurrect removed inspection");
    assert.equal(view.profileCache.visible.length, 0); listKey("enter"); assert.equal(view.selected, null);
    click("metadata-reset"); assert.equal(view.selected, null); search("example"); parity();
    click("metadata-replace"); parity();
    assert.equal(view.updates.state.connected, "south"); assert.equal(view.updates.metadata.profile("south"), null);
    assert.equal(labels.some((label) => typeof label === "string" && label.startsWith("south ·")), true);
    const current = view.updates.freshness.request(), revision = view.updates.freshness.revision;
    const canary = [{ ...data.fixture.profiles[0], name: "REFUSED_SYNTHETIC_CANARY" }];
    assert.equal(view.updates.receive(current, { instance: current.instance, revision,
      phase: "connected", connected: "north" }, view.updates.generation + 1, canary), false);
    const old = current; click("source-loss"); click("source-attach");
    assert.equal(view.updates.receive(old, { instance: old.instance, revision: revision + 100,
      phase: "connected", connected: "north" }, view.updates.generation + 1, canary), false);
    search("REFUSED_SYNTHETIC_CANARY"); parity(); assert.equal(view.profileCache.visible.length, 0);
    search("пример"); parity(); search("ВОСТОЧНЫЙ"); parity();
    for (const collection of ["sample", "generated", "local", "all"]) { click(`collection-${collection}`); parity(); }
    search(""); click("large-list"); parity(); click("large-list"); parity();
    // A retained old callback is refused after its projection is replaced;
    // an old index must never target a different current row.
    search("replacement"); const oldHandler = keys[0], oldRows = view.profileCache.visible;
    search("node-09999"); view.highlighted = oldRows[0].id;
    const scrollCalls = view.listScroll.calls.length;
    byPhase("navigation", () => oldHandler(key("down"), context));
    assert.equal(view.highlighted, oldRows[0].id);
    assert.equal(view.listScroll.calls.length, scrollCalls);
    render();
    [view.search, view.listScroll, view.panelFocus, view.listFocus, view.profileCache]
      .forEach((owner, index) => assert.strictEqual(owner, owners[index]));
  }

  // Actual owning state changes BEFORE preparation. In particular the native
  // editor has already mutated its value before dispatching the change event.
  // An atomic cache alone must not turn a later palette/locale/status notify
  // into a render mixing old rows and new state.
  for (const locale of ["en", "ru"]) for (const mutation of ["query", "large", "collection", "rename", "remove"]) {
    resetDescriptions(); view = new Trial();
    byPhase("init", () => view.init({}, context)); view.locale = locale; render();
    const handlers = buttons, oldKey = keys[0], oldClick = virtual.click;
    const retainedRows = view.profileCache.visible, retainedPositions = view.profileCache.positions;
    const eventGet = data.ProfileCache.prototype.get;
    const noticesBeforeFailure = notices;
    data.ProfileCache.prototype.get = function() {
      assert.equal(phase, "event");
      throw new Error("synthetic preparation failed");
    };
    try {
      assert.throws(() => byPhase("event", () => {
        if (mutation === "query") {
          view.search.text = "UNPUBLISHED_RAW_QUERY_CANARY";
          view.search.events.get("change")({}, context);
        } else handlers.get(mutation === "large" ? "large-list" : mutation === "collection"
          ? "collection-generated" : `metadata-${mutation}`)({}, context);
      }), /synthetic preparation failed/);
    } finally { data.ProfileCache.prototype.get = eventGet; }
    assert.equal(notices, noticesBeforeFailure, "preparation must finish before the owning notify");
    assert.equal(view.projectionCurrent(), false);
    assert.strictEqual(view.profileCache.visible, retainedRows);
    assert.strictEqual(view.profileCache.positions, retainedPositions);
    const selected = view.selected, highlighted = view.highlighted, scrollCount = view.listScroll.calls.length;
    for (const notify of ["theme-light", "locale", "update-burst"]) {
      const beforeNotify = notices;
      byPhase("event", () => handlers.get(notify)({}, context));
      assert.ok(notices > beforeNotify, "unrelated actual handler must notify before guarded render");
      assert.throws(() => render(), (error) => error.message === "g1-profile-projection-unavailable");
      assert.equal(labels.length, 0, "no new descriptions/metadata should be emitted before the fixed refusal");
      assert.equal(virtual, null);
      for (const action of ["down", "enter"]) byPhase("navigation", () => oldKey(key(action), context));
      byPhase("event", () => oldClick("north", context));
      assert.equal(view.selected, selected); assert.equal(view.highlighted, highlighted);
      assert.equal(view.listScroll.calls.length, scrollCount);
    }
    view.panelFocus.focused = false;
    byPhase("navigation", () => oldKey(key("escape"), context));
    assert.equal(view.panelFocus.focused, true, "stale-projection refusal must not trap focus");
    // Only a subsequent successful owning input event republishes the whole
    // current projection. No render/layout fallback or automatic retry.
    byPhase("event", () => view.search.events.get("change")({}, context));
    assert.equal(view.projectionCurrent(), true); render(); parity();
    assert.equal(view.selected, selected);
    assert.equal(view.updates.state.connected, "south");
  }
  assert.equal(counts.render, 0); assert.equal(counts.navigation, 0);
  console.log("PASS: actual EN/RU handlers; full10006 legacy parity; render/navigation scans0; indexed18 keys; injected query/large/collection/metadata failure→unrelated notify render refusal; stale callbacks/focus/continuity (native UNRUN)");
} finally {
  String.prototype.toLocaleLowerCase = lower;
  data.ProfileCache.prototype.get = get;
  Array.prototype.findIndex = findIndex;
  Array.prototype.filter = filter;
  Array.prototype.map = map;
  Array.prototype.some = some;
}
