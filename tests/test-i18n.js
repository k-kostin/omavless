const assert = require("assert")
const I18n = require("../I18n.js")

assert.strictEqual(I18n.normalizeLocale("ru_RU.UTF-8"), "ru")
assert.strictEqual(I18n.normalizeLocale("ru-RU"), "ru")
assert.strictEqual(I18n.normalizeLocale("en_US"), "en")
assert.strictEqual(I18n.normalizeLocale("de_DE"), "en")
assert.strictEqual(I18n.normalizeLocale(""), "en")

assert.strictEqual(I18n.translate("common.cancel", "en"), "Cancel")
assert.strictEqual(I18n.translate("common.cancel", "ru"), "Отмена")
assert.strictEqual(I18n.plural("provider", 1, "ru"), "1 провайдер")
assert.strictEqual(I18n.plural("provider", 2, "ru"), "2 провайдера")
assert.strictEqual(I18n.plural("provider", 5, "ru"), "5 провайдеров")
assert.strictEqual(I18n.plural("provider", 11, "ru"), "11 провайдеров")
assert.strictEqual(I18n.plural("provider", 21, "ru"), "21 провайдер")
assert.strictEqual(I18n.plural("provider", 2, "en"), "2 providers")
assert.strictEqual(I18n.plural("server", 22, "ru"), "22 сервера")
assert.strictEqual(I18n.plural("server", 55, "ru"), "55 серверов")
assert.strictEqual(I18n.plural("rule", 21, "ru"), "21 правило")
assert.strictEqual(I18n.plural("rule_set", 22, "ru"), "22 набора правил")
assert.strictEqual(I18n.plural("custom_rule", 25, "ru"), "25 пользовательских правил")
assert.strictEqual(I18n.translate("language.system", "ru"), "Системный")
assert.strictEqual(I18n.translate("settings.language", "ru"), "Язык")
assert.strictEqual(I18n.translate("routing.summary.global", "ru"),
  "Весь трафик идёт через VPN · наборы правил не используются")
assert.strictEqual(I18n.translate("onboarding.title", "ru"), "НАСТРОЙКА OMAVLESS")
assert.strictEqual(I18n.translate("startup_prompt.autoconnect", "ru"), "Автоподключение")
assert.strictEqual(I18n.translate("routing_tools.title", "ru"),
  "ИНСТРУМЕНТЫ МАРШРУТИЗАЦИИ")
assert.strictEqual(I18n.translate("routing_preset.recommended", "ru"), "Рекомендуется")
assert.strictEqual(I18n.plural("connection", 1, "ru"), "1 подключение")
assert.strictEqual(I18n.plural("connection", 3, "ru"), "3 подключения")
assert.strictEqual(I18n.plural("connection", 12, "ru"), "12 подключений")
assert.strictEqual(
  I18n.translate("missing.key", "ru"),
  "Missing translation: missing.key"
)

const providerText = "<b>provider-owned</b>"
const interpolated = I18n.translate("profile.showing", "ru", {name: providerText})
assert.ok(interpolated.includes(providerText))
assert.ok(!Object.prototype.hasOwnProperty.call(I18n.CATALOG, providerText))

const controlled = I18n.translate("profiles.no_match", "ru", {
  query: "unsafe\u0000value"
})
assert.ok(!controlled.includes("\u0000"))
assert.ok(controlled.includes("unsafe value"))

const bounded = I18n.translate("profile.showing", "en", {name: "x".repeat(1000)})
assert.ok(bounded.length <= I18n.MAX_TEXT_LENGTH)
assert.ok(!bounded.includes("x".repeat(I18n.MAX_VALUE_LENGTH + 1)))

const routeTarget = "MATCH·provider-controlled.example"
const localizedRoute = I18n.translate("routing_tools.result.route", "ru", {
  rule: "DOMAIN-SUFFIX",
  target: routeTarget
})
assert.ok(localizedRoute.includes("DOMAIN-SUFFIX"))
assert.ok(localizedRoute.includes(routeTarget))
assert.ok(!Object.prototype.hasOwnProperty.call(I18n.CATALOG, routeTarget))

for (const [key, value] of Object.entries(I18n.CATALOG)) {
  assert.ok(key.length > 0 && key.length <= 96)
  assert.strictEqual(typeof value.en, "string")
  assert.ok(value.en.length > 0)
  assert.ok(value.en.length <= I18n.MAX_TEXT_LENGTH)
  if (value.ru !== undefined) {
    assert.strictEqual(typeof value.ru, "string")
    assert.ok(value.ru.length > 0)
    assert.ok(value.ru.length <= I18n.MAX_TEXT_LENGTH)
  }
}

const protocolErrors = [
  "invalid_request", "unsupported_version", "unknown_method",
  "invalid_argument", "not_found", "conflict", "busy",
  "capability_unavailable", "permission_denied", "core_rejected",
  "transition_failed_restored", "manual_recovery_required",
  "daemon_restarting", "internal_error"
]
for (const code of protocolErrors) {
  assert.ok(I18n.CATALOG["error." + code])
  assert.ok(I18n.translate("error." + code, "ru").length > 0)
}

console.log("i18n contracts: ok")
