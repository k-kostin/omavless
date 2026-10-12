// SPDX-License-Identifier: MIT
import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Ui
import "I18n.js" as I18n

// Acquisition only: canonical classification and confirmation remain in Rust
// and Service. No clipboard access occurs merely by opening this dialog.
Item {
  id: prompt
  objectName: "omavlessImportEntryPrompt"
  property string locale: "en"
  property bool available: false
  signal submitted(string value)
  signal canceled()
  visible: false
  focus: visible
  function tr(key) { return I18n.translate(key, locale) }
  function clear() {
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
  onVisibleChanged: if (!visible) clear()
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
          text: prompt.tr("import.entry.title")
          color: Color.foreground
          font.family: Style.font.family
          font.pixelSize: Style.font.title
          wrapMode: Text.Wrap
        }
        PlainText {
          width: parent.width
          text: prompt.tr("import.entry.hint")
          color: Color.foreground
          font.family: Style.font.family
          font.pixelSize: Style.font.bodySmall
          wrapMode: Text.Wrap
        }
        SafeTextField {
          id: entry
          objectName: "omavlessImportEntry"
          width: parent.width
          locale: prompt.locale
          maximumLength: 32768
          placeholderText: prompt.tr("import.entry.placeholder")
          onAccepted: if (review.enabled) prompt.submitted(text)
          KeyNavigation.tab: pasteButton
          KeyNavigation.backtab: review.enabled ? review : cancel
          Keys.onEscapePressed: prompt.canceled()
        }
        PlainText {
          width: parent.width
          text: prompt.tr("import.entry.private")
          color: Qt.darker(Color.foreground, 1.55)
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
          wrapMode: Text.Wrap
        }
        RowLayout {
          width: parent.width
          spacing: Style.space(8)
          Button {
            id: pasteButton
            objectName: "omavlessImportPaste"
            KeyNavigation.priority: KeyNavigation.BeforeItem
            text: prompt.tr("common.paste")
            bordered: true
            focusable: true
            enabled: entry.canPaste
            KeyNavigation.tab: cancel
            KeyNavigation.backtab: entry
            onClicked: entry.requestPaste()
          }
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
            KeyNavigation.tab: entry
            KeyNavigation.backtab: cancel
            onClicked: prompt.submitted(entry.text)
          }
        }
      }
    }
  }
}
