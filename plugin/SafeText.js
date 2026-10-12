// SPDX-License-Identifier: MIT
// Presentation bounds only; multiline is opt-in for explicit import, never a
// domain parser. Existing name/search/URL fields remain single-line.
function valid(value, maximum, multiline) {
  var forbidden = multiline === true ? /[\u0000-\u0009\u000b\u000c\u000e-\u001f\u007f\u2028\u2029]/ : /[\u0000-\u001f\u007f\u2028\u2029]/
  if (typeof value !== "string" || value.length > maximum || forbidden.test(value)) return false
  try { return encodeURIComponent(value).replace(/%[0-9A-F]{2}|./g, "x").length <= 32768 }
  catch (_) { return false }
}
function insertion(current, value, start, end, maximum, multiline) {
  if (!valid(current, maximum, multiline) || !valid(value, maximum, multiline)
      || !Number.isInteger(start) || !Number.isInteger(end)
      || start < 0 || end < start || end > current.length) return null
  var result = current.substring(0, start) + value + current.substring(end)
  return valid(result, maximum, multiline) ? {text:result, cursor:start + value.length} : null
}
