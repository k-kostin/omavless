// SPDX-License-Identifier: MIT
// Adapted from Omarchy VPN: https://github.com/jkoestinger/omarchy-vpn
// Copyright (c) 2026 Justin Köstinger
// Copyright (c) 2026 OmaVLESS contributors
// See LICENSE and THIRD_PARTY_NOTICES.md.

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "kdk.omavless"
  ipcTarget: "kdk.omavless"
  manageIpc: false

  property string focusSection: "header"
  property string page: "main"
  property int subscriptionIndex: 0
  property int configIndex: 0
  property var expandedSubscriptions: ({})
  // Per-provider presentation only; probe data itself remains session-only in
  // Service.qml. Values are "default", "pingAsc" or "pingDesc".
  property var subscriptionSortModes: ({})
  property bool cursorActive: false
  // Pointer hover may move the visual cursor, but only keyboard navigation
  // is allowed to scroll a row into view. This guard distinguishes the two.
  property bool pointerSelectingConfig: false
  property string hoveredSubscriptionServerUuid: ""
  property string profileFilter: ""
  // Attached scrollbars overlay Flickable content. Reserve a real gutter so
  // latency/status text never sits directly against the thumb.
  readonly property real scrollGutter: Style.space(16)
  // Profile ({uuid, name}) awaiting delete confirmation; non-null opens the
  // dialog. A profile object, not a name — names are not unique.
  property var pendingDelete: null
  property var pendingSubscriptionDelete: null
  property var editingSubscription: null
  // Incoming config already parsed into a redacted preview and awaiting a
  // name; "file" | "text" | "" (no prompt open).
  property string importKind: ""
  property string importPayload: ""
  // Profile ({uuid, name}) awaiting a new display name; non-null while the
  // rename dialog is open.
  property var pendingRename: null
  // Profile whose pencil was clicked — the chooser asks whether to edit
  // the config or the name; keyboard users go straight there with e / n.
  property var pendingEdit: null
  // Set when the panel closed to get out of the editor's way. The panel is
  // where lastError is read, so a handoff that produced no editor has to
  // bring it back; an IPC edit never sets this and stays headless.
  property bool editHandedOff: false
  property bool onboardingDismissed: false
  // Relative update/test ages need one inexpensive shared clock. Without it,
  // text bound through helper functions would stay frozen until other state
  // happened to change.
  property double ageClock: Date.now()

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property color dim: Qt.darker(foreground, 1.55)
  // Deliberately distinct from the often near-white theme accent: these are
  // data-series colors, shared by the legend and chart. Saturation keeps the
  // two paths legible in a small panel without borrowing the urgent red.
  readonly property color trafficRxColor: "#55d6be"
  readonly property color trafficTxColor: "#c792ea"
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  readonly property color iconColor: vless.active ? foreground : dim
  // Keep the tiny bar mark to a clean silhouette; the link detail is readable
  // only at the hero's display size. Both use Material Design glyphs from the
  // same Nerd Font vocabulary as Omarchy's stock widgets. State changes the
  // silhouette as well as color for low-contrast themes.
  readonly property string barDisconnectedIcon: "󰒙"
  readonly property string barConnectedIcon: "󰒘"
  readonly property string heroDisconnectedIcon: "󰴴"
  readonly property string heroConnectedIcon: "󰴳"
  // Octicons shield-x has a deliberately heavier X than the Material alert
  // mark, so the failure state survives the bar's small pixel size.
  readonly property string problemIcon: ""
  readonly property string barStatusIcon: vless.lastError !== ""
    ? problemIcon
    : (vless.active ? barConnectedIcon : barDisconnectedIcon)
  readonly property string heroStatusIcon: vless.lastError !== ""
    ? problemIcon
    : (vless.active ? heroConnectedIcon : heroDisconnectedIcon)
  // Urgent trumps everything: a failed operation or an externally dropped
  // tunnel must be visible without opening the panel.
  readonly property color barIconColor: vless.lastError !== ""
    ? (bar ? bar.urgent : Color.urgent)
    : (vless.active ? barForeground : Qt.darker(barForeground, 1.55))
  readonly property string toggleHint: vless.active
    ? "Disconnect"
    : (vless.toggleTarget !== "" ? "Connect " + vless.toggleTarget : "Connect")
  readonly property bool headerHasCursor: cursorActive && focusSection === "header" && vless.profiles.length > 0
  readonly property string barTooltip: {
    if (vless.lastError !== "") return vless.plainText("OmaVLESS · " + vless.lastError, 180)
    if (!vless.active) {
      var down = "OmaVLESS · Disconnected"
      return vless.hasRoutingConflict ? down + " · Possible conflict: " + vless.conflictSummary : down
    }
    var line = vless.primaryName + " · " + vless.routingTitle
    if (vless.exitIp !== "") line += " · " + vless.exitIp
    if (vless.hasRoutingConflict) line += " · Possible conflict: " + vless.conflictSummary
    return vless.plainText(line, 220)
  }
  readonly property string mihomoInstallCommand: "omarchy pkg aur add mihomo-bin"
  readonly property string mihomoCapabilityCommand: "sudo setcap cap_net_admin,cap_net_raw,cap_net_bind_service=+ep "
    + (vless.coreSetup.path !== "" ? Util.shellQuote(vless.coreSetup.path)
      : '"$(command -v mihomo)"')
  readonly property string mihomoVerifyCommand: "mihomo -v && getcap "
    + (vless.coreSetup.path !== "" ? Util.shellQuote(vless.coreSetup.path)
      : '"$(command -v mihomo)"')

  // Omarchy's shared tooltip components render AutoText. Keep the safety
  // boundary at the sink as well as in Service's public-data decoder so a
  // future adapter cannot turn provider metadata into rich-text markup.
  function safeTooltip(value, maximum) {
    return vless.plainText(value, maximum === undefined ? 512 : maximum)
  }

  function formatUptime(seconds) {
    var total = Math.max(0, Math.floor(Number(seconds) || 0))
    if (total < 60) return "<1m"
    var minutes = Math.floor(total / 60)
    if (minutes < 60) return minutes + "m"
    var hours = Math.floor(minutes / 60)
    if (hours < 24) return hours + "h " + (minutes % 60) + "m"
    return Math.floor(hours / 24) + "d " + (hours % 24) + "h"
  }

  function profileMatches(profile, subscription, query) {
    if (query === "") return true
    var fields = [profile.name, profile.rawName, profile.server, profile.sourceName]
    if (subscription) fields.push(subscription.name, subscription.rawName)
    for (var i = 0; i < fields.length; i++) {
      if (String(fields[i] || "").toLowerCase().indexOf(query) >= 0) return true
    }
    return false
  }

  readonly property string importNameClean: importDialog.value.trim()
  readonly property bool importNameValid: vless.isValidName(importNameClean)
  // Importing under the same display name edits that profile in place.
  readonly property int importNameCount: importNameValid ? vless.countByName(importNameClean) : 0
  readonly property bool importReplaces: importNameCount === 1
  // Several existing profiles use the typed name — "replace" cannot
  // know which one is meant, so the import is refused under this name.
  readonly property bool importAmbiguous: importNameCount > 1
  readonly property bool importAccepted: importNameValid && !importAmbiguous
  readonly property var importReplaceTarget: importReplaces ? vless.findByName(importNameClean) : null
  readonly property string importSourceLabel: importKind === "text"
    ? "Import from clipboard"
    : "Import " + String(importPayload).split("/").pop()
  readonly property string importHintText: !importNameValid
    ? "Use a non-empty name up to 80 characters"
    : (importAmbiguous
      ? importNameCount + " profiles use the name " + importNameClean + " — pick another name"
      : (importReplaces
        ? "Replaces the existing profile " + (importReplaceTarget ? importReplaceTarget.name : importNameClean)
        : "Imports as profile " + importNameClean))

  // Rename touches the local display label only, so spaces are
  // fine. Duplicates are refused: every name-based entry point in the
  // widget treats an ambiguous name as an error, so don't let one be made.
  readonly property string renameClean: renameWindow.value.trim()
  readonly property bool renameNameValid: vless.isValidName(renameClean)
  readonly property bool renameDuplicate: renameClean !== ""
    && (pendingRename === null || renameClean !== pendingRename.name)
    && vless.countByName(renameClean) > 0
  readonly property bool renameAccepted: pendingRename !== null && renameNameValid && !renameDuplicate
  readonly property string renameHint: !renameNameValid
    ? "Use a non-empty name up to 80 characters"
    : (renameDuplicate
      ? "A profile named " + renameClean + " already exists"
      : "The profile link remains unchanged")

  readonly property string subscriptionNameClean: subscriptionPrompt.nameValue.trim()
  readonly property string subscriptionUrlClean: subscriptionPrompt.urlValue.trim()
  readonly property bool subscriptionNameValid: vless.isValidName(subscriptionNameClean)
  readonly property bool subscriptionUrlValid: /^https?:\/\/[^\s]+$/i.test(subscriptionUrlClean)
  readonly property bool subscriptionAccepted: subscriptionNameValid && subscriptionUrlValid
  readonly property string subscriptionHint: vless.subscriptionError !== ""
    ? vless.subscriptionError
    : (!subscriptionNameValid
    ? "Use a non-empty provider name up to 80 characters"
    : (!subscriptionUrlValid
      ? "Use the http:// or https:// URL supplied by your provider"
      : "URL stays private and is shown only in this editor"))

  function openSubscriptions() {
    if (!vless.supports("subscriptions")) return
    page = "subscriptions"
    cursorActive = false
    subscriptionIndex = Math.max(0, Math.min(subscriptionIndex, vless.subscriptions.length - 1))
    if (!vless.probingProfiles) vless.clearSubscriptionMessage()
    if (panelFlick) panelFlick.contentY = 0
  }

  function closeSubscriptions() {
    page = "main"
    pendingSubscriptionDelete = null
    editingSubscription = null
    subscriptionPrompt.dismiss()
  }

  function openSettings() {
    page = "settings"
    cursorActive = false
    if (settingsFlick) settingsFlick.contentY = 0
  }

  function openOnboarding(step) {
    onboardingDismissed = false
    onboardingWizard.openAt(step || 1)
  }

  function dismissOnboarding() {
    onboardingDismissed = true
    onboardingWizard.dismiss()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function finishOnboarding() {
    if (!vless.completeOnboarding()) return
    onboardingWizard.dismiss()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function saveStartup(enabled, target, profileId, mode) {
    if (!vless.configureStartup(enabled, target, profileId, mode)) return
    startupPrompt.dismiss()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function openRoutingTools() {
    vless.loadCustomRules()
    routingToolsPrompt.openTools()
  }

  function closeRoutingTools() {
    routingToolsPrompt.dismiss()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function closeSettings() {
    page = "main"
  }

  function openAdvancedDiagnostics() {
    page = "diagnostics"
    cursorActive = false
    advancedDiagnosticsPage.resetSearchFocus()
  }

  function closeAdvancedDiagnostics() {
    page = "settings"
    cursorActive = false
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function requestRoutingMode(mode) {
    var value = String(mode || "")
    if (value === "rule" && !vless.routingPresetConfigured) {
      routingPresetPrompt.openWith(vless.routing.preset || "roscomvpn-default")
      return
    }
    vless.setRoutingMode(value)
  }

  function applyFirstRoutingPreset(preset) {
    routingPresetPrompt.dismiss()
    vless.useRoutingPreset(preset, false)
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function setWidgetSetting(key, value, jsonValue) {
    if (!bar || typeof bar.run !== "function") {
      vless.lastError = "Omarchy bar settings are unavailable"
      return false
    }
    var encoded = jsonValue ? JSON.stringify(value) : String(value)
    var command = "omarchy bar set " + Util.shellQuote(root.moduleName)
      + " " + Util.shellQuote(String(key)) + " " + Util.shellQuote(encoded)
      + (jsonValue ? " --json" : "")
    // Reflect the choice immediately; Omarchy persists the same value and
    // reinjects the canonical settings object into the widget afterwards.
    var updated = {}
    for (var current in root.settings) updated[current] = root.settings[current]
    updated[key] = value
    root.settings = updated
    bar.run(command)
    return true
  }

  function addSubscription() {
    if (vless.busy || vless.probingProfiles || vless.subscriptionEditorLoading) return
    vless.clearSubscriptionMessage()
    editingSubscription = null
    subscriptionPrompt.title = "Add subscription"
    subscriptionPrompt.confirmLabel = "Add"
    subscriptionPrompt.openWith("", "")
  }

  function editSubscription(subscription) {
    if (vless.busy || vless.probingProfiles || vless.subscriptionEditorLoading || !subscription) return
    editingSubscription = subscription
    subscriptionPrompt.title = "Edit " + subscription.name
    subscriptionPrompt.confirmLabel = "Save"
    subscriptionPrompt.openWith(subscription.rawName || subscription.name, "")
    vless.loadSubscriptionUrl(subscription)
  }

  function confirmSubscription() {
    if (!subscriptionAccepted) return
    var uuid = editingSubscription ? editingSubscription.uuid : ""
    if (!vless.saveSubscription(subscriptionNameClean, uuid, subscriptionUrlClean)) return
    editingSubscription = null
    subscriptionPrompt.dismiss()
  }

  function subscriptionAge(updatedAt) {
    var value = Number(updatedAt)
    if (!isFinite(value) || value <= 0) return "Never updated"
    var seconds = Math.max(0, Math.floor((ageClock - value) / 1000))
    if (seconds < 60) return "Updated just now"
    if (seconds < 3600) return "Updated " + Math.floor(seconds / 60) + "m ago"
    if (seconds < 86400) return "Updated " + Math.floor(seconds / 3600) + "h ago"
    return "Updated " + Math.floor(seconds / 86400) + "d ago"
  }

  function probeAge(subscriptionUuid) {
    var value = vless.subscriptionProbeTime(subscriptionUuid)
    if (value <= 0) return ""
    var seconds = Math.max(0, Math.floor((ageClock - value) / 1000))
    if (seconds < 60) return "tested just now"
    if (seconds < 3600) return "tested " + Math.floor(seconds / 60) + "m ago"
    return "tested " + Math.floor(seconds / 3600) + "h ago"
  }

  function plural(count, singular, pluralForm) {
    return count + " " + (count === 1 ? singular : pluralForm)
  }

  function favoriteCount(profiles) {
    var count = 0
    for (var i = 0; i < profiles.length; i++) {
      if (profiles[i].favorite) count++
    }
    return count
  }

  function subscriptionMessageText() {
    if (vless.subscriptionError !== "") return vless.subscriptionError
    if (vless.subscriptionStatus === "") return ""
    return vless.probingProfiles
      ? vless.subscriptionStatus + " · " + vless.probeElapsedSeconds + "s"
      : vless.subscriptionStatus
  }

  function toggleSubscriptionProbe(subscription) {
    if (!subscription) return
    if (vless.probingProfiles && vless.probingSubscriptionUuid === subscription.uuid)
      vless.cancelProbe()
    else vless.probeSubscription(subscription)
  }

  function selectedSubscription() {
    if (vless.subscriptions.length === 0) return null
    return vless.subscriptions[Math.max(0, Math.min(
      subscriptionIndex, vless.subscriptions.length - 1
    ))]
  }

  function setSubscriptionCursor(index) {
    cursorActive = true
    subscriptionIndex = Math.max(0, Math.min(index, vless.subscriptions.length - 1))
  }

  function scrollSubscriptionCursorIntoView() {
    if (page !== "subscriptions" || !subscriptionsRepeater) return
    var item = subscriptionsRepeater.itemAt(subscriptionIndex)
    if (!subscriptionsFlick || !item) return
    Qt.callLater(function() {
      var point = item.mapToItem(subscriptionsFlick.contentItem, 0, 0)
      var margin = Style.space(6)
      var top = point.y
      var bottom = top + item.height
      var maxY = Math.max(0, subscriptionsFlick.contentHeight - subscriptionsFlick.height)
      if (top < subscriptionsFlick.contentY + margin)
        subscriptionsFlick.contentY = Math.max(0, top - margin)
      else if (bottom > subscriptionsFlick.contentY + subscriptionsFlick.height - margin)
        subscriptionsFlick.contentY = Math.min(maxY, bottom + margin - subscriptionsFlick.height)
    })
  }

  function subscriptionProfiles(subscriptionUuid) {
    var target = String(subscriptionUuid || "")
    var out = []
    for (var i = 0; i < vless.profiles.length; i++) {
      if (vless.profiles[i].subscriptionUuid === target) out.push(vless.profiles[i])
    }
    var sortMode = subscriptionSortMode(target)
    out.sort(function(a, b) {
      // A connected profile stays first in every order: the live tunnel is
      // stronger information than a transient endpoint test.
      if (a.active !== b.active) return a.active ? -1 : 1
      if (a.favorite !== b.favorite) return a.favorite ? -1 : 1
      if (sortMode === "default")
        return a.name < b.name ? -1 : (a.name > b.name ? 1 : 0)
      var ar = vless.probeResult(a.uuid)
      var br = vless.probeResult(b.uuid)
      var aRank = ar === null ? 1 : (ar.reachable ? 0 : (ar.resolved ? 3 : 2))
      var bRank = br === null ? 1 : (br.reachable ? 0 : (br.resolved ? 3 : 2))
      if (aRank !== bRank) return aRank - bRank
      if (aRank === 0 && ar.latencyMs !== br.latencyMs)
        return sortMode === "pingDesc"
          ? br.latencyMs - ar.latencyMs : ar.latencyMs - br.latencyMs
      return a.name < b.name ? -1 : (a.name > b.name ? 1 : 0)
    })
    return out
  }

  function subscriptionSortMode(subscriptionUuid) {
    var value = subscriptionSortModes[String(subscriptionUuid || "")]
    return value === "pingAsc" || value === "pingDesc" ? value : "default"
  }

  function cycleSubscriptionPingSort(subscriptionUuid) {
    var target = String(subscriptionUuid || "")
    if (target === "" || subscriptionProbeSummary(target).tested === 0) return
    rememberCursor()
    var next = {}
    for (var key in subscriptionSortModes) next[key] = subscriptionSortModes[key]
    next[target] = subscriptionSortMode(target) === "pingAsc" ? "pingDesc" : "pingAsc"
    subscriptionSortModes = next
    Qt.callLater(restoreCursor)
  }

  function subscriptionProbeSummary(subscriptionUuid) {
    var profiles = subscriptionProfiles(subscriptionUuid)
    var tested = 0
    var unavailable = 0
    var unresolved = 0
    for (var i = 0; i < profiles.length; i++) {
      var result = vless.probeResult(profiles[i].uuid)
      if (result === null) continue
      tested++
      if (!result.resolved) unresolved++
      else if (!result.reachable) unavailable++
    }
    return { tested: tested, unavailable: unavailable, unresolved: unresolved }
  }

  function activeSubscriptionProfile(subscriptionUuid) {
    var profiles = subscriptionProfiles(subscriptionUuid)
    for (var i = 0; i < profiles.length; i++) {
      if (profiles[i].active) return profiles[i]
    }
    return null
  }

  function probeLabel(profileUuid) {
    var result = vless.probeResult(profileUuid)
    if (result === null) return ""
    return result.reachable ? result.latencyMs + " ms"
      : (result.resolved ? "Unavailable" : "DNS failed")
  }

  function buildProfileRows() {
    var top = []
    var query = profileFilter.trim().toLowerCase()
    for (var i = 0; i < vless.profiles.length; i++) {
      var profile = vless.profiles[i]
      if (!profile.managed && profileMatches(profile, null, query)) {
        top.push({
          kind: "profile", profile: profile, active: profile.active,
          favorite: profile.favorite, name: profile.name
        })
      }
    }
    for (var s = 0; s < vless.subscriptions.length; s++) {
      var subscription = vless.subscriptions[s]
      var allProfiles = subscriptionProfiles(subscription.uuid)
      var providerMatch = query !== "" && (
        String(subscription.name || "").toLowerCase().indexOf(query) >= 0
        || String(subscription.rawName || "").toLowerCase().indexOf(query) >= 0)
      var visibleProfiles = []
      for (var m = 0; m < allProfiles.length; m++) {
        if (providerMatch || profileMatches(allProfiles[m], subscription, query))
          visibleProfiles.push(allProfiles[m])
      }
      if (query !== "" && !providerMatch && visibleProfiles.length === 0) continue
      var activeProfile = activeSubscriptionProfile(subscription.uuid)
      top.push({
        kind: "subscription", subscription: subscription,
        profiles: query === "" ? allProfiles : visibleProfiles,
        active: activeProfile !== null, favorite: favoriteCount(allProfiles) > 0,
        name: subscription.name
      })
    }
    top.sort(function(a, b) {
      if (a.active !== b.active) return a.active ? -1 : 1
      if (a.favorite !== b.favorite) return a.favorite ? -1 : 1
      return a.name < b.name ? -1 : (a.name > b.name ? 1 : 0)
    })
    var rows = []
    for (var t = 0; t < top.length; t++) {
      rows.push(top[t])
      if (top[t].kind !== "subscription"
          || (query === "" && expandedSubscriptions[top[t].subscription.uuid] !== true))
        continue
      rows.push({ kind: "subscription-sort", subscription: top[t].subscription })
      for (var p = 0; p < top[t].profiles.length; p++) {
        rows.push({ kind: "profile", profile: top[t].profiles[p], nested: true })
      }
    }
    return rows
  }

  readonly property var profileRows: buildProfileRows()

  function rowKey(row) {
    if (!row) return ""
    return row.kind === "subscription"
      ? "subscription:" + row.subscription.uuid
      : (row.kind === "subscription-sort"
        ? "subscription-sort:" + row.subscription.uuid
      : "profile:" + row.profile.uuid
      )
  }

  function toggleSubscriptionGroup(subscriptionUuid) {
    var target = String(subscriptionUuid || "")
    if (target === "") return
    rememberCursor()
    var next = {}
    for (var key in expandedSubscriptions) next[key] = expandedSubscriptions[key]
    next[target] = next[target] !== true
    expandedSubscriptions = next
    Qt.callLater(restoreCursor)
  }

  // Rows can re-sort after a connection or latency test, so the cursor tracks
  // the item it was on instead of the numeric slot that item used to occupy.
  property string cursorKey: ""

  function rememberCursor() {
    cursorKey = rowKey(selectedRow())
  }

  function restoreCursor() {
    if (focusSection === "configs" && cursorKey !== "") {
      for (var i = 0; i < profileRows.length; i++) {
        if (rowKey(profileRows[i]) === cursorKey) {
          configIndex = i
          break
        }
      }
    }
    ensureCursor()
  }

  function ensureCursor() {
    if (profileRows.length === 0) {
      focusSection = "header"
      configIndex = 0
      return
    }
    if (focusSection !== "configs" && focusSection !== "header") focusSection = "configs"
    if (configIndex >= profileRows.length) configIndex = Math.max(0, profileRows.length - 1)
    if (configIndex < 0) configIndex = 0
    // Whatever the cursor ended up on is what it should follow next time —
    // a clamp that lands on a different profile must not keep chasing the
    // one it left behind.
    rememberCursor()
  }

  function moveCursor(dx, dy) {
    cursorActive = true
    if (page === "subscriptions") {
      if (vless.subscriptions.length === 0 || dy === 0) return
      subscriptionIndex = Math.max(0, Math.min(
        vless.subscriptions.length - 1, subscriptionIndex + dy
      ))
      scrollSubscriptionCursorIntoView()
      return
    }
    ensureCursor()
    if (dy === 0) return
    if (focusSection === "header") {
      if (dy > 0 && profileRows.length > 0) {
        focusSection = "configs"
        configIndex = 0
        scrollCursorIntoView()
      }
      return
    }
    if (focusSection === "configs") {
      if (dy < 0 && configIndex === 0) {
        setHeaderCursor()
        return
      }
      configIndex = Math.max(0, Math.min(profileRows.length - 1, configIndex + dy))
      scrollCursorIntoView()
    }
  }

  function setHeaderCursor() {
    cursorActive = true
    focusSection = "header"
    if (panelFlick) panelFlick.contentY = 0
  }

  function setConfigCursor(index) {
    cursorActive = true
    focusSection = "configs"
    pointerSelectingConfig = true
    configIndex = index
    pointerSelectingConfig = false
    // Explicit, because hovering the row the cursor already sits on after a
    // reorder changes no index and so fires no change handler.
    rememberCursor()
  }

  function selectedRow() {
    if (profileRows.length === 0) return null
    return profileRows[Math.max(0, Math.min(configIndex, profileRows.length - 1))]
  }

  function selectedProfile() {
    var row = selectedRow()
    return row && row.kind === "profile" ? row.profile : null
  }

  function activateConfig(profile) {
    if (vless.busy || !profile) return
    if (profile.active) vless.disconnectOne(profile)
    else vless.connectTo(profile)
  }

  function activateCursor() {
    if (page === "subscriptions") {
      var subscription = selectedSubscription()
      if (subscription) toggleSubscriptionGroup(subscription.uuid)
      return
    }
    ensureCursor()
    if (focusSection === "header") vless.toggle()
    else if (focusSection === "configs") {
      var row = selectedRow()
      if (row && row.kind === "subscription") toggleSubscriptionGroup(row.subscription.uuid)
      else if (row && row.kind === "subscription-sort")
        cycleSubscriptionPingSort(row.subscription.uuid)
      else activateConfig(row ? row.profile : null)
    }
  }

  function requestDelete(profile) {
    if (vless.busy || vless.editing || vless.importSourceBusy || !profile) return
    if (profile.managed) {
      openSubscriptions()
      vless.subscriptionStatus = "Managed by " + profile.sourceName + " — edit or remove the subscription here"
      return
    }
    pendingDelete = profile
  }

  function beginImport(kind, payload, suggested) {
    importKind = String(kind)
    importPayload = String(payload)
    // A provider filename can come back empty; offer a neutral profile name.
    importDialog.openWith(suggested !== "" ? String(suggested) : vless.suggestName())
  }

  function cancelImport() {
    importKind = ""
    importPayload = ""
    importDialog.dismiss()
    keyCatcher.forceActiveFocus()
  }

  function requestEdit(profile) {
    if (vless.busy || vless.editing || vless.importSourceBusy || !profile) return
    if (profile.managed) {
      openSubscriptions()
      vless.subscriptionStatus = "Managed by " + profile.sourceName + " — edit the subscription instead"
      return
    }
    pendingEdit = profile
  }

  // Either target takes the panel with it: the config goes to a zenity window
  // that would otherwise open behind a layer surface holding exclusive
  // keyboard focus, and the name goes to a centred window of its own.
  function confirmEdit(kind) {
    var profile = pendingEdit
    pendingEdit = null
    if (!profile || kind === "") return
    // A refused rename (a profile went away, an operation is running) leaves
    // nothing on screen — closing then would just lose the list.
    if (kind === "name") {
      if (!requestRename(profile)) return
    } else {
      if (!handOffToEditor(profile)) return
    }
    close()
  }

  function handOffToEditor(profile) {
    if (profile && profile.managed) {
      openSubscriptions()
      vless.subscriptionStatus = "Managed by " + profile.sourceName + " — edit the subscription instead"
      return false
    }
    if (!vless.editConfig(profile, "")) return false
    editHandedOff = true
    return true
  }

  // Returns whether the prompt opened, so callers know whether the panel has
  // anything to hand over to.
  function requestRename(profile) {
    if (vless.busy || vless.editing || vless.importSourceBusy || !profile) return false
    if (profile.managed) {
      openSubscriptions()
      vless.subscriptionStatus = "Profile names are supplied by " + profile.sourceName
      return false
    }
    // Two overlay surfaces with exclusive keyboard focus would fight; the
    // code the user is no longer looking at loses.
    if (vless.qrVisible) vless.closeQr()
    pendingRename = profile
    renameWindow.openWith(profile.rawName || profile.name)
    return true
  }

  function cancelRename() {
    pendingRename = null
    renameWindow.dismiss()
  }

  function confirmRename() {
    if (!renameAccepted) return
    var profile = pendingRename
    var name = renameClean
    cancelRename()
    vless.renameConfig(profile, name)
  }

  function confirmImport() {
    if (!importAccepted) return
    var kind = importKind
    var payload = importPayload
    var name = importNameClean
    cancelImport()
    if (kind === "file") vless.importFile(payload, name)
    else if (kind === "text") vless.importText(payload, name)
  }

  function copyToClipboard(value) {
    vless.copyText(value)
  }

  function scrollCursorIntoView() {
    if (focusSection !== "configs" || !configRepeater) return
    // itemAt, not configColumn.children[i]: the Repeater itself sits in the
    // children list ahead of its delegates, so raw indexing is off by one.
    var item = configRepeater.itemAt(configIndex)
    if (!panelFlick || !item) return
    Qt.callLater(function() {
      if (!item) return
      var margin = Style.space(6)
      var point = item.mapToItem(panelFlick.contentItem, 0, 0)
      var top = point.y
      var bottom = top + item.height
      var viewTop = panelFlick.contentY
      var viewBottom = viewTop + panelFlick.height
      var maxY = Math.max(0, panelFlick.contentHeight - panelFlick.height)
      if (top < viewTop + margin) panelFlick.contentY = Math.max(0, top - margin)
      else if (bottom > viewBottom - margin) panelFlick.contentY = Math.min(maxY, bottom + margin - panelFlick.height)
    })
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  // Closing tears down neither the QR nor the rename prompt — both live in
  // windows of their own, and showing either closes this panel, so a teardown
  // here would kill the very thing the user just asked for. Opening is the
  // other half of that exclusion: those windows are full-screen with
  // exclusive keyboard focus, so a panel opened underneath one (IPC `open`,
  // or a `pickConfigFile` landing) would be invisible, unclickable and
  // unfocused until it went away.
  onOpenedChanged: {
    pendingDelete = null
    pendingSubscriptionDelete = null
    pendingEdit = null
    editingSubscription = null
    subscriptionPrompt.dismiss()
    routingPresetPrompt.dismiss()
    startupPrompt.dismiss()
    onboardingWizard.dismiss()
    routingToolsPrompt.dismiss()
    cancelImport()
    if (opened) {
      page = "main"
      onboardingDismissed = false
      if (vless.qrVisible) vless.closeQr()
      cancelRename()
      // The error surface is back on screen; whatever happens to the editor
      // now needs no rescue.
      editHandedOff = false
      cursorActive = false
      hoveredSubscriptionServerUuid = ""
      if (panelFlick) panelFlick.contentY = 0
      vless.refresh()
      Qt.callLater(function() {
        keyCatcher.forceActiveFocus()
        if (vless.onboardingNeeded && !root.onboardingDismissed) root.openOnboarding(1)
      })
    }
  }
  onConfigIndexChanged: {
    rememberCursor()
    if (!pointerSelectingConfig) scrollCursorIntoView()
  }
  onProfileRowsChanged: restoreCursor()

  Timer {
    interval: 60000
    repeat: true
    running: root.opened
    triggeredOnStart: true
    onTriggered: root.ageClock = Date.now()
  }

  Service {
    id: vless
    settings: root.settings
    panelVisible: root.opened
    diagnosticsPageVisible: root.opened && root.page === "diagnostics"
    trafficMonitoring: (root.opened && root.page === "main") || vless.showBarThroughput
    pingMonitoring: root.opened && root.page === "main"
  }

  Connections {
    target: vless
    function onProfilesChanged() { root.restoreCursor() }
    function onOnboardingNeededChanged() {
      if (root.opened && vless.onboardingNeeded && !root.onboardingDismissed)
        root.openOnboarding(1)
    }
    function onSubscriptionsChanged() {
      root.subscriptionIndex = Math.max(0, Math.min(
        root.subscriptionIndex, vless.subscriptions.length - 1
      ))
    }
    function onSubscriptionUrlReady(uuid, url) {
      if (!root.subscriptionPrompt.visible || root.editingSubscription === null
          || root.editingSubscription.uuid !== uuid) return
      root.subscriptionPrompt.urlValue = url
      root.subscriptionPrompt.focusUrl()
    }
    // The picker runs whether or not the popup is open (bar right-click,
    // IPC); open the popup so the name prompt has somewhere to appear.
    function onImportReady(kind, payload, suggestedName) {
      if (!root.opened) root.open()
      root.beginImport(kind, payload, suggestedName)
    }
    // The QR window is centred on the screen and takes keyboard focus; the
    // panel behind it is in the way, so it goes — as does a rename prompt,
    // which holds the same kind of surface. One handler covers every entry
    // point — the q key, the row button and IPC.
    function onQrVisibleChanged() {
      if (!vless.qrVisible) return
      if (root.opened) root.close()
      if (root.pendingRename !== null) root.cancelRename()
    }
    // The panel stepped aside for an editor that never appeared — and it is
    // the only place lastError is read, so it comes back to say why. Cancel
    // and no-change do not reach this: they are not failures.
    function onEditFailed(reason) {
      if (!root.editHandedOff) return
      root.editHandedOff = false
      if (!root.opened) root.open()
    }
    // Cancel, no-change and completed saves are terminal but not failures.
    // Retire the UI-only marker so a later headless editor failure cannot
    // mistake this panel for the caller that needs reopening.
    function onEditFinished() { root.editHandedOff = false }
  }

  IpcHandler {
    target: root.ipcTarget
    function open() { root.open() }
    function subscriptions() {
      if (root.opened) {
        root.openSubscriptions()
        return
      }
      root.open()
      // onOpenedChanged deliberately resets every fresh panel to the profile
      // page. Defer the requested destination until that lifecycle hook has
      // finished, otherwise `subscriptions` would visibly open the main page.
      Qt.callLater(root.openSubscriptions)
    }
    function close() { root.close() }
    function show() { root.open() }
    function hide() { root.close() }
    // VPN toggle, not panel visibility — open/close/show/hide already cover
    // the popup, and the bar's left click promises the same thing.
    function toggle(): string {
      return vless.toggle() ? "ok" : "error: " + vless.actionRejection
    }
    function refresh(): string {
      return vless.refresh() ? "ok" : "error: " + vless.actionRejection
    }
    function down(): string {
      return vless.disconnectAll() ? "ok" : "error: " + vless.actionRejection
    }
    function status(): string { return vless.statusText }
    function routing(): string { return vless.routingTitle + " · " + vless.routingSummary }
    // Credential-free support snapshot. Names, ids, endpoints, provider URLs
    // and free-form error strings stay out because they can identify a user
    // even when they do not contain the complete profile credential.
    function diagnostics(): string {
      return JSON.stringify({
        active: vless.active,
        profiles: vless.profiles.length,
        favorites: vless.favoriteProfileCount,
        subscriptions: vless.subscriptions.length,
        routingMode: vless.routing.mode,
        routingSource: vless.routing.source,
        coreInstalled: vless.coreSetup.installed,
        tunReady: vless.coreSetup.tunReady,
        statusFailures: vless.statusFailureCount,
        statusRunning: vless.statusProcessRunning
      })
    }
    // The connection grid without the panel. Rates and ping only move while
    // something is watching them, so a headless caller sees the totals and
    // the addresses live, and "--" where a sample would have to be paid for.
    function details(): string { return vless.detailsText() }
    // Headless import — no prompt, the name is derived from the filename.
    // (`import` is a JS keyword, hence the longer name.)
    function importConfig(path: string): string {
      var name = vless.sanitizeName(path)
      if (!vless.isValidName(name)) return "error: cannot derive a profile name from " + path
      if (vless.countByName(name) > 1) return "error: ambiguous profile name " + name
      return vless.importFile(path, name) ? name : "error: " + vless.actionRejection
    }
    // Takes a profile name or id; a name shared by several
    // profiles is refused rather than resolved to an arbitrary one.
    function edit(target: string): string {
      var resolved = vless.resolveTarget(target)
      if (!resolved.profile) return "error: " + resolved.error
      return vless.editConfig(resolved.profile, "") ? "ok" : "error: " + vless.actionRejection
    }
    // Same target resolution as edit; the new name is a display label, so
    // anything single-line goes — except a name another profile already
    // holds, which would poison every name-based entry point.
    function rename(target: string, newName: string): string {
      var resolved = vless.resolveTarget(target)
      if (!resolved.profile) return "error: " + resolved.error
      var profile = resolved.profile
      var value = String(newName || "").trim()
      if (value === "") return "error: the new name must not be empty"
      if (value !== profile.name && vless.countByName(value) > 0) return "error: a profile named " + value + " already exists"
      return vless.renameConfig(profile, value) ? "ok" : "error: " + vless.actionRejection
    }
    function importPick(): string {
      return vless.pickConfigFile() ? "ok" : "error: " + vless.actionRejection
    }
    function importPaste(): string {
      return vless.pasteConfig() ? "ok" : "error: " + vless.actionRejection
    }
    // Headless export — no warning dialog: an explicit path in argv is
    // already deliberate in a way a panel click is not. The file lands 0600.
    function exportConfig(target: string, path: string): string {
      var resolved = vless.resolveTarget(target)
      if (!resolved.profile) return "error: " + resolved.error
      if (String(path || "") === "") return "error: no destination path"
      return vless.exportToPath(resolved.profile, path) ? "ok" : "error: " + vless.actionRejection
    }
    // The QR has its own window, so this never touches the panel — a
    // headless caller gets the code centred on screen and nothing else.
    function qr(target: string): string {
      var resolved = vless.resolveTarget(target)
      if (!resolved.profile) return "error: " + resolved.error
      // "ok" has to mean a code is coming: with no panel in the way, the
      // return value is the only thing a headless caller gets back.
      var problem = vless.showQr(resolved.profile)
      return problem === "" ? "ok" : "error: " + problem
    }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: vless.showBarThroughput && vless.active && vless.barThroughput !== ""
      ? root.barStatusIcon + " " + vless.barThroughput
      : root.barStatusIcon
    slotSize: vless.showBarThroughput && vless.active && vless.barThroughput !== "" && !vertical
      ? Style.bar.iconSlot * 4 : Style.bar.iconSlot
    tooltipText: root.safeTooltip(root.barTooltip, 220)
    foreground: root.barIconColor
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.RightButton) {
        if (vless.active) vless.disconnectAll()
        else root.open()
      } else if (buttonCode === Qt.MiddleButton) {
        vless.refresh()
        vless.refreshExitIp()
      } else {
        // Left click opens/closes the panel — the VPN toggle lives on the
        // hero switch, the `t` key, and the IPC `toggle` command.
        root.toggle()
      }
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    // Wide enough for the detail grid: two label/value pairs per row, with a
    // real endpoint ("185.55.56.151:51820", not a short TEST-NET one) and a
    // /32 tunnel address both reading in full, and the six pixels the grid
    // gives back to align with the hero switch already deducted. A hostname
    // endpoint can still outgrow it — that is what the tooltip is for.
    contentWidth: panel.fittedContentWidth(Style.space(460))
    contentHeight: panel.fittedContentHeight(
      root.page === "subscriptions" ? subscriptionsColumn.implicitHeight
        : (root.page === "settings" ? settingsColumn.implicitHeight
          : (root.page === "diagnostics"
            ? advancedDiagnosticsPage.implicitHeight : column.implicitHeight)),
      Style.space(600)
    )

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      blocked: root.pendingDelete !== null || root.pendingSubscriptionDelete !== null
        || root.pendingEdit !== null || importDialog.visible || subscriptionPrompt.visible
        || routingPresetPrompt.visible || startupPrompt.visible || onboardingWizard.visible
        || routingToolsPrompt.visible || profileSearch.activeFocus
        || advancedDiagnosticsPage.searchActive
      onMoveRequested: function(dx, dy) {
        if (!root.cursorActive) { root.cursorActive = true; return }
        root.moveCursor(dx, dy)
      }
      onActivateRequested: if (root.cursorActive) root.activateCursor()
      onCloseRequested: {
        if (root.page === "subscriptions") root.closeSubscriptions()
        else if (root.page === "diagnostics") root.closeAdvancedDiagnostics()
        else if (root.page === "settings") root.closeSettings()
        else root.close()
      }
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onDeleteRequested: {
        if (root.page === "subscriptions") {
          if (root.cursorActive && !vless.probingProfiles)
            root.pendingSubscriptionDelete = root.selectedSubscription()
        } else if (root.cursorActive && root.focusSection === "configs") {
          root.requestDelete(root.selectedProfile())
        }
      }
      onTextKey: function(t) {
        if (root.page === "diagnostics") {
          if (t === "r" || t === "R") vless.refreshAdvancedDiagnostics()
          else if (t === "/") advancedDiagnosticsPage.resetSearchFocus()
          return
        }
        if (root.page === "settings") {
          if (t === "s" || t === "S") root.openSubscriptions()
          else if (t === "r" || t === "R") vless.refresh()
          return
        }
        if (root.page === "subscriptions") {
          if (t === "a" || t === "A") root.addSubscription()
          else if (t === "r" || t === "R") vless.refreshAllSubscriptions()
          else if ((t === "e" || t === "E") && root.cursorActive)
            root.editSubscription(root.selectedSubscription())
          else if ((t === "p" || t === "P") && root.cursorActive)
            root.toggleSubscriptionProbe(root.selectedSubscription())
          else if ((t === "x" || t === "X") && root.cursorActive && !vless.probingProfiles)
            root.pendingSubscriptionDelete = root.selectedSubscription()
          return
        }
        if (t === "t" || t === "T") vless.toggle()
        else if (t === "g" || t === "G") root.openSettings()
        else if (t === "/" && vless.supports("subscriptionSearch")) profileSearch.forceActiveFocus()
        else if (t === "r" || t === "R") vless.refresh()
        else if (t === "d" || t === "D") vless.disconnectAll()
        else if (t === "i" || t === "I") vless.pickConfigFile()
        else if (t === "v" || t === "V") vless.pasteConfig()
        else if (t === "f" || t === "F") {
          var favoriteProfile = root.selectedProfile()
          if (root.cursorActive && root.focusSection === "configs" && favoriteProfile)
            vless.toggleFavorite(favoriteProfile)
        }
        else if ((t === "s" || t === "S") && vless.supports("subscriptions")) root.openSubscriptions()
        else if (t === "p" || t === "P") {
          var row = root.selectedRow()
          if (root.cursorActive && root.focusSection === "configs"
              && row && row.kind === "subscription")
            root.toggleSubscriptionProbe(row.subscription)
        }
        // e and n skip the chooser, but not the closing: the panel is as much
        // in the way of zenity and of the rename window as it is of the QR.
        else if (t === "e" || t === "E") {
          if (root.cursorActive && root.focusSection === "configs") {
            if (root.handOffToEditor(root.selectedProfile())) root.close()
          }
        }
        else if (t === "n" || t === "N") {
          if (root.cursorActive && root.focusSection === "configs"
              && root.requestRename(root.selectedProfile())) root.close()
        }
        else if (t === "q" || t === "Q") {
          if (root.cursorActive && root.focusSection === "configs") vless.showQr(root.selectedProfile())
          else if (vless.active) vless.showQr(vless.primaryProfile)
        }
      }

      // The row highlight is the keyboard cursor, and only a cursor target
      // ever moved it — so leaving a row for the hero or the CONFIGS header
      // changed nothing and the row it left stayed lit. This sink lies under
      // the whole panel and takes the hover the instant no cursor target
      // holds it, which is exactly "the pointer left the list". Rows and
      // buttons sit above it and keep their hover, so a row holds its cursor
      // while the pointer is on its own actions. NoButton so clicks still
      // reach the items above it.
      MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.NoButton
        hoverEnabled: true
        onEntered: root.cursorActive = false
      }

      // The sink catches an exit that lands somewhere else in the panel; it
      // cannot catch one that lands nowhere. A pointer leaving the window
      // from a row enters nothing, so the row it left kept the cursor and
      // stayed lit under a pointer that was gone. A non-blocking handler,
      // hovered for the whole panel: rows and buttons below it keep their
      // own hover, and this only goes false when the pointer is out.
      HoverHandler {
        onHoveredChanged: if (!hovered) root.cursorActive = false
      }

      AdvancedDiagnostics {
        id: advancedDiagnosticsPage
        anchors.fill: parent
        visible: root.page === "diagnostics"
        service: vless
        foreground: root.foreground
        dim: root.dim
        urgent: root.urgent
        fontFamily: root.fontFamily
        onBackRequested: root.closeAdvancedDiagnostics()
        onRefreshRequested: vless.refreshAdvancedDiagnostics()
        onRefreshProvidersRequested: vless.refreshRuleProviders()
      }

      Flickable {
        id: panelFlick
        visible: root.page === "main"
        anchors.fill: parent
        contentWidth: width
        contentHeight: column.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.VerticalFlick
        interactive: contentHeight > height
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        Column {
          id: column
          width: Math.max(0, panelFlick.width - root.scrollGutter)
          spacing: Style.space(12)

          Item {
            id: header
            width: parent.width
            implicitHeight: hero.implicitHeight
            // Exposed for the hero's trailingControl, whose `root` resolves to
            // PanelHero (not this Panel) — reach panel state via `header`.
            readonly property bool ringVisible: root.headerHasCursor

            // Only keyboard navigation owns the persistent header cursor.
            // A pointer already gives each control its own hover state; making
            // every hero action select the header as well caused the power
            // switch's outer cursor ring to light up while the pointer was on
            // Settings, QR or Test. Besides looking like a joined hover state,
            // that left the ring behind while crossing between controls.
            function clearKeyboardCursorOnHover(isHovered) {
              if (isHovered) root.cursorActive = false
            }

            PanelHero {
              id: hero
              width: parent.width
              title: "OmaVLESS"
              // PanelHero uses stock Text/AutoText for its metadata line.
              // Keep provider-controlled profile names inert at this sink too,
              // even though Service already normalizes its public model.
              meta: vless.active
                ? root.safeTooltip("Connected: " + vless.activeNames.join(", "), 220)
                : "Disconnected"
              foreground: root.foreground
              fontFamily: root.fontFamily
              iconOpacity: vless.active ? 1.0 : 0.5
              // At display size the protected-link detail remains legible.
              iconComponent: Component {
                PlainText {
                  text: root.heroStatusIcon
                  color: vless.lastError !== "" ? root.urgent : root.iconColor
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.display
                }
              }

              // The switch stays last, hard against the trailing edge — the
              // detail grid below aligns to its track, and anything parked to
              // its right would break that.
              trailingControl: Component {
                Row {
                  spacing: Style.space(4)

                  // Keep uptime on the same optical centerline as the QR,
                  // Test and power controls. PanelHero's built-in detail pill
                  // belongs to the title row and therefore sits visibly high
                  // when the hero also has a second metadata line.
                  BorderSurface {
                    visible: vless.active && vless.uptimeSeconds > 0
                    width: uptimeText.implicitWidth + Style.space(10)
                    height: Style.space(28)
                    anchors.verticalCenter: parent.verticalCenter
                    color: "transparent"
                    borderSpec: Border.controlSpec("normal", hero.foreground, Color.accent)
                    radius: Style.cornerRadius

                    PlainText {
                      id: uptimeText
                      anchors.centerIn: parent
                      text: root.formatUptime(vless.uptimeSeconds)
                      color: root.dim
                      font.family: root.fontFamily
                      font.pixelSize: Style.font.body
                      font.bold: true
                    }
                  }

                  PanelActionButton {
                    iconText: "󰒓"
                    tooltipText: "OmaVLESS settings"
                    anchors.verticalCenter: parent.verticalCenter
                    foreground: hero.foreground
                    bordered: true
                    fontFamily: hero.fontFamily
                    fontSize: Style.font.subtitle * 1.35
                    enabled: !vless.busy && !vless.editing && !vless.importSourceBusy
                    onHovered: function(on) { header.clearKeyboardCursorOnHover(on) }
                    onClicked: root.openSettings()
                  }

                  // The QR of the tunnel you are on, without going to its row
                  // first. Only while something is up: with nothing connected
                  // it would have no subject.
                  PanelActionButton {
                    iconText: "󰐲"
                    tooltipText: root.safeTooltip(
                      "Show " + vless.primaryName + " as a QR code (q)", 180)
                    visible: vless.active && vless.supports("qr")
                    anchors.verticalCenter: parent.verticalCenter
                    // Full brightness at rest, like every other hero control:
                    // dimming until hover is for the row actions, which are
                    // many and would otherwise shout over the list. This one
                    // is alone beside the switch and reads as disabled when
                    // it is merely unhovered. Hover then adds only its fill.
                    foreground: hero.foreground
                    bordered: true
                    fontFamily: hero.fontFamily
                    // A hero action, not a row action: the same size the
                    // network panel gives the glyph it parks here, rather
                    // than the row-sized default the CONFIGS buttons use.
                    fontSize: Style.font.subtitle * 1.5
                    enabled: !vless.busy && !vless.editing && !vless.importSourceBusy
                    onHovered: function(on) { header.clearKeyboardCursorOnHover(on) }
                    onClicked: vless.showQr(vless.primaryProfile)
                  }

                  // A deliberate, one-shot sample of the same live TUN path
                  // shown in the detail grid. It sits between the QR action
                  // and power switch, preserving the switch's edge alignment.
                  Button {
                    text: "Test"
                    iconText: vless.testingConnection ? "󰑓" : ""
                    iconSpinning: vless.testingConnection
                    tooltipText: root.safeTooltip(vless.testingConnection
                      ? "Testing the active tunnel…"
                      : "Test the active tunnel to " + vless.pingHost, 180)
                    visible: vless.active && vless.supports("connectionTest")
                    anchors.verticalCenter: parent.verticalCenter
                    bordered: true
                    foreground: hero.foreground
                    fontFamily: hero.fontFamily
                    fontSize: Style.font.bodySmall
                    enabled: !vless.testingConnection && !vless.busy
                      && !vless.editing && !vless.importSourceBusy
                      && vless.primaryDevice !== "" && vless.pingHost !== ""
                    onHovered: function(on) { header.clearKeyboardCursorOnHover(on) }
                    onClicked: vless.testActiveConnection()
                  }

                  ToggleSwitch {
                    id: powerSwitch
                    visible: vless.profiles.length > 0
                    anchors.verticalCenter: parent.verticalCenter
                    checked: vless.active
                    busy: vless.busy
                    hasCursor: header.ringVisible
                    foreground: hero.foreground
                    onHovered: function(on) { header.clearKeyboardCursorOnHover(on) }
                    onToggled: vless.toggle()

                    PanelToolTip {
                      visible: powerSwitch.containsMouse
                      text: root.safeTooltip(root.toggleHint, 120)
                      fontFamily: hero.fontFamily
                    }
                  }
                }
              }
            }
          }

          RowLayout {
            visible: vless.actionStatus !== "" || vless.lastError !== ""
            width: parent.width
            spacing: Style.space(6)

            PlainText {
              Layout.fillWidth: true
              text: vless.actionStatus !== "" ? vless.actionStatus : vless.lastError
              color: vless.lastError !== "" && vless.actionStatus === "" ? root.urgent : root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.WordWrap
            }

            PanelActionButton {
              visible: vless.messageDismissible
              iconText: "󰅖"
              tooltipText: "Dismiss message"
              foreground: vless.lastError !== "" ? root.urgent : root.dim
              hoverColor: root.foreground
              fontFamily: root.fontFamily
              Layout.alignment: Qt.AlignTop
              onClicked: vless.clearMessage()
            }
          }

          BorderSurface {
            visible: vless.supports("conflictDetection") && vless.hasRoutingConflict
            width: parent.width
            implicitHeight: conflictRow.implicitHeight + Style.space(12)
            color: "transparent"
            borderSpec: Border.flat(Util.alpha(root.foreground, 0.34), Style.normalBorderWidth)
            radius: 0

            RowLayout {
              id: conflictRow
              anchors.fill: parent
              anchors.margins: Style.space(6)
              spacing: Style.space(8)

              PlainText {
                text: "󰀦"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.body
              }
              PlainText {
                Layout.fillWidth: true
                text: "Possible full-tunnel conflict · " + vless.conflictSummary
                color: root.dim
                font.family: root.fontFamily
                font.pixelSize: Style.font.bodySmall
                wrapMode: Text.WordWrap
              }
            }
          }

          // Mode and policy are separate facts. A plain "Rule" badge used to
          // imply smart split routing even when the template only exempted the
          // LAN and sent every public destination to the VPN. Keep the source
          // and the human outcome together, visible before connection too.
          Column {
            visible: vless.supports("routingModes")
            width: parent.width
            spacing: Style.space(8)

            RowLayout {
              width: parent.width
              spacing: Style.space(10)

              PanelSectionHeader {
                text: vless.active ? "ACTIVE ROUTING" : "ROUTING ON CONNECT"
                foreground: root.foreground
                fontFamily: root.fontFamily
              }

              PlainText {
                Layout.fillWidth: true
                text: vless.routingTitle
                color: vless.routingUnavailable ? root.urgent : root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.bodySmall
                horizontalAlignment: Text.AlignRight
                elide: Text.ElideRight
              }
            }

            Row {
              id: routingModeButtons
              width: parent.width
              spacing: Style.space(6)

              Repeater {
                model: [
                  { label: "Full VPN", mode: "global" },
                  { label: "Routing", mode: "rule" },
                  { label: "Direct", mode: "direct" }
                ]

                BorderSurface {
                  required property var modelData

                  readonly property bool selected: vless.routing.mode === modelData.mode
                  readonly property bool available: !vless.busy && !vless.routingUnavailable

                  width: (routingModeButtons.width - routingModeButtons.spacing * 2) / 3
                  height: Style.space(34)
                  color: selected ? Util.alpha(Color.foreground, 0.08) : "transparent"
                  borderSpec: Border.flat(selected
                    ? Color.accent
                    : Util.alpha(root.foreground, available ? 0.38 : 0.18),
                    Style.normalBorderWidth)
                  radius: 0

                  PlainText {
                    anchors.centerIn: parent
                    text: modelData.label
                    color: selected
                      ? Color.accent
                      : (available ? root.foreground : root.dim)
                    font.family: root.fontFamily
                    font.pixelSize: Style.font.caption
                  }

                  MouseArea {
                    anchors.fill: parent
                    enabled: parent.available && !parent.selected
                    hoverEnabled: enabled
                    cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: root.requestRoutingMode(parent.modelData.mode)
                  }
                }
              }
            }

            PlainText {
              width: parent.width
              text: vless.routingSummary
              color: vless.routingUnavailable ? root.urgent : root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              wrapMode: Text.WordWrap
            }
          }

          // What the tunnel is doing, in the shape the network panel uses for
          // the same job. Rates and totals come from the TUN interface counters,
          // the address and endpoint from the profile — nothing here needs a
          // privilege the widget does not already have. Only the first active
          // tunnel is described: one endpoint and one ping cannot stand for
          // two, and the rows below keep their own traffic lines.
          Column {
            visible: vless.active && vless.supports("liveTraffic")
            // Short of the full width by the hero switch's cursor-ring pad:
            // ToggleSwitch reserves that ring outside its track, so the item
            // is flush with the edge while the switch you can see is not.
            // Optical alignment beats geometric here — the numbers and the
            // track are what the eye lines up.
            width: parent.width - Style.space(6)
            spacing: Style.spacing.labelGap

            // Says which tunnel the grid picked when several are active.
            PlainText {
              visible: text !== ""
              width: parent.width
              text: vless.activeNames.length > 1 ? "Showing " + vless.primaryName : ""
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              elide: Text.ElideRight
              bottomPadding: Style.spacing.labelGap
            }

            // Two columns of pairs, not four columns of cells: a four-column
            // grid sizes every column to its own widest value, and one long
            // endpoint then makes the right half visibly wider than the left.
            // Each pair takes exactly half instead, whatever is in it.
            GridLayout {
              width: parent.width
              columns: 2
              columnSpacing: Style.space(14)
              rowSpacing: Style.spacing.labelGap

              // The whole row goes when the probe is off — a permanent pair of
              // dashes would suggest a measurement that failed rather than one
              // nobody asked for. GridLayout skips invisible items, so the
              // rows below close the gap.
              DetailPair {
                visible: vless.pingHost !== ""
                label: "Ping"
                value: vless.fmtPing(vless.pingLatency)
                valueColor: root.foreground
              }
              DetailPair {
                visible: vless.pingHost !== ""
                label: "Packet Loss"
                value: vless.fmtLoss(vless.pingLoss)
                valueColor: root.foreground
              }

              // "--" until a rate has actually been measured: the first
              // sample of a session has no interval behind it, and printing
              // its zero would claim an idle tunnel on no evidence.
              DetailPair {
                label: "Receiving"
                value: vless.trafficRate(vless.primaryDevice, "rxRate")
              }
              DetailPair {
                label: "Sending"
                value: vless.trafficRate(vless.primaryDevice, "txRate")
              }

              // Session totals from the Mihomo TUN interface.
              DetailPair {
                label: "Downloaded"
                value: vless.trafficTotal(vless.primaryDevice, "rx")
              }
              DetailPair {
                label: "Uploaded"
                value: vless.trafficTotal(vless.primaryDevice, "tx")
              }

              DetailPair {
                label: "TUN Address"
                value: root.detailText(vless.detail("address"))
                tooltipText: "Copy the tunnel address"
              }
              DetailPair {
                label: "Server"
                value: root.detailText(vless.detail("server"))
                tooltipText: "Copy the endpoint"
              }

              // Copyable like the two above, and for the same reason: a route
              // list is exactly what a user pastes into the next config.
              DetailPair {
                label: "Transport"
                value: root.detailText(vless.detail("transport"))
                tooltipText: "Copy transport and security"
              }
              DetailPair {
                label: "SNI"
                value: root.detailText(vless.detail("sni"))
                tooltipText: "Copy the server name"
              }

              DetailPair {
                visible: vless.supports("exitIp")
                label: "Exit IP"
                value: vless.exitIpFetching ? "Checking…"
                  : (vless.exitIp !== "" ? vless.exitIp : "--")
                tooltipText: vless.exitIp !== ""
                  ? "Copy observed exit IP; this does not verify every routing rule" : ""
              }
            }

            PlainText {
              visible: vless.supports("exitIp") && vless.exitIp !== ""
              width: parent.width
              text: "Exit IP is this request's observed path, not proof of the complete routing policy."
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              wrapMode: Text.WordWrap
            }

            Column {
              visible: vless.supports("trafficHistory")
                && (vless.rxHistory.length > 1 || vless.txHistory.length > 1)
              width: parent.width
              spacing: Style.space(10)

              RowLayout {
                width: parent.width
                PlainText {
                  text: "TRAFFIC · LAST " + (vless.historyMaxPoints * 2) + "S"
                  color: root.dim
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.caption
                }
                Item { Layout.fillWidth: true }
                PlainText {
                  text: "↓ " + vless.trafficRate(vless.primaryDevice, "rxRate")
                  color: root.trafficRxColor
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.caption
                  font.bold: true
                }
                PlainText {
                  text: "↑ " + vless.trafficRate(vless.primaryDevice, "txRate")
                  color: root.trafficTxColor
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.caption
                  font.bold: true
                }
              }

              Sparkline {
                width: parent.width
                height: Style.space(42)
                rxValues: vless.rxHistory
                txValues: vless.txHistory
                rxColor: root.trafficRxColor
                txColor: root.trafficTxColor
                guideColor: Util.alpha(root.foreground, 0.16)
              }
            }
          }

          PanelSeparator {
            foreground: root.foreground
          }

          Column {
            width: parent.width
            spacing: Style.space(10)

            Item {
              width: parent.width
              implicitHeight: Math.max(sectionLabel.implicitHeight, importActions.implicitHeight)

              PanelSectionHeader {
                id: sectionLabel
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                text: "PROFILES"
                foreground: root.foreground
                fontFamily: root.fontFamily
              }

              Row {
                id: importActions
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: Style.space(2)

                Button {
                  id: subscriptionsButton
                  text: "Subscriptions…"
                  visible: vless.supports("subscriptions")
                  tooltipText: "Manage profile subscriptions"
                  bordered: true
                  foreground: root.foreground
                  fontFamily: root.fontFamily
                  enabled: !vless.busy && !vless.importSourceBusy && !vless.editing
                  onHovered: function(on) { if (on) root.cursorActive = false }
                  onClicked: root.openSubscriptions()
                }

                // These two are not cursor targets, but they are the one thing
                // close enough to the first row that a fast pointer can reach
                // them without ever crossing the sink below. Drop the cursor
                // here too, or that row stays lit while the pointer is here.
                PanelActionButton {
                  iconText: "󰐕"
                  tooltipText: vless.filePicker.available
                    ? "Import a profile link file (i)"
                    : "File import unavailable — install zenity"
                  foreground: root.dim
                  hoverColor: root.foreground
                  fontFamily: root.fontFamily
                  size: subscriptionsButton.implicitHeight
                  anchors.verticalCenter: parent.verticalCenter
                  enabled: !vless.busy && !vless.importSourceBusy && !vless.editing
                  onHovered: function(on) { if (on) root.cursorActive = false }
                  onClicked: vless.pickConfigFile()
                }

                PanelActionButton {
                  iconText: "󰅌"
                  tooltipText: "Import from clipboard (v)"
                  foreground: root.dim
                  hoverColor: root.foreground
                  fontFamily: root.fontFamily
                  size: subscriptionsButton.implicitHeight
                  anchors.verticalCenter: parent.verticalCenter
                  enabled: !vless.busy && !vless.importSourceBusy && !vless.editing
                  onHovered: function(on) { if (on) root.cursorActive = false }
                  onClicked: vless.pasteConfig()
                }
              }
            }

            SubscriptionStatusRow {
              width: parent.width
            }

            TextField {
              id: profileSearch
              visible: vless.supports("subscriptionSearch") && vless.profiles.length >= 8
              width: parent.width
              placeholderText: "Search profiles, countries, hosts…  (/)"
              text: root.profileFilter
              color: root.foreground
              placeholderTextColor: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
              selectByMouse: true
              leftPadding: Style.space(10)
              rightPadding: Style.space(10)
              onTextChanged: root.profileFilter = text
              Keys.onEscapePressed: function(event) {
                if (text !== "") text = ""
                else keyCatcher.forceActiveFocus()
                event.accepted = true
              }
              background: Rectangle {
                color: profileSearch.activeFocus
                  ? Util.alpha(root.foreground, 0.06) : "transparent"
                border.color: profileSearch.activeFocus
                  ? Color.accent : Util.alpha(root.foreground, 0.38)
                border.width: Style.normalBorderWidth
                radius: 0
              }
            }

            PlainText {
              visible: vless.profiles.length === 0
              width: parent.width
              text: "No profiles yet\nImport a link file with + or paste one from the clipboard with v"
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.body
              wrapMode: Text.WrapAnywhere
              horizontalAlignment: Text.AlignHCenter
            }

            PlainText {
              visible: vless.profiles.length > 0 && root.profileRows.length === 0
                && root.profileFilter.trim() !== ""
              width: parent.width
              text: "No profiles match “" + root.profileFilter.trim() + "”"
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.body
              wrapMode: Text.WordWrap
              horizontalAlignment: Text.AlignHCenter
            }

            Column {
              id: configColumn
              visible: root.profileRows.length > 0
              width: parent.width
              spacing: Style.space(6)

              Repeater {
                id: configRepeater
                model: root.profileRows
                Item {
                  required property var modelData
                  required property int index
                  width: configColumn.width
                  implicitHeight: modelData.kind === "subscription"
                    ? groupRow.implicitHeight
                    : (modelData.kind === "subscription-sort"
                      ? sortRow.implicitHeight : profileRow.implicitHeight)

                  SubscriptionGroupRow {
                    id: groupRow
                    visible: parent.modelData.kind === "subscription"
                    width: parent.width
                    subscription: visible ? parent.modelData.subscription : null
                    profiles: visible ? parent.modelData.profiles : []
                    rowIndex: parent.index
                  }

                  SubscriptionSortRow {
                    id: sortRow
                    visible: parent.modelData.kind === "subscription-sort"
                    width: parent.width
                    subscription: visible ? parent.modelData.subscription : null
                    rowIndex: parent.index
                  }

                  ConfigRow {
                    id: profileRow
                    visible: parent.modelData.kind === "profile"
                    width: parent.width
                    profile: visible ? parent.modelData.profile : null
                    nested: visible && parent.modelData.nested === true
                    rowIndex: parent.index
                  }
                }
              }
            }

            Item {
              width: 1
              height: Style.space(8)
            }
          }
        }
      }

      Flickable {
        id: settingsFlick
        anchors.fill: parent
        visible: root.page === "settings"
        contentWidth: width
        contentHeight: settingsColumn.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.VerticalFlick
        interactive: contentHeight > height
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        Column {
          id: settingsColumn
          width: Math.max(0, settingsFlick.width - root.scrollGutter)
          spacing: Style.space(12)

          RowLayout {
            width: parent.width
            spacing: Style.space(8)

            PanelActionButton {
              iconText: "󰁍"
              tooltipText: "Back to profiles (Esc)"
              foreground: root.foreground
              hoverColor: Color.accent
              fontFamily: root.fontFamily
              onClicked: root.closeSettings()
            }

            ColumnLayout {
              Layout.fillWidth: true
              spacing: 0
              PlainText {
                Layout.fillWidth: true
                text: "SETTINGS"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.title
              }
              PlainText {
                Layout.fillWidth: true
                text: "Routing, connections, privacy and panel display"
                color: root.dim
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
              }
            }
          }

          PlainText {
            visible: vless.actionStatus !== "" || vless.lastError !== ""
            width: parent.width
            text: vless.actionStatus !== "" ? vless.actionStatus : vless.lastError
            color: vless.lastError !== "" && vless.actionStatus === "" ? root.urgent : root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
            wrapMode: Text.WordWrap
          }

          PanelSeparator { foreground: root.foreground }

          PanelSectionHeader {
            text: "SETUP & STARTUP"
            foreground: root.foreground
            fontFamily: root.fontFamily
          }

          SettingsActionRow {
            title: "Mihomo core"
            description: vless.coreSetupLabel
              + (vless.coreSetup.path !== "" ? " · " + vless.coreSetup.path : "")
            actionText: vless.coreSetup.tunReady ? "Ready" : "Setup"
            onAction: root.openOnboarding(1)
          }

          SettingsActionRow {
            title: "Start VPN at login"
            description: vless.startupSummary
            actionText: "Configure"
            onAction: startupPrompt.openWith(vless.startup)
          }

          PanelSeparator { foreground: root.foreground }

          PanelSectionHeader {
            text: "ROUTING PROFILE"
            foreground: root.foreground
            fontFamily: root.fontFamily
          }

          PlainText {
            width: parent.width
            text: !vless.routingPresetConfigured
              ? "Choose a country preset before the first use of Routing."
              : (vless.activeRoutingPreset
                ? "Selected: " + vless.routingPresetName + ". Full VPN and Direct remain independent."
                : "Current template: " + vless.routingSourceLabel
                  + ". Choose a country preset below or keep the existing policy.")
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            wrapMode: Text.WordWrap
          }

          Repeater {
            model: vless.routingPresets

            BorderSurface {
              id: routingPresetCard
              required property var modelData
              readonly property bool selected: vless.routingPresetConfigured
                && vless.routing.preset === modelData.id

              width: settingsColumn.width
              height: routingPresetContent.implicitHeight + Style.space(18)
              color: selected ? Util.alpha(Color.foreground, 0.08) : "transparent"
              borderSpec: Border.flat(selected ? Color.accent : Util.alpha(root.foreground, 0.30),
                Style.normalBorderWidth)
              radius: Style.cornerRadius

              Column {
                id: routingPresetContent
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.leftMargin: Style.space(10)
                anchors.rightMargin: Style.space(10)
                spacing: Style.space(5)

                RowLayout {
                  width: parent.width
                  spacing: Style.space(8)

                  PlainText {
                    Layout.fillWidth: true
                    text: routingPresetCard.modelData.country + " · " + routingPresetCard.modelData.name
                    color: routingPresetCard.selected ? Color.accent : root.foreground
                    font.family: root.fontFamily
                    font.pixelSize: Style.font.body
                    font.bold: true
                    elide: Text.ElideRight
                  }

                  Button {
                    text: routingPresetCard.selected ? "Selected" : "Use"
                    bordered: true
                    enabled: !routingPresetCard.selected && !vless.busy
                    foreground: enabled ? root.foreground : root.dim
                    fontFamily: root.fontFamily
                    onClicked: vless.useRoutingPreset(routingPresetCard.modelData.id, true)
                  }
                }

                PlainText {
                  width: parent.width
                  text: routingPresetCard.modelData.summary
                  color: root.dim
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.caption
                  wrapMode: Text.WordWrap
                }

                Button {
                  text: "Source · " + routingPresetCard.modelData.source
                  tooltipText: "Open routing rule source"
                  bordered: false
                  foreground: root.dim
                  fontFamily: root.fontFamily
                  fontSize: Style.font.caption
                  onClicked: Qt.openUrlExternally(routingPresetCard.modelData.sourceUrl)
                }
              }
            }
          }

          SettingsActionRow {
            title: "Routing tools"
            description: root.plural(vless.routing.customRuleCount,
              "custom rule", "custom rules") + " · check a domain"
            actionText: "Open"
            onAction: root.openRoutingTools()
          }

          SettingsActionRow {
            title: "Remote rule data"
            description: vless.routing.rulesUpdatedAt > 0
              ? root.subscriptionAge(vless.routing.rulesUpdatedAt)
              : "Automatic schedule · not checked manually"
            actionText: vless.busy ? "Updating…" : "Refresh"
            actionEnabled: vless.routing.ruleUpdateAvailable && !vless.busy
            onAction: vless.refreshRuleProviders()
          }

          PanelSeparator { foreground: root.foreground }

          PanelSectionHeader {
            text: "CONNECTIONS"
            foreground: root.foreground
            fontFamily: root.fontFamily
          }

          SettingsActionRow {
            title: "Subscriptions"
            description: root.plural(vless.subscriptions.length, "provider", "providers")
              + " · " + (vless.latestSubscriptionUpdatedAt > 0
                ? root.subscriptionAge(vless.latestSubscriptionUpdatedAt).toLowerCase()
                : "never updated")
            actionText: "Manage"
            onAction: root.openSubscriptions()
          }

          SettingsActionRow {
            title: "Connection monitoring"
            description: "Open every " + vless.refreshIntervalSec
              + "s · background every " + Math.max(30, vless.refreshIntervalSec)
              + "s · latency " + (vless.pingHost !== "" ? vless.pingHost : "disabled")
            actionText: "Configured"
            actionEnabled: false
          }

          PanelSeparator { foreground: root.foreground }

          PanelSectionHeader {
            text: "DIAGNOSTICS & PRIVACY"
            foreground: root.foreground
            fontFamily: root.fontFamily
          }

          SettingsActionRow {
            title: "Live Mihomo diagnostics"
            description: "Loaded rules and rule providers · private controller only"
            actionText: "Open"
            actionEnabled: !vless.busy
            onAction: root.openAdvancedDiagnostics()
          }

          SettingsActionRow {
            title: "Safe diagnostics"
            description: vless.diagnosticsStatus !== ""
              ? vless.diagnosticsStatus
              : "No profile credentials, keys, server names or subscription URLs"
            actionText: vless.diagnosticsExporting ? "Exporting…" : "Export"
            actionEnabled: !vless.diagnosticsExporting && !vless.busy
            onAction: vless.exportDiagnostics()
          }

          SettingsActionRow {
            title: "Observed Exit IP"
            description: "Show a bounded external path check in connection details"
            actionText: vless.showExitIp ? "On" : "Off"
            onAction: root.setWidgetSetting("showExitIp", !vless.showExitIp, true)
          }

          SettingsActionRow {
            title: "Live throughput in bar"
            description: "Keep the compact bar icon quiet unless explicitly enabled"
            actionText: vless.showBarThroughput ? "On" : "Off"
            onAction: root.setWidgetSetting("showBarThroughput", !vless.showBarThroughput, true)
          }

          Item { width: 1; height: Style.space(8) }
        }
      }

      Flickable {
        id: subscriptionsFlick
        anchors.fill: parent
        visible: root.page === "subscriptions"
        contentWidth: width
        contentHeight: subscriptionsColumn.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.VerticalFlick
        interactive: contentHeight > height
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        Column {
          id: subscriptionsColumn
          width: Math.max(0, subscriptionsFlick.width - root.scrollGutter)
          spacing: Style.space(12)

          RowLayout {
            width: parent.width
            spacing: Style.space(8)

            PanelActionButton {
              iconText: "󰁍"
              tooltipText: "Back to profiles (Esc)"
              foreground: root.foreground
              hoverColor: Color.accent
              fontFamily: root.fontFamily
              onClicked: root.closeSubscriptions()
            }

            ColumnLayout {
              Layout.fillWidth: true
              spacing: 0
              PlainText {
                Layout.fillWidth: true
                text: "SUBSCRIPTIONS"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.title
              }
              PlainText {
                Layout.fillWidth: true
                text: root.plural(vless.subscriptions.length, "provider", "providers")
                  + " · " + root.plural(vless.managedProfileCount,
                    "managed profile", "managed profiles")
                color: root.dim
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
              }
            }

            PanelActionButton {
              iconText: "󰑓"
              tooltipText: "Update all subscriptions (r)"
              foreground: root.dim
              hoverColor: root.foreground
              fontFamily: root.fontFamily
              enabled: !vless.busy && !vless.probingProfiles && vless.subscriptions.length > 0
              onClicked: vless.refreshAllSubscriptions()
            }

            Button {
              text: "Add…"
              tooltipText: "Add a profile subscription (a)"
              bordered: true
              foreground: root.foreground
              fontFamily: root.fontFamily
              enabled: !vless.busy && !vless.probingProfiles && !vless.subscriptionEditorLoading
              onClicked: root.addSubscription()
            }
          }

          PlainText {
            width: parent.width
            text: "Managed profiles update only when you ask. Test performs an end-to-end proxy check through every server; results stay in this session."
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            wrapMode: Text.WordWrap
          }

          SubscriptionStatusRow {
            width: parent.width
          }

          PanelSeparator { foreground: root.foreground }

          PlainText {
            visible: vless.subscriptions.length === 0
            width: parent.width
            text: "No subscriptions yet\nAdd the URL supplied by your provider"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            topPadding: Style.space(18)
          }

          Column {
            width: parent.width
            visible: vless.subscriptions.length > 0
            spacing: Style.space(6)
          Repeater {
            id: subscriptionsRepeater
              model: vless.subscriptions
              SubscriptionRow {
                required property var modelData
                required property int index
                width: parent.width
                subscription: modelData
                rowIndex: index
              }
            }
          }

          Item {
            width: 1
            height: Style.space(8)
          }
        }
      }

      OnboardingWizard {
        id: onboardingWizard
        anchors.fill: parent
        coreSetup: vless.coreSetup
        filePicker: vless.filePicker
        presets: vless.routingPresets
        profiles: vless.profiles
        routingPreset: vless.routingPresetConfigured ? vless.routing.preset : ""
        busy: vless.busy || vless.importSourceBusy
        installCommand: root.mihomoInstallCommand
        capabilityCommand: root.mihomoCapabilityCommand
        verifyCommand: root.mihomoVerifyCommand
        foreground: root.foreground
        dim: root.dim
        urgent: root.urgent
        fontFamily: root.fontFamily
        onCopyCommand: function(command) { vless.copyText(command) }
        onRefreshRequested: vless.refresh()
        onPresetChosen: function(preset) {
          if (vless.useRoutingPreset(preset, false)) onboardingWizard.step = 3
        }
        onPasteRequested: vless.pasteConfig()
        onFileRequested: vless.pickConfigFile()
        onFinishRequested: root.finishOnboarding()
        onCanceled: root.dismissOnboarding()
      }

      StartupPrompt {
        id: startupPrompt
        anchors.fill: parent
        profiles: vless.profiles
        startup: vless.startup
        routingAvailable: vless.routingPresetConfigured
        coreReady: vless.coreSetup.tunReady
        busy: vless.busy
        foreground: root.foreground
        dim: root.dim
        urgent: root.urgent
        fontFamily: root.fontFamily
        onConfirmed: function(enabled, target, profileId, mode) {
          root.saveStartup(enabled, target, profileId, mode)
        }
        onSetupRequested: {
          startupPrompt.dismiss()
          root.openOnboarding(1)
        }
        onCanceled: {
          dismiss()
          Qt.callLater(function() { keyCatcher.forceActiveFocus() })
        }
      }

      RoutingToolsPrompt {
        id: routingToolsPrompt
        anchors.fill: parent
        rules: vless.customRules
        result: vless.routeCheckResult
        loading: vless.routingToolsLoading || vless.routeChecking
        busy: vless.busy || vless.routingToolsLoading || vless.routeChecking
        refreshAvailable: vless.routing.ruleUpdateAvailable
        rulesUpdatedLabel: vless.routing.rulesUpdatedAt > 0
          ? root.subscriptionAge(vless.routing.rulesUpdatedAt)
          : "Automatic schedule · not checked manually"
        statusText: vless.routingToolStatus
        errorText: vless.routingToolError
        foreground: root.foreground
        dim: root.dim
        urgent: root.urgent
        fontFamily: root.fontFamily
        onAddRule: function(kind, action, value) {
          vless.addCustomRule(kind, action, value)
        }
        onDeleteRule: function(rule) { vless.deleteCustomRule(rule) }
        onCheckRoute: function(value) { vless.checkRoute(value) }
        onRefreshRules: vless.refreshRuleProviders()
        onCanceled: root.closeRoutingTools()
      }

      RoutingPresetPrompt {
        id: routingPresetPrompt
        anchors.fill: parent
        presets: vless.routingPresets
        accepted: selectedPreset !== "" && !vless.busy
        foreground: root.foreground
        dim: root.dim
        fontFamily: root.fontFamily
        onConfirmed: function(preset) { root.applyFirstRoutingPreset(preset) }
        onCanceled: {
          dismiss()
          Qt.callLater(function() { keyCatcher.forceActiveFocus() })
        }
      }

      // Backend-parsed, redacted preview shared by file and clipboard import.
      // The raw URI never becomes a displayed QML property.
      ImportPreviewPrompt {
        id: importDialog
        anchors.fill: parent
        title: root.importSourceLabel
        preview: vless.importPreview
        hint: root.importHintText
        accepted: root.importAccepted
        confirmLabel: root.importReplaces ? "Replace" : "Import"
        foreground: root.foreground
        dim: root.dim
        urgent: root.urgent
        fontFamily: root.fontFamily
        onConfirmed: root.confirmImport()
        onCanceled: root.cancelImport()
      }

      SubscriptionPrompt {
        id: subscriptionPrompt
        anchors.fill: parent
        hint: root.subscriptionHint
        accepted: root.subscriptionAccepted
        loading: root.editingSubscription !== null && vless.subscriptionEditorLoading
        error: vless.subscriptionError !== ""
        foreground: root.foreground
        dim: root.dim
        urgent: root.urgent
        fontFamily: root.fontFamily
        onConfirmed: root.confirmSubscription()
        onCanceled: {
          root.editingSubscription = null
          dismiss()
          keyCatcher.forceActiveFocus()
        }
      }

      // One pencil, two targets: the chooser splits "edit" into the config
      // text (zenity round-trip) and the display name (rename prompt).
      // Keyboard users never see it — e and n go straight to either.
      Item {
        id: editChooser
        anchors.fill: parent
        visible: root.pendingEdit !== null

        property int selectedIndex: 1
        readonly property var choices: [
          { label: "Cancel", kind: "" },
          { label: "Config", kind: "config" },
          { label: "Name", kind: "name" }
        ]

        onVisibleChanged: {
          if (visible) {
            selectedIndex = 1
            editChooser.forceActiveFocus()
          } else {
            // Harmless when a choice closed the panel: reopening puts the
            // focus back on the key catcher anyway.
            keyCatcher.forceActiveFocus()
          }
        }
        Keys.onPressed: function(event) {
          if (event.key === Qt.Key_Escape) root.pendingEdit = null
          else if (event.key === Qt.Key_Left || event.key === Qt.Key_Backtab)
            selectedIndex = (selectedIndex + choices.length - 1) % choices.length
          else if (event.key === Qt.Key_Right || event.key === Qt.Key_Tab)
            selectedIndex = (selectedIndex + 1) % choices.length
          else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
            root.confirmEdit(choices[selectedIndex].kind)
          else if (event.key === Qt.Key_E || event.key === Qt.Key_C) root.confirmEdit("config")
          else if (event.key === Qt.Key_N) root.confirmEdit("name")
          else return
          event.accepted = true
        }

        Rectangle {
          anchors.fill: parent
          color: Util.alpha(Color.background, 0.7)

          MouseArea { anchors.fill: parent; onClicked: root.pendingEdit = null }

          BorderSurface {
            id: editCard
            width: Math.min(parent.width - Style.space(32), Style.space(340))
            height: editCard.contentTopInset + editCard.contentBottomInset
              + editMessage.implicitHeight + Style.space(20) + Style.space(34)
            anchors.centerIn: parent
            color: Color.background
            borderSpec: Border.flat(Color.accent, Style.normalBorderWidth)
            padding: Style.space(18)
            radius: Style.cornerRadius

            MouseArea { anchors.fill: parent; onClicked: {} }

            Item {
              anchors.fill: parent
              anchors.topMargin: editCard.contentTopInset
              anchors.rightMargin: editCard.contentRightInset
              anchors.bottomMargin: editCard.contentBottomInset
              anchors.leftMargin: editCard.contentLeftInset

              PlainText {
                id: editMessage
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                text: "Edit " + (root.pendingEdit ? root.pendingEdit.name : "") + "?"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.title
                wrapMode: Text.WordWrap
              }

              Row {
                id: editButtonsRow
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                spacing: Style.space(10)

                Repeater {
                  model: editChooser.choices

                  BorderSurface {
                    required property int index
                    required property var modelData

                    readonly property bool selected: editChooser.selectedIndex === index

                    // Three equal shares of the card, not the fixed 88 the
                    // two-button ConfirmDialog gets away with — three of
                    // those overflow this card's width.
                    width: (editButtonsRow.width - Style.space(10) * 2) / 3
                    height: Style.space(34)
                    color: selected ? Util.alpha(Color.foreground, 0.08) : "transparent"
                    borderSpec: Border.flat(selected
                      ? Color.accent
                      : Util.alpha(root.foreground, 0.38), Style.normalBorderWidth)
                    radius: 0

                    PlainText {
                      anchors.centerIn: parent
                      text: modelData.label
                      color: selected ? Color.accent : root.foreground
                      font.family: root.fontFamily
                      font.pixelSize: Style.font.caption
                    }

                    MouseArea {
                      anchors.fill: parent
                      hoverEnabled: true
                      cursorShape: Qt.PointingHandCursor
                      onEntered: editChooser.selectedIndex = index
                      onClicked: root.confirmEdit(modelData.kind)
                    }
                  }
                }
              }
            }
          }
        }
      }

      ConfirmDialog {
        id: deleteDialog
        anchors.fill: parent
        opened: root.pendingDelete !== null
        // ConfirmDialog is a shared Omarchy control whose message uses an
        // AutoText-capable Text item. Keep imported metadata inert at this
        // final sink even though Service already normalizes public names.
        message: root.safeTooltip("Delete profile "
          + (root.pendingDelete ? root.pendingDelete.name : "") + "?", 180)
        confirmText: "Delete"
        foreground: root.foreground
        fontFamily: root.fontFamily
        Keys.onPressed: function(event) { event.accepted = deleteDialog.handleKey(event) }
        onOpenedChanged: {
          if (opened) {
            selectedIndex = 0
            forceActiveFocus()
          } else {
            keyCatcher.forceActiveFocus()
          }
        }
        onCanceled: root.pendingDelete = null
        onConfirmed: {
          var profile = root.pendingDelete
          root.pendingDelete = null
          vless.deleteConfig(profile)
        }
      }

      ConfirmDialog {
        id: subscriptionDeleteDialog
        anchors.fill: parent
        opened: root.pendingSubscriptionDelete !== null
        message: root.safeTooltip("Remove subscription "
          + (root.pendingSubscriptionDelete ? root.pendingSubscriptionDelete.name : "")
          + " and its managed profiles?", 220)
        confirmText: "Remove"
        foreground: root.foreground
        fontFamily: root.fontFamily
        Keys.onPressed: function(event) { event.accepted = subscriptionDeleteDialog.handleKey(event) }
        onOpenedChanged: {
          if (opened) {
            selectedIndex = 0
            forceActiveFocus()
          } else keyCatcher.forceActiveFocus()
        }
        onCanceled: root.pendingSubscriptionDelete = null
        onConfirmed: {
          var subscription = root.pendingSubscriptionDelete
          root.pendingSubscriptionDelete = null
          vless.deleteSubscription(subscription)
        }
      }
    }
  }

  // Screen-centred, not panel-bound: a profile URI makes a far denser
  // code than the popup can show at a scannable size. Closing it deletes
  // the PNG in XDG_RUNTIME_DIR.
  QrWindow {
    id: qrWindow
    anchorItem: button
    open: vless.qrVisible
    name: vless.qrName
    path: vless.qrPath
    loading: vless.qrLoading
    error: vless.qrError
    foreground: root.foreground
    dim: root.dim
    urgent: root.urgent
    fontFamily: root.fontFamily
    onCloseRequested: vless.closeQr()
  }

  // Screen-centred like the QR, and for the same reason: the popup is pinned
  // under its bar icon, so a prompt inside it opens in a corner with a field
  // squeezed to the column's width. Opening it closes the panel; closing it
  // leaves the panel closed.
  RenameWindow {
    id: renameWindow
    anchorItem: button
    open: root.pendingRename !== null
    title: "Rename " + (root.pendingRename ? root.pendingRename.name : "")
    placeholder: "Profile name"
    hint: root.renameHint
    accepted: root.renameAccepted
    confirmLabel: "Rename"
    foreground: root.foreground
    dim: root.dim
    urgent: root.urgent
    fontFamily: root.fontFamily
    onConfirmed: root.confirmRename()
    onCanceled: root.cancelRename()
  }

  // Every cell in the grid holds its place from the moment the tunnel comes
  // up, so a value that is not in yet reads as "--" rather than collapsing
  // the row and shoving the list below it around.
  function detailText(value) {
    var text = String(value || "")
    return text === "" ? "--" : text
  }

  component InfoLabel: PlainText {
    color: root.foreground
    opacity: 0.6
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
  }

  component SubscriptionStatusRow: RowLayout {
    visible: vless.subscriptionStatus !== "" || vless.subscriptionError !== ""
    spacing: Style.space(6)

    PlainText {
      Layout.fillWidth: true
      text: root.subscriptionMessageText()
      color: vless.subscriptionError !== "" ? root.urgent : root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      wrapMode: Text.WordWrap
    }

    PanelActionButton {
      visible: !vless.probingProfiles
      iconText: "󰅖"
      tooltipText: "Dismiss message"
      foreground: vless.subscriptionError !== "" ? root.urgent : root.dim
      hoverColor: root.foreground
      fontFamily: root.fontFamily
      Layout.alignment: Qt.AlignTop
      onClicked: vless.clearSubscriptionMessage()
    }
  }

  component SettingsActionRow: BorderSurface {
    id: settingRow

    property string title: ""
    property string description: ""
    property string actionText: ""
    property bool actionEnabled: true
    signal action()

    width: settingsColumn.width
    height: settingContent.implicitHeight + Style.space(16)
    color: "transparent"
    borderSpec: Border.flat(Util.alpha(root.foreground, 0.28), Style.normalBorderWidth)
    radius: Style.cornerRadius

    RowLayout {
      id: settingContent
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      anchors.leftMargin: Style.space(10)
      anchors.rightMargin: Style.space(10)
      spacing: Style.space(10)

      ColumnLayout {
        Layout.fillWidth: true
        spacing: Style.space(2)
        PlainText {
          Layout.fillWidth: true
          text: settingRow.title
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
        }
        PlainText {
          Layout.fillWidth: true
          text: settingRow.description
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          wrapMode: Text.WordWrap
        }
      }

      Button {
        text: settingRow.actionText
        bordered: true
        enabled: settingRow.actionEnabled
        foreground: enabled ? root.foreground : root.dim
        fontFamily: root.fontFamily
        onClicked: settingRow.action()
      }
    }
  }

  // Label left, value hard against the right edge of its half. The label
  // keeps its natural width and the value takes the rest, so a long endpoint
  // elides inside its own half instead of stealing width from the half next
  // to it.
  component DetailPair: Item {
    id: pair

    property string label: ""
    property string value: ""
    property color valueColor: root.foreground
    // A tooltip is what marks a value as worth copying — the ones that are
    // are exactly the ones a user retypes elsewhere.
    property string tooltipText: ""
    readonly property bool copyable: tooltipText !== "" && value !== "--"

    // Equal preferred widths plus fillWidth is what makes the halves match:
    // ask for nothing, and the layout hands both cells the same share.
    Layout.fillWidth: true
    Layout.preferredWidth: 0
    implicitHeight: Math.max(pairLabel.implicitHeight, pairValue.implicitHeight)

    InfoLabel {
      id: pairLabel
      anchors.left: parent.left
      anchors.verticalCenter: parent.verticalCenter
      text: pair.label
    }

    PlainText {
      id: pairValue
      anchors.left: pairLabel.right
      anchors.leftMargin: Style.space(8)
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      text: pair.value
      color: pair.valueColor
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      horizontalAlignment: Text.AlignRight
      elide: Text.ElideRight

      MouseArea {
        id: valueMouse
        anchors.fill: parent
        enabled: pair.copyable && !vless.copying
        hoverEnabled: enabled
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.copyToClipboard(pair.value)
      }

      // An elided value is unreadable, and these are the ones worth reading —
      // so the tooltip carries the whole string when the cell could not.
      PanelToolTip {
        visible: valueMouse.enabled && valueMouse.containsMouse
        text: root.safeTooltip(pairValue.truncated
          ? pair.value + " · " + pair.tooltipText : pair.tooltipText, 512)
        fontFamily: root.fontFamily
      }
    }
  }

  component SubscriptionGroupRow: CursorSurface {
    id: groupRow
    property var subscription: null
    property var profiles: []
    property int rowIndex: 0
    readonly property bool expanded: subscription
      && (root.profileFilter.trim() !== ""
        || root.expandedSubscriptions[subscription.uuid] === true)
    readonly property var activeProfile: subscription
      ? root.activeSubscriptionProfile(subscription.uuid) : null
    readonly property var probeSummary: subscription
      ? root.subscriptionProbeSummary(subscription.uuid)
      : ({ tested: 0, unavailable: 0, unresolved: 0 })

    hasCursor: root.cursorActive && root.focusSection === "configs"
      && root.configIndex === rowIndex
    current: activeProfile !== null
    bordered: true
    foreground: root.foreground
    implicitHeight: groupContent.implicitHeight + Style.spacing.rowPaddingX

    MouseArea {
      id: groupMouse
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onEntered: root.setConfigCursor(groupRow.rowIndex)
      onClicked: if (groupRow.subscription)
        root.toggleSubscriptionGroup(groupRow.subscription.uuid)
    }

    RowLayout {
      id: groupContent
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      anchors.leftMargin: Style.space(10)
      anchors.rightMargin: Style.space(10)
      spacing: Style.space(8)

      PlainText {
        text: groupRow.expanded ? "󰅀" : "󰅂"
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.icon
        Layout.alignment: Qt.AlignVCenter
      }

      PlainText {
        text: groupRow.current ? "󰄬" : "󰌷"
        color: groupRow.current ? root.foreground : root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.icon
        Layout.alignment: Qt.AlignVCenter
      }

      ColumnLayout {
        Layout.fillWidth: true
        spacing: Style.space(1)
        PlainText {
          id: groupTitle
          Layout.fillWidth: true
          text: groupRow.subscription ? groupRow.subscription.name : ""
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
          elide: Text.ElideRight
        }
        PlainText {
          id: groupSummary
          Layout.fillWidth: true
          text: {
            if (!groupRow.subscription) return ""
            var line = root.plural(groupRow.profiles.length, "server", "servers")
            var pinned = root.favoriteCount(groupRow.profiles)
            if (pinned > 0) line += " · " + root.plural(pinned, "pinned", "pinned")
            if (groupRow.activeProfile) line += " · connected: " + groupRow.activeProfile.name
            if (groupRow.probeSummary.tested > 0) {
              var age = root.probeAge(groupRow.subscription.uuid)
              line += age === "" ? " · tested" : " · " + age
              var available = groupRow.probeSummary.tested
                - groupRow.probeSummary.unavailable - groupRow.probeSummary.unresolved
              line += " · " + available + " available"
              if (groupRow.probeSummary.unavailable > 0)
                line += " · " + groupRow.probeSummary.unavailable + " unavailable"
              if (groupRow.probeSummary.unresolved > 0)
                line += " · " + groupRow.probeSummary.unresolved + " DNS failed"
            }
            return line
          }
          // A few unreachable provider nodes are routine availability data,
          // not a broken plugin. Fatal probe/config errors still use the
          // dedicated urgent message and bar state.
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideRight
        }
      }

      Button {
        readonly property bool testing: vless.probingSubscriptionUuid
          === (groupRow.subscription ? groupRow.subscription.uuid : "")
        text: testing ? "Cancel" : "Test"
        iconText: testing ? "󰑓" : ""
        iconSpinning: testing
        tooltipText: testing
          ? "Cancel this server test"
          : "Run an end-to-end proxy check through every server (p)"
        bordered: true
        foreground: root.foreground
        fontFamily: root.fontFamily
        enabled: groupRow.subscription && groupRow.profiles.length > 0 && !vless.busy
          && (!vless.probingProfiles || testing)
        onClicked: root.toggleSubscriptionProbe(groupRow.subscription)
      }
    }

    PanelToolTip {
      visible: groupMouse.containsMouse && (groupTitle.truncated || groupSummary.truncated)
      text: root.safeTooltip(groupTitle.text + "\n" + groupSummary.text, 320)
      fontFamily: root.fontFamily
    }
  }

  component SubscriptionServerRow: CursorSurface {
    id: serverRow
    property var profile: null
    readonly property var probe: vless.probeResult(profile ? profile.uuid : "")
    hasCursor: profile !== null && root.hoveredSubscriptionServerUuid === profile.uuid
    current: profile !== null && profile.active === true
    bordered: true
    foreground: root.foreground
    implicitHeight: serverContent.implicitHeight + Style.space(10)

    MouseArea {
      id: serverMouse
      anchors.fill: parent
      enabled: !vless.busy && serverRow.profile !== null
      hoverEnabled: enabled
      cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
      onEntered: {
        root.cursorActive = false
        root.hoveredSubscriptionServerUuid = serverRow.profile ? serverRow.profile.uuid : ""
      }
      onExited: if (root.hoveredSubscriptionServerUuid
          === (serverRow.profile ? serverRow.profile.uuid : ""))
        root.hoveredSubscriptionServerUuid = ""
      onClicked: root.activateConfig(serverRow.profile)
    }

    RowLayout {
      id: serverContent
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      anchors.leftMargin: Style.space(30)
      anchors.rightMargin: Style.space(10)
      spacing: Style.space(8)

      PlainText {
        text: serverRow.profile && serverRow.profile.active ? "󰄬" : "󰌘"
        color: serverRow.profile && serverRow.profile.active ? root.foreground : root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.icon
      }
      ColumnLayout {
        Layout.fillWidth: true
        spacing: Style.space(1)
        PlainText {
          Layout.fillWidth: true
          text: serverRow.profile ? serverRow.profile.name : ""
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          elide: Text.ElideRight
        }
        PlainText {
          Layout.fillWidth: true
          text: serverRow.profile && serverRow.profile.active
            ? "Connected — click to disconnect" : "Click to connect"
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideRight
        }
      }
      PlainText {
        visible: serverRow.probe !== null
        text: serverRow.probe === null ? "" : (serverRow.probe.reachable
          ? serverRow.probe.latencyMs + " ms"
          : (serverRow.probe.resolved ? "Unavailable" : "DNS failed"))
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
      PanelActionButton {
        iconText: serverRow.profile && serverRow.profile.favorite ? "󰓎" : "󰓒"
        tooltipText: root.safeTooltip(
          (serverRow.profile && serverRow.profile.favorite ? "Unpin " : "Pin ")
            + (serverRow.profile ? serverRow.profile.name : "profile"), 160)
        foreground: serverRow.profile && serverRow.profile.favorite ? Color.accent : root.dim
        hoverColor: Color.accent
        fontFamily: root.fontFamily
        enabled: !vless.busy && serverRow.profile !== null
        visible: serverRow.hasCursor || (serverRow.profile && serverRow.profile.favorite)
        onClicked: vless.toggleFavorite(serverRow.profile)
      }
    }
  }

  component SubscriptionSortRow: CursorSurface {
    id: sortRow
    property var subscription: null
    property int rowIndex: -1
    readonly property string subscriptionUuid: subscription ? subscription.uuid : ""
    readonly property string sortMode: root.subscriptionSortMode(subscriptionUuid)
    readonly property int testedCount: root.subscriptionProbeSummary(subscriptionUuid).tested

    hasCursor: root.cursorActive && root.focusSection === "configs"
      && root.configIndex === rowIndex
    current: sortMode !== "default"
    foreground: root.foreground
    // RowLayout's implicit height is not propagated while it is fully
    // anchored inside a Column child on every Qt 6 build Omarchy ships.
    // Give the compact toolbar a token-sized row so it cannot collapse to 0.
    implicitHeight: Style.space(38)

    RowLayout {
      id: sortContent
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      anchors.leftMargin: Style.space(30)
      anchors.rightMargin: Style.space(10)
      spacing: Style.space(8)

      PlainText {
        Layout.fillWidth: true
        text: sortRow.testedCount > 0
          ? "Failed checks stay last"
          : "Run Test to sort by latency"
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        elide: Text.ElideRight
      }

      Button {
        text: sortRow.sortMode === "pingAsc" ? "Ping ↑"
          : (sortRow.sortMode === "pingDesc" ? "Ping ↓" : "Sort by ping")
        bordered: true
        tooltipText: sortRow.sortMode === "default"
          ? "Sort reachable servers from fastest to slowest"
          : "Reverse the latency order; failed checks stay last"
        foreground: root.foreground
        fontFamily: root.fontFamily
        enabled: sortRow.subscription !== null && sortRow.testedCount > 0
          && !vless.probingProfiles
        onClicked: root.cycleSubscriptionPingSort(sortRow.subscriptionUuid)
      }
    }
  }

  component SubscriptionRow: CursorSurface {
    id: subscriptionRow
    property var subscription: null
    property int rowIndex: 0
    readonly property bool expanded: subscription
      && root.expandedSubscriptions[subscription.uuid] === true
    readonly property var profiles: subscription
      ? root.subscriptionProfiles(subscription.uuid) : []
    readonly property var probeSummary: subscription
      ? root.subscriptionProbeSummary(subscription.uuid)
      : ({ tested: 0, unavailable: 0, unresolved: 0 })

    implicitHeight: subscriptionColumn.implicitHeight + Style.space(12)
    hasCursor: root.cursorActive && root.page === "subscriptions"
      && root.subscriptionIndex === rowIndex
    current: root.activeSubscriptionProfile(subscription ? subscription.uuid : "") !== null
    bordered: true
    foreground: root.foreground

    Column {
      id: subscriptionColumn
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.space(4)

      Item {
        width: parent.width
        implicitHeight: subscriptionContent.implicitHeight

        MouseArea {
          id: subscriptionMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onEntered: root.setSubscriptionCursor(subscriptionRow.rowIndex)
          onClicked: if (subscriptionRow.subscription)
            root.toggleSubscriptionGroup(subscriptionRow.subscription.uuid)
        }

        RowLayout {
          id: subscriptionContent
          anchors.left: parent.left
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          anchors.leftMargin: Style.space(10)
          anchors.rightMargin: Style.space(8)
          spacing: Style.space(8)

          PlainText {
            text: subscriptionRow.expanded ? "󰅀" : "󰅂"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.icon
          }
          PlainText {
            text: subscriptionRow.subscription && subscriptionRow.subscription.staleCount > 0 ? "󰀦" : "󰌷"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.icon
          }
          ColumnLayout {
            Layout.fillWidth: true
            spacing: Style.space(1)
            PlainText {
              id: subscriptionTitle
              Layout.fillWidth: true
              text: subscriptionRow.subscription ? subscriptionRow.subscription.name : ""
              color: root.foreground
              font.family: root.fontFamily
              font.pixelSize: Style.font.body
              elide: Text.ElideRight
            }
            PlainText {
              id: subscriptionSummary
              Layout.fillWidth: true
              text: {
                if (!subscriptionRow.subscription) return ""
                var line = root.plural(subscriptionRow.profiles.length, "server", "servers") + " · "
                  + root.subscriptionAge(subscriptionRow.subscription.updatedAt)
                if (subscriptionRow.probeSummary.tested > 0) {
                  var age = root.probeAge(subscriptionRow.subscription.uuid)
                  line += age === "" ? " · tested" : " · " + age
                  var available = subscriptionRow.probeSummary.tested
                    - subscriptionRow.probeSummary.unavailable
                    - subscriptionRow.probeSummary.unresolved
                  line += " · " + available + " available"
                  if (subscriptionRow.probeSummary.unavailable > 0)
                    line += " · " + subscriptionRow.probeSummary.unavailable + " unavailable"
                  if (subscriptionRow.probeSummary.unresolved > 0)
                    line += " · " + subscriptionRow.probeSummary.unresolved + " DNS failed"
                }
                if (subscriptionRow.subscription.staleCount > 0)
                  line += " · " + root.plural(subscriptionRow.subscription.staleCount,
                    "stale profile", "stale profiles")
                return line
              }
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              elide: Text.ElideRight
            }
          }

          Button {
            readonly property bool testing: vless.probingSubscriptionUuid
              === (subscriptionRow.subscription ? subscriptionRow.subscription.uuid : "")
            text: testing ? "Cancel" : "Test"
            iconText: testing ? "󰑓" : ""
            iconSpinning: testing
            tooltipText: testing
              ? "Cancel this server test"
              : "Run an end-to-end proxy check through every server (p)"
            bordered: true
            foreground: root.foreground
            fontFamily: root.fontFamily
            enabled: subscriptionRow.subscription && subscriptionRow.profiles.length > 0
              && !vless.busy && (!vless.probingProfiles || testing)
            onClicked: root.toggleSubscriptionProbe(subscriptionRow.subscription)
          }
          PanelActionButton {
            iconText: "󰏫"
            tooltipText: "Edit subscription…"
            foreground: root.dim
            hoverColor: root.foreground
            fontFamily: root.fontFamily
            enabled: !vless.busy && !vless.probingProfiles && !vless.subscriptionEditorLoading
            onClicked: root.editSubscription(subscriptionRow.subscription)
          }
          PanelActionButton {
            iconText: "󰑓"
            tooltipText: "Update subscription"
            foreground: root.dim
            hoverColor: root.foreground
            fontFamily: root.fontFamily
            enabled: !vless.busy && !vless.probingProfiles
            onClicked: vless.refreshSubscription(subscriptionRow.subscription)
          }
          PanelActionButton {
            iconText: "󰆴"
            tooltipText: "Remove subscription and managed profiles"
            foreground: root.dim
            hoverColor: root.urgent
            fontFamily: root.fontFamily
            enabled: !vless.busy && !vless.probingProfiles
            onClicked: root.pendingSubscriptionDelete = subscriptionRow.subscription
          }
        }

        PanelToolTip {
          visible: subscriptionMouse.containsMouse
            && (subscriptionTitle.truncated || subscriptionSummary.truncated)
          text: root.safeTooltip(
            subscriptionTitle.text + "\n" + subscriptionSummary.text, 320)
          fontFamily: root.fontFamily
        }
      }

      Column {
        visible: subscriptionRow.expanded
        width: parent.width
        spacing: Style.space(2)
        SubscriptionSortRow {
          width: parent.width
          subscription: subscriptionRow.subscription
        }
        Repeater {
          model: subscriptionRow.profiles
          SubscriptionServerRow {
            required property var modelData
            width: subscriptionColumn.width
            profile: modelData
          }
        }
      }
    }
  }

  component ConfigRow: CursorSurface {
    id: configRow
    // {uuid, name, active} — the row keeps the whole profile so its actions
    // hit exactly this profile even when names collide.
    property var profile: null
    property int rowIndex: 0
    property bool nested: false
    readonly property bool connected: profile ? profile.active === true : false

    hasCursor: root.cursorActive && root.focusSection === "configs" && root.configIndex === rowIndex
    current: connected
    bordered: true
    foreground: root.foreground

    implicitHeight: rowContent.implicitHeight + Style.spacing.rowPaddingX

    MouseArea {
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: vless.busy ? Qt.ArrowCursor : Qt.PointingHandCursor
      onEntered: root.setConfigCursor(configRow.rowIndex)
      onClicked: root.activateConfig(configRow.profile)
    }

    RowLayout {
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      anchors.leftMargin: configRow.nested ? Style.space(32) : Style.space(10)
      anchors.rightMargin: Style.space(10)
      spacing: Style.space(8)

      PlainText {
        text: configRow.connected ? "󰄬" : "󰌘"
        color: configRow.connected ? root.foreground : root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.icon
        Layout.alignment: Qt.AlignVCenter
      }

      ColumnLayout {
        id: rowContent
        Layout.fillWidth: true
        spacing: Style.space(1)

        PlainText {
          Layout.fillWidth: true
          text: configRow.profile ? configRow.profile.name : ""
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
          elide: Text.ElideRight
        }

        PlainText {
          Layout.fillWidth: true
          // The grid above carries the primary tunnel's numbers in full, so
          // this row states what it is instead of repeating them. Only a
          // second active tunnel — which the grid does not describe, and
          // which nothing in this widget can bring up — keeps a traffic
          // line of its own.
          text: {
            if (configRow.profile && configRow.profile.missing)
              return "Removed from " + configRow.profile.sourceName + " · disconnect to clean up"
            var source = configRow.profile && configRow.profile.managed && !configRow.nested
              ? configRow.profile.sourceName + " · " : ""
            if (!configRow.connected) return source + "Click to connect"
            if (configRow.profile && configRow.profile.uuid === vless.primaryUuid)
              return source + "Connected — click to disconnect"
            var line = vless.trafficLine(configRow.profile ? configRow.profile.ifname : "")
            return source + (line !== "" ? line : "Connected — click to disconnect")
          }
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideRight
        }
      }

      PlainText {
        visible: configRow.nested && root.probeLabel(configRow.profile ? configRow.profile.uuid : "") !== ""
        text: root.probeLabel(configRow.profile ? configRow.profile.uuid : "")
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        Layout.alignment: Qt.AlignVCenter
      }

      PanelActionButton {
        iconText: configRow.profile && configRow.profile.favorite ? "󰓎" : "󰓒"
        tooltipText: root.safeTooltip(
          (configRow.profile && configRow.profile.favorite ? "Unpin " : "Pin ")
            + (configRow.profile ? configRow.profile.name : "profile"), 160)
        foreground: configRow.profile && configRow.profile.favorite ? Color.accent : root.dim
        hoverColor: Color.accent
        fontFamily: root.fontFamily
        enabled: !vless.busy && !vless.editing && !vless.importSourceBusy
        visible: configRow.hasCursor || (configRow.profile && configRow.profile.favorite)
        Layout.alignment: Qt.AlignVCenter
        onClicked: vless.toggleFavorite(configRow.profile)
      }

      PanelActionButton {
        iconText: "󰏫"
        tooltipText: root.safeTooltip("Edit config or name for "
          + (configRow.profile ? configRow.profile.name : "profile") + " (e / n)", 180)
        foreground: root.dim
        hoverColor: root.foreground
        fontFamily: root.fontFamily
        enabled: !vless.busy && !vless.editing && !vless.importSourceBusy
        visible: configRow.hasCursor && configRow.profile && !configRow.profile.managed
        Layout.alignment: Qt.AlignVCenter
        onClicked: root.requestEdit(configRow.profile)
      }

      PanelActionButton {
        iconText: "󰐲"
        tooltipText: root.safeTooltip("Show "
          + (configRow.profile ? configRow.profile.name : "profile")
          + " as a QR code (q)", 180)
        foreground: root.dim
        hoverColor: root.foreground
        fontFamily: root.fontFamily
        enabled: !vless.busy && !vless.editing && !vless.importSourceBusy
        visible: configRow.hasCursor
        Layout.alignment: Qt.AlignVCenter
        onClicked: vless.showQr(configRow.profile)
      }

      PanelActionButton {
        iconText: "󰆴"
        tooltipText: root.safeTooltip("Delete "
          + (configRow.profile ? configRow.profile.name : "profile") + " (x)", 160)
        foreground: root.dim
        hoverColor: root.urgent
        fontFamily: root.fontFamily
        enabled: !vless.busy && !vless.editing && !vless.importSourceBusy
        visible: configRow.hasCursor && configRow.profile && !configRow.profile.managed
        Layout.alignment: Qt.AlignVCenter
        onClicked: root.requestDelete(configRow.profile)
      }
    }
  }
}
