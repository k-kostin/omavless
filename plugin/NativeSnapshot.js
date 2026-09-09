// SPDX-License-Identifier: MIT
// Pure parser: returns a fresh projection or null; never changes UI state.
function object(value, keys) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false
  var found = Object.keys(value)
  return found.length === keys.length && found.every(function(k) { return keys.indexOf(k) >= 0 })
}
function text(value, max, empty) {
  if (typeof value !== "string" || (!empty && !value.length) || /[\x00-\x1f\x7f]/.test(value)) return false
  // Domain bounds count Unicode scalar values, not UTF-16 code units.
  var count = 0
  for (var i = 0; i < value.length; i++, count++) {
    var c = value.charCodeAt(i)
    if (c >= 0xd800 && c <= 0xdbff) {
      var next = value.charCodeAt(++i)
      if (!(next >= 0xdc00 && next <= 0xdfff)) return false
    } else if (c >= 0xdc00 && c <= 0xdfff) return false
  }
  return count <= max
}
function number(value, max) {
  return typeof value === "number" && isFinite(value) && value >= 0 && Math.floor(value) === value && value <= max
}
function id(value, empty) {
  return text(value, 64, empty) && /^[\x21-\x7e]*$/.test(value)
}
function parse(raw, previous) {
  try {
    if (typeof raw !== "string" || raw.length > 262144 || unescape(encodeURIComponent(raw)).length > 262144) return null
    var p = JSON.parse(raw)
    if (!object(p, ["api", "version", "id", "ok", "revision", "result"])
        || p.api !== "omavless.control" || p.version !== 1 || p.ok !== true
        || !id(p.id, false) || !number(p.revision, 9007199254740991)) return null
    var r = p.result
    if (!object(r, ["schemaVersion", "scope", "desired", "lastKnownActual", "healthFresh", "liveHealth", "profiles", "lastProfileId", "subscriptions", "startup", "onboardingComplete", "routing", "instanceId", "transition"])
        || r.schemaVersion !== 1 || r.scope !== "private_ui_metadata"
        || !id(r.instanceId, false) || r.transition !== null
        || r.healthFresh !== false || r.liveHealth !== "unavailable"
        || ["disconnected", "starting", "connected", "reconnecting", "stopping", "failed", "manualRecoveryRequired"].indexOf(r.lastKnownActual) < 0
        || typeof r.onboardingComplete !== "boolean") return null
    if (previous && previous.instanceId === r.instanceId && p.revision < previous.revision) return null
    var d = r.desired
    if (!object(d, ["connected", "profileId", "mode", "generation"])
        || typeof d.connected !== "boolean" || !text(d.profileId, 64, !d.connected)
        || !/^[\x21-\x7e]*$/.test(d.profileId) || (!d.connected && d.profileId !== "")
        || ["rule", "global", "direct"].indexOf(d.mode) < 0 || !number(d.generation, 9007199254740991)) return null
    var s = r.startup
    if (!object(s, ["configured", "enabled", "target", "profileId", "mode"])
        || typeof s.configured !== "boolean" || typeof s.enabled !== "boolean"
        || ["last", "profile"].indexOf(s.target) < 0 || !id(s.profileId, true)
        || ["rule", "global"].indexOf(s.mode) < 0) return null
    if (!object(r.routing, ["storedPreset", "customRuleCount"])
        || !text(r.routing.storedPreset, 64, true) || !number(r.routing.customRuleCount, 128)
        || !Array.isArray(r.profiles) || r.profiles.length > 256
        || !Array.isArray(r.subscriptions) || r.subscriptions.length > 64 || !id(r.lastProfileId, true)) return null
    var subscriptions = [], profiles = [], seen = Object.create(null), subs = Object.create(null)
    for (var j = 0; j < r.subscriptions.length; j++) {
      var sub = r.subscriptions[j]
      if (!object(sub, ["id", "name", "updatedAt", "profileCount", "staleCount"])
          || !id(sub.id, false) || subs[sub.id] || !text(sub.name, 80, false)
          || !number(sub.updatedAt, 9007199254740991) || !number(sub.profileCount, 256)
          || !number(sub.staleCount, sub.profileCount)) return null
      subs[sub.id] = true
      subscriptions.push({id: sub.id, name: sub.name, profileCount: sub.profileCount, staleCount: sub.staleCount, updatedAt: sub.updatedAt})
    }
    for (var k = 0; k < r.profiles.length; k++) {
      var profile = r.profiles[k]
      if (!object(profile, ["id", "name", "protocol", "subscriptionId", "missing", "favorite"])
          || !id(profile.id, false) || seen[profile.id] || !text(profile.name, 80, false)
          || ["vless", "trojan", "hysteria2", "tuic"].indexOf(profile.protocol) < 0
          || !id(profile.subscriptionId, true) || (profile.subscriptionId && !subs[profile.subscriptionId])
          || typeof profile.missing !== "boolean" || typeof profile.favorite !== "boolean") return null
      seen[profile.id] = true
      profiles.push({id: profile.id, name: profile.name, protocol: profile.protocol, subscriptionId: profile.subscriptionId, missing: profile.missing, favorite: profile.favorite})
    }
    if (r.lastProfileId && !seen[r.lastProfileId]) return null
    return {instanceId: r.instanceId, revision: p.revision, desired: d, lastKnownActual: r.lastKnownActual,
      healthFresh: false, liveHealth: "unavailable", profiles: profiles, subscriptions: subscriptions,
      startup: s, routing: r.routing, onboardingComplete: r.onboardingComplete, lastProfileId: r.lastProfileId}
  } catch (_) { return null }
}

