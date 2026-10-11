// SPDX-License-Identifier: MIT
// Single-line presentation bounds only; domain validation remains in Rust.
function valid(value, maximum) {
  if (typeof value !== "string" || value.length > maximum || /[\u0000-\u001f\u007f\u2028\u2029]/.test(value)) return false
  try { return encodeURIComponent(value).replace(/%[0-9A-F]{2}|./g, "x").length <= 32768 }
  catch (_) { return false }
}
function insertion(current, value, start, end, maximum) {
  if (!valid(current, maximum) || !valid(value, maximum)
      || !Number.isInteger(start) || !Number.isInteger(end)
      || start < 0 || end < start || end > current.length) return null
  var result = current.substring(0, start) + value + current.substring(end)
  return valid(result, maximum) ? {text:result, cursor:start + value.length} : null
}
