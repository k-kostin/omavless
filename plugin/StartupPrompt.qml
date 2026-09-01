// SPDX-License-Identifier: MIT
// Copyright (c) 2026 OmaVLESS contributors

import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui
import "I18n.js" as I18n

// Login behavior is edited in one focused sheet so the general Settings page
// can describe the result in a single row.
Item {
  id: prompt

  property var profiles: []
  property var startup: ({ enabled: false, target: "last", profileId: "", mode: "rule" })
  property bool routingAvailable: false
  property bool coreReady: false
  property bool busy: false
  property color foreground: Color.foreground
  property color dim: Qt.darker(foreground, 1.55)
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family
  property string locale: "en"

  function textFor(key, values) {
    return I18n.translate(key, locale, values || {})
  }

  // Button and tooltip labels come from shared Omarchy controls whose text
  // format may be AutoText. Keep imported profile metadata inert at the sink.
  function safeTooltip(value) {
    return String(value === undefined || value === null ? "" : value)
      .replace(/[\u0000-\u001f\u007f]/g, " ").replace(/\s+/g, " ").trim()
      .substring(0, 80)
      .replace(/&/g, "＆").replace(/</g, "‹").replace(/>/g, "›")
  }

  property bool enabledChoice: false
  property string targetChoice: "last"
  property string profileChoice: ""
  property string modeChoice: "rule"
  readonly property bool valid: !enabledChoice || (coreReady && profiles.length > 0
    && (targetChoice === "last" || profileChoice !== "")
    && (modeChoice !== "rule" || routingAvailable))

  signal confirmed(bool enabled, string target, string profileId, string mode)
  signal canceled()
  signal setupRequested()

  visible: false
  focus: visible

  function openWith(value) {
    var source = value || ({})
    enabledChoice = source.enabled === true
    targetChoice = source.target === "profile" ? "profile" : "last"
    profileChoice = String(source.profileId || "")
    modeChoice = source.mode === "global" ? "global" : "rule"
    visible = true
    Qt.callLater(function() { prompt.forceActiveFocus() })
  }

  function dismiss() { visible = false }

  Keys.onEscapePressed: canceled()

  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Color.background, 0.80)
    MouseArea { anchors.fill: parent; onClicked: prompt.canceled() }

    BorderSurface {
      id: card
      width: Math.min(parent.width - Style.space(24), Style.space(430))
      height: Math.min(parent.height - Style.space(24), body.implicitHeight
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
        contentHeight: body.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        Column {
          id: body
          width: parent.width
          spacing: Style.space(11)

          PlainText {
            width: parent.width
            text: prompt.textFor("startup_prompt.title")
            color: prompt.foreground
            font.family: prompt.fontFamily
            font.pixelSize: Style.font.title
          }

          PlainText {
            width: parent.width
            text: prompt.textFor("startup_prompt.help")
            color: prompt.dim
            font.family: prompt.fontFamily
            font.pixelSize: Style.font.caption
            wrapMode: Text.WordWrap
          }

          ChoiceRow {
            label: prompt.textFor("startup_prompt.autoconnect")
            leftText: prompt.textFor("common.off")
            rightText: prompt.textFor("common.on")
            rightSelected: prompt.enabledChoice
            onLeftChosen: prompt.enabledChoice = false
            onRightChosen: prompt.enabledChoice = true
          }

          Column {
            width: parent.width
            spacing: Style.space(9)
            visible: prompt.enabledChoice

            ChoiceRow {
              label: prompt.textFor("startup_prompt.server")
              leftText: prompt.textFor("startup.last_used")
              rightText: prompt.textFor("common.choose")
              rightSelected: prompt.targetChoice === "profile"
              onLeftChosen: prompt.targetChoice = "last"
              onRightChosen: prompt.targetChoice = "profile"
            }

            BorderSurface {
              visible: prompt.targetChoice === "profile"
              width: parent.width
              height: Math.min(Style.space(190), Math.max(Style.space(52), profileList.contentHeight))
              color: "transparent"
              borderSpec: Border.flat(Util.alpha(prompt.foreground, 0.28), Style.normalBorderWidth)
              radius: Style.cornerRadius

              Flickable {
                id: profileList
                anchors.fill: parent
                anchors.margins: Style.space(5)
                contentWidth: width
                contentHeight: profileColumn.implicitHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                interactive: contentHeight > height
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

                Column {
                  id: profileColumn
                  width: parent.width
                  spacing: Style.space(3)
                  Repeater {
                    model: prompt.profiles
                    Button {
                      required property var modelData
                      width: profileColumn.width
                      text: prompt.safeTooltip(modelData.name)
                      tooltipText: prompt.safeTooltip(modelData.name)
                      bordered: prompt.profileChoice === modelData.uuid
                      foreground: prompt.profileChoice === modelData.uuid ? Color.accent : prompt.foreground
                      fontFamily: prompt.fontFamily
                      onClicked: prompt.profileChoice = modelData.uuid
                    }
                  }
                }
              }
            }

            PlainText {
              visible: prompt.profiles.length === 0
              width: parent.width
              text: prompt.textFor("startup_prompt.profile_required")
              color: prompt.urgent
              font.family: prompt.fontFamily
              font.pixelSize: Style.font.caption
              wrapMode: Text.WordWrap
            }

            ChoiceRow {
              label: prompt.textFor("startup_prompt.mode")
              leftText: prompt.textFor("mode.full_vpn")
              rightText: prompt.textFor("mode.routing")
              rightSelected: prompt.modeChoice === "rule"
              onLeftChosen: prompt.modeChoice = "global"
              onRightChosen: prompt.modeChoice = "rule"
            }

            PlainText {
              visible: prompt.modeChoice === "rule" && !prompt.routingAvailable
              width: parent.width
              text: prompt.textFor("startup_prompt.routing_required")
              color: prompt.urgent
              font.family: prompt.fontFamily
              font.pixelSize: Style.font.caption
              wrapMode: Text.WordWrap
            }

            Button {
              visible: !prompt.coreReady
              text: prompt.textFor("startup_prompt.open_mihomo_setup")
              bordered: true
              foreground: prompt.foreground
              fontFamily: prompt.fontFamily
              onClicked: prompt.setupRequested()
            }
          }

          Item {
            width: parent.width
            implicitHeight: actions.implicitHeight
            Row {
              id: actions
              anchors.right: parent.right
              spacing: Style.space(8)
              Button {
                text: prompt.textFor("common.cancel")
                bordered: true
                foreground: prompt.foreground
                fontFamily: prompt.fontFamily
                onClicked: prompt.canceled()
              }
              Button {
                text: prompt.textFor("common.save")
                bordered: true
                enabled: prompt.valid && !prompt.busy
                foreground: enabled ? prompt.foreground : prompt.dim
                fontFamily: prompt.fontFamily
                onClicked: prompt.confirmed(
                  prompt.enabledChoice, prompt.targetChoice,
                  prompt.profileChoice, prompt.modeChoice
                )
              }
            }
          }
        }
      }
    }
  }

  component ChoiceRow: Item {
    id: choiceRow
    property string label: ""
    property string leftText: ""
    property string rightText: ""
    property bool rightSelected: false
    signal leftChosen()
    signal rightChosen()
    width: body.width
    implicitHeight: Math.max(choiceLabel.implicitHeight, choices.implicitHeight)

    PlainText {
      id: choiceLabel
      anchors.left: parent.left
      anchors.verticalCenter: parent.verticalCenter
      text: choiceRow.label
      color: prompt.foreground
      font.family: prompt.fontFamily
      font.pixelSize: Style.font.bodySmall
    }
    Row {
      id: choices
      anchors.right: parent.right
      spacing: Style.space(5)
      Button {
        text: choiceRow.leftText
        bordered: !choiceRow.rightSelected
        foreground: !choiceRow.rightSelected ? Color.accent : prompt.dim
        fontFamily: prompt.fontFamily
        onClicked: choiceRow.leftChosen()
      }
      Button {
        text: choiceRow.rightText
        bordered: choiceRow.rightSelected
        foreground: choiceRow.rightSelected ? Color.accent : prompt.dim
        fontFamily: prompt.fontFamily
        onClicked: choiceRow.rightChosen()
      }
    }
  }
}
