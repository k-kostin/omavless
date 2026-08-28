// SPDX-License-Identifier: MIT
// Copyright (c) 2026 OmaVLESS contributors

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qs.Commons
import qs.Ui

// A compact, read-only view of the state Mihomo actually loaded. The service
// has already reduced controller data to bounded plain fields and terminal
// VPN/DIRECT/REJECT outcomes; this component still uses PlainText at every
// controller-controlled sink.
Item {
  id: page

  property var service: null
  property color foreground: Color.foreground
  property color dim: Qt.darker(foreground, 1.55)
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family
  readonly property real controlHeight: Style.space(32)
  readonly property int visibleRuleLimit: 300
  readonly property bool searchActive: searchField.activeFocus
  readonly property bool keyboardControlActive: backButton.activeFocus
    || refreshButton.activeFocus || searchField.activeFocus
    || providersRefreshButton.activeFocus
  readonly property var filteredRules: filterRules()
  readonly property int filteredRuleTotal: countFilteredRules()

  signal backRequested()
  signal refreshRequested()
  signal refreshProvidersRequested()

  focus: visible
  implicitHeight: content.implicitHeight
  Keys.onEscapePressed: backRequested()

  function resetSearchFocus() {
    searchField.text = ""
    Qt.callLater(function() { searchField.forceActiveFocus() })
  }

  function normalizedQuery() {
    return String(searchField.text || "").trim().toLowerCase()
  }

  function ruleMatches(rule, query) {
    if (query === "") return true
    return String(rule.type || "").toLowerCase().indexOf(query) >= 0
      || String(rule.payload || "").toLowerCase().indexOf(query) >= 0
      || String(rule.target || "").toLowerCase().indexOf(query) >= 0
  }

  function filterRules() {
    var source = service ? service.loadedRules : []
    var query = normalizedQuery()
    var result = []
    for (var i = 0; i < source.length && result.length < visibleRuleLimit; i++) {
      if (ruleMatches(source[i], query)) result.push(source[i])
    }
    return result
  }

  function countFilteredRules() {
    var source = service ? service.loadedRules : []
    var query = normalizedQuery()
    var count = 0
    for (var i = 0; i < source.length; i++) if (ruleMatches(source[i], query)) count++
    return count
  }

  function providerSummary(provider) {
    var parts = []
    if (provider.behavior !== "") parts.push(provider.behavior)
    parts.push(provider.ruleCount >= 0
      ? provider.ruleCount + (provider.ruleCount === 1 ? " rule" : " rules")
      : "count unavailable")
    parts.push(provider.status)
    return parts.join(" · ")
  }

  function providerUpdated(provider) {
    return provider.updatedAt !== ""
      ? "Last successful update · " + provider.updatedAt
      : "Last successful update not reported"
  }

  Flickable {
    id: diagnosticsFlick
    anchors.fill: parent
    contentWidth: width
    contentHeight: content.implicitHeight
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    flickableDirection: Flickable.VerticalFlick
    interactive: contentHeight > height
    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

    Column {
      id: content
      width: Math.max(0, diagnosticsFlick.width - Style.space(16))
      spacing: Style.space(12)

      RowLayout {
        width: parent.width
        spacing: Style.space(8)

        Button {
          id: backButton
          text: "Back"
          bordered: true
          foreground: page.foreground
          fontFamily: page.fontFamily
          Layout.preferredHeight: page.controlHeight
          focusable: true
          KeyNavigation.tab: refreshButton
          KeyNavigation.backtab: providersRefreshButton
          Keys.onEscapePressed: page.backRequested()
          onClicked: page.backRequested()
        }

        ColumnLayout {
          Layout.fillWidth: true
          spacing: 0
          PlainText {
            Layout.fillWidth: true
            text: "LIVE MIHOMO DIAGNOSTICS"
            color: page.foreground
            font.family: page.fontFamily
            font.pixelSize: Style.font.title
            elide: Text.ElideRight
          }
          PlainText {
            Layout.fillWidth: true
            text: "Read-only state from OmaVLESS's private controller"
            color: page.dim
            font.family: page.fontFamily
            font.pixelSize: Style.font.caption
            elide: Text.ElideRight
          }
        }

        Button {
          id: refreshButton
          text: service && service.advancedDiagnosticsLoading ? "Loading…" : "Refresh"
          bordered: true
          enabled: service && !service.advancedDiagnosticsLoading
          foreground: enabled ? page.foreground : page.dim
          fontFamily: page.fontFamily
          Layout.preferredHeight: page.controlHeight
          focusable: true
          KeyNavigation.tab: searchField
          KeyNavigation.backtab: backButton
          Keys.onEscapePressed: page.backRequested()
          onClicked: page.refreshRequested()
        }
      }

      PlainText {
        visible: service && service.advancedDiagnosticsError !== ""
        width: parent.width
        text: service ? service.advancedDiagnosticsError : ""
        color: page.dim
        font.family: page.fontFamily
        font.pixelSize: Style.font.bodySmall
        wrapMode: Text.WordWrap
      }

      PlainText {
        visible: service && service.advancedDiagnosticsLoading
          && service.loadedRules.length === 0
        width: parent.width
        text: "Reading loaded rules and providers…"
        color: page.dim
        font.family: page.fontFamily
        font.pixelSize: Style.font.bodySmall
      }

      PanelSeparator { foreground: page.foreground }

      PlainText {
        width: parent.width
        text: "LOADED RULES"
        color: Color.accent
        font.family: page.fontFamily
        font.pixelSize: Style.font.subtitle
        font.bold: true
      }

      TextField {
        id: searchField
        width: parent.width
        height: page.controlHeight
        placeholderText: "Search payload, rule type or VPN / DIRECT / REJECT"
        foreground: page.foreground
        font.family: page.fontFamily
        KeyNavigation.tab: providersRefreshButton
        KeyNavigation.backtab: refreshButton
        Keys.onEscapePressed: page.backRequested()
      }

      PlainText {
        visible: service && service.loadedRules.length > 0
        width: parent.width
        text: {
          var line = filteredRuleTotal + " matching · "
            + service.loadedRuleTotal + " loaded"
          if (filteredRuleTotal > visibleRuleLimit)
            line += " · showing first " + visibleRuleLimit
          if (service.loadedRulesTruncated) line += " · controller list bounded"
          return line
        }
        color: page.dim
        font.family: page.fontFamily
        font.pixelSize: Style.font.caption
        wrapMode: Text.WordWrap
      }

      PlainText {
        visible: service && !service.advancedDiagnosticsLoading
          && service.advancedDiagnosticsError === "" && filteredRuleTotal === 0
        width: parent.width
        text: service && service.loadedRules.length === 0
          ? "Mihomo reported no loaded rules."
          : "No loaded rule matches this search."
        color: page.dim
        font.family: page.fontFamily
        font.pixelSize: Style.font.bodySmall
      }

      Repeater {
        model: page.filteredRules

        BorderSurface {
          required property var modelData
          width: content.width
          height: ruleRow.implicitHeight + Style.space(14)
          color: "transparent"
          borderSpec: Border.flat(Util.alpha(page.foreground, 0.24), Style.normalBorderWidth)
          radius: Style.cornerRadius

          RowLayout {
            id: ruleRow
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: Style.space(9)
            anchors.rightMargin: Style.space(9)
            spacing: Style.space(8)

            ColumnLayout {
              Layout.fillWidth: true
              spacing: Style.space(2)
              PlainText {
                Layout.fillWidth: true
                text: modelData.type !== "" ? modelData.type : "Rule"
                color: page.foreground
                font.family: page.fontFamily
                font.pixelSize: Style.font.bodySmall
                font.bold: true
                elide: Text.ElideRight
              }
              PlainText {
                Layout.fillWidth: true
                text: modelData.payload !== "" ? modelData.payload : "No payload"
                color: page.dim
                font.family: page.fontFamily
                font.pixelSize: Style.font.caption
                wrapMode: Text.WrapAnywhere
                maximumLineCount: 2
                elide: Text.ElideRight
              }
            }

            PlainText {
              text: modelData.target
              color: modelData.target === "REJECT" ? page.urgent : Color.accent
              font.family: page.fontFamily
              font.pixelSize: Style.font.bodySmall
              font.bold: true
              Layout.alignment: Qt.AlignVCenter
            }
          }
        }
      }

      PanelSeparator { foreground: page.foreground }

      RowLayout {
        width: parent.width
        spacing: Style.space(8)
        ColumnLayout {
          Layout.fillWidth: true
          spacing: 0
          PlainText {
            Layout.fillWidth: true
            text: "RULE PROVIDERS"
            color: Color.accent
            font.family: page.fontFamily
            font.pixelSize: Style.font.subtitle
            font.bold: true
          }
          PlainText {
            Layout.fillWidth: true
            text: service
              ? service.loadedRuleProviderTotal + " loaded by Mihomo"
              : ""
            color: page.dim
            font.family: page.fontFamily
            font.pixelSize: Style.font.caption
          }
        }
        Button {
          id: providersRefreshButton
          text: service && service.busy ? "Updating…" : "Update all"
          bordered: true
          enabled: service && service.loadedRefreshableProviderCount > 0 && !service.busy
          foreground: enabled ? page.foreground : page.dim
          fontFamily: page.fontFamily
          Layout.preferredHeight: page.controlHeight
          focusable: true
          KeyNavigation.tab: backButton
          KeyNavigation.backtab: searchField
          Keys.onEscapePressed: page.backRequested()
          onClicked: page.refreshProvidersRequested()
        }
      }

      PlainText {
        visible: service && (service.routingToolStatus !== ""
          || service.routingToolError !== "")
        width: parent.width
        text: service.routingToolError !== ""
          ? service.routingToolError : service.routingToolStatus
        color: service.routingToolError !== "" ? page.urgent : page.dim
        font.family: page.fontFamily
        font.pixelSize: Style.font.bodySmall
        wrapMode: Text.WordWrap
      }

      PlainText {
        visible: service && !service.advancedDiagnosticsLoading
          && service.advancedDiagnosticsError === ""
          && service.loadedRuleProviders.length === 0
        width: parent.width
        text: "Mihomo reported no loaded rule providers."
        color: page.dim
        font.family: page.fontFamily
        font.pixelSize: Style.font.bodySmall
      }

      Repeater {
        model: service ? service.loadedRuleProviders : []

        BorderSurface {
          required property var modelData
          width: content.width
          height: providerBody.implicitHeight + Style.space(14)
          color: "transparent"
          borderSpec: Border.flat(Util.alpha(page.foreground, 0.24), Style.normalBorderWidth)
          radius: Style.cornerRadius

          Column {
            id: providerBody
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: Style.space(9)
            anchors.rightMargin: Style.space(9)
            spacing: Style.space(2)
            PlainText {
              width: parent.width
              text: modelData.name
              color: page.foreground
              font.family: page.fontFamily
              font.pixelSize: Style.font.bodySmall
              font.bold: true
              elide: Text.ElideRight
            }
            PlainText {
              width: parent.width
              text: page.providerSummary(modelData)
              color: page.dim
              font.family: page.fontFamily
              font.pixelSize: Style.font.caption
              elide: Text.ElideRight
            }
            PlainText {
              width: parent.width
              text: page.providerUpdated(modelData)
              color: page.dim
              font.family: page.fontFamily
              font.pixelSize: Style.font.caption
              wrapMode: Text.WrapAnywhere
              maximumLineCount: 2
              elide: Text.ElideRight
            }
          }
        }
      }

      PlainText {
        visible: service && service.loadedRuleProvidersTruncated
        width: parent.width
        text: "Provider output was bounded for safe display."
        color: page.dim
        font.family: page.fontFamily
        font.pixelSize: Style.font.caption
      }

      Item { width: 1; height: Style.space(8) }
    }
  }
}
