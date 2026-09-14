// SPDX-License-Identifier: MIT
// Fixed local helper inventory only: no core/TUN readiness or live health.
function desktopCapabilities(raw) {
  try {
    if (typeof raw !== "string" || raw.length > 2048) return null
    // The helper emits a flat ASCII object. Refuse escapes, nesting and duplicate
    // keys before JSON.parse can silently collapse them.
    if (!/^\s*\{\s*"[A-Za-z0-9]+"\s*:\s*(?:true|false|null|1|"[a-z0-9]+")\s*(?:,\s*"[A-Za-z0-9]+"\s*:\s*(?:true|false|null|1|"[a-z0-9]+")\s*)*\}\s*$/.test(raw)) return null
    var names = raw.match(/"[A-Za-z0-9]+"\s*:/g)
    if (!names || names.length !== 7) return null
    var p = JSON.parse(raw)
    if (!object(p, ["schemaVersion", "clipboardReadAvailable", "clipboardWriteAvailable", "filePicker", "configEditorAvailable", "qrEncoderAvailable", "gtk4FallbackAvailable"])
        || p.schemaVersion !== 1 || [null, "zenity", "kdialog", "yad"].indexOf(p.filePicker) < 0
        || typeof p.clipboardReadAvailable !== "boolean" || typeof p.clipboardWriteAvailable !== "boolean"
        || typeof p.configEditorAvailable !== "boolean" || typeof p.qrEncoderAvailable !== "boolean"
        || p.gtk4FallbackAvailable !== false) return null
    return {filePicker:p.filePicker, configEditorAvailable:p.configEditorAvailable,
      qrEncoderAvailable:p.qrEncoderAvailable, clipboardReadAvailable:p.clipboardReadAvailable,
      clipboardWriteAvailable:p.clipboardWriteAvailable}
  } catch (_) { return null }
}
function parseCoreSetupFacts(raw) {
  try {
    if (!editorText(raw, 8192)) return null
    var p = JSON.parse(raw)
    if (!object(p, ["schemaVersion", "scope", "installed", "version", "tunDevice", "fileNetworkCapabilities", "servicePermissionReadiness", "coverage", "remediation"])
        || p.schemaVersion !== 1 || p.scope !== "desktop_setup_facts" || typeof p.installed !== "boolean"
        || !(p.version === null || (typeof p.version === "string" && /^[0-9]{1,8}\.[0-9]{1,8}\.[0-9]{1,8}$/.test(p.version)))
        || ["present", "unavailable", "unknown"].indexOf(p.tunDevice) < 0
        || ["present", "missing", "unknown", "not_applicable"].indexOf(p.fileNetworkCapabilities) < 0
        || p.servicePermissionReadiness !== "not_verified"
        || !object(p.coverage, ["serviceContextVerified", "tunCreationVerified", "controllerQueried"])
        || p.coverage.serviceContextVerified !== false || p.coverage.tunCreationVerified !== false || p.coverage.controllerQueried !== false
        || p.remediation !== (p.installed ? "verify_native_host_setup" : "install_core_using_host_setup")
        || (!p.installed && (p.version !== null || p.fileNetworkCapabilities !== "not_applicable"))
        || (p.installed && p.fileNetworkCapabilities === "not_applicable")) return null
    return {installed:p.installed, version:p.version, tunDevice:p.tunDevice, fileNetworkCapabilities:p.fileNetworkCapabilities}
  } catch (_) { return null }
}

