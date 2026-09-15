// SPDX-License-Identifier: MIT
// A bootstrap response contains only one fixed word, never a native snapshot.
function parse(raw, exitCode) {
  if (exitCode !== 0 || typeof raw !== "string" || raw.length > 64) return "needs_attention"
  var value = raw.replace(/\n$/, "")
  return ["ready", "needs_package", "needs_activation", "needs_attention", "release_unavailable"].indexOf(value) >= 0
    ? value : "needs_attention"
}
function canInstall(state) { return state === "needs_package" || state === "needs_activation" }
if (typeof module !== "undefined") module.exports = {parse:parse, canInstall:canInstall}
