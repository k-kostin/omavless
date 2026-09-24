// Shared semantic keys must not silently diverge during the isolated T2 trial.
const assert = require("assert");
const catalog = require("../crates/omavless-tui/locales.json");
const qml = require("../plugin/I18n.js");
// This action also targets an empty feed directly; no selected profile exists.
assert.strictEqual(catalog["tui.refresh_subscription"].en, "Refresh subscription");
assert.strictEqual(catalog["tui.refresh_subscription"].ru, "Обновить подписку");
for (const [key,entry] of Object.entries(catalog)) {
  for (const locale of ["en","ru"]) {
    assert(typeof entry[locale] === "string" && entry[locale].length > 0 && entry[locale].length <= 200);
    assert(!/[\x00-\x1f\x7f\u202a-\u202e\u2066-\u2069]/.test(entry[locale]));
    if (!key.startsWith("tui.")) assert.strictEqual(entry[locale],qml.translate(key,locale),`${key}:${locale}`);
  }
}
console.log(`TUI catalog: ${Object.keys(catalog).length} bounded EN/RU keys; shared QML keys match`);
