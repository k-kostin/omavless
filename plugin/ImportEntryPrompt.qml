// SPDX-License-Identifier: MIT
import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "I18n.js" as I18n

// Acquisition only: canonical classification and confirmation remain in Rust
// and Service. No clipboard access occurs merely by opening this dialog.
Item {
  id: prompt
  objectName: "omavlessImportEntryPrompt"
  property string locale: "en"
  property bool available: false
  property bool revealed: false
  signal submitted(string value)
  signal canceled()
  visible: false
  focus: visible
  function tr(key) { return I18n.translate(key, locale) }
  function clear() {
    revealed = false
    entry.cancelPaste()
    entry.text = ""
    entry.previousText = ""
    entry.resetHistory()
    entry.pasteError = ""
  }
  function open() {
    clear()
    visible = true
    Qt.callLater(function() { if (prompt.visible) entry.forceActiveFocus() })
  }
  function dismiss() { visible = false; clear() }
  function navigateEntry(event) {
    if (!visible || (event.key !== Qt.Key_Tab && event.key !== Qt.Key_Backtab)) return
    var back = event.key === Qt.Key_Backtab || (event.modifiers & Qt.ShiftModifier)
    var target = back ? (review.enabled ? review : cancel) : reveal
    target.forceActiveFocus(back ? Qt.BacktabFocusReason : Qt.TabFocusReason)
    event.accepted = true
  }
  onVisibleChanged: if (!visible) clear()
  Keys.onEscapePressed: canceled()
  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Commons.Color.background, 0.78)
    MouseArea { anchors.fill: parent; onClicked: prompt.canceled() }
    BorderSurface {
      id: card
      width: Math.min(parent.width - Style.space(24), Style.space(420))
      height: card.contentTopInset + card.contentBottomInset + body.implicitHeight
      anchors.centerIn: parent
      color: Commons.Color.background
      borderSpec: Border.flat(Commons.Color.accent, Style.normalBorderWidth)
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
          text: prompt.tr("import.entry.title")
          color: Commons.Color.foreground
          font.family: Style.font.family
          font.pixelSize: Style.font.title
          wrapMode: Text.Wrap
        }
        PlainText {
          width: parent.width
          text: prompt.tr("import.entry.hint")
          color: Commons.Color.foreground
          font.family: Style.font.family
          font.pixelSize: Style.font.bodySmall
          wrapMode: Text.Wrap
        }
        Item {
          width: parent.width
          height: Style.space(96)
          SafeTextField {
            id: entry
            objectName: "omavlessImportEntry"
            anchors.fill: parent
            // Keep the bounded TextArea acquisition path even while masked.
            // Opacity suppresses all private glyphs/selection/cursor painting;
            // the inert overlay has no clipboard or separate input model.
            opacity: prompt.revealed ? 1 : 0
            locale: prompt.locale
            multiline: true
            maximumLength: 32768
            placeholderText: prompt.tr("import.entry.placeholder")
            onAccepted: if (review.enabled) prompt.submitted(text)
            // Qt can deliver Shift+Tab as Tab plus a modifier. Handle both
            // representations before attached navigation consumes forward Tab.
            KeyNavigation.priority: KeyNavigation.AfterItem
            KeyNavigation.tab: reveal
            KeyNavigation.backtab: review.enabled ? review : cancel
            Keys.priority: Keys.BeforeItem
            Keys.onPressed: function(event) { prompt.navigateEntry(event) }
            Keys.onEscapePressed: prompt.canceled()
          }
          Rectangle {
            anchors.fill: parent
            visible: !prompt.revealed
            color: Commons.Color.background
            border.color: entry.activeFocus ? Commons.Color.accent : Commons.Color.foreground
            border.width: Style.normalBorderWidth
            radius: Style.cornerRadius
            PlainText {
              anchors.fill: parent
              anchors.margins: Style.space(10)
              text: entry.text.length ? "••••••••" : prompt.tr("import.entry.placeholder")
              color: Commons.Color.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.body
              wrapMode: Text.Wrap
            }
            MouseArea { anchors.fill: parent; onClicked: entry.forceActiveFocus() }
          }
        }
        PlainText {
          width: parent.width
          text: prompt.tr("import.entry.private")
          color: Qt.darker(Commons.Color.foreground, 1.55)
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
          wrapMode: Text.Wrap
        }
        PlainText {
          objectName: "omavlessImportUnavailable"
          width: parent.width
          visible: !prompt.available
          text: prompt.tr("import.entry.unavailable")
          color: Commons.Color.foreground
          font.family: Style.font.family
          font.pixelSize: Style.font.bodySmall
          wrapMode: Text.Wrap
        }
        Flow {
          width: parent.width
          spacing: Style.space(8)
          Button {
            id: reveal
            objectName: "omavlessImportReveal"
            KeyNavigation.priority: KeyNavigation.BeforeItem
            text: prompt.tr(prompt.revealed ? "import.entry.hide" : "import.entry.show")
            bordered: true
            focusable: true
            KeyNavigation.tab: pasteButton
            KeyNavigation.backtab: entry
            onClicked: { entry.cancelPaste(); prompt.revealed = !prompt.revealed; entry.forceActiveFocus() }
          }
          Button {
            id: pasteButton
            objectName: "omavlessImportPaste"
            KeyNavigation.priority: KeyNavigation.BeforeItem
            text: prompt.tr("common.paste")
            bordered: true
            focusable: true
            enabled: entry.canPaste
            KeyNavigation.tab: cancel
            KeyNavigation.backtab: reveal
            onClicked: entry.requestPaste()
          }
        }
        RowLayout {
          width: parent.width
          spacing: Style.space(8)
          Item { Layout.fillWidth: true }
          Button {
            id: cancel
            objectName: "omavlessImportCancel"
            KeyNavigation.priority: KeyNavigation.BeforeItem
            text: prompt.tr("common.cancel")
            bordered: true
            focusable: true
            KeyNavigation.tab: review.enabled ? review : entry
            KeyNavigation.backtab: pasteButton
            onClicked: prompt.canceled()
          }
          Button {
            id: review
            objectName: "omavlessImportReview"
            KeyNavigation.priority: KeyNavigation.BeforeItem
            text: prompt.tr("import.entry.review")
            bordered: true
            focusable: true
            enabled: prompt.available && entry.text.trim().length > 0
            opacity: enabled ? 1 : 0.45
            KeyNavigation.tab: entry
            KeyNavigation.backtab: cancel
            onClicked: prompt.submitted(entry.text)
          }
        }
      }
    }
  }
}