// Explicit private UI observation, never shareable support or tunnel proof.
function connectionTest(raw, context) {
  try {
    if (!context || typeof raw !== "string" || raw.length > 2048) return null
    var p = JSON.parse(raw), r = p.result
    if (!object(p, ["api", "version", "id", "ok", "revision", "result"])
        || p.api !== "omavless.control" || p.version !== 1 || !id(p.id, false)
        || p.ok !== true || p.revision !== context.revision
        || !object(r, ["schemaVersion", "scope", "https", "observedIp", "elapsedMs", "code", "instanceId"])
        || r.instanceId !== context.instanceId || r.schemaVersion !== 1 || r.scope !== "current_route_https"
        || typeof r.https !== "boolean" || !number(r.elapsedMs, 3000)
        || r.code !== (r.https ? "ok" : "request_failed")) return null
    if (r.https) {
      if (typeof r.observedIp !== "string" || r.observedIp.length > 45 || r.observedIp.length < 3 || !/^[0-9a-fA-F:.]+$/.test(r.observedIp)) return null
    } else if (r.observedIp !== null) return null
    return {https:r.https, elapsedMs:r.elapsedMs, observedIp:r.observedIp, code:r.code}
  } catch (_) { return null }
}
function parseProfileDetails(raw, revision) {
  try {
    if (!editorText(raw, 16384)) return null
    var p = JSON.parse(raw)
    if (!object(p, ["api", "version", "id", "ok", "revision", "result"])
        || p.api !== "omavless.control" || p.version !== 1 || p.ok !== true
        || !id(p.id, false) || !number(p.revision, 9007199254740991) || p.revision !== revision
        || !object(p.result, ["version", "name", "protocol", "server", "transport", "security", "sni"])) return null
    var d = p.result
    if (d.version !== 1 || !text(d.name, 80, false) || !editorText(d.name, 320)
        || ["vless", "trojan", "hysteria2", "tuic"].indexOf(d.protocol) < 0
        || !text(d.server, 259, false) || !editorText(d.server, 1018)
        || !text(d.transport, 32, false) || !editorText(d.transport, 32)
        || !text(d.security, 16, false) || !editorText(d.security, 16)
        || !text(d.sni, 253, true) || !editorText(d.sni, 1012)) return null
    return {name:d.name, protocol:d.protocol, server:d.server, transport:d.transport, security:d.security, sni:d.sni}
  } catch (_) { return null }
}
// Independent diagnostic sample: this response has no daemon instance proof
// and must never update connection health or ownership observations.
function parseDiagnosticsSummary(raw) {
  try {
    if (!editorText(raw, 262144) || raw.length === 0) return null
    var p = JSON.parse(raw)
    if (!object(p, ["api", "version", "id", "ok", "revision", "result"])
        || p.api !== "omavless.control" || p.version !== 1 || p.ok !== true
        || !id(p.id, false) || !number(p.revision, 9007199254740991)
        || !object(p.result, ["version", "rules", "providers"]) || p.result.version !== 1) return null
    var r = p.result.rules, s = p.result.providers
    function rows(value, shownMax, totalMax) {
      return object(value, ["total", "shown", "truncated", "items"])
        && Array.isArray(value.items) && value.items.length <= shownMax
        && number(value.total, totalMax) && value.total >= value.items.length
        && value.shown === value.items.length
        && typeof value.truncated === "boolean" && value.truncated === (value.shown < value.total)
    }
    function field(value, max) { return text(value, max, true) && editorText(value, max) }
    if (!rows(r, 2048, 65536) || !rows(s, 256, 256)) return null
    var rules = [], providers = []
    for (var i = 0; i < r.items.length; i++) {
      var rule = r.items[i]
      if (!object(rule, ["type", "payload", "target"]) || !field(rule.type, 80)
          || !field(rule.payload, 512) || ["VPN", "DIRECT", "REJECT"].indexOf(rule.target) < 0) return null
      rules.push({type:rule.type, payload:rule.payload, target:rule.target})
    }
    for (var j = 0; j < s.items.length; j++) {
      var provider = s.items[j]
      if (!object(provider, ["name", "behavior", "ruleCount", "updatedAt", "status", "refreshable"])
          || !field(provider.name, 160) || !field(provider.behavior, 80) || !field(provider.updatedAt, 80)
          || !(provider.ruleCount === -1 || number(provider.ruleCount, 1000000000))
          || typeof provider.refreshable !== "boolean"
          || provider.status !== (provider.ruleCount < 0 ? "unknown" : provider.ruleCount === 0 ? "empty" : "loaded")) return null
      providers.push({name:provider.name, behavior:provider.behavior, ruleCount:provider.ruleCount,
        updatedAt:provider.updatedAt, status:provider.status, refreshable:false})
    }
    return {version:1, rules:{total:r.total, shown:r.shown, truncated:r.truncated, items:rules},
      providers:{total:s.total, shown:s.shown, truncated:s.truncated, items:providers}}
  } catch (_) { return null }
}

// Pure parser: returns a fresh projection or null; never changes UI state.
function startupCapability(raw, context) {
  try {
    if (!context || typeof raw !== "string" || raw.length > 16384) return null
    var p = envelope(raw), r = p && p.result
    if (!object(p, ["api", "version", "id", "ok", "revision", "result"]) || p.ok !== true
        || p.revision !== context.revision || !object(r, ["runtimeOwnership", "mutations", "methods"])
        || typeof r.runtimeOwnership !== "boolean" || typeof r.mutations !== "boolean"
        || !Array.isArray(r.methods) || r.methods.length > 128) return null
    var seen = Object.create(null)
    for (var i = 0; i < r.methods.length; i++) {
      var method = r.methods[i]
      if (typeof method !== "string" || method.length > 80 || !/^[a-z][a-z0-9_.]*$/.test(method) || seen[method]) return null
      seen[method] = true
    }
    return {instanceId:context.instanceId, revision:p.revision,
      available:r.runtimeOwnership && r.mutations && seen["startup.configure"] === true}
  } catch (_) { return null }
}

