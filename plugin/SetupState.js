// SPDX-License-Identifier: MIT
// A bootstrap response contains only one fixed word, never a native snapshot.
function parse(raw, exitCode) {
  if (exitCode !== 0 || typeof raw !== "string" || raw.length > 64) return "needs_attention"
  var value = raw.replace(/\n$/, "")
  return ["ready", "needs_package", "needs_update", "needs_pair_update", "needs_pair_update_manual", "needs_pair_install", "needs_pair_mismatch", "needs_activation", "needs_companion", "needs_selection", "needs_broker", "needs_dns_setup", "needs_broker_stopped", "needs_runtime_stop", "needs_runtime_start", "needs_attention", "release_unavailable"].indexOf(value) >= 0
    ? value : "needs_attention"
}
function canInstall(state) { return state === "needs_update" || state === "needs_pair_update" || state === "needs_pair_install" || state === "needs_package" || state === "needs_activation" || state === "needs_selection" || state === "needs_broker_stopped" || state === "needs_dns_setup" }
function inventory(raw, exitCode) {
  if (exitCode !== 0 || typeof raw !== "string" || raw.length > 128) return {state:"needs_attention", coreInstalled:null}
  var match = /^(ready|needs_package|needs_update|needs_pair_update|needs_pair_update_manual|needs_pair_install|needs_pair_mismatch|needs_activation|needs_companion|needs_selection|needs_broker|needs_dns_setup|needs_broker_stopped|needs_runtime_stop|needs_runtime_start|needs_attention|release_unavailable)\t(present|missing)(?:\t(current|stale|stale_manual))?\n?$/.exec(raw)
  if (!match || match[0].length !== raw.length) return {state:"needs_attention", coreInstalled:null}
  var facts = {state:match[1], coreInstalled:match[2] === "present"}
  if (match[3]) facts.delivery = match[3]
  return facts
}
function hasUpdate(facts) { return facts.delivery === "stale" || facts.delivery === "stale_manual" }
function updateMessage(facts) { return hasUpdate(facts) ? (facts.delivery === "stale" ? "needs_pair_update" : "needs_pair_update_manual") : facts.state }
function canOfferPairUpdate(facts) {
  return facts.delivery === "stale" && facts.coreInstalled === true
    && ["ready", "needs_broker_stopped", "needs_runtime_start", "needs_selection", "needs_dns_setup", "needs_runtime_stop"].indexOf(facts.state) >= 0
}
function appMissing(facts) { return facts.state === "needs_package" || facts.state === "release_unavailable" }
function missingAction(facts) {
  if (appMissing(facts)) return facts.state === "needs_package" ? "install" : ""
  if (facts.state === "needs_update") return "upgrade"
  if (facts.state === "needs_pair_update" || facts.state === "needs_pair_install") return "update-pair"
  if (facts.state === "needs_pair_update_manual" || facts.state === "needs_pair_mismatch" || facts.state === "needs_companion") return "update-help"
  if (facts.coreInstalled !== true) return ""
  if (facts.state === "needs_dns_setup") return "resume-dns"
  if (facts.state === "needs_runtime_start") return "start-app"
  if (facts.state === "needs_activation") return "install"
  if (facts.state === "needs_broker_stopped") return "start-broker"
  return facts.state === "needs_selection" ? "finish-selection" : ""
}
function canRunAction(facts, action) {
  return action !== "" && (action === missingAction(facts)
    || (canOfferPairUpdate(facts) && action === "update-pair")
    || (hasUpdate(facts) && action === "update-help")
    || (["needs_update", "needs_pair_update", "needs_pair_install"].indexOf(facts.state) >= 0 && action === "update-help")
    || (facts.state === "needs_broker_stopped" && facts.coreInstalled === true && action === "restore-enrollment"))
}
function needsAttention(facts) { return facts.state !== "ready" || facts.coreInstalled !== true || hasUpdate(facts) }
// Presentation only: desktop permissions are not service/TUN/network health.
function coreReview(core, facts, busy, failed, terminalOpened) {
  if (terminalOpened) return "terminal"
  if (busy || facts.state === "checking") return "checking"
  if (!failed && core === null && facts.state === "ready") return "checking"
  // Explicit incompatible application states require an update; advisory
  // delivery freshness must not override confirmed compatible readiness.
  if (["needs_update", "needs_pair_update", "needs_pair_update_manual"].indexOf(facts.state) >= 0) return "update"
  if (appMissing(facts) || facts.coreInstalled === false || (core && core.installed === false)) return "missing"
  if (!failed && facts.state === "ready" && facts.coreInstalled === true && core
      && core.installed === true && typeof core.version === "string" && core.version !== ""
      && core.tunDevice === "present" && core.fileNetworkCapabilities === "present") return "ready"
  if (hasUpdate(facts)) return "update"
  return "attention"
}
function bootstrapRequired(facts, nativeOwner, hasSnapshot, actionRunning, pending, outcomeUnknown) {
  if (facts.state === "ready") return false
  var stopped = (facts.state === "needs_runtime_start" || facts.state === "needs_broker_stopped" || facts.state === "needs_dns_setup")
    && !actionRunning && !pending && !outcomeUnknown
  return appMissing(facts) || !nativeOwner || !hasSnapshot || stopped
}
if (typeof module !== "undefined") module.exports = {parse:parse, canInstall:canInstall, inventory:inventory, hasUpdate:hasUpdate, updateMessage:updateMessage, canOfferPairUpdate:canOfferPairUpdate, appMissing:appMissing, missingAction:missingAction, canRunAction:canRunAction, needsAttention:needsAttention, coreReview:coreReview, bootstrapRequired:bootstrapRequired}
