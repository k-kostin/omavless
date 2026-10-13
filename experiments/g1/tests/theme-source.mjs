// SPDX-License-Identifier: MIT
// Owned public HOME-cache files only. No script-runtime fs grant or GUI launch.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { DARK, LIGHT, BROKEN } from "../shell/palette.js";
import { MAX_THEME_BYTES, MAX_THEME_SEQUENCE, REQUIRED, OPTIONAL, DEFAULT_PALETTE, prepareTheme, paletteText, ThemeUpdates } from "../shell/theme-source.js";
import { fixture, ProfileCache } from "../shell/data.js";
import { Metadata } from "../shell/metadata.js";

const matrix = JSON.parse(fs.readFileSync(new URL("../theme-fixtures.json", import.meta.url), "utf8"));
assert.equal(matrix.maxBytes, MAX_THEME_BYTES);
assert.deepEqual(matrix.required, REQUIRED);
assert.deepEqual(matrix.optional, OPTIONAL);
assert.deepEqual(matrix.default, DEFAULT_PALETTE);
const inputs = { dark: DARK, light: LIGHT, broken: BROKEN, missing: null,
  oversize: " ".repeat(MAX_THEME_BYTES + 1), rgba: DARK.replaceAll("#7aa2f7", "#7aa2f7ff"),
  duplicate: `${DARK}\naccent = "#ffffff"`, unknown: `${DARK}\nname = "Public fixture"`,
  prototype: `${DARK}\n__proto__ = "#ffffff"`, control: `${DARK}\n#\x1b` };
for (const row of matrix.cases) {
  const prepared = prepareTheme(inputs[row.input]);
  assert.equal(prepared.status, row.status);
  assert.equal(prepared.palette.mode, row.mode);
  if (prepared.status !== "good") assert.strictEqual(prepared.palette, DEFAULT_PALETTE);
}
assert.equal(prepareTheme("é".repeat(3000)).status, "oversize");
assert.equal(prepareTheme("\ud800").status, "malformed");

const updates = new ThemeUpdates();
assert.equal(updates.receive(1, prepareTheme(DARK)), true);
updates.flush();
const retained = updates.current.palette;
for (let sequence = 2; sequence <= 100000; sequence++) assert.equal(updates.receive(sequence, prepareTheme(DARK)), true);
assert.equal(updates.receive(99999, prepareTheme(BROKEN)), false);
assert.equal(updates.receive(MAX_THEME_SEQUENCE + 1, prepareTheme(LIGHT)), false);
assert.equal(updates.receive(100001, { status: "missing", palette: prepareTheme(LIGHT).palette }), false);
assert.equal(updates.receive(100001, Object.assign(Object.create({ status: "good", palette: DEFAULT_PALETTE }), { a: 1, b: 2 })), false);
assert.equal(updates.flush().sample.sequence, 100000);
assert.strictEqual(updates.current.palette, retained);
assert.equal(updates.flush(), null);

// Execute the actual Shell consumer, with only toolkit-global color operations
// substituted. Native rendering/scroll positioning is not established here.
const main = fs.readFileSync(new URL("../shell/main.js", import.meta.url), "utf8");
const signature = "  applyThemeSample(sequence, prepared, context) {";
const begin = main.indexOf(signature) + signature.length;
let depth = 1, end = begin;
while (depth && end < main.length) { if (main[end] === "{") depth++; if (main[end] === "}") depth--; end++; }
assert.equal(depth, 0);
let paints = 0, notices = 0;
const apply = new Function("paletteText", "omarchyTheme", "set_theme", "applyOmarchyRoles",
  `return function(sequence, prepared, context) { ${main.slice(begin, end - 1)} };`)(paletteText,
  (source) => ({ source }), () => paints++, () => {});
const metadata = Metadata.fixture();
const cache = new ProfileCache();
const rows = cache.get("north", true, "all", metadata);
const view = { themeUpdates: new ThemeUpdates(), baseTheme: {}, themeTokens: {}, themeChoice: "dark",
  query: "north", selected: "north", highlighted: "south", confirmed: "south",
  listScroll: {}, panelFocus: {}, profileCache: cache, metadata };