function parseQrExport(raw, revision) {
  try {
    if (typeof raw !== "string" || raw.length > 262144 || unescape(encodeURIComponent(raw)).length > 262144) return null
    var p = JSON.parse(raw)
    if (!object(p, ["api", "version", "id", "ok", "revision", "result"])
        || p.api !== "omavless.control" || p.version !== 1 || p.ok !== true
        || !id(p.id, false) || !number(p.revision, 9007199254740991) || p.revision !== revision
        || !object(p.result, ["format", "content"]) || p.result.format !== "uri"
        || !text(p.result.content, 32768, false)
        || unescape(encodeURIComponent(p.result.content)).length > 32768) return null
    return p.result.content
  } catch (_) { return null }
}
function qrDataUri(raw) {
  // The fixed Rust helper emits only PNG, without a trailing newline.
  if (typeof raw !== "string" || raw.length > 5592430
      || !/^data:image\/png;base64,iVBORw0KGgo[A-Za-z0-9+/]*={0,2}$/.test(raw)) return ""
  var body = raw.slice(22)
  var padding = body.endsWith("==") ? 2 : body.endsWith("=") ? 1 : 0
  if (body.length % 4 !== 0 || body.length / 4 * 3 - padding > 4194304) return ""
  return raw
}
function editorText(value, max) {
  try {
    return typeof value === "string" && value.indexOf("\u0000") < 0
      && value.length <= max && unescape(encodeURIComponent(value)).length <= max
  } catch (_) { return false }
}
function parseEditorInput(raw, revision) {
  try {
    var p = envelope(raw)
    if (!p || !object(p, ["api", "version", "id", "ok", "revision", "result"])
        || p.ok !== true || p.revision !== revision
        || !object(p.result, ["name", "input"])
        || !text(p.result.name, 80, false) || !editorText(p.result.input, 32768)
        || p.result.input === "") return null
    return {name:p.result.name, input:p.result.input}
  } catch (_) { return null }
}
function object(value, keys) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false
  var found = Object.keys(value)
  return found.length === keys.length && found.every(function(k) { return keys.indexOf(k) >= 0 })
}

