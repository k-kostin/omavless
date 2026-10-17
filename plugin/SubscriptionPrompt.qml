// SPDX-License-Identifier: MIT
// Copyright (c) 2026 OmaVLESS contributors

import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "I18n.js" as I18n
import "SafeText.js" as SafeText

// Subscription URLs behave like bearer credentials. The URL is shown only
// inside this explicit editor and starts obscured on every open.
Item {
  id: prompt

  property string locale: "en"
  property string title: textFor("subscription.title")
  property string hint: ""
  property bool accepted: false
  property bool loading: false
  property bool error: false
  property string confirmLabel: textFor("common.save")
  property alias nameValue: nameField.text
  property alias urlValue: urlField.text
  property color foreground: Commons.Color.foreground
  property color dim: Qt.darker(foreground, 1.55)
  property color urgent: Commons.Color.urgent
  property string fontFamily: Style.font.family
  property bool autoName: true
  property bool fromFile: false

  signal confirmed()
  signal canceled()

  function textFor(key, values) {
    return I18n.translate(key, locale, values || {})
  }

  visible: false
  focus: visible
  Keys.onEscapePressed: canceled()

  function openWith(name, url) {
    urlField.pasteContext = null
    urlClipboard.cancel()
    urlEditor.cancelPaste()
    fromFile = false
    nameField.text = String(name || "")
    urlField.text = String(url || "")
    autoName = nameField.text === ""
    reveal.checked = false
    visible = true
    Qt.callLater(function() { urlField.forceActiveFocus() })
  }

  function openFromFile(name) {
    fromFile = true
    nameField.text = String(name || textFor("subscription.title"))
    urlField.text = ""
    autoName = false
    reveal.checked = false
    visible = true
    Qt.callLater(function() { nameField.forceActiveFocus(); nameField.selectAll() })
  }

  function suggestedName(value) {
    var match = String(value || "").match(/^https?:\/\/([^\/:?#]+)/i)
    return match ? match[1].replace(/^www\./i, "").substring(0, 80) : ""
  }

  function dismiss() {
    visible = false
    fromFile = false
    nameField.text = ""
    urlField.text = ""
    reveal.checked = false
  }
  function focusUrl() {
    urlField.forceActiveFocus()
    if (reveal.checked) urlEditor.selectAll()
  }

  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Commons.Color.background, 0.76)
    MouseArea { anchors.fill: parent; onClicked: prompt.canceled() }

    BorderSurface {
      id: card
      width: Math.min(parent.width - Style.space(24), Style.space(410))
      height: card.contentTopInset + card.contentBottomInset + content.implicitHeight
      anchors.centerIn: parent
      color: Commons.Color.background
      borderSpec: Border.flat(Commons.Color.accent, Style.normalBorderWidth)
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

        SafeTextField {
          id: nameField
          locale: prompt.locale
          maximumLength: 80
          width: parent.width
          placeholderText: prompt.textFor("subscription.provider_name")
          foreground: prompt.foreground
          font.family: prompt.fontFamily
          onTextEdited: prompt.autoName = false
          KeyNavigation.tab: prompt.fromFile ? cancelButton : urlField
          KeyNavigation.backtab: confirmButton
          Keys.onEscapePressed: prompt.canceled()
        }

        FocusScope {
          id: urlField
          objectName: "omavlessSubscriptionUrl"
          property string text: ""
          property var pasteContext: null
          signal accepted()
          signal textEdited()
          visible: !prompt.fromFile
          width: parent.width
          implicitHeight: hiddenUrl.implicitHeight
          enabled: !prompt.loading
          KeyNavigation.priority: KeyNavigation.BeforeItem
          onTextEdited: {
            pasteContext = null
            urlClipboard.cancel()
            if (prompt.autoName) nameField.text = prompt.suggestedName(text)
          }
          onAccepted: if (prompt.accepted) prompt.confirmed()
          KeyNavigation.tab: reveal
          KeyNavigation.backtab: nameField
          Keys.onEscapePressed: prompt.canceled()
          onVisibleChanged: if (!visible) { pasteContext = null; urlClipboard.cancel() }
          onEnabledChanged: if (!enabled) { pasteContext = null; urlClipboard.cancel() }
          onActiveFocusChanged: if (!activeFocus) { pasteContext = null; urlClipboard.cancel() }
          function paste(replaceAll) {
            if (!visible || !enabled || urlClipboard.running) return false
            forceActiveFocus()
            pasteContext = {text:text, revealed:reveal.checked,
              start:replaceAll || !reveal.checked ? 0 : urlEditor.selectionStart,
              end:replaceAll || !reveal.checked ? text.length : urlEditor.selectionEnd,
              cursor:urlEditor.cursorPosition, replaceAll:replaceAll || !reveal.checked}
            return urlClipboard.request()
          }
          TextField {
            id: hiddenUrl
            objectName: "omavlessMaskedSubscriptionUrl"
            anchors.fill: parent
            visible: !reveal.checked
            focus: !reveal.checked
            // Never toggle this false: Qt would immediately read the clipboard.
            readOnly: true
            echoMode: TextInput.Password
            text: urlField.text
            foreground: prompt.foreground
            font.family: prompt.fontFamily
            placeholderText: prompt.textFor("input.url_hidden_hint")
            Keys.priority: Keys.BeforeItem
            Keys.onPressed: function(event) {
              if (event.matches(StandardKey.Paste)
                  || (event.key === Qt.Key_V && (event.modifiers & Qt.ControlModifier))) {
                event.accepted = true; urlField.paste(true)
              } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                event.accepted = true; urlField.accepted()
              }
            }
            KeyNavigation.tab: reveal
            KeyNavigation.backtab: nameField
          }
          SafeTextField {
            id: urlEditor
            objectName: "omavlessSubscriptionUrlEditor"
            anchors.fill: parent
            visible: reveal.checked
            focus: reveal.checked
            locale: prompt.locale
            text: urlField.text
            foreground: prompt.foreground
            font.family: prompt.fontFamily
            maximumLength: 8192
            externalPaste: true
            externalPasteBusy: urlClipboard.running
            onPasteRequested: urlField.paste(false)
            placeholderText: "https://provider.example/subscription"
            onTextEdited: { urlField.text = text; urlField.textEdited() }
            onAccepted: urlField.accepted()
            KeyNavigation.tab: reveal
            KeyNavigation.backtab: nameField
          }
          BoundedClipboard {
            id: urlClipboard
            onReceived: function(value) {
              var original = urlField.pasteContext
              urlField.pasteContext = null
              if (original === null || !urlField.visible || !urlField.enabled || !urlField.activeFocus
                  || original.text !== urlField.text || original.revealed !== reveal.checked
                  || (!original.replaceAll && (original.start !== urlEditor.selectionStart
                    || original.end !== urlEditor.selectionEnd || original.cursor !== urlEditor.cursorPosition))) return
              var result = SafeText.insertion(urlField.text, value, original.start, original.end, 8192)
              if (result) {
                if (reveal.checked) urlEditor.insertBounded(value, original.start, original.end)
                else { urlField.text = result.text; urlField.textEdited() }
                urlEditor.pasteError = ""
              }
              else urlEditor.pasteError = "input"
            }
            onRefused: { urlField.pasteContext = null; urlEditor.pasteError = "clipboard" }
          }
        }

        PlainText {
          visible: !prompt.fromFile && !reveal.checked
          width: parent.width
          text: prompt.textFor("input.url_hidden_hint")
          color: prompt.dim
          font.family: prompt.fontFamily
          font.pixelSize: Style.font.caption
          wrapMode: Text.Wrap
        }

        PlainText {
          visible: prompt.fromFile
          width: parent.width
          text: prompt.textFor("subscription.file_validated")
          color: prompt.dim
          font.family: prompt.fontFamily
          font.pixelSize: Style.font.bodySmall
          wrapMode: Text.WordWrap
        }

        PlainText {
          visible: !prompt.fromFile && urlEditor.pasteError !== ""
          width: parent.width
          text: prompt.textFor(urlEditor.pasteError === "input" ? "input.error.invalid" : "input.error.clipboard")
          color: prompt.urgent
          font.family: prompt.fontFamily
          font.pixelSize: Style.font.caption
          wrapMode: Text.Wrap
        }

        RowLayout {
          width: parent.width
          spacing: Style.space(6)

          Button {
            id: reveal
            visible: !prompt.fromFile
            property bool checked: false
            onCheckedChanged: { urlField.pasteContext = null; urlClipboard.cancel(); urlEditor.cancelPaste() }
            text: checked
              ? prompt.textFor("common.hide_url") : prompt.textFor("common.show_url")
            tooltipText: prompt.textFor("subscription.url_sensitive")
            bordered: true
            foreground: prompt.foreground
            fontFamily: prompt.fontFamily
            enabled: !prompt.loading
            focusable: true
            KeyNavigation.tab: pasteUrl
            KeyNavigation.backtab: urlField
            onClicked: { checked = !checked; urlField.forceActiveFocus() }
          }

          Button {
            id: pasteUrl
            visible: !prompt.fromFile
            text: prompt.textFor("common.paste")
            bordered: true
            foreground: prompt.foreground
            fontFamily: prompt.fontFamily
            enabled: !prompt.loading && !urlClipboard.running
            focusable: true
            KeyNavigation.tab: cancelButton
            KeyNavigation.backtab: reveal
            onClicked: urlField.paste(true)
          }

          PlainText {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
            text: prompt.loading ? prompt.textFor("subscription.loading_private") : prompt.hint
            color: prompt.error || (!prompt.accepted && !prompt.loading)
              ? prompt.urgent : prompt.dim
            font.family: prompt.fontFamily
            font.pixelSize: Style.font.caption
            wrapMode: Text.Wrap
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
              id: cancelButton
              text: prompt.textFor("common.cancel")
              bordered: true
              foreground: prompt.foreground
              fontFamily: prompt.fontFamily
              focusable: true
              KeyNavigation.tab: confirmButton
              KeyNavigation.backtab: prompt.fromFile ? nameField : pasteUrl
              onClicked: prompt.canceled()
            }
            Button {
              id: confirmButton
              text: prompt.confirmLabel
              bordered: true
              enabled: prompt.accepted && !prompt.loading
              foreground: enabled ? prompt.foreground : prompt.dim
              fontFamily: prompt.fontFamily
              focusable: true
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
