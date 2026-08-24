// SPDX-License-Identifier: MIT
// Copyright (c) 2026 OmaVLESS contributors

import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui

// Advanced routing stays behind Settings. The default panel remains a
// three-button connection surface; this sheet exposes only user-level ideas:
// what to match, where it goes, and which rule decided a test destination.
Item {
  id: prompt

  property var rules: []
  property var result: null
  property bool loading: false
  property bool busy: false
  property bool refreshAvailable: false
  property string rulesUpdatedLabel: "Never checked manually"
  property string statusText: ""
  property string errorText: ""
  property color foreground: Color.foreground
  property color dim: Qt.darker(foreground, 1.55)
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family
  property string matchChoice: "suffix"
  property string actionChoice: "proxy"

  signal addRule(string kind, string action, string value)
  signal deleteRule(var rule)
  signal checkRoute(string value)
  signal refreshRules()
  signal canceled()

  visible: false
  focus: visible

  function openTools() {
    visible = true
    Qt.callLater(function() { checkField.forceActiveFocus() })
  }

  function dismiss() {
    visible = false
    checkField.text = ""
    ruleField.text = ""
  }

  function outcomeTitle(value) {
    if (!value) return ""
    if (value.outcome === "vpn") return "VPN"
    if (value.outcome === "direct") return "Direct"
    if (value.outcome === "block") return "Blocked"
    return "Needs an active Routing connection"
  }

  function resultExplanation(value) {
    if (!value) return ""
    if (value.source === "disconnected")
      return "Connect in Routing mode to test the selected preset's remote rule sets."
    var rule = value.ruleType
    if (value.rulePayload !== "") rule += " · " + value.rulePayload
    if (value.target !== "") rule += " · route " + value.target
    return rule
  }

  function kindLabel(kind) {
    if (kind === "domain") return "Exact domain"
    if (kind === "suffix") return "Domain + subdomains"
    return "IP range"
  }

  function actionLabel(action) {
    if (action === "proxy") return "VPN"
    if (action === "direct") return "Direct"
    return "Block"
  }

  Keys.onEscapePressed: canceled()

  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Color.background, 0.82)
    MouseArea { anchors.fill: parent; onClicked: prompt.canceled() }

    BorderSurface {
      id: card
      width: Math.min(parent.width - Style.space(24), Style.space(470))
      height: Math.min(parent.height - Style.space(24), content.implicitHeight
        + card.contentTopInset + card.contentBottomInset)
      anchors.centerIn: parent
      color: Color.background
      borderSpec: Border.flat(Color.accent, Style.normalBorderWidth)
      padding: Style.space(18)
      radius: Style.cornerRadius

      MouseArea { anchors.fill: parent; onClicked: {} }

      Flickable {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.topMargin: card.contentTopInset
        anchors.bottomMargin: card.contentBottomInset
        anchors.leftMargin: card.contentLeftInset
        anchors.rightMargin: card.contentRightInset
        contentWidth: width
        contentHeight: content.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        Column {
          id: content
          width: parent.width
          spacing: Style.space(11)

          PlainText {
            width: parent.width
            text: "ROUTING TOOLS"
            color: prompt.foreground
            font.family: prompt.fontFamily
            font.pixelSize: Style.font.title
          }

          PlainText {
            width: parent.width
            text: "Where will this destination go?"
            color: Color.accent
            font.family: prompt.fontFamily
            font.pixelSize: Style.font.subtitle
            font.bold: true
          }

          Row {
            width: parent.width
            spacing: Style.space(7)
            TextField {
              id: checkField
              width: parent.width - checkButton.width - parent.spacing
              placeholderText: "example.com or 203.0.113.10"
              foreground: prompt.foreground
              font.family: prompt.fontFamily
              enabled: !prompt.busy
              onAccepted: if (text.trim() !== "") prompt.checkRoute(text)
              Keys.onEscapePressed: prompt.canceled()
            }
            Button {
              id: checkButton
              text: prompt.loading ? "Checking…" : "Check"
              bordered: true
              enabled: checkField.text.trim() !== "" && !prompt.busy
              foreground: enabled ? prompt.foreground : prompt.dim
              fontFamily: prompt.fontFamily
              onClicked: prompt.checkRoute(checkField.text)
            }
          }

          PlainText {
            width: parent.width
            text: "A custom rule is checked locally. Otherwise, an active Routing check opens one TCP connection to destination:443 through Mihomo; no page is loaded."
            color: prompt.dim
            font.family: prompt.fontFamily
            font.pixelSize: Style.font.caption
            wrapMode: Text.WordWrap
          }

          BorderSurface {
            visible: prompt.result !== null
            width: parent.width
            height: resultBody.implicitHeight + Style.space(16)
            color: "transparent"
            borderSpec: Border.flat(Util.alpha(prompt.foreground, 0.28), Style.normalBorderWidth)
            radius: Style.cornerRadius
            Column {
              id: resultBody
              anchors.left: parent.left
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              anchors.leftMargin: Style.space(10)
              anchors.rightMargin: Style.space(10)
              spacing: Style.space(2)
              PlainText {
                width: parent.width
                text: prompt.outcomeTitle(prompt.result)
                color: prompt.result && prompt.result.outcome === "block"
                  ? prompt.urgent : Color.accent
                font.family: prompt.fontFamily
                font.pixelSize: Style.font.body
                font.bold: true
              }
              PlainText {
                width: parent.width
                text: prompt.resultExplanation(prompt.result)
                color: prompt.dim
                font.family: prompt.fontFamily
                font.pixelSize: Style.font.caption
                wrapMode: Text.WrapAnywhere
              }
            }
          }

          PanelSeparator { foreground: prompt.foreground }

          PlainText {
            width: parent.width
            text: "Custom rules"
            color: Color.accent
            font.family: prompt.fontFamily
            font.pixelSize: Style.font.subtitle
            font.bold: true
          }

          PlainText {
            width: parent.width
            text: "Custom rules run before the selected country preset. Earlier rules win."
            color: prompt.dim
            font.family: prompt.fontFamily
            font.pixelSize: Style.font.caption
            wrapMode: Text.WordWrap
          }

          Row {
            spacing: Style.space(5)
            Repeater {
              model: [
                { label: "Exact", value: "domain" },
                { label: "Domain + subdomains", value: "suffix" },
                { label: "IP range", value: "ipcidr" }
              ]
              Button {
                required property var modelData
                text: modelData.label
                bordered: prompt.matchChoice === modelData.value
                foreground: prompt.matchChoice === modelData.value ? Color.accent : prompt.dim
                fontFamily: prompt.fontFamily
                onClicked: prompt.matchChoice = modelData.value
              }
            }
          }

          TextField {
            id: ruleField
            width: parent.width
            placeholderText: prompt.matchChoice === "ipcidr"
              ? "203.0.113.0/24" : "example.com"
            foreground: prompt.foreground
            font.family: prompt.fontFamily
            enabled: !prompt.busy
            Keys.onEscapePressed: prompt.canceled()
          }

          Row {
            width: parent.width
            spacing: Style.space(7)
            Row {
              id: actionChoices
              spacing: Style.space(5)
              Repeater {
                model: [
                  { label: "VPN", value: "proxy" },
                  { label: "Direct", value: "direct" },
                  { label: "Block", value: "reject" }
                ]
                Button {
                  required property var modelData
                  text: modelData.label
                  bordered: prompt.actionChoice === modelData.value
                  foreground: prompt.actionChoice === modelData.value ? Color.accent : prompt.dim
                  fontFamily: prompt.fontFamily
                  onClicked: prompt.actionChoice = modelData.value
                }
              }
            }
            Item {
              width: Math.max(0, parent.width - actionChoices.width - addButton.width
                - parent.spacing * 2)
              height: 1
            }
            Button {
              id: addButton
              text: "Add rule"
              bordered: true
              enabled: ruleField.text.trim() !== "" && !prompt.busy
              foreground: enabled ? prompt.foreground : prompt.dim
              fontFamily: prompt.fontFamily
              onClicked: {
                prompt.addRule(prompt.matchChoice, prompt.actionChoice, ruleField.text)
                ruleField.text = ""
              }
            }
          }

          PlainText {
            visible: !prompt.loading && prompt.rules.length === 0
            width: parent.width
            text: "No custom rules"
            color: prompt.dim
            font.family: prompt.fontFamily
            font.pixelSize: Style.font.caption
          }

          Repeater {
            model: prompt.rules
            BorderSurface {
              id: ruleRow
              required property var modelData
              width: content.width
              height: ruleBody.implicitHeight + Style.space(14)
              color: "transparent"
              borderSpec: Border.flat(Util.alpha(prompt.foreground, 0.24), Style.normalBorderWidth)
              radius: Style.cornerRadius
              Row {
                id: ruleBody
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.leftMargin: Style.space(9)
                anchors.rightMargin: Style.space(9)
                spacing: Style.space(8)
                Column {
                  width: parent.width - removeButton.width - parent.spacing
                  PlainText {
                    width: parent.width
                    text: ruleRow.modelData.value
                    color: prompt.foreground
                    font.family: prompt.fontFamily
                    font.pixelSize: Style.font.bodySmall
                    elide: Text.ElideMiddle
                  }
                  PlainText {
                    width: parent.width
                    text: prompt.kindLabel(ruleRow.modelData.kind) + " · "
                      + prompt.actionLabel(ruleRow.modelData.action)
                    color: prompt.dim
                    font.family: prompt.fontFamily
                    font.pixelSize: Style.font.caption
                  }
                }
                Button {
                  id: removeButton
                  text: "Remove"
                  bordered: true
                  enabled: !prompt.busy
                  foreground: enabled ? prompt.urgent : prompt.dim
                  fontFamily: prompt.fontFamily
                  onClicked: prompt.deleteRule(ruleRow.modelData)
                }
              }
            }
          }

          PanelSeparator { foreground: prompt.foreground }

          Row {
            width: parent.width
            spacing: Style.space(8)
            Column {
              width: parent.width - refreshButton.width - parent.spacing
              PlainText {
                width: parent.width
                text: "Remote rule data"
                color: prompt.foreground
                font.family: prompt.fontFamily
                font.pixelSize: Style.font.bodySmall
              }
              PlainText {
                width: parent.width
                text: prompt.rulesUpdatedLabel
                color: prompt.dim
                font.family: prompt.fontFamily
                font.pixelSize: Style.font.caption
                wrapMode: Text.WordWrap
              }
            }
            Button {
              id: refreshButton
              text: "Refresh now"
              bordered: true
              enabled: prompt.refreshAvailable && !prompt.busy
              foreground: enabled ? prompt.foreground : prompt.dim
              fontFamily: prompt.fontFamily
              onClicked: prompt.refreshRules()
            }
          }

          PlainText {
            visible: !prompt.refreshAvailable
            width: parent.width
            text: "Start a Routing connection to refresh its remote rule sets now."
            color: prompt.dim
            font.family: prompt.fontFamily
            font.pixelSize: Style.font.caption
            wrapMode: Text.WordWrap
          }

          PlainText {
            visible: prompt.errorText !== "" || prompt.statusText !== ""
            width: parent.width
            text: prompt.errorText !== "" ? prompt.errorText : prompt.statusText
            color: prompt.errorText !== "" ? prompt.urgent : prompt.dim
            font.family: prompt.fontFamily
            font.pixelSize: Style.font.caption
            wrapMode: Text.WrapAnywhere
          }

          Item {
            width: parent.width
            implicitHeight: closeButton.implicitHeight
            Button {
              id: closeButton
              anchors.right: parent.right
              text: "Close"
              bordered: true
              foreground: prompt.foreground
              fontFamily: prompt.fontFamily
              onClicked: prompt.canceled()
            }
          }
        }
      }
    }
  }
}
