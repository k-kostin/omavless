// SPDX-License-Identifier: MIT
// Adapted from Omarchy VPN: https://github.com/jkoestinger/omarchy-vpn
// Copyright (c) 2026 Justin Köstinger
// Copyright (c) 2026 OmaVLESS contributors
// See LICENSE and THIRD_PARTY_NOTICES.md.

import QtQuick
import Quickshell
import Quickshell.Io

// Headless state for the OmaVLESS widget. backend.sh keeps private links in a
// 0600 store and runs a dedicated Mihomo user service.
Item {
  id: root

  property var settings: ({})

  readonly property string backendPath: String(Qt.resolvedUrl("../backend.sh")).replace(/^file:\/\//, "")

  // Local profiles as {uuid, name, protocol, active}. Mutating actions address the
  // generated id, never the display label.
  property var profiles: []
  // Public subscription metadata only. Bearer URLs are intentionally absent
  // from the status protocol and loaded through a one-shot editor pipe.
  property var subscriptions: []
  property string subscriptionStatus: ""
  property string subscriptionError: ""
  // Ephemeral endpoint reachability, keyed by profile UUID. Results are not
  // persisted: a latency number goes stale quickly and must never become a
  // hidden reason to choose a server on the user's next login.
  property var profileProbes: ({})
  // Session-only completion times let the UI distinguish fresh measurements
  // from numbers that have been sitting in an open panel for a while.
  property var subscriptionProbeTimes: ({})
  property string probingSubscriptionUuid: ""
  property int probeElapsedSeconds: 0
  property int probeCompletedCount: 0
  property int probeTotalCount: 0
  property double _probeStartedAt: 0
  property bool _probeCancelRequested: false
  property bool _probeSawComplete: false
  property bool _probeStreamValid: true
  property var _probeSeenIds: ({})
  property var _probeSummary: ({ tested: 0, unavailable: 0, unresolved: 0 })
  property string _probeName: ""
  readonly property bool probingProfiles: probeProcess.running
  readonly property bool subscriptionEditorLoading: subscriptionUrlProcess.running
  signal subscriptionUrlReady(string uuid, string url)
  property var customRules: []
  property string routingToolStatus: ""
  property string routingToolError: ""
  property var routeCheckResult: null
  readonly property bool routingToolsLoading: customRulesProcess.running
  readonly property bool routeChecking: routeCheckProcess.running
  property string _routeCheckInput: ""
  // Names of the profiles currently active
  readonly property var activeNames: {
    var out = []
    for (var i = 0; i < profiles.length; i++) {
      if (profiles[i].active) out.push(profiles[i].name)
    }
    return out
  }
  readonly property bool active: activeNames.length > 0
  readonly property int managedProfileCount: {
    var count = 0
    for (var i = 0; i < profiles.length; i++) {
      if (profiles[i].managed) count++
    }
    return count
  }
  readonly property int favoriteProfileCount: {
    var count = 0
    for (var i = 0; i < profiles.length; i++) {
      if (profiles[i].favorite) count++
    }
    return count
  }
  readonly property double latestSubscriptionUpdatedAt: {
    var latest = 0
    for (var i = 0; i < subscriptions.length; i++)
      latest = Math.max(latest, Number(subscriptions[i].updatedAt) || 0)
    return latest
  }
  // UUID of the most recently connected profile, persisted across restarts
  // so the hero toggle reconnects what you actually used last. A UUID, not
  // a name: with duplicate names a name would resolve to the wrong profile.
  property string lastUuid: ""
  property string actionStatus: ""
  property string lastError: ""
  readonly property bool pickingFile: pickerProcess.running
  readonly property bool readingClipboard: clipboardProcess.running
  readonly property bool editing: editProcess.running
  readonly property bool importSourceBusy: pickingFile || readingClipboard || importPreviewLoading
  readonly property bool exporting: exportProcess.running
  readonly property bool importPreviewLoading: previewProcess.running
  property var importPreview: ({})
  readonly property bool diagnosticsExporting: diagnosticsProcess.running
  property string diagnosticsStatus: ""
  // Live Mihomo data is deliberately page-scoped. It never contributes to
  // lastError, so a controller/API problem cannot turn a healthy VPN shield
  // into a fatal-looking bar state.
  property bool diagnosticsPageVisible: false
  readonly property bool advancedDiagnosticsLoading: advancedDiagnosticsProcess.running
  property string advancedDiagnosticsErrorCode: ""
  property string advancedDiagnosticsError: ""
  property var loadedRules: []
  property int loadedRuleTotal: 0
  property bool loadedRulesTruncated: false
  property var loadedRuleProviders: []
  property int loadedRuleProviderTotal: 0
  property bool loadedRuleProvidersTruncated: false
  readonly property int loadedRefreshableProviderCount: {
    var count = 0
    for (var i = 0; i < loadedRuleProviders.length; i++)
      if (loadedRuleProviders[i].refreshable) count++
    return count
  }
  property double advancedDiagnosticsLoadedAt: 0
  readonly property bool copying: copyProcess.running
  readonly property bool messageDismissible:
    lastError !== "" && actionStatus === "" ||
    actionStatus !== "" && actionStatus === _transientActionStatus
  // Public action methods return true only after they actually started work.
  // IPC uses actionRejection to avoid acknowledging a request that a busy
  // widget would otherwise quietly discard.
  property string actionRejection: ""
  // Repeated backend failures back off exponentially instead of spawning a
  // new process at the normal cadence forever. A manual refresh remains
  // available, and one success restores the configured interval.
  property int statusFailureCount: 0
  property bool panelVisible: false
  readonly property int statusBaseIntervalSec:
    panelVisible ? refreshIntervalSec : Math.max(30, refreshIntervalSec)
  readonly property int statusPollIntervalMs:
    statusBaseIntervalSec * 1000 * Math.pow(2, Math.min(statusFailureCount, 5))

  // A mode change remains busy through the first authoritative status read.
  // The privileged process exiting only says that its transaction ended; it
  // does not make an earlier staged-template observation safe to present.
  readonly property bool busy: controlProcess.running || routingModePending
  readonly property bool routingModePending: _pendingRoutingMode !== ""
  readonly property bool statusProcessRunning: statusProcess.running
  // Structured from the exact generated config while connected, or from the
  // template that the next connection will use while disconnected. "Rule" by
  // itself is not enough: five LAN exceptions and forty remote rule sets both
  // have that mode while producing very different routes.
  property var routing: ({
    mode: "unknown",
    source: "unknown",
    preset: "",
    configured: false,
    ruleCount: 0,
    providerCount: 0,
    customRuleCount: 0,
    rulesUpdatedAt: 0,
    ruleUpdateAvailable: false
  })
  property var coreSetup: ({ installed: false, tunReady: false, path: "" })
  property var filePicker: ({ available: false, provider: "" })
  property var desktopHelpers: ({
    configEditorAvailable: false,
    qrEncoderAvailable: false
  })
  // Stable local presentation code. It never contains a helper path, profile
  // value or backend stderr and lets the panel localize the recovery action.
  property string desktopHelperErrorCode: ""
  readonly property string configEditorMissingFallback:
    "Profile editing unavailable — install Zenity. Run “omarchy pkg add zenity”"
  property var startup: ({
    enabled: false,
    configured: true,
    target: "last",
    profileId: "",
    mode: "rule"
  })
  property bool onboardingComplete: true
  readonly property bool onboardingNeeded: !onboardingComplete
  readonly property string coreSetupLabel: !coreSetup.installed
    ? "Mihomo not installed" : (coreSetup.tunReady ? "Mihomo ready" : "TUN access required")
  readonly property var startupProfile: startup.target === "profile"
    ? findByUuid(startup.profileId) : toggleProfile
  readonly property string startupSummary: {
    if (!startup.enabled) return "Off"
    if (!startup.configured) return "On · review legacy login behavior"
    var target = startup.target === "last"
      ? "Last used server"
      : (startupProfile ? startupProfile.name : "Selected server unavailable")
    return target + " · " + (startup.mode === "global" ? "Full VPN" : "Routing")
  }
  readonly property var routingPresets: [
    {
      id: "roscomvpn-default",
      country: "Russia",
      name: "RoscomVPN Default",
      shortName: "Russia",
      summary: "RU/BY and selected local services direct · remaining traffic via VPN",
      source: "hydraponique/roscomvpn-routing",
      sourceUrl: "https://github.com/hydraponique/roscomvpn-routing"
    },
    {
      id: "china-cn-direct",
      country: "China",
      name: "CN Direct",
      shortName: "China",
      summary: "Mainland China and private networks direct · remaining traffic via VPN",
      source: "MetaCubeX/meta-rules-dat",
      sourceUrl: "https://github.com/MetaCubeX/meta-rules-dat"
    },
    {
      id: "iran-ir-direct",
      country: "Iran",
      name: "IR Direct",
      shortName: "Iran",
      summary: "Iran and private networks direct · remaining traffic via VPN",
      source: "Chocolate4U/Iran-clash-rules",
      sourceUrl: "https://github.com/Chocolate4U/Iran-clash-rules"
    }
  ]
  function routingPresetById(value) {
    var id = String(value || "")
    for (var i = 0; i < routingPresets.length; i++) {
      if (routingPresets[i].id === id) return routingPresets[i]
    }
    return null
  }
  readonly property var activeRoutingPreset: routingPresetById(routing.preset)
  readonly property bool routingPresetConfigured: routing.configured === true
  readonly property string routingModeLabel: {
    if (routing.mode === "rule") return "Rule"
    if (routing.mode === "global") return "Global"
    if (routing.mode === "direct") return "Direct"
    return "Unknown"
  }
  readonly property string routingSourceLabel: {
    if (activeRoutingPreset) return activeRoutingPreset.shortName
    if (routing.source === "roscomvpn") return "RoscomVPN"
    if (routing.source === "china") return "China"
    if (routing.source === "iran") return "Iran"
    if (routing.source === "custom") return "Custom"
    if (routing.source === "basic") return "Basic"
    if (routing.source === "none") return "No rules"
    return "Unavailable"
  }
  readonly property string routingTitle: routing.mode === "rule"
    ? routingSourceLabel + " · " + routingModeLabel
    : routingModeLabel
  readonly property string routingSummary: {
    if (routing.mode === "global") return "All traffic uses the VPN · rule sets ignored"
    if (routing.mode === "direct") return "VPN bypassed · rule sets ignored"
    if (routing.mode !== "rule") return "Could not read the effective routing policy"
    if (routing.source === "roscomvpn")
      return "RU/BY direct · selected services via VPN · " + routing.providerCount + " rule sets"
    if (routing.source === "china")
      return "Mainland China direct · remaining traffic via VPN · " + routing.providerCount + " rule sets"
    if (routing.source === "iran")
      return "Iran direct · remaining traffic via VPN · " + routing.providerCount + " rule sets"
    if (routing.source === "basic")
      return "Local networks direct · all internet via VPN · " + routing.ruleCount + " rules"
    if (routing.source === "custom")
      return routing.ruleCount + " rules · " + routing.providerCount + " remote rule sets"
    if (routing.source === "none") return "No rule list found · Mihomo fallback applies"
    return "Could not read the effective routing policy"
  }
  readonly property string routingPresetName: activeRoutingPreset
    ? activeRoutingPreset.name : routingSourceLabel
  readonly property string routingSourceName: activeRoutingPreset
    ? activeRoutingPreset.source : routingSourceLabel
  readonly property string routingSourceUrl: activeRoutingPreset
    ? activeRoutingPreset.sourceUrl : ""
  readonly property bool routingUnavailable: routing.mode === "unknown" || routing.source === "unknown"
  property var conflicts: []
  readonly property bool hasRoutingConflict: conflicts.length > 0
  readonly property string conflictSummary: conflicts.join(" · ")
  property int uptimeSeconds: 0
  readonly property string statusText: active
    ? "VPN: " + activeNames.join(", ") + " · " + routingTitle
    : "VPN disconnected · on connect: " + routingTitle
  // What toggle() would bring up: the last used profile if it still exists,
  // otherwise the first one. A pre-UUID state file holds a name, so a name
  // match is the fallback before giving up on the stored value.
  readonly property var toggleProfile: {
    var byName = null
    for (var i = 0; i < profiles.length; i++) {
      if (profiles[i].uuid === lastUuid) return profiles[i]
      if (byName === null && profiles[i].name === lastUuid) byName = profiles[i]
    }
    if (byName !== null) return byName
    return profiles.length > 0 ? profiles[0] : null
  }
  readonly property string toggleTarget: toggleProfile ? toggleProfile.name : ""

  readonly property int refreshIntervalSec: intSetting("refreshIntervalSec", 10, 2, 3600)
  readonly property bool showBarThroughput: boolSetting("showBarThroughput", false)
  readonly property bool showExitIp: boolSetting("showExitIp", true)

  // Supported backend behaviour is deliberately separate from presentation.
  // Buttons bind to these flags rather than assuming that every future core
  // or protocol can perform the same operations.
  property var capabilities: ({
    subscriptions: true,
    subscriptionSearch: true,
    routingModes: true,
    connectionTest: true,
    liveTraffic: true,
    trafficHistory: true,
    exitIp: showExitIp,
    conflictDetection: true,
    qr: true,
    trojanExperimental: true,
    hysteria2Experimental: true,
    tuicExperimental: true,
    protocols: ["vless", "trojan", "hysteria2", "tuic"],
    core: "mihomo"
  })

  function supports(name) {
    var key = String(name)
    if (key === "connectionTest") return capabilities[key] === true && pingHost !== ""
    if (key === "exitIp") return capabilities[key] === true && showExitIp
    return capabilities[key] === true
  }

  // Traffic sampling: byte counters from /sys for the active tunnels'
  // devices — readable without privilege. This is activity, not health:
  // A proxy profile has no connection state, and a tunnel that is silent is not
  // thereby broken. Sampled on a short timer only while the panel is open.
  property bool trafficMonitoring: false
  property bool pingMonitoring: false
  // device -> {rx, tx, at, rxRate, txRate}. The raw counters double as
  // session totals come from Mihomo's TUN interface.
  property var traffic: ({})
  property var rxHistory: []
  property var txHistory: []
  readonly property int historyMaxPoints: 30
  readonly property string barThroughput: {
    var t = trafficOf(primaryDevice)
    if (!trafficLive(t) || !t.rated) return ""
    return "↓" + fmtBytes(t.rxRate) + " ↑" + fmtBytes(t.txRate)
  }

  property string exitIp: ""
  property bool exitIpFetching: false
  property bool exitIpFailed: false
  property string _exitIpFor: ""
  property string _lastExitRoutingMode: ""

  readonly property var activeDevices: {
    var out = []
    for (var i = 0; i < profiles.length; i++) {
      if (profiles[i].active && profiles[i].ifname !== "") out.push(profiles[i].ifname)
    }
    return out
  }

  // The tunnel the detail grid describes — the first active profile in the
  // sorted list, which is also the one the hero names first. A profile can
  // run several at once; the rest keep their per-row traffic line, because
  // one endpoint, one address and one ping cannot describe two tunnels.
  readonly property var primaryProfile: {
    for (var i = 0; i < profiles.length; i++) {
      if (profiles[i].active) return profiles[i]
    }
    return null
  }
  readonly property string primaryUuid: primaryProfile ? primaryProfile.uuid : ""
  readonly property string primaryName: primaryProfile ? primaryProfile.name : ""
  readonly property string primaryDevice: primaryProfile ? primaryProfile.ifname : ""

  // Connection facts from `backend.sh details` — address, endpoint,
  // transport and SNI. Kept with the UUID they describe so a switch shows the
  // new tunnel's numbers or nothing at all, never the old tunnel's.
  property var details: ({})
  property string detailsUuid: ""
  readonly property bool hasDetails: detailsUuid !== "" && detailsUuid === primaryUuid

  function detail(key) {
    if (!hasDetails) return ""
    var value = details[String(key)]
    return value === undefined ? "" : String(value)
  }

  // Latency through the tunnel: ICMP bound to the TUN device, so a split
  // tunnel is measured on the path it actually routes. Unprivileged — ping
  // sockets are open to all users on Omarchy. Set pingHost to "" to switch
  // the probe off entirely.
  readonly property string pingHost: String(setting("pingHost", "1.1.1.1")).trim()
  // Rate, not health, again: a lost packet is a lost packet, not a warning
  // about the tunnel. Samples are ms, or -1 for a timeout.
  property var pingSamples: []
  // True only for a user-requested one-shot sample. The automatic monitor
  // uses the same bound probe, but never makes the hero action look busy.
  property bool _manualPingRequested: false
  readonly property bool testingConnection: _manualPingRequested
  readonly property bool hasPing: pingSamples.length > 0
  readonly property real pingLatency: {
    var total = 0
    var count = 0
    for (var i = 0; i < pingSamples.length; i++) {
      if (pingSamples[i] < 0) continue
      total += pingSamples[i]
      count++
    }
    return count > 0 ? total / count : -1
  }
  readonly property int pingLoss: {
    if (pingSamples.length === 0) return 0
    var lost = 0
    for (var i = 0; i < pingSamples.length; i++) {
      if (pingSamples[i] < 0) lost++
    }
    return Math.round(lost * 100 / pingSamples.length)
  }

  // The grid's static half — endpoint, address, routes — changes only when
  // the profile is edited, so it is fetched on a switch and on opening,
  // never on a timer. Both halves are dropped with the tunnel they describe:
  // showing the old numbers under a new name would be worse than "--".
  onPrimaryUuidChanged: {
    details = ({})
    detailsUuid = ""
    pingSamples = []
    _manualPingRequested = false
    rxHistory = []
    txHistory = []
    exitIp = ""
    exitIpFailed = false
    fetchDetails()
    exitIpDelay.restart()
  }

  onTrafficMonitoringChanged: {
    if (trafficMonitoring) fetchDetails()
    else {
      traffic = ({})
      rxHistory = []
      txHistory = []
    }
  }

  onPingMonitoringChanged: {
    if (pingMonitoring) fetchDetails()
    // A window's worth of stale samples would otherwise be the first thing
    // the panel shows on reopening.
    else {
      pingSamples = []
      _manualPingRequested = false
    }
  }

  onDiagnosticsPageVisibleChanged: {
    _advancedDiagnosticsGeneration++
    _advancedDiagnosticsRefreshPending = false
    advancedDiagnosticsErrorCode = ""
    advancedDiagnosticsError = ""
    if (diagnosticsPageVisible) {
      loadedRules = []
      loadedRuleTotal = 0
      loadedRulesTruncated = false
      loadedRuleProviders = []
      loadedRuleProviderTotal = 0
      loadedRuleProvidersTruncated = false
      advancedDiagnosticsLoadedAt = 0
      refreshAdvancedDiagnostics()
    }
  }

  function setting(name, fallback) {
    var value = settings ? settings[name] : undefined
    return value === undefined || value === null ? fallback : value
  }

  function intSetting(name, fallback, min, max) {
    var n = parseInt(String(setting(name, fallback)), 10)
    if (!isFinite(n)) n = fallback
    if (n < min) n = min
    if (n > max) n = max
    return n
  }

  function boolSetting(name, fallback) {
    var value = setting(name, fallback)
    if (typeof value === "boolean") return value
    var text = String(value).toLowerCase()
    if (text === "true" || text === "1" || text === "yes") return true
    if (text === "false" || text === "0" || text === "no") return false
    return fallback
  }

  // QML Text defaults to AutoText. Public strings therefore pass through one
  // bounded plain-text boundary before any stock Omarchy component sees them.
  function plainText(value, maximum) {
    var limit = Math.max(0, Math.min(Number(maximum) || 256, 4096))
    var text = String(value === undefined || value === null ? "" : value)
      .replace(/[\u0000-\u001f\u007f]/g, " ")
      .replace(/\s+/g, " ").trim()
    if (text.length > limit) text = text.substring(0, Math.max(0, limit - 1)) + "…"
    // Keep stock AutoText labels from interpreting provider-controlled names.
    return text.replace(/&/g, "＆").replace(/</g, "‹").replace(/>/g, "›")
  }

  function rejectAction(reason, semanticCode) {
    desktopHelperErrorCode = String(semanticCode || "")
    actionRejection = String(reason)
    lastError = actionRejection
    return false
  }

  function clearMessage() {
    transientStatusTimer.stop()
    _transientActionStatus = ""
    actionStatus = ""
    lastError = ""
    desktopHelperErrorCode = ""
    _dropWarningText = ""
  }

  function showTransientStatus(message) {
    var value = String(message || "")
    lastError = ""
    desktopHelperErrorCode = ""
    actionStatus = value
    _transientActionStatus = value
    if (value !== "") transientStatusTimer.restart()
  }

  function refresh() {
    // Timer and manual refreshes coalesce. This is not an operation failure:
    // recording it in lastError would make the bar urgent forever after a
    // normal timer overlap, but IPC can still return the rejection reason.
    if (statusProcess.running) {
      actionRejection = "a refresh is already running"
      return false
    }
    actionRejection = ""
    // The observation time for mark-active is when this snapshot is
    // *requested* — anything that happens while the poll runs or waits to
    // be parsed is "after the observation" and must keep its marker.
    _statusStartedAt = Date.now()
    _statusRequestGeneration++
    _runningStatusGeneration = _statusRequestGeneration
    statusProcess.running = true
    return true
  }

  // Control operations need a post-change snapshot, even if an earlier status
  // poll is in flight. Timers and manual refreshes deliberately do not queue:
  // otherwise a slow status command could keep polling forever.
  function refreshAfterChange() {
    if (statusProcess.running) {
      _refreshAfterStatus = true
      postChangeRefreshTimer.restart()
      return false
    }
    _refreshAfterStatus = false
    return refresh()
  }

  function refreshAdvancedDiagnostics() {
    if (!diagnosticsPageVisible) return false
    if (advancedDiagnosticsProcess.running) {
      // A re-open must get a fresh answer after the request from the previous
      // page lifetime exits. Repeated refresh clicks in one lifetime do not
      // queue another process.
      if (_advancedDiagnosticsRequestGeneration !== _advancedDiagnosticsGeneration)
        _advancedDiagnosticsRefreshPending = true
      return false
    }
    advancedDiagnosticsErrorCode = ""
    advancedDiagnosticsError = ""
    _advancedDiagnosticsRequestGeneration = _advancedDiagnosticsGeneration
    advancedDiagnosticsProcess.command = ["bash", backendPath, "advanced-diagnostics"]
    advancedDiagnosticsProcess.running = true
    return true
  }

  function refreshAdvancedDiagnosticsAfterChange() {
    if (!diagnosticsPageVisible) return false
    if (advancedDiagnosticsProcess.running) {
      _advancedDiagnosticsRefreshPending = true
      return false
    }
    return refreshAdvancedDiagnostics()
  }

  function applyAdvancedDiagnostics(raw) {
    var text = String(raw || "")
    if (text.length === 0 || text.length > 393216) return false
    var payload
    try { payload = JSON.parse(text) } catch (error) { return false }
    if (!payload || payload.version !== 1 || !payload.rules || !payload.providers)
      return false
    var rules = payload.rules
    var providers = payload.providers
    if (!Array.isArray(rules.items) || rules.items.length > 2048
        || typeof rules.total !== "number" || typeof rules.shown !== "number"
        || !isFinite(rules.total) || Math.floor(rules.total) !== rules.total
        || rules.total < rules.items.length || rules.total > 65536
        || rules.shown !== rules.items.length
        || typeof rules.truncated !== "boolean"
        || !Array.isArray(providers.items) || providers.items.length > 256
        || typeof providers.total !== "number" || typeof providers.shown !== "number"
        || !isFinite(providers.total) || Math.floor(providers.total) !== providers.total
        || providers.total < providers.items.length
        || providers.total > 256
        || providers.shown !== providers.items.length
        || typeof providers.truncated !== "boolean") return false
    var nextRules = []
    for (var i = 0; i < rules.items.length; i++) {
      var rule = rules.items[i]
      if (!rule || typeof rule.type !== "string" || rule.type.length > 80
          || typeof rule.payload !== "string" || rule.payload.length > 512
          || ["VPN", "DIRECT", "REJECT"].indexOf(rule.target) < 0) return false
      nextRules.push({
        type: plainText(rule.type, 80), payload: plainText(rule.payload, 512),
        target: rule.target
      })
    }
    var nextProviders = []
    for (var p = 0; p < providers.items.length; p++) {
      var provider = providers.items[p]
      if (!provider || typeof provider.name !== "string" || provider.name.length > 160
          || typeof provider.behavior !== "string" || provider.behavior.length > 80
          || typeof provider.updatedAt !== "string" || provider.updatedAt.length > 80
          || typeof provider.ruleCount !== "number"
          || provider.ruleCount < -1 || provider.ruleCount > 1000000000
          || typeof provider.refreshable !== "boolean"
          || ["loaded", "empty", "unknown"].indexOf(provider.status) < 0) return false
      nextProviders.push({
        name: plainText(provider.name, 160),
        behavior: plainText(provider.behavior, 80),
        ruleCount: Math.floor(provider.ruleCount),
        updatedAt: plainText(provider.updatedAt, 80),
        status: provider.status,
        refreshable: provider.refreshable
      })
    }
    loadedRules = nextRules
    loadedRuleTotal = Math.floor(rules.total)
    loadedRulesTruncated = rules.truncated
    loadedRuleProviders = nextProviders
    loadedRuleProviderTotal = Math.floor(providers.total)
    loadedRuleProvidersTruncated = providers.truncated
    advancedDiagnosticsLoadedAt = Date.now()
    advancedDiagnosticsErrorCode = ""
    advancedDiagnosticsError = ""
    return true
  }

  function sampleTraffic() {
    if (trafficProcess.running || activeDevices.length === 0) return
    trafficProcess.command = ["bash", "-c", trafficScript, "profile"].concat(activeDevices)
    trafficProcess.running = true
  }

  function fetchDetails() {
    if (detailsProcess.running || primaryUuid === "") return
    _detailsFor = primaryUuid
    detailsProcess.command = ["bash", backendPath, "details", primaryUuid, primaryDevice]
    detailsProcess.running = true
  }

  function applyDetails(raw) {
    var payload
    try {
      payload = JSON.parse(String(raw || ""))
    } catch (error) {
      return false
    }
    if (!payload || payload.version !== 1
        || typeof payload.address !== "string" || typeof payload.server !== "string"
        || typeof payload.transport !== "string" || typeof payload.sni !== "string")
      return false
    details = {
      version: 1,
      address: plainText(payload.address, 128),
      server: plainText(payload.server, 320),
      transport: plainText(payload.transport, 160),
      sni: plainText(payload.sni, 253)
    }
    detailsUuid = _detailsFor
    return true
  }

  function samplePing() {
    if (pingProcess.running || pingHost === "" || primaryDevice === "") return
    // Tagged with the tunnel it was sent for: a probe takes up to two
    // seconds, and a switch or a closing panel inside that window clears the
    // samples — the reply must not land in the window that replaced them.
    _pingFor = primaryUuid
    // The tunnel address is the fallback binding for kernels that refuse
    // SO_BINDTODEVICE to an unprivileged ping; without either, the probe
    // would measure the physical link and call it the tunnel's latency.
    pingProcess.command = ["bash", "-c", pingScript, "profile",
      primaryDevice, detail("address").split("/")[0], pingHost]
    pingProcess.running = true
  }

  function testActiveConnection() {
    if (!active || primaryUuid === "" || primaryDevice === "" || pingHost === "")
      return false
    // A manual check starts a fresh window. If the regular three-second
    // sampler is already in flight, claim that exact tunnel-bound result
    // instead of launching a duplicate ping beside it.
    pingSamples = []
    _manualPingRequested = true
    if (!pingProcess.running) samplePing()
    refreshExitIp()
    return true
  }

  function parsePublicIp(raw) {
    var value = String(raw || "").trim()
    if (value.length < 3 || value.length > 45 || /[^0-9A-Fa-f:.]/.test(value)) return ""
    if (value.indexOf(":") >= 0)
      return /^[0-9A-Fa-f:]+$/.test(value) && value.indexOf(":::") < 0 ? value : ""
    var parts = value.split(".")
    if (parts.length !== 4) return ""
    for (var i = 0; i < parts.length; i++) {
      if (!/^\d{1,3}$/.test(parts[i]) || Number(parts[i]) > 255) return ""
    }
    return value
  }

  function refreshExitIp() {
    if (!supports("exitIp") || !active || exitIpProcess.running) return false
    _exitIpFor = primaryUuid + "|" + routing.mode
    exitIpFetching = true
    exitIpFailed = false
    exitIpProcess.running = true
    return true
  }

  // -1 is a timeout, which counts as loss; a probe that could not run at all
  // (no ping binary, a rejected bind) samples nothing rather than reporting
  // a tunnel-shaped problem that is really a local one.
  function addPingSample(ms) {
    var next = pingSamples.slice()
    next.push(ms)
    while (next.length > 10) next.shift()
    pingSamples = next
  }

  function applyTraffic(raw) {
    var now = Date.now()
    var next = {}
    var lines = String(raw || "").split("\n")
    for (var i = 0; i < lines.length; i++) {
      var parts = lines[i].trim().split(/\s+/)
      if (parts.length !== 3) continue
      var dev = parts[0]
      var rx = Number(parts[1])
      var tx = Number(parts[2])
      if (!isFinite(rx) || !isFinite(tx)) continue
      var prev = traffic[dev]
      var dt = prev ? (now - prev.at) / 1000 : 0
      // A first sample, restarted counters (the interface was recreated
      // behind our back) or a gap left by a closed panel all make the delta
      // meaningless: keep the totals, hold the rate at zero for one tick.
      // `rated` marks a rate that was actually measured: the zero a first
      // sample carries means "no interval yet", and reporting that as
      // 0 B/s claims an idle tunnel on no evidence.
      if (!prev || rx < prev.rx || tx < prev.tx || dt <= 0 || dt > 30) {
        next[dev] = { rx: rx, tx: tx, at: now, rxRate: 0, txRate: 0, rated: false }
      } else {
        next[dev] = { rx: rx, tx: tx, at: now, rated: true,
          rxRate: (rx - prev.rx) / dt, txRate: (tx - prev.tx) / dt }
      }
    }
    // Wholesale replacement drops devices that vanished with their tunnels.
    traffic = next
    var primary = next[primaryDevice]
    if (primary && primary.rated) {
      var nextRx = rxHistory.slice()
      var nextTx = txHistory.slice()
      nextRx.push(Math.max(0, primary.rxRate))
      nextTx.push(Math.max(0, primary.txRate))
      while (nextRx.length > historyMaxPoints) nextRx.shift()
      while (nextTx.length > historyMaxPoints) nextTx.shift()
      rxHistory = nextRx
      txHistory = nextTx
    }
  }

  // Single-letter units and one decimal at most: the line has to share a
  // caption row with three action buttons, so every character counts.
  function fmtBytes(n) {
    var v = Number(n) || 0
    if (v < 1024) return Math.round(v) + "B"
    var units = ["K", "M", "G", "T"]
    for (var i = 0; i < units.length; i++) {
      v /= 1024
      if (v < 1024 || i === units.length - 1) break
    }
    return (v >= 100 ? v.toFixed(0) : v.toFixed(1)) + units[i]
  }

  // The detail grid has a column of its own to fill, so it gets the spaced,
  // two-letter units the rest of the shell uses — fmtBytes stays compact for
  // the row caption it shares with three buttons.
  function fmtSize(n) {
    var v = Number(n)
    if (!isFinite(v) || v < 0) v = 0
    if (v < 1024) return Math.round(v) + " B"
    if (v < 1048576) return (v / 1024).toFixed(1) + " KB"
    if (v < 1073741824) return (v / 1048576).toFixed(1) + " MB"
    return (v / 1073741824).toFixed(2) + " GB"
  }

  function fmtRate(n) {
    return fmtSize(n) + "/s"
  }

  function fmtPing(ms) {
    if (!hasPing) return "--"
    var v = Number(ms)
    if (!isFinite(v) || v < 0) return "Timeout"
    return v.toFixed(v > 0 && v < 10 ? 1 : 0) + " ms"
  }

  function fmtLoss(percent) {
    return hasPing ? String(percent) + "%" : "--"
  }

  // Live counters for one device, or zeros — the grid keeps its rows in
  // place from the moment a tunnel comes up, so it needs an answer before
  // the first sample lands.
  function trafficOf(dev) {
    var t = traffic[String(dev || "")]
    return t ? t : { rx: 0, tx: 0, at: 0, rxRate: 0, txRate: 0, rated: false }
  }

  // Sampling stops with the panel, so a stored sample is only worth printing
  // for as long as it can still be true. Past that the honest answer is
  // "--": zeros would claim a tunnel that has moved nothing, and the last
  // rate would claim traffic that stopped being measured minutes ago.
  readonly property int trafficStaleMs: 10000

  function trafficLive(t) {
    return !!t && t.at > 0 && (Date.now() - t.at) <= trafficStaleMs
  }

  function trafficTotal(dev, key) {
    var t = trafficOf(dev)
    return trafficLive(t) ? fmtSize(t[key]) : "--"
  }

  function trafficRate(dev, key) {
    var t = trafficOf(dev)
    return trafficLive(t) && t.rated ? fmtRate(t[key]) : "--"
  }

  // The panel's grid as one line, for scripts and for anyone who would
  // rather not open a popup to read six numbers.
  function detailsText() {
    if (!active) return "VPN disconnected"
    var parts = [primaryName]
    parts.push("routing=" + routingTitle + " (" + routingSummary + ")")
    parts.push("address=" + (detail("address") !== "" ? detail("address") : "--"))
    parts.push("server=" + (detail("server") !== "" ? detail("server") : "--"))
    parts.push("transport=" + (detail("transport") !== "" ? detail("transport") : "--"))
    parts.push("sni=" + (detail("sni") !== "" ? detail("sni") : "--"))
    // Counters are sampled only while the panel is open, so a headless
    // caller usually gets "--" here rather than a total from whenever it
    // was last looked at.
    parts.push("rx=" + trafficTotal(primaryDevice, "rx") + " (" + trafficRate(primaryDevice, "rxRate") + ")")
    parts.push("tx=" + trafficTotal(primaryDevice, "tx") + " (" + trafficRate(primaryDevice, "txRate") + ")")
    parts.push("ping=" + fmtPing(pingLatency) + " loss=" + fmtLoss(pingLoss))
    return parts.join(" · ")
  }

  // One caption line: current rate, then session totals. Empty until there
  // is something live to say — the row falls back to its plain state text,
  // which beats a line of stale numbers.
  function trafficLine(dev) {
    var t = traffic[String(dev || "")]
    if (!trafficLive(t)) return ""
    return "↓ " + fmtBytes(t.rxRate) + "/s ↑ " + fmtBytes(t.txRate) + "/s"
      + " · ↓ " + fmtBytes(t.rx) + " ↑ " + fmtBytes(t.tx)
  }

  // First profile with this name, or null. Only for the name-based entry
  // points (import replace, IPC), and only once countByName has ruled out
  // duplicates — with several matches the first one is the wrong answer as
  // often as the right one. Everything row-bound carries its own profile.
  function findByName(name) {
    var value = String(name || "")
    for (var i = 0; i < profiles.length; i++) {
      if ((profiles[i].rawName || profiles[i].name) === value) return profiles[i]
    }
    return null
  }

  function findByUuid(uuid) {
    var value = String(uuid || "")
    for (var i = 0; i < profiles.length; i++) {
      if (profiles[i].uuid === value) return profiles[i]
    }
    return null
  }

  // One resolver for every name-or-UUID IPC action. Keeping ambiguity rules
  // here prevents edit, rename, export and QR from drifting apart as the
  // profile model evolves.
  function resolveTarget(target) {
    var value = String(target || "")
    var profile = findByUuid(value)
    if (profile) return { profile: profile, error: "" }
    var count = countByName(value)
    if (count === 0) return { profile: null, error: "no such profile: " + value }
    if (count > 1) {
      return {
        profile: null,
        error: "ambiguous name: " + value + " — use a UUID: " + uuidsForName(value).join(" ")
      }
    }
    return { profile: findByName(value), error: "" }
  }

  function countByName(name) {
    var value = String(name || "")
    var n = 0
    for (var i = 0; i < profiles.length; i++) {
      if ((profiles[i].rawName || profiles[i].name) === value) n++
    }
    return n
  }

  function uuidsForName(name) {
    var value = String(name || "")
    var out = []
    for (var i = 0; i < profiles.length; i++) {
      if ((profiles[i].rawName || profiles[i].name) === value) out.push(profiles[i].uuid)
    }
    return out
  }

  function applyStatus(raw) {
    var payload
    try {
      payload = JSON.parse(String(raw || ""))
    } catch (error) {
      return rejectStatus()
    }
    if (!payload || payload.version !== 1 || !Array.isArray(payload.profiles)
        || payload.profiles.length > 256) return rejectStatus()
    var featureSource = payload.capabilities
    var featureNames = ["subscriptions", "subscriptionSearch", "routingModes",
      "connectionTest", "liveTraffic", "trafficHistory", "exitIp",
      "conflictDetection", "qr", "trojanExperimental", "hysteria2Experimental",
      "tuicExperimental"]
    if (!featureSource || typeof featureSource.core !== "string"
        || featureSource.core.length > 32 || !Array.isArray(featureSource.protocols)
        || featureSource.protocols.length > 16)
      return rejectStatus()
    var featureMap = { core: plainText(featureSource.core, 32), protocols: [] }
    for (var f = 0; f < featureNames.length; f++) {
      if (typeof featureSource[featureNames[f]] !== "boolean") return rejectStatus()
      featureMap[featureNames[f]] = featureSource[featureNames[f]]
    }
    for (var fp = 0; fp < featureSource.protocols.length; fp++) {
      if (typeof featureSource.protocols[fp] !== "string"
          || featureSource.protocols[fp].length > 32) return rejectStatus()
      featureMap.protocols.push(plainText(featureSource.protocols[fp], 32))
    }
    var route = payload.routing
    if (!route || typeof route.mode !== "string" || typeof route.source !== "string"
        || typeof route.preset !== "string" || route.preset.length > 64
        || typeof route.configured !== "boolean"
        || typeof route.ruleCount !== "number" || typeof route.providerCount !== "number"
        || typeof route.customRuleCount !== "number"
        || typeof route.rulesUpdatedAt !== "number"
        || typeof route.ruleUpdateAvailable !== "boolean"
        || !isFinite(route.ruleCount) || !isFinite(route.providerCount)
        || !isFinite(route.customRuleCount) || !isFinite(route.rulesUpdatedAt)
        || route.ruleCount < 0 || route.providerCount < 0
        || route.customRuleCount < 0 || route.customRuleCount > 128
        || route.rulesUpdatedAt < 0)
      return rejectStatus()
    var setup = payload.coreSetup
    var picker = payload.filePicker
    var helpers = payload.desktopHelpers
    var startupSource = payload.startup
    if (!setup || typeof setup.installed !== "boolean"
        || typeof setup.tunReady !== "boolean" || typeof setup.path !== "string"
        || setup.path.length > 4096 || !picker
        || typeof picker.available !== "boolean"
        || typeof picker.provider !== "string"
        || ["", "zenity", "kdialog", "yad", "gtk4"].indexOf(picker.provider) < 0
        || picker.available !== (picker.provider !== "") || !helpers
        || typeof helpers.configEditorAvailable !== "boolean"
        || typeof helpers.qrEncoderAvailable !== "boolean" || !startupSource
        || typeof startupSource.enabled !== "boolean"
        || typeof startupSource.configured !== "boolean"
        || (startupSource.target !== "last" && startupSource.target !== "profile")
        || typeof startupSource.profileId !== "string" || startupSource.profileId.length > 64
        || (startupSource.mode !== "rule" && startupSource.mode !== "global")
        || typeof payload.onboardingComplete !== "boolean")
      return rejectStatus()
    var subscriptionSource = payload.subscriptions === undefined ? [] : payload.subscriptions
    if (!Array.isArray(subscriptionSource) || subscriptionSource.length > 64) return rejectStatus()
    var conflictSource = payload.conflicts === undefined ? [] : payload.conflicts
    if (!Array.isArray(conflictSource) || conflictSource.length > 16
        || typeof payload.uptimeSeconds !== "number" || !isFinite(payload.uptimeSeconds)
        || payload.uptimeSeconds < 0 || payload.uptimeSeconds > 315360000)
      return rejectStatus()
    var conflictList = []
    for (var c = 0; c < conflictSource.length; c++) {
      if (typeof conflictSource[c] !== "string" || conflictSource[c].length > 120)
        return rejectStatus()
      conflictList.push(plainText(conflictSource[c], 120))
    }
    var subscriptionList = []
    var subscriptionIds = {}
    for (var s = 0; s < subscriptionSource.length; s++) {
      var sub = subscriptionSource[s]
      if (!sub || typeof sub.id !== "string" || sub.id === ""
          || sub.id.length > 64 || typeof sub.name !== "string" || sub.name === ""
          || sub.name.length > 80
          || typeof sub.updatedAt !== "number" || !isFinite(sub.updatedAt) || sub.updatedAt < 0
          || typeof sub.profileCount !== "number" || !isFinite(sub.profileCount) || sub.profileCount < 0
          || typeof sub.staleCount !== "number" || !isFinite(sub.staleCount) || sub.staleCount < 0
          || subscriptionIds[sub.id] !== undefined)
        return rejectStatus()
      var publicSub = {
        uuid: sub.id, rawName: sub.name, name: plainText(sub.name, 80),
        updatedAt: Math.floor(sub.updatedAt),
        profileCount: Math.floor(sub.profileCount), staleCount: Math.floor(sub.staleCount)
      }
      subscriptionIds[sub.id] = publicSub
      subscriptionList.push(publicSub)
    }
    var list = []
    var byUuid = {}
    var activeCount = 0
    for (var i = 0; i < payload.profiles.length; i++) {
      var source = payload.profiles[i]
      if (!source || typeof source.id !== "string" || source.id === ""
          || source.id.length > 64 || typeof source.name !== "string" || source.name === ""
          || source.name.length > 80 || typeof source.device !== "string"
          || source.device.length > 64 || typeof source.active !== "boolean"
          || byUuid[source.id] !== undefined)
        return rejectStatus()
      var subscriptionId = source.subscriptionId === undefined ? "" : source.subscriptionId
      var sourceName = source.sourceName === undefined ? "" : source.sourceName
      var server = source.server === undefined ? "" : source.server
      var protocol = source.protocol === undefined ? "" : source.protocol
      var missing = source.missing === undefined ? false : source.missing
      var favorite = source.favorite === undefined ? false : source.favorite
      if (typeof subscriptionId !== "string" || subscriptionId.length > 64
          || typeof sourceName !== "string" || sourceName.length > 80
          || typeof server !== "string" || server.length > 253
          || typeof protocol !== "string" || protocol === "" || protocol.length > 32
          || featureMap.protocols.indexOf(protocol) < 0
          || typeof missing !== "boolean" || typeof favorite !== "boolean"
          || (subscriptionId !== "" && subscriptionIds[subscriptionId] === undefined))
        return rejectStatus()
      var entry = {
        uuid: source.id,
        ifname: plainText(source.device, 64),
        rawName: source.name,
        name: plainText(source.name, 80),
        protocol: plainText(protocol, 32),
        server: plainText(server, 253),
        active: source.active === true,
        subscriptionUuid: subscriptionId,
        sourceName: plainText(sourceName, 80),
        missing: missing,
        favorite: favorite,
        managed: subscriptionId !== ""
      }
      list.push(entry)
      byUuid[entry.uuid] = entry
      if (entry.active) activeCount++
    }
    if (activeCount > 1 || typeof payload.activeId !== "string"
        || typeof payload.lastId !== "string") return rejectStatus()
    var firstUp = payload.activeId !== "" && byUuid[payload.activeId]
      && byUuid[payload.activeId].active ? payload.activeId : ""
    if ((activeCount === 1) !== (firstUp !== "")) return rejectStatus()
    if (payload.lastId !== "" && byUuid[payload.lastId] === undefined) return rejectStatus()
    lastUuid = payload.lastId
    coreSetup = {
      installed: setup.installed,
      tunReady: setup.tunReady,
      path: plainText(setup.path, 4096)
    }
    filePicker = {
      available: picker.available,
      provider: picker.provider
    }
    desktopHelpers = {
      configEditorAvailable: helpers.configEditorAvailable,
      qrEncoderAvailable: helpers.qrEncoderAvailable
    }
    startup = {
      enabled: startupSource.enabled,
      configured: startupSource.configured,
      target: startupSource.target,
      profileId: plainText(startupSource.profileId, 64),
      mode: startupSource.mode
    }
    onboardingComplete = payload.onboardingComplete
    var routingModeStatusIsFinal = _pendingRoutingMode !== ""
      && !controlProcess.running
      && _routingModeRequiredStatusGeneration > 0
      && _runningStatusGeneration >= _routingModeRequiredStatusGeneration
    var displayedRoutingMode = _pendingRoutingMode !== "" && !routingModeStatusIsFinal
      ? _routingModeBeforeChange : route.mode
    routing = {
      mode: displayedRoutingMode,
      source: route.source,
      preset: plainText(route.preset, 64),
      configured: route.configured,
      ruleCount: Math.floor(route.ruleCount),
      providerCount: Math.floor(route.providerCount),
      customRuleCount: Math.floor(route.customRuleCount),
      rulesUpdatedAt: Math.floor(route.rulesUpdatedAt),
      ruleUpdateAvailable: route.ruleUpdateAvailable
    }
    if (_lastExitRoutingMode !== displayedRoutingMode) {
      _lastExitRoutingMode = displayedRoutingMode
      exitIp = ""
      exitIpFailed = false
      rxHistory = []
      txHistory = []
      exitIpDelay.restart()
    }
    if (routingModeStatusIsFinal) {
      _pendingRoutingMode = ""
      _routingModeBeforeChange = ""
      _routingModeRequiredStatusGeneration = 0
      actionStatus = ""
      if (_pendingSaveUuid !== "") Qt.callLater(_flushPendingSave)
    }
    conflicts = conflictList
    uptimeSeconds = Math.floor(payload.uptimeSeconds)
    capabilities = featureMap
    // Transitions against the previous poll. Every profile that went down
    // is queued for notify-drop — the backend's intent markers decide,
    // under the lock, which drops were ours; judging only the first could
    // hide an external drop behind an intentional one. Every profile that
    // came up re-arms its marker via mark-active, so an activation the
    // widget merely observed through systemd or another client gets its
    // notifications back.
    var droppedNow = []
    for (var uuid0 in _prevActive) {
      var cur = byUuid[uuid0]
      if (cur === undefined || !cur.active) droppedNow.push({ uuid: uuid0, name: _prevActive[uuid0] })
    }
    var nowActive = {}
    var observedAt = String(_statusStartedAt > 0 ? _statusStartedAt : Date.now())
    for (var k = 0; k < list.length; k++) {
      if (!list[k].active) continue
      nowActive[list[k].uuid] = list[k].name
      // Queued, not fired directly: the process may be busy, and a lost
      // activation would leave a stale marker muting a real drop for up to
      // the TTL. The observation time rides along so a delayed mark-active
      // cannot erase the record of a down that happened after it.
      if (_prevActive[list[k].uuid] === undefined) _markQueue.push(list[k].uuid + ":" + observedAt)
    }
    _prevActive = nowActive
    for (var d = 0; d < droppedNow.length; d++) _dropQueue.push(droppedNow[d])
    _flushDrops()
    _flushMarkActive()
    // Sorted after the active flags are in, because the flag is the first
    // sort key: what is up is what the panel is opened for, and the grid
    // above describes the profile that lands at the top. Name breaks ties,
    // so an otherwise unchanged list never reshuffles.
    list.sort(function(a, b) {
      if (a.active !== b.active) return a.active ? -1 : 1
      if (a.favorite !== b.favorite) return a.favorite ? -1 : 1
      return a.name < b.name ? -1 : (a.name > b.name ? 1 : 0)
    })
    profiles = list
    subscriptions = subscriptionList
    // A drop warning describes the absence of a tunnel, not a permanent
    // fault. A later successful observation of any active profile proves the
    // VPN recovered (often by switching servers), so retract only that exact
    // warning while leaving unrelated control errors untouched.
    if (firstUp !== "" && _dropWarningText !== "" && lastError === _dropWarningText) {
      lastError = ""
      _dropWarningText = ""
    }
    // Track connects made outside the widget too (systemd or another client).
    if (firstUp !== "") rememberLast(firstUp)
    // A successful poll retracts only its own earlier failure — an error (or
    // warning) from a control operation must outlive the refresh that every
    // operation triggers, or it flashes for one poll and vanishes.
    if (_pollError) {
      _pollError = false
      lastError = ""
    }
    return true
  }

  function rejectStatus() {
    lastError = "Failed to read OmaVLESS status"
    _pollError = true
    return false
  }

  function rememberLast(uuid) {
    var value = String(uuid || "")
    if (value === "" || value === lastUuid) return
    lastUuid = value
  }

  function connectTo(profile) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    if (!profile || !profile.uuid) return rejectAction("no such profile")
    actionRejection = ""
    actionStatus = "Connecting " + profile.name + "…"
    _pendingConnect = String(profile.uuid)
    runControl(["connect", profile.uuid])
    return true
  }

  function disconnectOne(profile) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    if (!profile || !profile.uuid) return rejectAction("no such profile")
    actionRejection = ""
    actionStatus = "Disconnecting " + profile.name + "…"
    runControl(["down", profile.uuid])
    return true
  }

  function disconnectAll() {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    actionRejection = ""
    actionStatus = "Disconnecting…"
    runControl(["down-all"])
    return true
  }

  function setRoutingMode(mode) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    var value = String(mode || "")
    if (value !== "rule" && value !== "global" && value !== "direct")
      return rejectAction("unsupported routing mode")
    // Idempotent clicks are real successes and must not bounce the tunnel.
    if (routing.mode === value) {
      actionRejection = ""
      lastError = ""
      actionStatus = ""
      return true
    }
    actionRejection = ""
    actionStatus = value === "rule"
      ? "Enabling routed VPN…"
      : (value === "global" ? "Sending all traffic through VPN…" : "Bypassing VPN…")
    // The backend temporarily stages the requested template before the
    // privileged reconnect completes. Keep showing the last authoritative
    // mode until a poll requested after process completion confirms the
    // actual success or rollback.
    _pendingRoutingMode = value
    _routingModeBeforeChange = routing.mode
    _routingModeRequiredStatusGeneration = 0
    runControl(["set-mode", value])
    return true
  }

  function useRoutingPreset(profile, keepMode) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    var value = String(profile || "")
    if (!routingPresetById(value)) return rejectAction("unsupported routing preset")
    if (routingPresetConfigured && routing.preset === value) {
      if (keepMode || routing.mode === "rule") {
        actionRejection = ""
        lastError = ""
        actionStatus = ""
        return true
      }
      return setRoutingMode("rule")
    }
    actionRejection = ""
    actionStatus = "Applying " + routingPresetById(value).name + "…"
    var args = ["use-routing", value]
    if (keepMode) args.push("--keep-mode")
    runControl(args)
    return true
  }

  function configureStartup(enabled, target, profileUuid, mode) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    var wantedTarget = String(target || "")
    var wantedProfile = String(profileUuid || "")
    var wantedMode = String(mode || "")
    if (wantedTarget !== "last" && wantedTarget !== "profile")
      return rejectAction("unsupported login autoconnect target")
    if (wantedMode !== "rule" && wantedMode !== "global")
      return rejectAction("login autoconnect supports Routing or Full VPN")
    if (enabled && wantedTarget === "profile" && !findByUuid(wantedProfile))
      return rejectAction("choose a profile for login autoconnect")
    actionRejection = ""
    actionStatus = enabled ? "Saving login autoconnect…" : "Disabling login autoconnect…"
    runControl([
      "startup-configure", enabled ? "on" : "off", wantedTarget,
      wantedTarget === "profile" ? wantedProfile : "", wantedMode
    ])
    return true
  }

  function completeOnboarding() {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    actionRejection = ""
    actionStatus = "Finishing setup…"
    runControl(["onboarding-complete"])
    return true
  }

  function loadCustomRules() {
    if (customRulesProcess.running) return false
    routingToolError = ""
    customRulesProcess.command = ["bash", backendPath, "custom-rules"]
    customRulesProcess.running = true
    return true
  }

  function addCustomRule(kind, action, value) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    var matchKind = String(kind || "")
    var routeAction = String(action || "")
    var matchValue = String(value || "").trim()
    if (["domain", "suffix", "ipcidr"].indexOf(matchKind) < 0)
      return rejectAction("unsupported custom routing match")
    if (["proxy", "direct", "reject"].indexOf(routeAction) < 0)
      return rejectAction("unsupported custom routing action")
    if (matchValue === "" || matchValue.length > 1024)
      return rejectAction("enter a domain or IP range")
    routingToolError = ""
    routingToolStatus = "Saving custom rule…"
    runControl(["custom-rule-add", matchKind, routeAction], matchValue)
    return true
  }

  function deleteCustomRule(rule) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    if (!rule || !rule.id) return rejectAction("no such custom routing rule")
    routingToolError = ""
    routingToolStatus = "Removing custom rule…"
    runControl(["custom-rule-delete", String(rule.id)])
    return true
  }

  function refreshRuleProviders() {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    routingToolError = ""
    routingToolStatus = "Refreshing remote rule data…"
    runControl(["rule-providers-refresh"])
    return true
  }

  function checkRoute(value) {
    if (routeCheckProcess.running) return false
    var query = String(value || "").trim()
    if (query === "" || query.length > 1024) {
      routingToolError = "Enter a domain or IP address"
      return false
    }
    routingToolError = ""
    routingToolStatus = "Checking route…"
    routeCheckResult = null
    _routeCheckInput = query
    routeCheckProcess.stdinEnabled = true
    routeCheckProcess.command = ["bash", backendPath, "route-check"]
    routeCheckProcess.running = true
    return true
  }

  function deleteConfig(profile) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    if (!profile || !profile.uuid) return rejectAction("no such profile")
    actionRejection = ""
    actionStatus = "Deleting " + profile.name + "…"
    runControl(["delete", profile.uuid])
    return true
  }

  function clearSubscriptionMessage() {
    subscriptionStatus = ""
    subscriptionError = ""
  }

  function cancelProbe() {
    if (!probeProcess.running) return false
    _probeCancelRequested = true
    subscriptionError = ""
    subscriptionStatus = "Stopping server test…"
    probeProcess.running = false
    return true
  }

  function rejectSubscriptionAction(reason) {
    subscriptionStatus = ""
    subscriptionError = String(reason)
    return false
  }

  function probeResult(profileUuid) {
    var value = profileProbes[String(profileUuid || "")]
    return value === undefined ? null : value
  }

  function subscriptionProbeTime(subscriptionUuid) {
    var value = Number(subscriptionProbeTimes[String(subscriptionUuid || "")])
    return isFinite(value) && value > 0 ? value : 0
  }

  function clearProbeResults(subscriptionUuid) {
    var target = String(subscriptionUuid || "")
    if (target === "") {
      profileProbes = ({})
      subscriptionProbeTimes = ({})
      return
    }
    var next = {}
    for (var i = 0; i < profiles.length; i++) {
      var profile = profiles[i]
      if (profile.subscriptionUuid === target) continue
      var result = profileProbes[profile.uuid]
      if (result !== undefined) next[profile.uuid] = result
    }
    profileProbes = next
    var nextTimes = {}
    for (var key in subscriptionProbeTimes) {
      if (key !== target) nextTimes[key] = subscriptionProbeTimes[key]
    }
    subscriptionProbeTimes = nextTimes
  }

  function probeSubscription(subscription) {
    if (!subscription || !subscription.uuid) {
      subscriptionError = "No such subscription"
      return false
    }
    if (probeProcess.running) {
      subscriptionError = "Another latency test is already running"
      return false
    }
    if (busy) {
      subscriptionError = "Wait for the current subscription update to finish"
      return false
    }
    var target = String(subscription.uuid)
    var serverCount = 0
    for (var i = 0; i < profiles.length; i++) {
      if (profiles[i].subscriptionUuid === target) serverCount++
    }
    if (serverCount === 0)
      return rejectSubscriptionAction("This subscription has no current servers to test")
    clearProbeResults(target)
    subscriptionError = ""
    subscriptionStatus = "Testing " + subscription.name + " · 0/" + serverCount + " checked"
    probingSubscriptionUuid = target
    _probeCancelRequested = false
    _probeSawComplete = false
    _probeStreamValid = true
    _probeSeenIds = ({})
    _probeSummary = ({ tested: 0, unavailable: 0, unresolved: 0 })
    _probeName = String(subscription.name || "subscription")
    probeCompletedCount = 0
    probeTotalCount = serverCount
    probeElapsedSeconds = 0
    _probeStartedAt = Date.now()
    probeProcess.command = ["bash", backendPath, "subscription-probe-stream", target]
    probeProcess.running = true
    return true
  }

  function applyProbeEvent(raw, expectedSubscriptionUuid) {
    var payload
    try {
      payload = JSON.parse(String(raw || ""))
    } catch (error) {
      return false
    }
    if (!payload || payload.version !== 1
        || payload.subscriptionId !== expectedSubscriptionUuid
        || typeof payload.type !== "string")
      return false

    if (payload.type === "start") {
      if (typeof payload.total !== "number" || !isFinite(payload.total)
          || Math.floor(payload.total) !== payload.total
          || payload.total < 0 || payload.total > 256)
        return false
      probeTotalCount = payload.total
      subscriptionStatus = "Testing " + _probeName + " · "
        + probeCompletedCount + "/" + probeTotalCount + " checked"
      return true
    }

    if (payload.type === "result") {
      var known = false
      for (var p = 0; p < profiles.length; p++) {
        if (profiles[p].subscriptionUuid === expectedSubscriptionUuid
            && profiles[p].uuid === payload.id) {
          known = true
          break
        }
      }
      if (!known || typeof payload.resolved !== "boolean"
          || typeof payload.reachable !== "boolean"
          || typeof payload.latencyMs !== "number" || !isFinite(payload.latencyMs)
          || (!payload.resolved && payload.reachable)
          || (payload.reachable && (payload.latencyMs < 0 || payload.latencyMs > 60000))
          || (!payload.reachable && payload.latencyMs !== -1))
        return false
      var next = Object.assign({}, profileProbes)
      next[payload.id] = {
        resolved: payload.resolved,
        reachable: payload.reachable,
        latencyMs: Math.round(payload.latencyMs)
      }
      profileProbes = next
      if (_probeSeenIds[payload.id] !== true) {
        var seen = Object.assign({}, _probeSeenIds)
        seen[payload.id] = true
        _probeSeenIds = seen
        probeCompletedCount++
      }
      subscriptionStatus = "Testing " + _probeName + " · "
        + probeCompletedCount + "/" + probeTotalCount + " checked"
      return true
    }

    if (payload.type === "complete") {
      var fields = [payload.tested, payload.unavailable, payload.unresolved]
      for (var i = 0; i < fields.length; i++) {
        if (typeof fields[i] !== "number" || !isFinite(fields[i])
            || Math.floor(fields[i]) !== fields[i] || fields[i] < 0 || fields[i] > 256)
          return false
      }
      if (payload.unavailable + payload.unresolved > payload.tested)
        return false
      _probeSummary = {
        tested: payload.tested,
        unavailable: payload.unavailable,
        unresolved: payload.unresolved
      }
      probeCompletedCount = payload.tested
      _probeSawComplete = true
      return true
    }
    return false
  }

  function applyProbeResults(raw, expectedSubscriptionUuid) {
    var payload
    try {
      payload = JSON.parse(String(raw || ""))
    } catch (error) {
      return null
    }
    if (!payload || payload.version !== 1
        || payload.subscriptionId !== expectedSubscriptionUuid
        || !Array.isArray(payload.results) || payload.results.length > 256)
      return null
    var known = {}
    for (var p = 0; p < profiles.length; p++) {
      if (profiles[p].subscriptionUuid === expectedSubscriptionUuid)
        known[profiles[p].uuid] = true
    }
    var next = Object.assign({}, profileProbes)
    var unavailable = 0
    var unresolved = 0
    for (var i = 0; i < payload.results.length; i++) {
      var item = payload.results[i]
      if (!item || typeof item.id !== "string" || known[item.id] !== true
          || typeof item.resolved !== "boolean" || typeof item.reachable !== "boolean"
          || typeof item.latencyMs !== "number" || !isFinite(item.latencyMs)
          || (!item.resolved && item.reachable)
          || (item.reachable && (item.latencyMs < 0 || item.latencyMs > 60000))
          || (!item.reachable && item.latencyMs !== -1))
        return null
      next[item.id] = {
        resolved: item.resolved,
        reachable: item.reachable,
        latencyMs: Math.round(item.latencyMs)
      }
      if (!item.resolved) unresolved++
      else if (!item.reachable) unavailable++
    }
    profileProbes = next
    return { tested: payload.results.length, unavailable: unavailable, unresolved: unresolved }
  }

  function saveSubscription(name, uuid, url) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    if (probingProfiles) return rejectSubscriptionAction("Wait for the latency test to finish")
    var clean = String(name || "").trim()
    var target = String(uuid || "")
    var value = String(url || "").trim()
    if (!isValidName(clean)) return rejectAction("use a non-empty name up to 80 characters")
    if (!/^https?:\/\/[^\s]+$/i.test(value)) return rejectAction("use an http:// or https:// subscription URL")
    actionRejection = ""
    subscriptionError = ""
    subscriptionStatus = target === "" ? "Adding " + clean + "…" : "Saving " + clean + "…"
    // Names are positional data, not argparse options. The explicit boundary
    // keeps a valid provider name such as "--fast" from being reinterpreted.
    runControl(["subscription-save", "--", clean, target], value)
    return true
  }

  function saveSubscriptionFile(name, uuid, path) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    if (probingProfiles) return rejectSubscriptionAction("Wait for the latency test to finish")
    var clean = String(name || "").trim()
    var target = String(uuid || "")
    var value = String(path || "")
    if (!isValidName(clean)) return rejectAction("use a non-empty name up to 80 characters")
    if (value === "" || value.length > 4096 || /[\x00-\x1f\x7f]/.test(value))
      return rejectAction("subscription import file path is invalid")
    actionRejection = ""
    subscriptionError = ""
    subscriptionStatus = "Adding " + clean + "…"
    runControl(["subscription-save-file", "--", clean, target, value])
    return true
  }

  function refreshSubscription(subscription) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    if (probingProfiles) return rejectSubscriptionAction("Wait for the latency test to finish")
    if (!subscription || !subscription.uuid) return rejectAction("no such subscription")
    actionRejection = ""
    clearProbeResults(subscription.uuid)
    subscriptionError = ""
    subscriptionStatus = "Updating " + subscription.name + "…"
    runControl(["subscription-refresh", subscription.uuid])
    return true
  }

  function refreshAllSubscriptions() {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    if (probingProfiles) return rejectSubscriptionAction("Wait for the latency test to finish")
    if (subscriptions.length === 0) return rejectAction("no subscriptions to update")
    actionRejection = ""
    clearProbeResults("")
    subscriptionError = ""
    subscriptionStatus = "Updating all subscriptions…"
    runControl(["subscription-refresh-all"])
    return true
  }

  function deleteSubscription(subscription) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    if (probingProfiles) return rejectSubscriptionAction("Wait for the latency test to finish")
    if (!subscription || !subscription.uuid) return rejectAction("no such subscription")
    actionRejection = ""
    clearProbeResults(subscription.uuid)
    subscriptionError = ""
    subscriptionStatus = "Removing " + subscription.name + "…"
    runControl(["subscription-delete", subscription.uuid])
    return true
  }

  function loadSubscriptionUrl(subscription) {
    if (subscriptionUrlProcess.running) return rejectAction("subscription editor is already loading")
    if (probingProfiles) return rejectSubscriptionAction("Wait for the latency test to finish")
    if (!subscription || !subscription.uuid) return rejectAction("no such subscription")
    actionRejection = ""
    subscriptionError = ""
    _subscriptionUrlUuid = String(subscription.uuid)
    subscriptionUrlProcess.command = ["bash", backendPath, "subscription-url", _subscriptionUrlUuid]
    subscriptionUrlProcess.running = true
    return true
  }

  // Changes only the local display label; the profile link stays intact.
  function renameConfig(profile, newName) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    if (!profile || !profile.uuid) return rejectAction("no such profile")
    var value = String(newName || "").trim()
    if (!isValidName(value)) return rejectAction("use a non-empty name up to 80 characters")
    // Idempotent success: no backend call is needed when it is already named
    // as requested, but IPC may still honestly answer ok.
    if (value === (profile.rawName || profile.name)) {
      actionRejection = ""
      lastError = ""
      actionStatus = ""
      return true
    }
    actionRejection = ""
    actionStatus = "Renaming " + profile.name + "…"
    runControl(["rename", profile.uuid, "--", value])
    return true
  }

  function toggleFavorite(profile) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    if (!profile || !profile.uuid) return rejectAction("no such profile")
    actionRejection = ""
    actionStatus = profile.favorite
      ? "Unpinning " + profile.name + "…"
      : "Pinning " + profile.name + "…"
    runControl(["favorite", profile.uuid, profile.favorite ? "off" : "on"])
    return true
  }

  function toggle() {
    if (active) return disconnectAll()
    if (toggleProfile !== null) return connectTo(toggleProfile)
    return rejectAction("no profile is available")
  }

  // One backend classifier owns both private import sources. A profile opens
  // the redacted profile prompt; a validated subscription URL opens the
  // existing masked subscription confirmation flow.
  signal importReady(string kind, string payload, string suggestedName)
  signal subscriptionImportReady(string kind, string payload, string suggestedName)

  function previewImport(kind, payload, suggestedName) {
    if (previewProcess.running) return rejectAction("another profile preview is already running")
    var sourceKind = String(kind || "")
    var sourcePayload = String(payload || "")
    if ((sourceKind !== "file" && sourceKind !== "text") || sourcePayload === "")
      return rejectAction("no profile input to preview")
    actionRejection = ""
    lastError = ""
    actionStatus = "Checking import…"
    importPreview = ({})
    _previewKind = sourceKind
    _previewPayload = sourcePayload
    _previewSuggested = String(suggestedName || "")
    previewProcess.stdinEnabled = sourceKind === "text"
    previewProcess.command = sourceKind === "file"
      ? ["bash", backendPath, "import-preview", "--", sourcePayload]
      : ["bash", backendPath, "import-preview"]
    previewProcess.running = true
    return true
  }

  function pickConfigFile() {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    if (editProcess.running) return rejectAction("close the profile editor before importing")
    if (clipboardProcess.running) return rejectAction("clipboard import is already running")
    if (qrVisible) return rejectAction("close the QR code before importing")
    if (pickerProcess.running) return rejectAction("the file picker is already open")
    if (!filePicker.available) return rejectAction(
      "File import unavailable — file picker missing. Run “omarchy pkg add zenity”")
    actionRejection = ""
    lastError = ""
    actionStatus = "Waiting for the file picker…"
    pickerProcess.running = true
    return true
  }

  function pasteConfig() {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    if (editProcess.running) return rejectAction("close the profile editor before importing")
    if (pickerProcess.running) return rejectAction("close the file picker before importing")
    if (qrVisible) return rejectAction("close the QR code before importing")
    if (clipboardProcess.running) return rejectAction("clipboard import is already running")
    actionRejection = ""
    lastError = ""
    actionStatus = "Reading clipboard…"
    clipboardProcess.running = true
    return true
  }

  function exportDiagnostics() {
    if (diagnosticsProcess.running)
      return rejectAction("diagnostics export is already running")
    actionRejection = ""
    diagnosticsStatus = "Choosing a destination…"
    diagnosticsProcess.command = [
      "bash", "-c", diagnosticsExportScript, "omavless", backendPath
    ]
    diagnosticsProcess.running = true
    return true
  }

  function copyText(value) {
    var text = String(value || "")
    if (text === "" || text === "--") return false
    if (copyProcess.running) return rejectAction("another copy is already running")
    actionRejection = ""
    _copyText = text
    copyProcess.stdinEnabled = true
    copyProcess.running = true
    return true
  }

  // Opens the profile link in zenity's editable view.
  // Saving goes back through import, so an edit gets the same parsing and
  // validation as an import. Whether the tunnel comes back up is the
  // backend's call, made inside the replace transaction from what is active
  // at that moment — not from a snapshot taken when the editor opened.
  // `seedText` reopens the editor on text that was rejected rather than on
  // what is stored, so a refused save costs a keystroke to fix instead of
  // the whole edit.
  //
  // Emitted when the handoff produced no editor at all — the caller may have
  // closed the panel to get out of zenity's way, and lastError has nowhere
  // to be read. A cancelled or unchanged edit is not a failure.
  signal editFailed(string reason)
  // Terminal editor outcomes that need no panel rescue: cancellation,
  // unchanged text, and a completed save. Panel uses this only to retire its
  // handoff marker, so a later headless failure cannot reopen the panel.
  signal editFinished()

  function editConfig(profile, seedText) {
    if (!profile || !profile.uuid) return rejectAction("no such profile")
    if (pickerProcess.running) return rejectAction("close the file picker before editing")
    if (clipboardProcess.running) return rejectAction("clipboard import is still running")
    if (qrVisible) return rejectAction("close the QR code before editing")
    if (editProcess.running) return rejectAction("the editor is already open")
    if (_pendingSaveUuid !== "" || _editRetryUuid !== "")
      return rejectAction("a previous editor save is still pending")
    if (!desktopHelpers.configEditorAvailable) {
      return rejectAction(configEditorMissingFallback, "config_editor_missing")
    }
    desktopHelperErrorCode = ""
    actionRejection = ""
    _editUuid = String(profile.uuid)
    _editName = String(profile.name)
    if (!seedText) lastError = ""
    actionStatus = "Editing " + _editName + "…"
    // The seed goes over stdin — it is a credential-bearing profile link,
    // and argv is world-readable via /proc.
    _editSeed = String(seedText || "")
    editProcess.stdinEnabled = true
    editProcess.command = ["bash", backendPath, "edit", _editUuid, "--", _editName]
    editProcess.running = true
    return true
  }

  // Export hands the credential-bearing profile link out of private storage.
  // File export is IPC-only (an explicit path in argv is already a
  // deliberate act); the QR is rendered by the backend into XDG_RUNTIME_DIR
  // and displayed by the centred QR window, which owns the PNG until closeQr.
  property string qrPath: ""
  property string qrName: ""
  // The QR window is the only surface a QR request reports through — the
  // panel closes the moment one starts, so a render failure has to be
  // visible there rather than in lastError (which would also leave the bar
  // icon urgent for a problem the user has already dismissed).
  property bool qrLoading: false
  // Stable local UI code only. Never forward qrencode/backend stderr to QML.
  property string qrErrorCode: ""
  readonly property bool qrVisible: qrLoading || qrPath !== "" || qrErrorCode !== ""

  function exportToPath(profile, path) {
    if (!profile || !profile.uuid) return rejectAction("no such profile")
    if (exportProcess.running) return rejectAction("an export is already running")
    var dest = String(path || "")
    if (dest === "") return rejectAction("no destination path")
    actionRejection = ""
    lastError = ""
    _exportDest = dest
    actionStatus = "Exporting " + profile.name + "…"
    exportProcess.command = ["bash", backendPath, "export-file", "--", profile.uuid, dest]
    exportProcess.running = true
    return true
  }

  // Returns "" when a code is on its way, or why nothing will appear.
  function showQr(profile) {
    if (!profile || !profile.uuid) return "no such profile"
    if (editProcess.running || pickerProcess.running)
      return "close the open editor or file picker first"
    if (qrProcess.running) {
      // closeQr retracts a render but lets it finish, so the process can be
      // busy with nothing on screen. Say so there rather than dropping the
      // request silently; while a wanted render is still running the window
      // already says what it is doing, so leave it alone.
      if (!_qrWanted) {
        qrName = String(profile.name)
        qrErrorCode = "busy"
      }
      return "another QR code is still rendering"
    }
    closeQr()
    _qrWanted = true
    // Named now, not on exit: the window opens on the request, so its title
    // has to read right while the code is still being rendered.
    qrName = String(profile.name)
    if (!desktopHelpers.qrEncoderAvailable) {
      qrErrorCode = "dependency_missing"
      return ""
    }
    qrLoading = true
    qrProcess.command = ["bash", backendPath, "qr-png", profile.uuid]
    qrProcess.running = true
    return ""
  }

  function closeQr() {
    // Also retracts a request still in flight: the window may close while
    // qr-png runs, and a PNG nobody is waiting for must not linger — the
    // process handler deletes an unwanted result instead of keeping it.
    // The process itself is left to finish: killing qrencode mid-write would
    // strand the file mktemp already created.
    _qrWanted = false
    removeQrFile(qrPath)
    qrPath = ""
    qrName = ""
    qrLoading = false
    qrErrorCode = ""
  }

  // Each path gets its own detached remover. A shared Process can have its
  // command overwritten by a second close or an unwanted render result,
  // leaving private-key PNGs behind; detached children also outlive Service
  // destruction long enough to perform this tiny, path-specific cleanup.
  function removeQrFile(path) {
    var knownPath = String(path || "")
    if (knownPath !== "") Quickshell.execDetached(["rm", "-f", "--", knownPath])
  }

  // A shell reload destroys this Service without a window-close signal.  The
  // current PNG is known to this instance, so remove only that path; do not
  // sweep omavless-qr.* broadly because another monitor can legitimately own one.
  Component.onDestruction: {
    closeQr()
    // Omarchy has no uninstall hook: `plugin remove` first unloads this QML,
    // then deletes the checkout. The detached guard waits through hot reloads
    // but cleans runtime units after an explicit disable or checkout removal.
    Quickshell.execDetached(["bash", backendPath, "watch-plugin-removal"])
  }
  // SIGKILL and a hard shell crash cannot run the destruction handler. The
  // backend identifies PNGs by this shell's parent PID, so startup safely
  // reaps files from a dead previous shell without touching a live monitor.
  Component.onCompleted: Quickshell.execDetached(["bash", backendPath, "cleanup-runtime"])

  function _flushDrops() {
    if (notifyProcess.running || _dropQueue.length === 0) return
    var drop = _dropQueue.shift()
    _notifyDropName = drop.name
    // The backend needs only the opaque UUID to deduplicate the desktop
    // notice. Keep provider-controlled display metadata out of its argv.
    notifyProcess.command = ["bash", backendPath, "notify-drop", drop.uuid]
    notifyProcess.running = true
  }

  function _flushMarkActive() {
    if (markActiveProcess.running || _markQueue.length === 0) return
    _markInFlight = _markQueue
    _markQueue = []
    markActiveProcess.command = ["bash", "-c", markActiveScript, "profile", backendPath].concat(_markInFlight)
    markActiveProcess.running = true
  }

  // Puts rejected text back in front of the user with the reason showing.
  // Deferred through a timer because the editor process that produced the
  // text is still winding down when the rejection lands.
  function retryEdit(uuid, name, text, message) {
    lastError = message
    _editRetryUuid = String(uuid)
    _editRetryName = String(name)
    _editRetryText = String(text)
    editRetryTimer.restart()
  }

  // Refuses an ambiguous replace outright: with several profiles sharing
  // the name, "replace the one that matched first" would silently destroy a
  // profile the user never pointed at. The panel blocks this earlier with a
  // clearer message; this is the backstop for the headless entry points.
  function importFile(path, name) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    if (!path || !name) return rejectAction("a config path and name are required")
    if (countByName(name) > 1) {
      return rejectAction("several profiles use the name " + name + " — not replacing an ambiguous match")
    }
    actionRejection = ""
    var existing = findByName(name)
    actionStatus = "Importing " + name + "…"
    runControl(["import", "--", String(name), existing ? existing.uuid : "", String(path)])
    return true
  }

  // Writes a queued editor save once controlProcess is free. Bypasses
  // importText's silent busy-guard on purpose: by the time the text exists
  // the user has already committed the edit, so it either writes now or
  // stays queued — it never just disappears.
  function _flushPendingSave() {
    if (_pendingSaveUuid === "" || busy) return
    var uuid = _pendingSaveUuid
    var name = _pendingSaveName
    var text = _pendingSaveText
    _pendingSaveUuid = ""
    _pendingSaveName = ""
    _pendingSaveText = ""
    // Held so a rejected save can be handed back to the editor instead of
    // being thrown away.
    _editRetryUuid = uuid
    _editRetryName = name
    _editRetryText = text
    actionStatus = "Saving " + name + "…"
    runControl(["import", "--", name, uuid], text)
  }

  function importText(text, name) {
    if (busy) return rejectAction("another OmaVLESS operation is already running")
    if (!text || !name) return rejectAction("config text and a name are required")
    if (countByName(name) > 1) {
      return rejectAction("several profiles use the name " + name + " — not replacing an ambiguous match")
    }
    actionRejection = ""
    var existing = findByName(name)
    actionStatus = "Importing " + name + "…"
    runControl(["import", "--", String(name), existing ? existing.uuid : ""], String(text))
    return true
  }

  // Keep provider filenames readable and strip only control characters.
  function sanitizeName(raw) {
    var base = String(raw || "").split("/").pop()
    base = base.replace(/\.(txt|url|conf)$/i, "")
    base = base.replace(/[\x00-\x1f\x7f]/g, "").trim()
    return base.substring(0, 80)
  }

  function isValidName(name) {
    var value = String(name || "")
    return value.trim() !== "" && value.length <= 80 && !/[\x00-\x1f\x7f]/.test(value)
  }

  function looksLikeConfig(text) {
    return /(^|\s)(vless|trojan|hysteria2|hy2|tuic):\/\//i.test(String(text || ""))
  }

  // QML deliberately does not parse a credential-bearing URI fragment.
  function suggestName() {
    for (var i = 0; i < 100; i++) {
      var name = "Profile " + (i + 1)
      if (!findByName(name)) return name
    }
    return "Profile"
  }

  // Credential-bearing profile text rides on stdin, never in argv: anything in the command line
  // is world-readable through /proc/<pid>/cmdline for as long as the process
    // lives, and the URI contains the access credential.
  function runControl(args, stdinData) {
    _controlError = ""
    _controlOperation = String(args[0])
    _controlStdin = stdinData === undefined ? "" : String(stdinData)
    controlProcess.stdinEnabled = true
    controlProcess.command = ["bash", backendPath].concat(args)
    controlProcess.running = true
  }

  function elide(text) {
    var value = String(text || "").replace(/\s+/g, " ").trim()
    return value.length > 140 ? value.substring(0, 137) + "…" : value
  }

  // Profile storage, conversion, validation and service control live in
  // backend.sh. Desktop integration helpers stay inline.

  // Device names come from the kernel — no spaces, no globs to worry about.
  readonly property string trafficScript:
    "for d in \"$@\"; do\n" +
    "  s=\"/sys/class/net/$d/statistics\"\n" +
    "  [ -r \"$s/rx_bytes\" ] && [ -r \"$s/tx_bytes\" ] || continue\n" +
    "  printf '%s %s %s\\n' \"$d\" \"$(cat \"$s/rx_bytes\")\" \"$(cat \"$s/tx_bytes\")\"\n" +
    "done\n"

  // ping bound to the tunnel device, falling back to the tunnel address:
  // iputils exits 2 for "could not even send" (no such device, a bind the
  // kernel refused) and 1 for "sent, nothing came back". Only the latter is
  // a timeout worth charting, so the fallback is tried on 2 alone — a real
  // timeout must not cost a second probe on every tick. Prints the average
  // RTT in milliseconds.
  readonly property string pingScript:
    "dev=\"$1\"; src=\"$2\"; host=\"$3\"\n" +
    "command -v ping >/dev/null 2>&1 || exit 2\n" +
    "rc=0\n" +
    "out=\"$(ping -n -q -c 1 -W 2 -I \"$dev\" -- \"$host\" 2>/dev/null)\" || rc=$?\n" +
    "if [ \"$rc\" != 0 ] && [ \"$rc\" != 1 ] && [ -n \"$src\" ]; then\n" +
    "  rc=0\n" +
    "  out=\"$(ping -n -q -c 1 -W 2 -I \"$src\" -- \"$host\" 2>/dev/null)\" || rc=$?\n" +
    "fi\n" +
    "[ \"$rc\" = 0 ] || exit \"$rc\"\n" +
    "printf '%s\\n' \"$out\" | awk -F/ '/^rtt|^round-trip/ {print $5; exit}'\n"

  // Two independent providers, queried only after a connection/routing
  // change or an explicit Test. The result is a useful observation of this
  // request, not a claim that every route follows the same policy.
  readonly property string exitIpScript:
    "for url in https://checkip.amazonaws.com https://api.ipify.org; do\n" +
    "  out=\"$(curl --silent --show-error --fail --noproxy '*' --max-time 6 \"$url\")\" || continue\n" +
    "  printf '%s\\n' \"$out\"\n" +
    "  exit 0\n" +
    "done\n" +
    "exit 1\n"

  readonly property string clipboardScript:
    "command -v wl-paste >/dev/null 2>&1 || exit 2\n" +
    "command -v timeout >/dev/null 2>&1 || exit 4\n" +
    "args=(--no-newline)\n" +
    "if wl-paste --list-types 2>/dev/null | grep -Fxq text/plain; then args+=(--type text/plain); fi\n" +
    "timeout 5s wl-paste \"${args[@]}\" 2>/dev/null | head -c 65537\n" +
    "rc=${PIPESTATUS[0]}\n" +
    "[ \"$rc\" = 0 ] && exit 0\n" +
    "[ \"$rc\" = 141 ] && exit 3\n" +
    "exit 4\n"

  readonly property string diagnosticsExportScript:
    "be=\"$1\"\n" +
    "folder=\"$HOME\"\n" +
    "if command -v xdg-user-dir >/dev/null 2>&1; then\n" +
    "  found=\"$(xdg-user-dir DOWNLOAD 2>/dev/null)\"\n" +
    "  [ -n \"$found\" ] && folder=\"$found\"\n" +
    "fi\n" +
    "suggest=\"$folder/omavless-diagnostics.json\"\n" +
    "if command -v zenity >/dev/null 2>&1; then\n" +
    "  dest=\"$(zenity --file-selection --save --confirm-overwrite \\\n" +
    "    --title='Export safe OmaVLESS diagnostics' --filename=\"$suggest\" \\\n" +
    "    --file-filter='JSON | *.json' --file-filter='All files | *')\" || exit 3\n" +
    "elif command -v kdialog >/dev/null 2>&1; then\n" +
    "  dest=\"$(kdialog --getsavefilename \"$suggest\" '*.json|JSON diagnostics')\" || exit 3\n" +
    "elif command -v yad >/dev/null 2>&1; then\n" +
    "  dest=\"$(yad --file --save --confirm-overwrite --title='Export safe OmaVLESS diagnostics' \\\n" +
    "    --filename=\"$suggest\")\" || exit 3\n" +
    "else\n" +
    "  exit 2\n" +
    "fi\n" +
    "[ -n \"$dest\" ] || exit 3\n" +
    "exec bash \"$be\" diagnostics-export -- \"$dest\"\n"

  property string _controlError: ""
  // Which backend command controlProcess is running: special exit codes
  // (6 for an incomplete edit rollback, 20/21 for connect recovery) mean
  // nothing outside their command.
  property string _controlOperation: ""
  property string _controlStdin: ""
  // A staged routing template is not committed presentation state. Wait for
  // one status request started after backend completion before settling the
  // selector, including after a cancelled polkit dialog.
  property string _pendingRoutingMode: ""
  property string _routingModeBeforeChange: ""
  property int _routingModeRequiredStatusGeneration: 0
  property string _previewKind: ""
  property string _previewPayload: ""
  property string _previewSuggested: ""
  property string _subscriptionUrlUuid: ""
  // True while lastError describes a failed status poll, so a successful
  // poll knows it may clear it.
  property bool _pollError: false
  // One coalesced poll requested while statusProcess was running. This keeps
  // post-control state fresh even if the normal interval is as high as 3600s.
  property bool _refreshAfterStatus: false
  property string _pendingConnect: ""
  property string _exportDest: ""
  property string _copyText: ""
  property string _transientActionStatus: ""
  // The UUID the running details query is about — the answer is filed under
  // it, so a query overtaken by a switch cannot label itself with the new
  // tunnel's UUID. _pingFor does the same for the probe in flight.
  property string _detailsFor: ""
  property string _pingFor: ""
  // uuid -> name of the profiles active at the previous status poll.
  property var _prevActive: ({})
  property string _notifyDropName: ""
  // Exact text of an external-drop warning. Matching the text before clearing
  // prevents a later recovery poll from erasing an unrelated operation error.
  property string _dropWarningText: ""
  // Observed drops waiting for their notify-drop verdict, one at a time.
  property var _dropQueue: []
  // Observed activations ("uuid:epoch") waiting to re-arm their markers,
  // and the batch currently being processed — returned to the queue if the
  // backend fails (clear_intent is idempotent, so replays are safe).
  property var _markQueue: []
  property var _markInFlight: []
  // Epoch milliseconds of the moment the running status poll was requested.
  property double _statusStartedAt: 0
  property int _statusRequestGeneration: 0
  property int _runningStatusGeneration: 0
  // A generation identifies one diagnostics-page lifetime. Controller
  // replies from a page that has already closed are ignored.
  property int _advancedDiagnosticsGeneration: 0
  property int _advancedDiagnosticsRequestGeneration: -1
  property bool _advancedDiagnosticsRefreshPending: false
  // False once the QR window closed — a result arriving afterwards is
  // deleted, not displayed.
  property bool _qrWanted: false

  // Each argument is "uuid:observed-epoch"; UUIDs contain no colons. Every
  // pair is attempted; any failure fails the batch, which the caller then
  // requeues whole — replays are idempotent.
  readonly property string markActiveScript:
    "be=\"$1\"; shift\n" +
    "rc=0\n" +
    "for pair in \"$@\"; do bash \"$be\" mark-active \"${pair%%:*}\" \"${pair#*:}\" || rc=1; done\n" +
    "exit $rc\n"
  property string _editUuid: ""
  property string _editName: ""
  property string _editSeed: ""
  // Edited text waiting for controlProcess to free up. The editor can close
  // while another operation runs (busy only gates controlProcess); a save
  // must queue, not silently vanish.
  property string _pendingSaveUuid: ""
  property string _pendingSaveName: ""
  property string _pendingSaveText: ""
  // Last edited text and its config, kept only until the write is known to
  // have succeeded.
  property string _editRetryUuid: ""
  property string _editRetryName: ""
  property string _editRetryText: ""

  Timer {
    id: editRetryTimer
    interval: 60
    repeat: false
    onTriggered: {
      // Still winding down — try again rather than let editConfig's guard
      // silently swallow the retry text.
      if (editProcess.running) {
        editRetryTimer.restart()
        return
      }
      var uuid = root._editRetryUuid
      var name = root._editRetryName
      var text = root._editRetryText
      root._editRetryUuid = ""
      root._editRetryName = ""
      root._editRetryText = ""
      root.editConfig({ uuid: uuid, name: name }, text)
    }
  }

  Timer {
    id: transientStatusTimer
    interval: 3500
    repeat: false
    onTriggered: {
      if (root.actionStatus === root._transientActionStatus)
        root.actionStatus = ""
      root._transientActionStatus = ""
    }
  }

  Timer {
    id: exitIpDelay
    interval: 1800
    repeat: false
    onTriggered: root.refreshExitIp()
  }

  Timer {
    interval: 1000
    repeat: true
    running: root.probingProfiles
    triggeredOnStart: true
    onTriggered: root.probeElapsedSeconds = root._probeStartedAt > 0
      ? Math.max(0, Math.floor((Date.now() - root._probeStartedAt) / 1000))
      : 0
  }

  Timer {
    interval: root.statusPollIntervalMs
    repeat: true
    running: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }

  // Process.onExited can run before Quickshell has cleared Process.running.
  // A one-shot callLater from that signal can therefore find the status
  // process busy again and strand the coalesced post-change refresh forever.
  // Keep retrying the bounded local scheduling check until a fresh status
  // request can actually start; no backend process is spawned while busy.
  Timer {
    id: postChangeRefreshTimer
    interval: 60
    repeat: false
    onTriggered: {
      if (!root._refreshAfterStatus) return
      if (statusProcess.running) {
        postChangeRefreshTimer.restart()
        return
      }
      root._refreshAfterStatus = false
      root.refresh()
    }
  }

  // Short-lived by design: refreshIntervalSec is far too coarse for a rate
  // readout, and a 2s cadence is only worth paying for while someone looks.
  Timer {
    interval: 2000
    repeat: true
    running: root.trafficMonitoring && root.activeDevices.length > 0
    triggeredOnStart: true
    onTriggered: root.sampleTraffic()
  }

  // Slower than the traffic tick on purpose: a probe can sit for its whole
  // 2s timeout, and a 30s window of ten samples is what makes the loss
  // figure mean anything.
  Timer {
    interval: 3000
    repeat: true
    running: root.pingMonitoring && root.primaryDevice !== "" && root.pingHost !== ""
    triggeredOnStart: true
    onTriggered: root.samplePing()
  }


  Process {
    id: statusProcess
    running: false
    command: ["bash", root.backendPath, "status"]
    stdout: StdioCollector {
      id: statusStdout
      waitForEnd: true
    }
    stderr: StdioCollector {
      id: statusStderr
      waitForEnd: true
    }
    onExited: function(exitCode) {
      // A failed poll must not read as "disconnected" — keep the last known
      // state and say why it could not be refreshed.
      if (exitCode === 0 && root.applyStatus(statusStdout.text)) {
        root.statusFailureCount = 0
      } else {
        root.lastError = root.elide(statusStderr.text || "Failed to read OmaVLESS status")
        root._pollError = true
        root.statusFailureCount = Math.min(root.statusFailureCount + 1, 5)
      }
      if (root._refreshAfterStatus) {
        postChangeRefreshTimer.restart()
      }
    }
  }

  // Kept out of runControl: these return data rather than pass/fail, and a
  // file dialog can sit open for a while — `busy` would freeze the panel.
  Process {
    id: pickerProcess
    running: false
    command: ["bash", backendPath, "pick-file"]
    stdout: StdioCollector {
      id: pickerStdout
      waitForEnd: true
    }
    stderr: StdioCollector {
      id: pickerStderr
      waitForEnd: true
    }
    onExited: function(exitCode) {
      root.actionStatus = ""
      if (exitCode === 2) {
        root.lastError = root.elide(pickerStderr.text
          || "File import unavailable — file picker missing. Run “omarchy pkg add zenity”")
        return
      }
      // Exit 3 is the user pressing Cancel; say nothing.
      if (exitCode === 3) return
      if (exitCode !== 0) {
        root.lastError = "Could not open the file picker"
        return
      }
      var path = String(pickerStdout.text || "").trim()
      if (path !== "") root.previewImport("file", path, root.sanitizeName(path))
    }
  }

  Process {
    id: editProcess
    running: false
    command: []
    stdinEnabled: false
    onStarted: {
      if (root._editSeed !== "") write(root._editSeed)
      root._editSeed = ""
      stdinEnabled = false
    }
    stdout: StdioCollector {
      id: editStdout
      waitForEnd: true
    }
    stderr: StdioCollector {
      id: editStderr
      waitForEnd: true
    }
    onExited: function(exitCode) {
      var uuid = root._editUuid
      var name = root._editName
      root._editUuid = ""
      root._editName = ""
      root.actionStatus = ""
      if (exitCode === 2) {
        root.desktopHelperErrorCode = "config_editor_missing"
        root.lastError = root.configEditorMissingFallback
        root.editFailed(root.lastError)
        return
      }
      // 3 = Cancel, 4 = nothing changed; neither is worth a message.
      if (exitCode === 3 || exitCode === 4) {
        root.editFinished()
        return
      }
      if (exitCode !== 0) {
        root.lastError = root.elide(editStderr.text || "Could not open " + name)
        root.editFailed(root.lastError)
        return
      }
      var text = String(editStdout.text || "")
      if (!root.looksLikeConfig(text)) {
        root.retryEdit(uuid, name, text, "Not saved: that is not a supported profile link")
        return
      }
      // Queued rather than written directly: busy only gates controlProcess,
      // so another operation may be mid-flight right now. The queue drains
      // from controlProcess.onExited — the save waits its turn instead of
      // being dropped.
      root._pendingSaveUuid = uuid
      root._pendingSaveName = name
      root._pendingSaveText = text
      Qt.callLater(root._flushPendingSave)
    }
  }

  Process {
    id: clipboardProcess
    running: false
    command: ["bash", "-c", root.clipboardScript, "profile"]
    stdout: StdioCollector {
      id: clipboardStdout
      waitForEnd: true
    }
    onExited: function(exitCode) {
      root.actionStatus = ""
      if (exitCode === 2) {
        root.lastError = "wl-clipboard is not installed"
        return
      }
      var text = String(clipboardStdout.text || "")
      if (exitCode === 3 || text.length > 65536) {
        root.lastError = "Clipboard content is too large to import"
        return
      }
      if (exitCode !== 0) {
        root.lastError = "Could not read the clipboard"
        return
      }
      root.previewImport("text", text, root.suggestName())
    }
  }

  Process {
    id: previewProcess
    running: false
    command: []
    stdinEnabled: false
    onStarted: {
      if (root._previewKind === "text") write(root._previewPayload)
      stdinEnabled = false
    }
    stdout: StdioCollector {
      id: previewStdout
      waitForEnd: true
    }
    stderr: StdioCollector {
      id: previewStderr
      waitForEnd: true
    }
    onExited: function(exitCode) {
      var kind = root._previewKind
      var payload = root._previewPayload
      var suggested = root._previewSuggested
      root._previewKind = ""
      root._previewPayload = ""
      root._previewSuggested = ""
      root.actionStatus = ""
      if (exitCode !== 0) {
        root.lastError = root.elide(previewStderr.text || "Could not classify that import")
        return
      }
      var result
      try { result = JSON.parse(String(previewStdout.text || "")) } catch (error) {
        root.lastError = "Import preview returned invalid data"
        return
      }
      if (!result || result.version !== 1
          || (result.kind !== "profile" && result.kind !== "subscription")) {
        root.lastError = "Import preview returned invalid data"
        return
      }
      if (result.kind === "subscription") {
        if (typeof result.suggestedName !== "string"
            || result.suggestedName === "" || result.suggestedName.length > 80
            || typeof result.duplicate !== "boolean") {
          root.lastError = "Import preview returned invalid data"
          return
        }
        if (result.duplicate) {
          root.lastError = "That subscription URL is already added"
          return
        }
        root.lastError = ""
        root.subscriptionImportReady(
          kind, payload, root.plainText(result.suggestedName, 80))
        return
      }
      var value = result.profile
      if (!value || value.version !== 1
          || typeof value.protocol !== "string"
          || root.capabilities.protocols.indexOf(value.protocol) < 0
          || typeof value.server !== "string" || value.server === ""
          || value.server.length > 253 || typeof value.port !== "number"
          || !isFinite(value.port) || value.port < 1 || value.port > 65535
          || ["tcp", "ws", "http", "h2", "grpc", "xhttp", "quic"].indexOf(value.transport) < 0
          || ["none", "tls", "reality"].indexOf(value.security) < 0
          || typeof value.sni !== "string" || value.sni.length > 253
          || typeof value.flow !== "string" || value.flow.length > 64
          || typeof value.insecure !== "boolean"
          || typeof value.advancedXhttp !== "boolean"
          || typeof value.experimental !== "boolean"
          || !Array.isArray(value.experimentalFeatures)
          || value.experimentalFeatures.length > 8
          || typeof value.compatibilityNote !== "string"
          || value.compatibilityNote.length > 1000
          || typeof value.credentialHint !== "string"
          || !/^••••(?:[0-9a-f]{4})?$/i.test(value.credentialHint)
          || typeof value.suggestedName !== "string" || value.suggestedName.length > 80) {
        root.lastError = "Profile preview returned invalid data"
        return
      }
      var experimentalFeatures = []
      for (var feature = 0; feature < value.experimentalFeatures.length; feature++) {
        if (typeof value.experimentalFeatures[feature] !== "string"
            || value.experimentalFeatures[feature].length > 64) {
          root.lastError = "Profile preview returned invalid data"
          return
        }
        experimentalFeatures.push(root.plainText(value.experimentalFeatures[feature], 64))
      }
      root.importPreview = {
        protocol: root.plainText(value.protocol, 32),
        server: root.plainText(value.server, 253), port: Math.floor(value.port),
        transport: value.transport, security: value.security,
        sni: root.plainText(value.sni, 253), flow: root.plainText(value.flow, 64),
        insecure: value.insecure, advancedXhttp: value.advancedXhttp,
        experimental: value.experimental, experimentalFeatures: experimentalFeatures,
        compatibilityNote: root.plainText(value.compatibilityNote, 1000),
        credentialHint: value.credentialHint,
        suggestedName: root.plainText(value.suggestedName, 80)
      }
      root.lastError = ""
      root.importReady(kind, payload,
        root.importPreview.suggestedName !== "" ? root.importPreview.suggestedName : suggested)
    }
  }

  Process {
    id: advancedDiagnosticsProcess
    running: false
    command: []
    stdout: StdioCollector {
      id: advancedDiagnosticsStdout
      waitForEnd: true
    }
    // Collected only to drain the child pipe. Public UI errors are stable and
    // never echo controller/configuration details from stderr.
    stderr: StdioCollector { waitForEnd: true }
    onExited: function(exitCode) {
      var stale = !root.diagnosticsPageVisible
        || root._advancedDiagnosticsRequestGeneration !== root._advancedDiagnosticsGeneration
      if (!stale) {
        if (exitCode === 0
            && root.applyAdvancedDiagnostics(advancedDiagnosticsStdout.text)) {
          root.advancedDiagnosticsErrorCode = ""
          root.advancedDiagnosticsError = ""
        } else {
          root.advancedDiagnosticsErrorCode = "unavailable"
          root.advancedDiagnosticsError = "Live Mihomo diagnostics are unavailable"
        }
      }
      if (root._advancedDiagnosticsRefreshPending && root.diagnosticsPageVisible) {
        root._advancedDiagnosticsRefreshPending = false
        Qt.callLater(root.refreshAdvancedDiagnostics)
      }
    }
  }

  Process {
    id: diagnosticsProcess
    running: false
    command: []
    stdout: StdioCollector {
      id: diagnosticsStdout
      waitForEnd: true
    }
    stderr: StdioCollector {
      id: diagnosticsStderr
      waitForEnd: true
    }
    onExited: function(exitCode) {
      if (exitCode === 3) {
        root.diagnosticsStatus = ""
        return
      }
      if (exitCode === 0) {
        root.diagnosticsStatus = "Saved with private file permissions"
        return
      }
      root.diagnosticsStatus = exitCode === 2
        ? "No file picker found — install zenity, kdialog or yad"
        : root.elide(diagnosticsStderr.text || "Could not export diagnostics")
    }
  }

  Process {
    id: exportProcess
    running: false
    command: []
    stderr: StdioCollector {
      id: exportStderr
      waitForEnd: true
    }
    onExited: function(exitCode) {
      var dest = root._exportDest
      root._exportDest = ""
      if (exitCode === 0) {
        // Success worth stating: the visible outcome is a file somewhere
        // else, not a change in the panel.
        root.showTransientStatus("Exported to " + dest)
      } else {
        root.actionStatus = ""
        root.lastError = root.elide(exportStderr.text || "Export failed")
      }
    }
  }


  Process {
    id: copyProcess
    running: false
    command: ["wl-copy"]
    stdinEnabled: false
    onStarted: {
      write(root._copyText)
      root._copyText = ""
      stdinEnabled = false
    }
    onExited: function(exitCode) {
      if (exitCode === 0) root.showTransientStatus("Copied to clipboard")
      else {
        root.actionStatus = ""
        root.lastError = "Could not copy to the clipboard — is wl-clipboard installed?"
      }
    }
  }

  Process {
    id: qrProcess
    running: false
    command: []
    stdout: StdioCollector {
      id: qrStdout
      waitForEnd: true
    }
    stderr: StdioCollector {
      id: qrStderr
      waitForEnd: true
    }
    onExited: function(exitCode) {
      root.qrLoading = false
      var path = exitCode === 0 ? String(qrStdout.text || "").trim() : ""
      // The window closed while we rendered: nobody is waiting for this, and
      // a successful result is key material — delete it and say nothing.
      if (!root._qrWanted) {
        root.removeQrFile(path)
        return
      }
      if (exitCode === 0 && path !== "") {
        root.qrPath = path
      } else {
        // 2 is the backend's fixed dependency-missing exit. All other
        // backend/qrencode output remains private and becomes one safe code.
        root.qrErrorCode = exitCode === 2 ? "dependency_missing" : "render_failed"
      }
    }
  }

  Process {
    id: notifyProcess
    running: false
    command: []
    onExited: function(exitCode) {
      var name = root._notifyDropName
      root._notifyDropName = ""
      // 0 = the first observer of an external drop, 2 = another monitor saw
      // the same drop after the desktop notification was already sent. Both
      // bars still turn urgent; only the toast is deduplicated by the backend.
      if ((exitCode === 0 || exitCode === 2) && name !== "" && !root.active) {
        var message = "Profile " + name + " was deactivated"
        root._dropWarningText = message
        root.lastError = message
      }
      Qt.callLater(root._flushDrops)
    }
  }

  Process {
    id: markActiveProcess
    running: false
    command: []
    onExited: function(exitCode) {
      if (exitCode === 0) {
        root._markInFlight = []
        // Drain whatever queued while this batch ran.
        Qt.callLater(root._flushMarkActive)
      } else {
        // Give the batch back and retry on the next poll — an immediate
        // retry against a persistently failing backend would spin.
        root._markQueue = root._markInFlight.concat(root._markQueue)
        root._markInFlight = []
      }
    }
  }

  Process {
    id: trafficProcess
    running: false
    command: []
    stdout: StdioCollector {
      id: trafficStdout
      waitForEnd: true
    }
    onExited: function(exitCode) {
      if (exitCode === 0) root.applyTraffic(trafficStdout.text)
    }
  }

  Process {
    id: exitIpProcess
    running: false
    command: ["bash", "-c", root.exitIpScript]
    stdout: StdioCollector {
      id: exitIpStdout
      waitForEnd: true
    }
    onExited: function(exitCode) {
      var stillCurrent = root._exitIpFor === root.primaryUuid + "|" + root.routing.mode
      var parsed = exitCode === 0 ? root.parsePublicIp(exitIpStdout.text) : ""
      root.exitIpFetching = false
      if (!stillCurrent || !root.active) return
      root.exitIp = parsed
      root.exitIpFailed = parsed === ""
    }
  }

  Process {
    id: detailsProcess
    running: false
    command: []
    stdout: StdioCollector {
      id: detailsStdout
      waitForEnd: true
    }
    onExited: function(exitCode) {
      // A failure says nothing in lastError: this is a decoration on a panel
      // whose actual job — listing and switching tunnels — is unaffected,
      // and the grid already reads "--" with no data.
      // A switch mid-query answered about the tunnel that just left, and the
      // fetch that switch triggered was refused by the busy guard — so the
      // stale answer schedules the one the panel is actually waiting for.
      // Only staleness retries: a query that simply failed is left alone,
      // because retrying a persistent failure would spin.
      var stale = root._detailsFor !== root.primaryUuid
      if (exitCode === 0 && !stale) root.applyDetails(detailsStdout.text)
      root._detailsFor = ""
      if (stale && root.primaryUuid !== "") Qt.callLater(root.fetchDetails)
    }
  }

  Process {
    id: pingProcess
    running: false
    command: []
    stdout: StdioCollector {
      id: pingStdout
      waitForEnd: true
    }
    onExited: function(exitCode) {
      // The tunnel moved, or the panel closed and emptied the window, while
      // this probe was in flight: its answer belongs to neither.
      var stale = root._pingFor !== root.primaryUuid || !root.pingMonitoring
      root._pingFor = ""
      if (stale) {
        root._manualPingRequested = false
        return
      }
      if (exitCode === 1) {
        root.addPingSample(-1)
        root._manualPingRequested = false
        return
      }
      if (exitCode !== 0) {
        root._manualPingRequested = false
        return
      }
      var ms = parseFloat(String(pingStdout.text || "").trim())
      if (isFinite(ms) && ms >= 0) root.addPingSample(ms)
      root._manualPingRequested = false
    }
  }

  Process {
    id: probeProcess
    running: false
    command: []
    stdout: SplitParser {
      onRead: function(line) {
        if (!root.applyProbeEvent(line, root.probingSubscriptionUuid))
          root._probeStreamValid = false
      }
    }
    stderr: StdioCollector {
      id: probeStderr
      waitForEnd: true
    }
    onExited: function(exitCode) {
      var target = root.probingSubscriptionUuid
      var cancelled = root._probeCancelRequested
      root.probingSubscriptionUuid = ""
      root._probeStartedAt = 0
      root._probeCancelRequested = false
      root._probeName = ""
      if (cancelled) {
        root.subscriptionError = ""
        root.subscriptionStatus = "Server test cancelled"
        root._probeSawComplete = false
        return
      }
      if (exitCode !== 0) {
        root.subscriptionStatus = ""
        root.subscriptionError = root.elide(
          probeStderr.text || "Could not test subscription endpoints"
        )
        return
      }
      if (!root._probeSawComplete || !root._probeStreamValid) {
        root.subscriptionStatus = ""
        root.subscriptionError = "Could not read endpoint test results"
        return
      }
      var summary = root._probeSummary
      root.subscriptionError = ""
      var nextTimes = Object.assign({}, root.subscriptionProbeTimes)
      nextTimes[target] = Date.now()
      root.subscriptionProbeTimes = nextTimes
      root.subscriptionStatus = "Tested " + summary.tested
        + (summary.tested === 1 ? " server" : " servers")
      if (summary.unavailable > 0)
        root.subscriptionStatus += " · " + summary.unavailable + " unavailable"
      if (summary.unresolved > 0)
        root.subscriptionStatus += " · " + summary.unresolved + " DNS failed"
      root._probeSawComplete = false
    }
  }

  Process {
    id: subscriptionUrlProcess
    running: false
    command: []
    stdout: StdioCollector {
      id: subscriptionUrlStdout
      waitForEnd: true
    }
    stderr: StdioCollector {
      id: subscriptionUrlStderr
      waitForEnd: true
    }
    onExited: function(exitCode) {
      var uuid = root._subscriptionUrlUuid
      root._subscriptionUrlUuid = ""
      if (exitCode === 0) {
        root.subscriptionUrlReady(uuid, String(subscriptionUrlStdout.text || "").trim())
      } else {
        root.subscriptionError = root.elide(subscriptionUrlStderr.text || "Could not load subscription editor")
      }
    }
  }

  Process {
    id: customRulesProcess
    running: false
    command: []
    stdout: StdioCollector {
      id: customRulesStdout
      waitForEnd: true
    }
    stderr: StdioCollector {
      id: customRulesStderr
      waitForEnd: true
    }
    onExited: function(exitCode) {
      if (exitCode !== 0) {
        root.routingToolError = root.elide(customRulesStderr.text || "Could not load custom rules")
        return
      }
      var payload
      try { payload = JSON.parse(String(customRulesStdout.text || "")) } catch (error) {
        root.routingToolError = "Custom rule data is invalid"
        return
      }
      if (!payload || payload.version !== 1 || !Array.isArray(payload.rules)
          || payload.rules.length > 128) {
        root.routingToolError = "Custom rule data is invalid"
        return
      }
      var rules = []
      var ids = {}
      for (var i = 0; i < payload.rules.length; i++) {
        var item = payload.rules[i]
        if (!item || typeof item.id !== "string" || item.id === ""
            || item.id.length > 64 || ids[item.id] !== undefined
            || ["domain", "suffix", "ipcidr"].indexOf(item.kind) < 0
            || ["proxy", "direct", "reject"].indexOf(item.action) < 0
            || typeof item.value !== "string" || item.value === ""
            || item.value.length > 1024) {
          root.routingToolError = "Custom rule data is invalid"
          return
        }
        ids[item.id] = true
        rules.push({
          id: item.id, kind: item.kind,
          value: root.plainText(item.value, 1024), action: item.action
        })
      }
      root.customRules = rules
      root.routingToolError = ""
    }
  }

  Process {
    id: routeCheckProcess
    running: false
    command: []
    stdinEnabled: false
    onStarted: {
      write(root._routeCheckInput)
      root._routeCheckInput = ""
      stdinEnabled = false
    }
    stdout: StdioCollector {
      id: routeCheckStdout
      waitForEnd: true
    }
    stderr: StdioCollector {
      id: routeCheckStderr
      waitForEnd: true
    }
    onExited: function(exitCode) {
      root.routingToolStatus = ""
      if (exitCode !== 0) {
        root.routingToolError = root.elide(routeCheckStderr.text || "Could not check that route")
        return
      }
      var payload
      try { payload = JSON.parse(String(routeCheckStdout.text || "")) } catch (error) {
        root.routingToolError = "Route check returned invalid data"
        return
      }
      if (!payload || payload.version !== 1 || typeof payload.query !== "string"
          || payload.query.length > 1024
          || ["vpn", "direct", "block", "unknown"].indexOf(payload.outcome) < 0
          || typeof payload.ruleType !== "string" || payload.ruleType.length > 80
          || typeof payload.rulePayload !== "string" || payload.rulePayload.length > 256
          || typeof payload.target !== "string" || payload.target.length > 80
          || typeof payload.source !== "string" || payload.source.length > 32) {
        root.routingToolError = "Route check returned invalid data"
        return
      }
      root.routeCheckResult = {
        query: root.plainText(payload.query, 1024), outcome: payload.outcome,
        ruleType: root.plainText(payload.ruleType, 80),
        rulePayload: root.plainText(payload.rulePayload, 256),
        target: root.plainText(payload.target, 80), source: payload.source
      }
      root.routingToolError = ""
    }
  }

  Process {
    id: controlProcess
    running: false
    command: []
    stdinEnabled: false
    onStarted: {
      if (root._controlStdin !== "") write(root._controlStdin)
      root._controlStdin = ""
      stdinEnabled = false
    }
    stdout: StdioCollector {
      id: controlStdout
      waitForEnd: true
    }
    stderr: StdioCollector {
      id: controlStderr
      waitForEnd: true
      onStreamFinished: root._controlError = text
    }
    onExited: function(exitCode) {
      var op = root._controlOperation
      root._controlOperation = ""
      var routingModeOperationPending = op === "set-mode" && root.routingModePending
      if (routingModeOperationPending) {
        // A poll already in flight may have observed only the temporary
        // template. Require a new request after this success/failure result.
        root._routingModeRequiredStatusGeneration = root._statusRequestGeneration + 1
      }
      var subscriptionOperation = op.indexOf("subscription-") === 0
      var routingOperation = op.indexOf("custom-rule-") === 0
        || op === "rule-providers-refresh"
      var completedEditorSave = op === "import" && root._editRetryName !== ""
      // 6 means import needs manual recovery: rollback or incomplete-profile
      // cleanup kept one or more replacements. Never reopen an editor
      // automatically: another save could hide that recovery state.
      var importStateUnknown = op === "import" && exitCode === 6
      if (exitCode === 0) {
        if (root._pendingConnect !== "") root.rememberLast(root._pendingConnect)
        root.lastError = ""
        root._dropWarningText = ""
        // Keep the bounded Switching state until the authoritative post-exit
        // status snapshot settles the requested or rolled-back mode.
        if (!routingModeOperationPending) root.actionStatus = ""
        root._editRetryUuid = ""
        root._editRetryName = ""
        root._editRetryText = ""
        if (completedEditorSave) root.editFinished()
        if (routingOperation) {
          root.routingToolError = ""
          if (op === "rule-providers-refresh") {
            var updateResult = null
            try { updateResult = JSON.parse(String(controlStdout.text || "{}")) } catch (error) {}
            var updated = updateResult && typeof updateResult.updated === "number"
              ? Math.floor(updateResult.updated) : 0
            root.routingToolStatus = updated > 0
              ? "Updated " + updated + (updated === 1 ? " rule set" : " rule sets")
              : "Rule data updated"
            Qt.callLater(root.refreshAdvancedDiagnosticsAfterChange)
          } else {
            root.routingToolStatus = op === "custom-rule-delete"
              ? "Custom rule removed" : "Custom rule saved"
            Qt.callLater(root.loadCustomRules)
          }
        }
        if (subscriptionOperation) {
          var result = null
          try { result = JSON.parse(String(controlStdout.text || "{}")) } catch (error) {}
          if (op === "subscription-delete") root.subscriptionStatus = "Subscription removed"
          else if (result && typeof result.total === "number") {
            var total = Math.floor(result.total)
            root.subscriptionStatus = "Updated " + total
              + (total === 1 ? " profile" : " profiles")
            if (result.skipped > 0) root.subscriptionStatus += " · skipped " + Math.floor(result.skipped)
            if (result.stale > 0) {
              var stale = Math.floor(result.stale)
              root.subscriptionStatus += " · " + stale
                + (stale === 1 ? " stale active profile" : " stale active profiles")
            }
          } else root.subscriptionStatus = "Subscriptions updated"
          root.subscriptionError = ""
        }
      } else {
        root.actionStatus = ""
        // 20/21 (connect only): the switch failed; the backend's stderr says
        // whether the previous tunnels were restored (20) or the rollback
        // itself failed (21). Either way the poll below shows what is up.
        var reason = root.elide(root._controlError || "OmaVLESS operation failed")
        if (importStateUnknown) {
          root.lastError = reason
          root.editFailed(reason)
          root._editRetryUuid = ""
          root._editRetryName = ""
          root._editRetryText = ""
        // A write refused by backend validation must not cost the edit that produced
        // it; hand the text back to the editor with the reason attached.
        } else if (op === "import" && root._editRetryName !== "") {
          root.retryEdit(root._editRetryUuid, root._editRetryName, root._editRetryText, reason)
        } else if (subscriptionOperation) {
          root.subscriptionStatus = ""
          root.subscriptionError = reason
        } else if (routingOperation) {
          root.routingToolStatus = ""
          root.routingToolError = reason
        } else root.lastError = reason
      }
      root._pendingConnect = ""
      root.refreshAfterChange()
      // An edit can move the address, the endpoint or the routes without
      // moving the tunnel, so the grid is refetched even when the primary
      // profile is the one it already describes.
      if (root.trafficMonitoring) Qt.callLater(root.fetchDetails)
      // A save queued while this operation ran; write it now.
      if (root._pendingSaveUuid !== "") Qt.callLater(root._flushPendingSave)
    }
  }
}