const owners = [view.listScroll, view.panelFocus, view.profileCache, view.metadata];
const context = { notify: () => notices++ };
const deliver = (sequence, source) => {
  apply.call(view, sequence, prepareTheme(source), context);
  assert.equal(view.query, "north");
  assert.equal(view.selected, "north");
  assert.equal(view.highlighted, "south");
  assert.equal(view.confirmed, fixture.scenes[0].connected);
  [view.listScroll, view.panelFocus, view.profileCache, view.metadata].forEach((owner, index) => assert.strictEqual(owner, owners[index]));
  assert.strictEqual(view.profileCache.get("north", true, "all", view.metadata), rows);
};
deliver(1, DARK);
deliver(2, LIGHT);
deliver(3, BROKEN);
assert.equal(view.themeFallback, true);
assert.equal(paints, 2); // Good dark startup, whole light, whole default: no mixed colors.
deliver(4, DARK);
const previousPaints = paints, previousNotices = notices;
deliver(5, DARK + "\n# unchanged colors\n");
assert.equal(paints, previousPaints);
assert.equal(notices, previousNotices);
deliver(3, LIGHT);
assert.equal(paints, previousPaints); // Stale delivery does not repaint.

// Source-fixture replacement, NOT a watcher test. Sole test writer mutates only
// this new checked public tree, then samples a bounded regular file.
let parent = os.homedir();
for (const part of [null, ".cache", "ovt"]) {
  if (part) { parent = path.join(parent, part); if (!fs.existsSync(parent)) fs.mkdirSync(parent, { mode: 0o700 }); }
  const st = fs.lstatSync(parent);
  assert.equal(st.isDirectory(), true);
  assert.equal(st.uid, process.getuid());
  assert.equal(st.mode & 0o022, 0);
  if (part === "ovt") assert.equal(st.mode & 0o777, 0o700);
}
const root = fs.mkdtempSync(path.join(parent, "g1t-"));
try {
  for (const [name, content] of [["dark", DARK], ["light", LIGHT]]) {
    fs.mkdirSync(path.join(root, name), { mode: 0o700 });
    fs.writeFileSync(path.join(root, name, "colors.toml"), content, { mode: 0o600 });
  }
  fs.mkdirSync(path.join(root, "current"), { mode: 0o700 });
  const current = path.join(root, "current", "theme");
  fs.symlinkSync("../dark", current);
  const sample = () => {
    let file;
    try { file = fs.realpathSync(path.join(current, "colors.toml")); }
    catch (error) { if (error.code === "ENOENT") return null; throw error; }
    assert.equal(file.startsWith(root + path.sep), true);
    const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
    try {
      assert.equal(fs.fstatSync(fd).isFile(), true);
      const bytes = Buffer.alloc(MAX_THEME_BYTES + 1);
      const count = fs.readSync(fd, bytes, 0, bytes.length, 0);
      return bytes.subarray(0, count).toString("utf8");
    } finally { fs.closeSync(fd); }
  };
  deliver(6, sample());
  fs.symlinkSync("../light", path.join(root, "current", "next"));
  fs.renameSync(path.join(root, "current", "next"), current);
  deliver(7, sample());
  assert.equal(view.themeUpdates.current.palette.mode, "light");
  fs.renameSync(path.join(root, "current"), path.join(root, "old-current"));
  fs.mkdirSync(path.join(root, "current"), { mode: 0o700 });
  fs.mkdirSync(current, { mode: 0o700 });
  const file = path.join(current, "colors.toml");
  fs.writeFileSync(file, BROKEN, { mode: 0o600 });
  deliver(8, sample());
  assert.strictEqual(view.themeUpdates.current.palette, DEFAULT_PALETTE);
  fs.unlinkSync(file);
  deliver(9, sample());
  assert.equal(view.themeUpdates.current.status, "missing");
  fs.writeFileSync(file, inputs.oversize, { mode: 0o600 });
  deliver(10, sample());
  assert.equal(view.themeUpdates.current.status, "oversize");
  fs.writeFileSync(file, LIGHT, { mode: 0o600 });
  deliver(11, sample());
  assert.equal(view.themeUpdates.current.palette.mode, "light");
} finally { fs.rmSync(root, { recursive: true }); }
console.log("PASS: shared bounded theme policy, latest-slot gates, actual Shell consumer/owner continuity, owned source replacements (watch/render UNRUN)");
