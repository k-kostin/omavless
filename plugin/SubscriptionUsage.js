// SPDX-License-Identifier: MIT
// Transient private provider assertions. Never copy into status/support/settings.
function object(value, fields) {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    && Object.keys(value).length === fields.length
    && Object.keys(value).every(function(key) { return fields.indexOf(key) >= 0 })
}
function decimal(value, maximum) {
  return typeof value === "string" && /^(0|[1-9][0-9]{0,19})$/.test(value)
    && (value.length < maximum.length || (value.length === maximum.length && value <= maximum))
}
function u64(value) { return decimal(value, "18446744073709551615") }
function compare(left, right) {
  return left.length === right.length ? (left === right ? 0 : left < right ? -1 : 1)
    : left.length < right.length ? -1 : 1
}
function add(left, right) {
  if (!u64(left) || !u64(right)) return null
  var result = "", carry = 0, i = left.length - 1, j = right.length - 1
  while (i >= 0 || j >= 0 || carry) {
    var digit = (i >= 0 ? left.charCodeAt(i--) - 48 : 0) + (j >= 0 ? right.charCodeAt(j--) - 48 : 0) + carry
    result = String(digit % 10) + result
    carry = Math.floor(digit / 10)
  }
  return u64(result) ? result : null
}
function remaining(total, used) {
  if (!u64(total) || !u64(used) || total === "0" || compare(total, used) < 0) return null
  var result = "", borrow = 0, j = used.length - 1
  for (var i = total.length - 1; i >= 0; i--) {
    var digit = total.charCodeAt(i) - 48 - borrow - (j >= 0 ? used.charCodeAt(j--) - 48 : 0)
    borrow = digit < 0 ? 1 : 0
    result = String((digit + 10) % 10) + result
  }
  return result.replace(/^0+(?=[0-9])/, "")
}
function utcDate(seconds) {
  if (!decimal(seconds, "253402300799") || seconds === "0") return null
  var date = new Date(Number(seconds) * 1000)
  if (!isFinite(date.getTime())) return null
  return date.toISOString().slice(0, 16).replace("T", " ") + " UTC"
}
function bytes(value, locale) {
  if (!u64(value)) return null
  var whole = value, unit = 0, remainder = 0, approximate = false
  while (compare(whole, "1024") >= 0 && unit < 6) {
    var quotient = "", carry = 0
    for (var i = 0; i < whole.length; i++) {
      var digit = carry * 10 + whole.charCodeAt(i) - 48
      quotient += String(Math.floor(digit / 1024))
      carry = digit % 1024
    }
    remainder = carry
    approximate = approximate || carry !== 0
    whole = quotient.replace(/^0+(?=[0-9])/, "")
    unit++
  }
  var fraction = Math.floor(remainder * 100 / 1024)
  var valueText = whole + (unit === 0 ? "" : (locale === "ru" ? "," : ".") + (fraction < 10 ? "0" : "") + String(fraction))
  var units = locale === "ru" ? ["Б", "КиБ", "МиБ", "ГиБ", "ТиБ", "ПиБ", "ЭиБ"] : ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"]
  return (approximate ? "≈ " : "") + valueText + " " + units[unit]
}
function parse(raw, context) {
  try {
    if (!context || typeof raw !== "string" || raw.length > 4096) return null
    var p = JSON.parse(raw), r = p.result
    if (!object(p, ["api", "version", "id", "ok", "revision", "result"])
        || p.api !== "omavless.control" || p.version !== 1 || p.ok !== true
        || typeof p.id !== "string" || !/^[\x21-\x7e]{1,64}$/.test(p.id)
        || !Number.isSafeInteger(p.revision) || p.revision < 0 || p.revision !== context.revision
        || !object(r, ["schemaVersion", "scope", "instanceId", "availability", "usage"])
        || r.schemaVersion !== 1 || r.scope !== "private_provider_reported_usage"
        || r.instanceId !== context.instanceId) return null
    if (r.availability === "not_provided" && r.usage === null) return {status:"notProvided"}
    var u = r.usage
    if (r.availability !== "reported"
        || !object(u, ["uploadBytes", "downloadBytes", "totalBytes", "expiryUnixSeconds"])
        || !u64(u.uploadBytes) || !u64(u.downloadBytes) || !u64(u.totalBytes)
        || (u.expiryUnixSeconds !== null && !decimal(u.expiryUnixSeconds, "253402300799"))) return null
    var used = add(u.uploadBytes, u.downloadBytes)
    return {status:"reported", upload:u.uploadBytes, download:u.downloadBytes,
      total:u.totalBytes === "0" ? null : u.totalBytes, used:used,
      remaining:used === null ? null : remaining(u.totalBytes, used),
      expiry:u.expiryUnixSeconds === null ? null : utcDate(u.expiryUnixSeconds)}
  } catch (_) { return null }
}