// Version-one long-operation projection, shared by fixed start/get/cancel.
// Private provider data and backend messages are never retained.
function parseOperation(raw, job, kind) {
  try {
    var p = envelope(raw)
    if (!p || !job || !number(job.revision, 9007199254740991)) return null
    var codes = ["invalid_request", "unsupported_version", "unknown_method", "invalid_argument", "not_found", "conflict", "busy", "permission_denied", "capability_unavailable", "core_unavailable", "core_rejected", "timeout", "cancelled", "daemon_restarting", "internal_error", "manual_recovery_required", "transition_failed_restored"]
    function error(value) {
      return object(value, ["code", "message", "retryable"]) && codes.indexOf(value.code) >= 0
        && text(value.message, 512, false) && typeof value.retryable === "boolean"
    }
    if (p.ok === false) {
      if (!object(p, ["api", "version", "id", "ok", "revision", "error"]) || !error(p.error)) return null
      return {ok:false, code:p.error.code}
    }
    if (!object(p, ["api", "version", "id", "ok", "revision", "result"]) || p.ok !== true
        || !object(p.result, kind === "cancel" ? ["accepted", "operation"] : ["operation"])
        || (kind === "cancel" && typeof p.result.accepted !== "boolean")) return null
    var o = p.result.operation, method = job.kind === "subscriptions" ? "subscriptions.refresh_all" : job.kind === "providers" ? "routing.refresh_providers" : job.kind === "probe" ? "subscriptions.probe" : ""
    if (!object(o, ["instanceId", "operationId", "method", "state", "baseRevision", "outcomeRevision", "progress", "cancelRequested", "cancellable", "error"])
        || !text(o.instanceId, 128, false) || !/^[\x21-\x7e]+$/.test(o.instanceId) || !id(o.operationId, false)
        || o.instanceId !== job.instanceId || o.operationId !== job.operationId || o.method !== method || method === ""
        || !number(o.baseRevision, 9007199254740991) || o.baseRevision !== job.revision
        || ["queued", "running", "succeeded", "failed", "cancelled"].indexOf(o.state) < 0
        || !object(o.progress, ["completed", "total"]) || !number(o.progress.total, job.kind === "subscriptions" ? 64 : 256)
        || !number(o.progress.completed, o.progress.total) || typeof o.cancelRequested !== "boolean" || typeof o.cancellable !== "boolean") return null
    var terminal = ["succeeded", "failed", "cancelled"].indexOf(o.state) >= 0
    if (terminal ? !number(o.outcomeRevision, 9007199254740991) || o.outcomeRevision < o.baseRevision || o.cancellable
        : o.outcomeRevision !== null) return null
    if ((o.state === "failed") !== (o.error !== null) || (o.error !== null && !error(o.error))
        || (o.state === "cancelled" && !o.cancelRequested) || (o.state === "queued" && o.progress.completed !== 0)
        || (o.state === "succeeded" && (o.progress.completed !== o.progress.total || o.outcomeRevision !== o.baseRevision + (o.progress.total === 0 || job.kind === "probe" ? 0 : 1)))) return null
    if (p.revision < o.baseRevision || (terminal && p.revision < o.outcomeRevision)) return null
    if (job.acknowledged && (o.progress.completed < job.completed || o.progress.total < job.total
        || (job.state === "running" && o.state === "queued") || (job.cancelRequested && !o.cancelRequested))) return null
    return {ok:true, state:o.state, completed:o.progress.completed, total:o.progress.total,
      cancelRequested:o.cancelRequested, cancellable:o.cancellable, terminal:terminal,
      errorCode:o.error === null ? "" : o.error.code, outcomeRevision:o.outcomeRevision}
  } catch (_) { return null }
}
// Explicit private probe-result response, never a status/support projection.
function parseProbeResults(raw, job, snapshot) {
  try {
    if (!editorText(raw, 65536) || !job || job.kind !== "probe" || job.state !== "succeeded"
        || !snapshot || snapshot.instanceId !== job.instanceId || snapshot.revision !== job.revision
        || !Array.isArray(job.profileIds) || job.profileIds.length > 256) return null
    var p = JSON.parse(raw), r = p.result
    if (!object(p, ["api", "version", "id", "ok", "revision", "result"]) || p.api !== "omavless.control"
        || p.version !== 1 || !id(p.id, false) || p.ok !== true || p.revision !== job.revision
        || !object(r, ["version", "subscriptionId", "results"]) || r.version !== 1 || r.subscriptionId !== job.subscriptionId
        || !Array.isArray(r.results) || r.results.length !== job.profileIds.length || r.results.length !== job.total) return null
    var members = snapshot.profiles.filter(function(p) { return p.subscriptionId === job.subscriptionId && !p.missing }).map(function(p) { return p.id })
    if (members.length !== job.profileIds.length || !members.every(function(id) { return job.profileIds.indexOf(id) >= 0 })) return null
    var seen = Object.create(null), rows = Object.create(null)
    for (var i = 0; i < r.results.length; i++) {
      var row = r.results[i]
      if (!object(row, ["id", "resolved", "reachable", "latencyMs"]) || !id(row.id, false)
          || job.profileIds.indexOf(row.id) < 0 || seen[row.id] || typeof row.resolved !== "boolean" || typeof row.reachable !== "boolean"
          || (!row.resolved && row.reachable) || (row.reachable ? !number(row.latencyMs, 60000) : row.latencyMs !== -1)) return null
      seen[row.id] = true
      rows[row.id] = {resolved:row.resolved, reachable:row.reachable, latencyMs:row.latencyMs}
    }
    return rows
  } catch (_) { return null }
}

function parsePing(raw, context) {
  try {
    if (typeof raw !== "string" || raw.length > 2048 || !context) return null
    var p = envelope(raw), r = p && p.result
    if (!object(p, ["api", "version", "id", "ok", "revision", "result"]) || p.ok !== true
        || p.revision !== context.revision || !object(r, ["schemaVersion", "scope", "availability", "sample", "code", "instanceId"])
        || r.schemaVersion !== 1 || r.scope !== "controller_attributed_tun_icmp" || r.instanceId !== context.instanceId) return null
    if (r.availability === "unavailable")
      return r.sample === null && r.code === "probe_unavailable" ? {available:false} : null
    var s = r.sample
    if (r.availability !== "observed" || !object(s, ["outcome", "latencyMs"])) return null
    if (s.outcome === "loss") return s.latencyMs === null && r.code === "timeout" ? {available:true, value:-1} : null
    return s.outcome === "reply" && r.code === "ok" && typeof s.latencyMs === "number"
      && isFinite(s.latencyMs) && s.latencyMs >= 0 && s.latencyMs <= 3000 ? {available:true, value:s.latencyMs} : null
  } catch (_) { return null }
}

// Same ten-sample arithmetic as the legacy addPingSample/pingLatency/pingLoss.
function pingWindow(samples) {
  var sum = 0, replies = 0, lost = 0
  if (!Array.isArray(samples) || samples.length > 10) return null
  for (var i = 0; i < samples.length; i++) {
    var value = samples[i]
    if (typeof value !== "number" || !isFinite(value) || value > 3000 || (value < 0 && value !== -1)) return null
    if (value === -1) lost++
    else { sum += value; replies++ }
  }
  return {count:samples.length, latency:replies ? sum / replies : null,
    loss:samples.length ? Math.round(lost * 100 / samples.length) : null}
}

