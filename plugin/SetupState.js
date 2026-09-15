// SPDX-License-Identifier: MIT
// A bootstrap response contains only one fixed word, never a native snapshot.
function parse(raw, exitCode) {
  if (exitCode !== 0 || typeof raw !== "string" || raw.length > 64) return "needs_attention"
  var value = raw.replace(/\n$/, "")
  return ["ready", "needs_package", "needs_activation", "needs_attention", "release_unavailable"].indexOf(value) >= 0
    ? value : "needs_attention"
}
function canInstall(state) { return state === "needs_package" || state === "needs_activation" }
function inventory(raw, exitCode) {
  if (exitCode !== 0 || typeof raw !== "string" || raw.length > 96) return {state:"needs_attention", coreInstalled:null}
  var match = /^(ready|needs_package|needs_activation|needs_attention|release_unavailable)\t(present|missing)\n?$/.exec(raw)
  return match && match[0].length === raw.length ? {state:match[1], coreInstalled:match[2] === "present"} : {state:"needs_attention", coreInstalled:null}
}
function appMissing(facts) { return facts.state === "needs_package" || facts.state === "release_unavailable" }
function missingAction(facts) {
  if (appMissing(facts)) return facts.state === "needs_package" ? "install" : ""
  return facts.coreInstalled === false && ["ready", "needs_activation"].indexOf(facts.state) >= 0 ? "install-core" : ""
}
function needsAttention(facts) { return facts.state !== "ready" || facts.coreInstalled !== true }
if (typeof module !== "undefined") module.exports = {parse:parse, canInstall:canInstall, inventory:inventory, appMissing:appMissing, missingAction:missingAction, needsAttention:needsAttention}
