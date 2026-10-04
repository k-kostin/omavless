// SPDX-License-Identifier: MIT
// Presentation of validated native metadata. No mutation or lifecycle ownership.
function project(snapshot, observation, failed, pending, outcomeUnknown) {
  var result = {state:"unavailable", connected:false, activeId:"", mode:"rule",
    modeConfirmed:false, profiles:[], subscriptions:[], lastProfileId:""}
  if (!snapshot) return result
  result.profiles = snapshot.profiles.slice(0, 256)
  result.subscriptions = snapshot.subscriptions.slice(0, 64)
  result.lastProfileId = snapshot.lastProfileId
  result.mode = snapshot.desired.mode
  if (failed || !observation || observation.availability !== "observed"
      || observation.instanceId !== snapshot.instanceId
      || observation.revision !== snapshot.revision
      || observation.lastKnownActual !== snapshot.lastKnownActual
      || observation.desired.generation !== snapshot.desired.generation
      || observation.desired.connected !== snapshot.desired.connected
      || observation.desired.mode !== snapshot.desired.mode) return result
  var facts = observation.facts
  if (!facts) return result
  // The visible inventory stays truthful: only the explicitly attributed
  // disposable no-TUN child is separate from the one requested tunnel core.
  var auxiliary = facts.ownedAuxiliaryMihomoCount === undefined ? 0 : facts.ownedAuxiliaryMihomoCount
  if ([0, 1].indexOf(auxiliary) < 0 || auxiliary > facts.visibleMihomoCount) return result
  var tunnelCores = facts.visibleMihomoCount - auxiliary
  // New runtimes retain whole-host facts but scope lifecycle to configured
  // device names. Old runtime responses keep their conservative total check.
  var tunnelCount = facts.managedTunCount === undefined ? facts.visibleTunCount : facts.managedTunCount
  if (typeof tunnelCount !== "number" || !isFinite(tunnelCount)
      || Math.floor(tunnelCount) !== tunnelCount || tunnelCount < 0
      || tunnelCount > facts.visibleTunCount) return result
  result.state = snapshot.lastKnownActual
  if (result.state === "connected") {
    result.connected = snapshot.desired.connected && facts.ownedCoreRunning
      && facts.desiredProfileMatchesOwned && facts.ownedControllerConfigVerified
      && tunnelCores === 1 && tunnelCount === 1
    if (result.connected) result.activeId = snapshot.desired.profileId
    else result.state = "unavailable"
  } else if (result.state === "disconnected"
      && (snapshot.desired.connected || facts.ownedCoreRunning
          || tunnelCores !== 0 || tunnelCount !== 0)) {
    result.state = "unavailable"
  }
  // Desired mode remains available for an explicit future action, but must not
  // look applied during a mutation, recovery, stale read or unknown outcome.
  // Confirmation is local ownership/configuration evidence, NOT DNS/route or
  // internet verification; disconnected confirmation is a saved preference.
  result.modeConfirmed = !pending && outcomeUnknown !== true
    && ((result.state === "connected" && result.connected) || result.state === "disconnected")
  return result
}

// Selection belongs to the editor/cursor, never to connection identity.
function activeProfile(view) {
  if (!view || !view.connected || view.state !== "connected" || !view.activeId) return null
  return view.profiles.find(function(profile) { return profile.id === view.activeId }) || null
}

function filtered(profiles, query) {
  var needle = typeof query === "string" ? query.slice(0,128).toLowerCase() : ""
  return (profiles || []).slice(0,256).filter(function(p) {
    return p.name.toLowerCase().indexOf(needle) !== -1
  }).sort(function(a,b) {
    return Number(b.favorite) - Number(a.favorite)
  })
}

