// SPDX-License-Identifier: MIT
// Copyright (c) 2026 OmaVLESS contributors

import QtQuick
import qs.Commons
import qs.Ui

// Subscription URLs behave like bearer credentials. The URL is shown only
// inside this explicit editor and starts obscured on every open.
Item {
  id: prompt

  property string title: "Subscription"
  property string hint: ""
  property bool accepted: false
  property bool loading: false
  property bool error: false
  property string confirmLabel: "Save"
  property alias nameValue: nameField.text
  property alias urlValue: urlField.text
  property color foreground: Color.foreground
  property color dim: Qt.darker(foreground, 1.55)
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family
  property bool autoName: true

  signal confirmed()
  signal canceled()

  visible: false

  function openWith(name, url) {
    nameField.text = String(name || "")
    urlField.text = String(url || "")
    autoName = nameField.text === ""
    reveal.checked = false
    visible = true
    Qt.callLater(function() { urlField.forceActiveFocus() })
  }

  function suggestedName(value) {
    var match = String(value || "").match(/^https?:\/\/([^\/:?#]+)/i)
    return match ? match[1].replace(/^www\./i, "").substring(0, 80) : ""
  }

  function dismiss() {
    visible = false
    nameField.text = ""
    urlField.text = ""
    reveal.checked = false
  }
  function focusUrl() {
    urlField.forceActiveFocus()
    urlField.selectAll()
  }

  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Color.background, 0.76)
    MouseArea { anchors.fill: parent; onClicked: prompt.canceled() }

    BorderSurface {
      id: card
      width: Math.min(parent.width - Style.space(24), Style.space(410))
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
          text: prompt.title
          color: prompt.foreground
          font.family: prompt.fontFamily
          font.pixelSize: Style.font.title
          elide: Text.ElideRight
        }

        TextField {
          id: nameField
          width: parent.width
          placeholderText: "Provider name"
          foreground: prompt.foreground
          font.family: prompt.fontFamily
          onTextEdited: prompt.autoName = false
          Keys.onEscapePressed: prompt.canceled()
        }

        TextField {
          id: urlField
          width: parent.width
          placeholderText: "https://provider.example/subscription"
          foreground: prompt.foreground
          font.family: prompt.fontFamily
          echoMode: reveal.checked ? TextInput.Normal : TextInput.Password
          enabled: !prompt.loading
          onTextEdited: if (prompt.autoName) nameField.text = prompt.suggestedName(text)
          onAccepted: if (prompt.accepted) prompt.confirmed()
          Keys.onEscapePressed: prompt.canceled()
        }

        Row {
          width: parent.width
          spacing: Style.space(6)

          Button {
            id: reveal
            property bool checked: false
            text: checked ? "Hide URL" : "Show URL"
            tooltipText: "Subscription URLs may contain access credentials"
            bordered: true
            foreground: prompt.foreground
            fontFamily: prompt.fontFamily
            enabled: !prompt.loading
            onClicked: checked = !checked
          }

          PlainText {
            width: parent.width - reveal.width - parent.spacing
            anchors.verticalCenter: reveal.verticalCenter
            text: prompt.loading ? "Loading private URL…" : prompt.hint
            color: prompt.error || (!prompt.accepted && !prompt.loading)
              ? prompt.urgent : prompt.dim
            font.family: prompt.fontFamily
            font.pixelSize: Style.font.caption
            wrapMode: Text.WrapAnywhere
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
              text: prompt.confirmLabel
              bordered: true
              enabled: prompt.accepted && !prompt.loading
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
