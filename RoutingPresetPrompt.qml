// SPDX-License-Identifier: MIT
// Copyright (c) 2026 OmaVLESS contributors

import QtQuick
import qs.Commons
import qs.Ui

// First-use country chooser. The normal panel stays deliberately compact;
// source attribution and later profile changes live on the Settings page.
Item {
  id: prompt

  property var presets: []
  property string selectedPreset: "roscomvpn-default"
  property bool accepted: selectedPreset !== ""
  property color foreground: Color.foreground
  property color dim: Qt.darker(foreground, 1.55)
  property string fontFamily: Style.font.family

  signal confirmed(string preset)
  signal canceled()

  visible: false
  focus: visible

  function openWith(preset) {
    selectedPreset = String(preset || "roscomvpn-default")
    visible = true
    Qt.callLater(function() { prompt.forceActiveFocus() })
  }

  function dismiss() {
    visible = false
  }

  function moveSelection(delta) {
    if (!presets || presets.length === 0) return
    var index = 0
    for (var i = 0; i < presets.length; i++) {
      if (presets[i].id === selectedPreset) { index = i; break }
    }
    index = (index + delta + presets.length) % presets.length
    selectedPreset = presets[index].id
  }

  Keys.onEscapePressed: canceled()
  Keys.onUpPressed: moveSelection(-1)
  Keys.onLeftPressed: moveSelection(-1)
  Keys.onDownPressed: moveSelection(1)
  Keys.onRightPressed: moveSelection(1)
  Keys.onReturnPressed: if (accepted) confirmed(selectedPreset)
  Keys.onEnterPressed: if (accepted) confirmed(selectedPreset)
  Keys.onSpacePressed: if (accepted) confirmed(selectedPreset)

  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Color.background, 0.78)

    MouseArea { anchors.fill: parent; onClicked: prompt.canceled() }

    BorderSurface {
      id: card
      width: Math.min(parent.width - Style.space(24), Style.space(420))
      height: card.contentTopInset + card.contentBottomInset + content.implicitHeight
      anchors.centerIn: parent
      color: Color.background
      borderSpec: Border.flat(Color.accent, Style.normalBorderWidth)
      padding: Style.space(18)
      radius: Style.cornerRadius

      MouseArea { anchors.fill: parent; onClicked: {} }

      Column {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.topMargin: card.contentTopInset
        anchors.leftMargin: card.contentLeftInset
        anchors.rightMargin: card.contentRightInset
        spacing: Style.space(10)

        PlainText {
          width: parent.width
          text: "Choose Routing preset"
          color: prompt.foreground
          font.family: prompt.fontFamily
          font.pixelSize: Style.font.title
        }

        PlainText {
          width: parent.width
          text: "Choose where you use this connection. You can change it later in Settings."
          color: prompt.dim
          font.family: prompt.fontFamily
          font.pixelSize: Style.font.caption
          wrapMode: Text.WordWrap
        }

        Repeater {
          model: prompt.presets

          BorderSurface {
            id: presetCard
            required property var modelData
            readonly property bool selected: prompt.selectedPreset === modelData.id

            width: content.width
            height: presetContent.implicitHeight + Style.space(18)
            color: selected ? Util.alpha(Color.foreground, 0.08) : "transparent"
            borderSpec: Border.flat(selected ? Color.accent : Util.alpha(prompt.foreground, 0.32),
              Style.normalBorderWidth)
            radius: Style.cornerRadius

            Column {
              id: presetContent
              anchors.left: parent.left
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              anchors.leftMargin: Style.space(10)
              anchors.rightMargin: Style.space(10)
              spacing: Style.space(2)

              Row {
                width: parent.width
                spacing: Style.space(6)
                PlainText {
                  text: presetCard.modelData.country
                  color: presetCard.selected ? Color.accent : prompt.foreground
                  font.family: prompt.fontFamily
                  font.pixelSize: Style.font.body
                  font.bold: true
                }
                PlainText {
                  visible: presetCard.modelData.id === "roscomvpn-default"
                  text: "Recommended"
                  color: prompt.dim
                  font.family: prompt.fontFamily
                  font.pixelSize: Style.font.caption
                }
              }

              PlainText {
                width: parent.width
                text: presetCard.modelData.summary
                color: prompt.dim
                font.family: prompt.fontFamily
                font.pixelSize: Style.font.caption
                wrapMode: Text.WordWrap
              }
            }

            MouseArea {
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: prompt.selectedPreset = parent.modelData.id
            }
          }
        }

        Item {
          width: parent.width
          implicitHeight: buttons.implicitHeight

          Row {
            id: buttons
            anchors.right: parent.right
            spacing: Style.space(10)

            Button {
              text: "Cancel"
              bordered: true
              foreground: prompt.foreground
              fontFamily: prompt.fontFamily
              onClicked: prompt.canceled()
            }

            Button {
              text: "Apply Routing"
              bordered: true
              enabled: prompt.accepted
              foreground: enabled ? prompt.foreground : prompt.dim
              fontFamily: prompt.fontFamily
              onClicked: prompt.confirmed(prompt.selectedPreset)
            }
          }
        }
      }
    }
  }
}
