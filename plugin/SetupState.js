// SPDX-License-Identifier: MIT
// A bootstrap response contains only one fixed word, never a native snapshot.
function parse(raw, exitCode) {
  if (exitCode !== 0 || typeof raw !== "string" || raw.length > 64) return "needs_attention"
  var value = raw.replace(/\n$/, "")
  return ["ready", "needs_package", "needs_activation", "needs_companion", "needs_selection", "needs_broker", "needs_broker_stopped", "needs_runtime_stop", "needs_runtime_start", "needs_attention", "release_unavailable"].indexOf(value) >= 0
    ? value : "needs_attention"
}
function canInstall(state) { return state === "needs_package" || state === "needs_activation" || state === "needs_selection" || state === "needs_broker_stopped" }
function inventory(raw, exitCode) {
  if (exitCode !== 0 || typeof raw !== "string" || raw.length > 96) return {state:"needs_attention", coreInstalled:null}
  var match = /^(ready|needs_package|needs_activation|needs_companion|needs_selection|needs_broker|needs_broker_stopped|needs_runtime_stop|needs_runtime_start|needs_attention|release_unavailable)\t(present|missing)\n?$/.exec(raw)
  return match && match[0].length === raw.length ? {state:match[1], coreInstalled:match[2] === "present"} : {state:"needs_attention", coreInstalled:null}
}
function appMissing(facts) { return facts.state === "needs_package" || facts.state === "release_unavailable" }
function missingAction(facts) {
  if (appMissing(facts)) return facts.state === "needs_package" ? "install" : ""
  if (facts.coreInstalled !== true) return ""
  if (facts.state === "needs_runtime_start") return "start-app"
  if (facts.state === "needs_activation") return "install"
  if (facts.state === "needs_broker_stopped") return "start-broker"
  return facts.state === "needs_selection" ? "finish-selection" : ""
}
function canRunAction(facts, action) {
  return action !== "" && (action === missingAction(facts)
    || (facts.state === "needs_broker_stopped" && facts.coreInstalled === true && action === "restore-enrollment"))
}
function needsAttention(facts) { return facts.state !== "ready" || facts.coreInstalled !== true }
function bootstrapRequired(facts, nativeOwner, hasSnapshot, actionRunning, pending, outcomeUnknown) {
  if (facts.state === "ready") return false
  var stopped = (facts.state === "needs_runtime_start" || facts.state === "needs_broker_stopped")
    && !actionRunning && !pending && !outcomeUnknown
  return appMissing(facts) || !nativeOwner || !hasSnapshot || stopped
}
if (typeof module !== "undefined") module.exports = {parse:parse, canInstall:canInstall, inventory:inventory, appMissing:appMissing, missingAction:missingAction, canRunAction:canRunAction, needsAttention:needsAttention, bootstrapRequired:bootstrapRequired}