function envelope(raw) {
  if (typeof raw !== "string" || raw.length > 8192 || unescape(encodeURIComponent(raw)).length > 8192) return null
  var p = JSON.parse(raw)
  if (!p || p.api !== "omavless.control" || p.version !== 1 || !id(p.id, false)
      || !number(p.revision, 9007199254740991)) return null
  return p
}

function parseObservation(raw) {
  try {
    var p = envelope(raw)
    if (!object(p, ["api", "version", "id", "ok", "revision", "result"]) || p.ok !== true) return null
    var r = p.result, d = r.desired, f = r.facts
    if (!object(r, ["schemaVersion", "scope", "availability", "desired", "lastKnownActual", "manualRecoveryRequired", "facts", "verification", "instanceId", "transition"])
        || r.schemaVersion !== 1 || r.scope !== "local_runtime_observation" || !id(r.instanceId, false) || r.transition !== null
        || ["observed", "unavailable"].indexOf(r.availability) < 0
        || ["disconnected", "starting", "connected", "reconnecting", "stopping", "failed", "manualRecoveryRequired"].indexOf(r.lastKnownActual) < 0
        || r.manualRecoveryRequired !== (r.lastKnownActual === "manualRecoveryRequired")) return null
    if (!object(d, ["connected", "mode", "generation"]) || typeof d.connected !== "boolean"
        || ["rule", "global", "direct"].indexOf(d.mode) < 0 || !number(d.generation, 9007199254740991)) return null
    if (!object(r.verification, ["serviceOwnership", "tunOwnership", "routes", "dns", "internet"])
        || !Object.keys(r.verification).every(function(k) { return r.verification[k] === false })) return null
    if (r.availability === "unavailable") { if (f !== null) return null }
    else if (!object(f, ["ownedCoreRunning", "visibleMihomoCount", "visibleTunCount", "ownedControllerConfigVerified", "desiredProfileMatchesOwned"])
        || typeof f.ownedCoreRunning !== "boolean" || typeof f.ownedControllerConfigVerified !== "boolean"
        || typeof f.desiredProfileMatchesOwned !== "boolean" || !number(f.visibleMihomoCount, 64) || !number(f.visibleTunCount, 8)
        || (f.desiredProfileMatchesOwned && (!f.ownedCoreRunning || !d.connected))
        || (f.ownedControllerConfigVerified && (!f.ownedCoreRunning || !f.desiredProfileMatchesOwned))) return null
    return {instanceId:r.instanceId, revision:p.revision, desired:d, lastKnownActual:r.lastKnownActual,
      manualRecoveryRequired:r.manualRecoveryRequired, facts:f, availability:r.availability}
  } catch (_) { return null }
}

function coherent(snapshot, observation) {
  return !!(snapshot && observation && snapshot.instanceId === observation.instanceId
    && snapshot.lastKnownActual === observation.lastKnownActual
    && snapshot.revision === observation.revision && snapshot.desired.generation === observation.desired.generation
    && snapshot.desired.connected === observation.desired.connected && snapshot.desired.mode === observation.desired.mode)
}

function parseAction(raw, pending) {
  try {
    var p = envelope(raw)
    if (!p || !pending || !id(pending.instanceId, false) || !id(pending.operationId, false)
        || !number(pending.revision, 9007199254740991) || ["connect", "disconnect", "mode"].indexOf(pending.action) < 0) return null
    if (p.ok === true) {
      var r = p.result
      if (!object(p, ["api", "version", "id", "ok", "revision", "result"])
          || !object(r, ["schemaVersion", "instanceId", "operationId", "action", "applied"])
          || r.schemaVersion !== 1 || r.instanceId !== pending.instanceId || r.operationId !== pending.operationId
          || r.action !== pending.action || r.applied !== true || p.revision < pending.revision) return null
      return {ok:true, revision:p.revision, code:""}
    }
    var e = p.error
    var codes = ["invalid_request", "unsupported_version", "unknown_method", "invalid_argument", "not_found", "conflict", "busy", "permission_denied", "capability_unavailable", "core_unavailable", "core_rejected", "timeout", "cancelled", "daemon_restarting", "internal_error", "manual_recovery_required", "transition_failed_restored"]
    if (p.ok !== false || !object(p, ["api", "version", "id", "ok", "revision", "error"])
        || !object(e, ["code", "message", "retryable"]) || codes.indexOf(e.code) < 0
        || typeof e.retryable !== "boolean" || !text(e.message, 512, false)) return null
    // Never render the backend message, even from an otherwise valid envelope.
    return {ok:false, revision:p.revision, code:e.code}
  } catch (_) { return null }
}