// Public shell IPC reads only already-parsed UI caches. Never start a helper,
// copy private rows/identifiers, or turn a cached observation into live proof.
function ipc(snapshot, observation, failed, pending, outcomeUnknown) {
  function count(value, maximum) {
    return typeof value === "number" && isFinite(value) && Math.floor(value) === value
      && value >= 0 && value <= maximum
  }
  var modes = ["rule", "global", "direct"]
  var states = ["disconnected", "starting", "connected", "reconnecting", "stopping", "failed", "manualRecoveryRequired"]
  var metadata = !!(snapshot && !failed && snapshot.desired
    && typeof snapshot.desired.connected === "boolean"
    && modes.indexOf(snapshot.desired.mode) >= 0
    && states.indexOf(snapshot.lastKnownActual) >= 0
    && Array.isArray(snapshot.profiles) && snapshot.profiles.length <= 256
    && Array.isArray(snapshot.subscriptions) && snapshot.subscriptions.length <= 64)
  var busy = pending !== null && pending !== undefined && pending !== false
  var unknown = outcomeUnknown === true
  var coherent = !!(metadata && observation && observation.desired
    && observation.availability === "observed"
    && observation.instanceId === snapshot.instanceId
    && observation.revision === snapshot.revision
    && observation.lastKnownActual === snapshot.lastKnownActual
    && observation.desired.generation === snapshot.desired.generation
    && observation.desired.connected === snapshot.desired.connected
    && observation.desired.mode === snapshot.desired.mode)
  var facts = coherent ? observation.facts : null
  var auxiliary = facts && facts.ownedAuxiliaryMihomoCount !== undefined ? facts.ownedAuxiliaryMihomoCount : 0
  var observed = !!(facts && !busy && !unknown
    && typeof facts.ownedCoreRunning === "boolean"
    && typeof facts.desiredProfileMatchesOwned === "boolean"
    && typeof facts.ownedControllerConfigVerified === "boolean"
    && count(facts.visibleMihomoCount, 64) && count(facts.visibleTunCount, 8)
    && count(auxiliary, 1) && auxiliary <= facts.visibleMihomoCount)
  var state = "unavailable"
  if (observed) {
    try { state = project(snapshot, observation, false).state } catch (_) { observed = false }
    if (states.indexOf(state) < 0) state = "unavailable"
  }
  var manual = metadata && snapshot.lastKnownActual === "manualRecoveryRequired"
  if (manual) state = "manualRecoveryRequired"
  else if (unknown) state = "outcomeUnknown"
  else if (busy) state = "pending"
  var mode = metadata ? snapshot.desired.mode : "unavailable"
  var d = {schemaVersion:1, scope:"cached_native_ipc", metadataAvailable:metadata,
    observationAvailable:observed, state:state, desiredMode:mode,
    desiredState:metadata ? snapshot.desired.connected ? "connected" : "disconnected" : "unavailable",
    pending:busy, outcomeUnknown:unknown, manualRecoveryRequired:manual,
    profiles:metadata ? snapshot.profiles.length : null,
    subscriptions:metadata ? snapshot.subscriptions.length : null,
    visibleMihomoCount:observed ? facts.visibleMihomoCount : null,
    ownedAuxiliaryMihomoCount:observed ? auxiliary : null,
    visibleTunCount:observed ? facts.visibleTunCount : null,
    ownedCoreRunning:observed ? facts.ownedCoreRunning : null,
    controllerConfigVerified:observed ? facts.ownedControllerConfigVerified : null,
    liveHealthVerified:false, routesVerified:false, dnsVerified:false, internetVerified:false}
  var labels = {disconnected:"disconnected", starting:"starting", connected:"connected",
    reconnecting:"reconnecting", stopping:"stopping", failed:"failed",
    manualRecoveryRequired:"manual recovery required", outcomeUnknown:"action outcome unknown",
    pending:"action pending", unavailable:"unavailable"}
  var modeLabels = {rule:"Routing", global:"Full VPN", direct:"Direct", unavailable:"unavailable"}
  return {status:"VPN cached state: " + labels[state] + " (not live health verification)",
    routing:"Cached desired mode: " + modeLabels[mode] + "; effective routes not verified",
    details:(observed ? "Cached local observation: Mihomo " + d.visibleMihomoCount
      + " (auxiliary " + d.ownedAuxiliaryMihomoCount + "), TUN " + d.visibleTunCount
      : "Cached local observation unavailable") + "; DNS and internet not verified",
    diagnostics:d}
}
