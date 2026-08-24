// SPDX-License-Identifier: MIT
// Adapted from Omarchy VPN: https://github.com/jkoestinger/omarchy-vpn
// Copyright (c) 2026 Justin Köstinger
// Copyright (c) 2026 OmaVLESS contributors
// See LICENSE and THIRD_PARTY_NOTICES.md.

import QtQuick
import qs.Commons
import qs.Ui

// Review only non-reusable connection facts before a credential is saved.
// Parsing and redaction happen in the backend; this surface never receives a
// full UUID, Reality key or complete VLESS URI in its preview model.
Item {
  id: prompt

  property string title: "Import VLESS profile"
  property string confirmLabel: "Import"
  property string hint: ""
  property bool accepted: false
  property var preview: ({})
  property alias value: nameField.text
  property color foreground: Color.foreground
  property color dim: Qt.darker(foreground, 1.55)
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family

  signal confirmed()
  signal canceled()

  visible: false
  focus: visible

  function openWith(text) {
    nameField.text = String(text)
    visible = true
    Qt.callLater(function() {
      nameField.forceActiveFocus()
      nameField.selectAll()
    })
  }

  function dismiss() { visible = false }

  Keys.onEscapePressed: canceled()

  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Color.background, 0.78)
    MouseArea { anchors.fill: parent; onClicked: prompt.canceled() }

    BorderSurface {
      id: card
      width: Math.min(parent.width - Style.space(24), Style.space(420))
      height: card.contentTopInset + card.contentBottomInset + body.implicitHeight
      anchors.centerIn: parent
      color: Color.background
      borderSpec: Border.flat(Color.accent, Style.normalBorderWidth)
      padding: Style.space(18)
      radius: Style.cornerRadius

      MouseArea { anchors.fill: parent; onClicked: {} }

      Column {
        id: body
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.topMargin: card.contentTopInset
        anchors.leftMargin: card.contentLeftInset
        anchors.rightMargin: card.contentRightInset
        spacing: Style.space(10)

        PlainText {
          width: parent.width
          text: prompt.title
          color: prompt.foreground
          font.family: prompt.fontFamily
          font.pixelSize: Style.font.title
          elide: Text.ElideMiddle
        }

        PlainText {
          width: parent.width
          text: "Review before import"
          color: Color.accent
          font.family: prompt.fontFamily
          font.pixelSize: Style.font.subtitle
          font.bold: true
        }

        BorderSurface {
          width: parent.width
          height: facts.implicitHeight + Style.space(16)
          color: "transparent"
          borderSpec: Border.flat(Util.alpha(prompt.foreground, 0.28), Style.normalBorderWidth)
          radius: Style.cornerRadius

          Column {
            id: facts
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: Style.space(10)
            anchors.rightMargin: Style.space(10)
            spacing: Style.space(5)

            Repeater {
              model: [
                { label: "Endpoint", value: String(prompt.preview.server || "")
                    + ":" + String(prompt.preview.port || "") },
                { label: "Connection", value: String(prompt.preview.transport || "")
                    + " / " + String(prompt.preview.security || "")
                    + (prompt.preview.flow ? " / " + prompt.preview.flow : "") },
                { label: "SNI", value: String(prompt.preview.sni || "—") },
                { label: "TLS check", value: prompt.preview.insecure
                    ? "Disabled by this key" : "Enabled" },
                { label: "Credential", value: String(prompt.preview.credentialHint || "••••")
                    + " · hidden" }
              ]

              Row {
                required property var modelData
                width: facts.width
                spacing: Style.space(8)
                PlainText {
                  width: Style.space(88)
                  text: parent.modelData.label
                  color: prompt.dim
                  font.family: prompt.fontFamily
                  font.pixelSize: Style.font.caption
                }
                PlainText {
                  width: parent.width - Style.space(88) - parent.spacing
                  text: parent.modelData.value
                  color: prompt.foreground
                  font.family: prompt.fontFamily
                  font.pixelSize: Style.font.caption
                  elide: Text.ElideMiddle
                }
              }
            }
          }
        }

        PlainText {
          width: parent.width
          text: "The complete access UUID and key parameters are intentionally not shown. Nothing is stored until you press Import."
          color: prompt.dim
          font.family: prompt.fontFamily
          font.pixelSize: Style.font.caption
          wrapMode: Text.WordWrap
        }

        PlainText {
          width: parent.width
          visible: text.length > 0
          text: String(prompt.preview.compatibilityNote || "")
          color: Color.accent
          font.family: prompt.fontFamily
          font.pixelSize: Style.font.caption
          wrapMode: Text.WordWrap
        }

        TextField {
          id: nameField
          width: parent.width
          placeholderText: "Profile name"
          foreground: prompt.foreground
          font.family: prompt.fontFamily
          onAccepted: if (prompt.accepted) prompt.confirmed()
          Keys.onEscapePressed: prompt.canceled()
        }

        PlainText {
          width: parent.width
          text: prompt.hint
          color: prompt.accepted ? prompt.dim : prompt.urgent
          font.family: prompt.fontFamily
          font.pixelSize: Style.font.caption
          wrapMode: Text.WrapAnywhere
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
              text: prompt.confirmLabel
              bordered: true
              enabled: prompt.accepted
              foreground: enabled ? prompt.foreground : prompt.dim
              fontFamily: prompt.fontFamily
              onClicked: prompt.confirmed()
            }
          }
        }
      }
    }
  }
}