function parseTraffic(raw, context) {
  try {
    var p = envelope(raw)
    if (!p || !context || !object(p, ["api", "version", "id", "ok", "revision", "result"]) || p.ok !== true || p.revision !== context.revision) return null
    var r = p.result
    if (!object(r, ["schemaVersion", "scope", "availability", "sample", "instanceId"]) || r.schemaVersion !== 1
        || r.scope !== "controller_attributed_tun_counters" || r.instanceId !== context.instanceId) return null
    if (r.availability === "unavailable") return r.sample === null ? {available:false} : null
    var s = r.sample
    if (r.availability !== "observed" || !object(s, ["identity", "rxBytes", "txBytes", "sampledAtMs"])
        || typeof s.identity !== "string" || !/^[0-9a-f]{64}$/.test(s.identity)
        || !number(s.rxBytes, 9007199254740991) || !number(s.txBytes, 9007199254740991) || !number(s.sampledAtMs, 9007199254740991)) return null
    return {available:true, identity:s.identity, rx:s.rxBytes, tx:s.txBytes, sampledAtMs:s.sampledAtMs}
  } catch (_) { return null }
}

// Original applyTraffic reset semantics, using daemon monotonic elapsed time
// rather than wall-clock subtraction. Identity changes reset the series.
function trafficDelta(sample, previous) {
  var dt = previous ? (sample.sampledAtMs - previous.sampledAtMs) / 1000 : 0
  var rated = !!previous && sample.identity === previous.identity && sample.rx >= previous.rx && sample.tx >= previous.tx && dt > 0 && dt <= 30
  return {identity:sample.identity, rx:sample.rx, tx:sample.tx, sampledAtMs:sample.sampledAtMs,
    rated:rated, rxRate:rated ? (sample.rx - previous.rx) / dt : 0, txRate:rated ? (sample.tx - previous.tx) / dt : 0}
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
    else if (!(object(f, ["ownedCoreRunning", "visibleMihomoCount", "visibleTunCount", "ownedControllerConfigVerified", "desiredProfileMatchesOwned"])
        || object(f, ["ownedCoreRunning", "visibleMihomoCount", "ownedAuxiliaryMihomoCount", "visibleTunCount", "ownedControllerConfigVerified", "desiredProfileMatchesOwned"]))
        || typeof f.ownedCoreRunning !== "boolean" || typeof f.ownedControllerConfigVerified !== "boolean"
        || typeof f.desiredProfileMatchesOwned !== "boolean" || !number(f.visibleMihomoCount, 64) || !number(f.visibleTunCount, 8)
        || (f.ownedAuxiliaryMihomoCount !== undefined && !number(f.ownedAuxiliaryMihomoCount, Math.min(1, f.visibleMihomoCount)))
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

function routingResult(raw, revision) {
  if (!editorText(raw, 262144)) return null
  var p = JSON.parse(raw)
  return object(p, ["api", "version", "id", "ok", "revision", "result"])
    && p.api === "omavless.control" && p.version === 1 && id(p.id, false)
    && p.ok === true && number(p.revision, 9007199254740991) && p.revision === revision ? p.result : null
}
// Shareable support projection. V1 remains configuration-only; v2 carries
// bounded fresh local facts without claiming DNS/routes/service ownership.
function supportObservation(r) {
  if (!object(r, ["availability", "desired", "facts", "verification"])
      || ["observed", "unavailable"].indexOf(r.availability) < 0
      || !object(r.desired, ["connected", "mode"]) || typeof r.desired.connected !== "boolean"
      || ["rule", "global", "direct"].indexOf(r.desired.mode) < 0
      || !object(r.verification, ["serviceOwnership", "tunOwnership", "routes", "dns", "internet"])
      || !Object.keys(r.verification).every(function(k) { return r.verification[k] === false })) return null
  var f = r.facts, facts = null
  if (r.availability === "unavailable") { if (f !== null) return null }
  else {
    if (!object(f, ["ownedCoreRunning", "visibleMihomoCount", "ownedAuxiliaryMihomoCount", "visibleTunCount", "ownedControllerConfigVerified", "desiredProfileMatchesOwned"])
        || typeof f.ownedCoreRunning !== "boolean" || typeof f.ownedControllerConfigVerified !== "boolean"
        || typeof f.desiredProfileMatchesOwned !== "boolean" || !number(f.visibleMihomoCount, 64) || !number(f.visibleTunCount, 8)
        || !number(f.ownedAuxiliaryMihomoCount, Math.min(1, f.visibleMihomoCount))
        || (f.desiredProfileMatchesOwned && (!f.ownedCoreRunning || !r.desired.connected))
        || (f.ownedControllerConfigVerified && (!f.ownedCoreRunning || !f.desiredProfileMatchesOwned))) return null
    facts = {ownedCoreRunning:f.ownedCoreRunning, visibleMihomoCount:f.visibleMihomoCount,
      ownedAuxiliaryMihomoCount:f.ownedAuxiliaryMihomoCount, visibleTunCount:f.visibleTunCount,
      ownedControllerConfigVerified:f.ownedControllerConfigVerified, desiredProfileMatchesOwned:f.desiredProfileMatchesOwned}
  }
  return {availability:r.availability, desired:{connected:r.desired.connected, mode:r.desired.mode}, facts:facts,
    verification:{serviceOwnership:false, tunOwnership:false, routes:false, dns:false, internet:false}}
}
function supportHost(r) {
  if (!object(r, ["core", "runtimeService", "loginService", "files", "configuredPolicy"])) return null
  function nullableBool(v) { return v === null || typeof v === "boolean" }
  var c = r.core, f = r.files, p = r.configuredPolicy
  if (c !== null && (!object(c, ["installed", "fileNetworkCapabilities", "tunDevicePresent"])
      || typeof c.installed !== "boolean" || !nullableBool(c.fileNetworkCapabilities) || !nullableBool(c.tunDevicePresent)
      || (!c.installed && c.fileNetworkCapabilities !== false))) return null
  for (var i = 0; i < 2; i++) {
    var s = i === 0 ? r.runtimeService : r.loginService
    if (s !== null && (!object(s, ["loaded", "active", "enabled", "ownsCurrentProcess"])
        || typeof s.loaded !== "boolean" || typeof s.active !== "boolean" || typeof s.enabled !== "boolean"
        || (i === 0 ? typeof s.ownsCurrentProcess !== "boolean" : s.ownsCurrentProcess !== null)
        || (!s.loaded && (s.active || s.enabled)) || (s.ownsCurrentProcess && !s.active))) return null
  }
  if (f !== null && (!object(f, ["store", "template", "generatedConfig", "runtimeUnit", "loginUnit"])
      || !Object.keys(f).every(function(k) { return nullableBool(f[k]) }))) return null
  if (p !== null && (!object(p, ["basis", "rules", "providers"])
      || ["template", "active_config"].indexOf(p.basis) < 0 || !number(p.rules, 100000) || !number(p.providers, 1024))) return null
  // Every member is exact typed public vocabulary; do not return a raw envelope.
  return {core:c === null ? null : {installed:c.installed,fileNetworkCapabilities:c.fileNetworkCapabilities,tunDevicePresent:c.tunDevicePresent},
    runtimeService:r.runtimeService === null ? null : {loaded:r.runtimeService.loaded,active:r.runtimeService.active,enabled:r.runtimeService.enabled,ownsCurrentProcess:r.runtimeService.ownsCurrentProcess},
    loginService:r.loginService === null ? null : {loaded:r.loginService.loaded,active:r.loginService.active,enabled:r.loginService.enabled,ownsCurrentProcess:null},
    files:f === null ? null : {store:f.store,template:f.template,generatedConfig:f.generatedConfig,runtimeUnit:f.runtimeUnit,loginUnit:f.loginUnit},
    configuredPolicy:p === null ? null : {basis:p.basis,rules:p.rules,providers:p.providers}}
}
function configurationReport(raw, revision) {
  try {
    var r = routingResult(raw, revision)
    if (!r) return null
    var extended = r.schemaVersion === 3, modern = extended || r.schemaVersion === 2
    if (!(modern ? object(r, extended ? ["schemaVersion", "scope", "runtime", "configuration", "coverage", "localObservation", "host"] : ["schemaVersion", "scope", "runtime", "configuration", "coverage", "localObservation"])
        && r.scope === "native_support" : object(r, ["schemaVersion", "scope", "runtime", "configuration", "coverage"])
        && r.schemaVersion === 1 && r.scope === "native_configuration")) return null
    var observation = modern ? supportObservation(r.localObservation) : null
    if (modern && !observation) return null
    var host = extended ? supportHost(r.host) : null
    if (extended && !host) return null
    var coreObserved = !!(host && host.core && host.core.fileNetworkCapabilities !== null && host.core.tunDevicePresent !== null)
    var servicesObserved = !!(host && host.runtimeService && host.loginService)
    var filesObserved = !!(host && host.files && Object.keys(host.files).every(function(k) { return host.files[k] !== null }))
    var h = r.runtime, c = r.configuration, v = r.coverage
    if (!object(h, ["implementation", "version", "lastKnownState", "routingTransactionPending"])
        || h.implementation !== "rust" || !text(h.version, 32, false) || !/^\d+\.\d+\.\d+$/.test(h.version)
        || ["disconnected", "starting", "connected", "reconnecting", "stopping", "failed", "manual_recovery_required"].indexOf(h.lastKnownState) < 0
        || typeof h.routingTransactionPending !== "boolean"
        || !(modern ? object(v, ["privateStoreValidated", "liveHostObservation", "controllerQuery", "loginActivationVerified", "coreSetupVerified", "serviceEnablementVerified", "loadedPolicyCounts", "fileReadiness"])
          && v.coreSetupVerified === coreObserved && v.serviceEnablementVerified === servicesObserved && v.loadedPolicyCounts === false && v.fileReadiness === filesObserved
          : object(v, ["privateStoreValidated", "liveHostObservation", "controllerQuery", "loginActivationVerified"]))
        || v.privateStoreValidated !== true || v.liveHostObservation !== (modern && observation.availability === "observed")
        || v.controllerQuery !== !!(modern && observation.facts && observation.facts.ownedControllerConfigVerified) || v.loginActivationVerified !== false
        || !object(c, ["inventory", "routing", "startup", "updates", "onboardingComplete"])) return null
    var i = c.inventory, s = c.startup, t = c.routing
    if (!object(i, ["profiles", "favorites", "subscriptions", "customRules"])
        || !number(i.profiles, 256) || !number(i.favorites, i.profiles) || !number(i.subscriptions, 64) || !number(i.customRules, 128)
        || !object(t, ["preset", "configured", "lastManualRuleUpdate"]) || !text(t.preset, 80, true)
        || typeof t.configured !== "boolean" || t.configured !== (t.preset !== "") || !number(t.lastManualRuleUpdate, 9007199254740991)
        || !object(s, ["configured", "enabled", "target", "mode"]) || typeof s.configured !== "boolean" || typeof s.enabled !== "boolean"
        || ["last", "profile"].indexOf(s.target) < 0 || ["rule", "global"].indexOf(s.mode) < 0
        || !object(c.updates, ["latestSubscription"]) || !number(c.updates.latestSubscription, 9007199254740991)
        || typeof c.onboardingComplete !== "boolean") return null
    // Unknown user-defined preset labels are never shareable identifiers.
    var preset = ["", "custom", "roscomvpn-default", "china-cn-direct", "iran-ir-direct"].indexOf(t.preset) >= 0 ? t.preset : "custom"
    var report = {schemaVersion:r.schemaVersion, scope:r.scope, runtime:{implementation:"rust", version:h.version,
      lastKnownState:h.lastKnownState, routingTransactionPending:h.routingTransactionPending}, configuration:{inventory:{profiles:i.profiles,
      favorites:i.favorites, subscriptions:i.subscriptions, customRules:i.customRules}, routing:{preset:preset, configured:t.configured,
      lastManualRuleUpdate:t.lastManualRuleUpdate}, startup:{configured:s.configured, enabled:s.enabled, target:s.target, mode:s.mode},
      updates:{latestSubscription:c.updates.latestSubscription}, onboardingComplete:c.onboardingComplete}, coverage:{privateStoreValidated:true,
      liveHostObservation:v.liveHostObservation, controllerQuery:v.controllerQuery, loginActivationVerified:false}}
    if (modern) {
      report.localObservation = observation
      report.coverage.coreSetupVerified = coreObserved
      report.coverage.serviceEnablementVerified = servicesObserved
      report.coverage.loadedPolicyCounts = false
      report.coverage.fileReadiness = filesObserved
    }
    if (extended) report.host = host
    return JSON.stringify(report, null, 2) + "\n"
  } catch (_) { return null }
}
function parseCustomRules(raw, revision) {
  try {
    var r = routingResult(raw, revision)
    if (!object(r, ["version", "rules"]) || r.version !== 1 || !Array.isArray(r.rules) || r.rules.length > 128) return null
    var seen = Object.create(null), result = []
    for (var i = 0; i < r.rules.length; i++) {
      var v = r.rules[i]
      if (!object(v, ["id", "kind", "action", "value"]) || !text(v.id, 36, false)
          || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v.id) || seen[v.id]
          || ["domain", "suffix", "ipcidr"].indexOf(v.kind) < 0 || ["proxy", "direct", "reject"].indexOf(v.action) < 0
          || !text(v.value, 1024, false) || !editorText(v.value, 1024)) return null
      seen[v.id] = true
      result.push({id:v.id, kind:v.kind, action:v.action, value:v.value})
    }
    return result
  } catch (_) { return null }
}
function parseRouteCheck(raw, revision) {
  try {
    var r = routingResult(raw, revision)
    if (!object(r, ["version", "query", "outcome", "ruleType", "rulePayload", "target", "source"]) || r.version !== 1
        || !text(r.query, 1024, false) || !editorText(r.query, 1024)
        || ["vpn", "direct", "block", "unknown"].indexOf(r.outcome) < 0
        || !text(r.ruleType, 80, true) || !text(r.rulePayload, 1024, true) || !editorText(r.rulePayload, 1024)
        || !text(r.target, 80, true) || ["mode", "custom", "disconnected", "live"].indexOf(r.source) < 0) return null
    return {query:r.query, outcome:r.outcome, ruleType:r.ruleType, rulePayload:r.rulePayload, target:r.target, source:r.source}
  } catch (_) { return null }
}

function parseAction(raw, pending) {
  try {
    var p = envelope(raw)
    if (!p || !pending || !id(pending.instanceId, false) || !id(pending.operationId, false)
        || !number(pending.revision, 9007199254740991) || ["connect", "disconnect", "mode", "profile-rename", "profile-favorite", "profile-delete", "profile-import", "profile-replace", "subscription-add", "subscription-update", "subscription-delete", "subscription-refresh", "routing-preset", "custom-rule-add", "custom-rule-delete", "startup-configure"].indexOf(pending.action) < 0) return null
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

// Native import is private UI data, never a diagnostic/log projection.
function importInput(value) {
  try {
    return typeof value === "string" && value.length > 0 && value.length <= 32768
      && value.indexOf("\u0000") < 0 && unescape(encodeURIComponent(value)).length <= 32768
  } catch (_) { return false }
}

function parseImportPreview(raw, revision) {
  try {
    if (typeof raw !== "string" || raw.length > 262144 || unescape(encodeURIComponent(raw)).length > 262144) return null
    var p = JSON.parse(raw)
    if (!object(p, ["api", "version", "id", "ok", "revision", "result"])
        || p.api !== "omavless.control" || p.version !== 1 || !id(p.id, false)
        || p.ok !== true || !number(p.revision, 9007199254740991) || p.revision !== revision) return null
    var r = p.result
    if (object(r, ["version", "kind", "suggestedName", "duplicate"]) && r.version === 1
        && r.kind === "subscription" && text(r.suggestedName, 80, false)
        && typeof r.duplicate === "boolean") return {kind:"subscription", duplicate:r.duplicate, suggestedName:r.suggestedName}
    if (!object(r, ["version", "kind", "profile"]) || r.version !== 1 || r.kind !== "profile") return null
    var v = r.profile
    if (!object(v, ["version", "protocol", "server", "port", "transport", "security", "sni", "flow", "insecure", "advancedXhttp", "experimental", "experimentalFeatures", "compatibilityNote", "credentialHint", "suggestedName"])
        || v.version !== 1 || ["vless", "trojan", "hysteria2", "tuic"].indexOf(v.protocol) < 0
        || !text(v.server, 253, false) || !number(v.port, 65535) || v.port < 1
        || ["tcp", "ws", "http", "h2", "grpc", "xhttp", "quic"].indexOf(v.transport) < 0
        || ["none", "tls", "reality"].indexOf(v.security) < 0 || !text(v.sni, 253, true)
        || !text(v.flow, 64, true) || typeof v.insecure !== "boolean"
        || typeof v.advancedXhttp !== "boolean" || typeof v.experimental !== "boolean"
        || !Array.isArray(v.experimentalFeatures) || v.experimentalFeatures.length > 8
        || !v.experimentalFeatures.every(function(f) { return text(f, 64, false) })
        || !text(v.compatibilityNote, 1000, true) || !text(v.suggestedName, 80, true)
        || typeof v.credentialHint !== "string" || !/^••••(?:[0-9a-f]{4})?$/i.test(v.credentialHint)) return null
    return {kind:"profile", profile:v}
  } catch (_) { return null }
}

function parseActionExit(raw, pending, exitCode) {
  // Reserved CLI exit proves local rejection before socket dispatch.
  if (exitCode === 74 && pending && ["profile-replace", "subscription-add", "subscription-update", "subscription-delete", "subscription-refresh", "routing-preset", "custom-rule-add", "custom-rule-delete", "onboarding-complete", "startup-configure"].indexOf(pending.action) >= 0)
    return {ok:false, code:"invalid_argument"}
  return parseAction(raw, pending)
}

// Explicit private editor read, never ordinary status or diagnostics. Canonical
// URL policy remains in Rust; this checks only the response shape and bounds.
function parseSubscriptionEditor(raw, revision) {
  try {
    if (!editorText(raw, 65536)) return null
    var p = JSON.parse(raw)
    if (!object(p, ["api", "version", "id", "ok", "revision", "result"])
        || p.api !== "omavless.control" || p.version !== 1 || !id(p.id, false)
        || p.ok !== true || p.revision !== revision
        || !object(p.result, ["name", "url"]) || !text(p.result.name, 80, false)
        || !text(p.result.url, 8192, false) || !editorText(p.result.url, 8192)) return null
    return {name:p.result.name, url:p.result.url}
  } catch (_) { return null }
}
