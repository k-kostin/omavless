// SPDX-License-Identifier: MIT
// Adapted from Omarchy VPN: https://github.com/jkoestinger/omarchy-vpn
// Copyright (c) 2026 Justin Köstinger
// Copyright (c) 2026 OmaVLESS contributors
// See LICENSE and THIRD_PARTY_NOTICES.md.

import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "I18n.js" as I18n
import "NativeSnapshot.js" as NativeSnapshot

// Review only non-reusable connection facts before a credential is saved.
// Parsing and redaction happen in the backend; this surface never receives a
// reusable password, full UUID, Reality key or complete URI in its preview model.
Item {
  id: prompt

  property string locale: "en"
  property string title: textFor("import.profile_title")
  property string confirmLabel: textFor("common.import")
  property string hint: ""
  property bool accepted: false
  property var preview: ({})
  readonly property bool nativeCredential: ["wireguard", "amneziawg"].indexOf(preview.protocol) >= 0
  property alias value: nameField.text
  readonly property alias flickable: reviewScroll
  readonly property var focusTargets: [nameField, cancelButton, confirmButton]
  property color foreground: Commons.Color.foreground
  property color dim: Qt.darker(foreground, 1.55)
  property color urgent: Commons.Color.urgent
  property string fontFamily: Style.font.family

  signal confirmed()
  signal canceled()

  function textFor(key, values) {
    return I18n.translate(key, locale, values || {})
  }

  function managedWarning() {
    return prompt.textFor(NativeSnapshot.managedImportNoteKey(prompt.preview.compatibilityNote))
  }

  visible: false
  focus: visible

  function openWith(text) {
    nameField.cancelPaste()
    nameField.resetHistory()
    nameField.text = String(text)
    reviewScroll.contentY = 0
    visible = true
    Qt.callLater(function() {
      nameField.forceActiveFocus()
      nameField.selectAll()
    })
  }

  function dismiss() { visible = false }

  // Keep the focused form control reachable at constrained panel heights,
  // using the same bounded scrolling pattern as the local Help surface.
  function reveal(control) {
    Qt.callLater(function() {
      if (!prompt.visible || !control.activeFocus) return
      var top = control.mapToItem(reviewScroll.contentItem, 0, 0).y
      var bottom = top + control.height
      var next = reviewScroll.contentY
      if (top < next) next = top
      else if (bottom > next + reviewScroll.height) next = bottom - reviewScroll.height
      reviewScroll.contentY = Math.max(0, Math.min(next,
        Math.max(0, reviewScroll.contentHeight - reviewScroll.height)))
    })
  }

  Keys.onEscapePressed: canceled()

  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Commons.Color.background, 0.78)
    MouseArea { anchors.fill: parent; onClicked: prompt.canceled() }

    BorderSurface {
      id: card
      width: Math.min(parent.width - Style.space(24), Style.space(420))
      height: Math.min(Math.max(0, parent.height - Style.space(24)),
        card.contentTopInset + card.contentBottomInset + body.implicitHeight)
      anchors.centerIn: parent
      color: Commons.Color.background
      borderSpec: Border.flat(Commons.Color.accent, Style.normalBorderWidth)
      padding: Style.space(18)
      radius: Style.cornerRadius

      MouseArea { anchors.fill: parent; onClicked: {} }

      Flickable {
        id: reviewScroll
        objectName: "importReviewScroll"
        anchors.fill: parent
        anchors.topMargin: card.contentTopInset
        anchors.bottomMargin: card.contentBottomInset
        anchors.leftMargin: card.contentLeftInset
        anchors.rightMargin: card.contentRightInset
        contentWidth: width
        contentHeight: body.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height
        ScrollBar.vertical: OmaScrollBar { policy: ScrollBar.AsNeeded }

      Column {
        id: body
        width: Math.max(0, parent.width - Style.space(16))
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
          text: prompt.textFor("import.review")
          color: Commons.Color.accent
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
                { label: prompt.textFor("import.protocol"), value: String(prompt.preview.protocol || "") },
                { label: prompt.textFor("import.endpoint"), value: String(prompt.preview.server || "")
                    + ":" + String(prompt.preview.port || "") },
                { label: prompt.textFor("import.connection"), value: String(prompt.preview.transport || "")
                    + " / " + String(prompt.preview.security || "")
                    + (prompt.preview.flow ? " / " + prompt.preview.flow : "")
                    + (prompt.preview.advancedXhttp ? " / advanced" : "")
                    + (prompt.preview.experimental ? " / experimental" : "") },
                { label: "SNI", value: String(prompt.preview.sni || "—") },
                { label: prompt.textFor("import.tls_check"), wrap: prompt.nativeCredential, value: prompt.preview.insecure
                    ? prompt.textFor("import.tls_disabled") : prompt.nativeCredential ? prompt.textFor("import.not_tls") : prompt.textFor("import.tls_enabled") },
                { label: prompt.textFor("import.credential"), value: String(prompt.preview.credentialHint || "••••")
                    + " · " + prompt.textFor("import.hidden") }
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
                  wrapMode: parent.modelData.wrap ? Text.WordWrap : Text.NoWrap
                  elide: parent.modelData.wrap ? Text.ElideNone : Text.ElideMiddle
                }
              }
            }
          }
        }

        PlainText {
          width: parent.width
          text: prompt.textFor("import.privacy_note")
          color: prompt.dim
          font.family: prompt.fontFamily
          font.pixelSize: Style.font.caption
          wrapMode: Text.WordWrap
        }

        PlainText {
          width: parent.width
          visible: text.length > 0
          text: prompt.nativeCredential ? prompt.managedWarning() : String(prompt.preview.compatibilityNote || "")
          color: Commons.Color.accent
          font.family: prompt.fontFamily
          font.pixelSize: Style.font.caption
          wrapMode: Text.WordWrap
        }

        SafeTextField {
          id: nameField
          objectName: "importProfileName"
          locale: prompt.locale
          maximumLength: 80
          width: parent.width
          placeholderText: prompt.textFor("import.profile_name")
          foreground: prompt.foreground
          font.family: prompt.fontFamily
          onAccepted: if (prompt.accepted) prompt.confirmed()
          onActiveFocusChanged: if (activeFocus) prompt.reveal(this)
          KeyNavigation.tab: cancelButton
          KeyNavigation.backtab: confirmButton
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
              id: cancelButton
              objectName: "importCancel"
              text: prompt.textFor("common.cancel")
              bordered: true
              foreground: prompt.foreground
              fontFamily: prompt.fontFamily
              focusable: true
              onActiveFocusChanged: if (activeFocus) prompt.reveal(this)
              KeyNavigation.tab: confirmButton
              KeyNavigation.backtab: nameField
              onClicked: prompt.canceled()
            }
            Button {
              id: confirmButton
              objectName: "importConfirm"
              text: prompt.confirmLabel
              bordered: true
              enabled: prompt.accepted
              foreground: enabled ? prompt.foreground : prompt.dim
              fontFamily: prompt.fontFamily
              focusable: true
              onActiveFocusChanged: if (activeFocus) prompt.reveal(this)
              KeyNavigation.tab: nameField
              KeyNavigation.backtab: cancelButton
              onClicked: prompt.confirmed()
            }
          }
        }
      }
      }
    }
  }
}
