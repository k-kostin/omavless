// SPDX-License-Identifier: MIT
import QtQuick
import QtQuick.Controls as Controls
import qs.Commons
import qs.Ui
import "SafeText.js" as SafeText
import "I18n.js" as I18n

// TextArea uses QQuickTextEdit's formats-only canPaste path, including on older
// Qt. TextField/TextInput performs synchronous unbounded clipboard reads there.
FocusScope {
  id: field
  objectName: "omavlessSafeTextField"
  property alias text: editor.text
  property alias font: editor.font
  property alias color: editor.color
  property alias placeholderText: editor.placeholderText
  property alias placeholderTextColor: editor.placeholderTextColor
  property alias selectByMouse: editor.selectByMouse
  property alias readOnly: editor.readOnly
  property alias cursorPosition: editor.cursorPosition
  property alias selectionStart: editor.selectionStart
  property alias selectionEnd: editor.selectionEnd
  property alias leftPadding: editor.leftPadding
  property alias rightPadding: editor.rightPadding
  property alias background: editor.background
  property color foreground: Color.foreground
  property string locale: "en"
  property int maximumLength: 8192
  property string pasteError: ""
  property string previousText: ""
  property var pasteContext: null
  signal accepted()
  signal textEdited()
  implicitWidth: editor.implicitWidth
  implicitHeight: editor.implicitHeight
  KeyNavigation.priority: KeyNavigation.BeforeItem
  function selectAll() { editor.selectAll() }
  function deselect() { editor.deselect() }
  function paste() { return requestPaste() }
  function requestPaste() {
    if (!visible || !enabled || readOnly || clipboard.running) return false
    forceActiveFocus()
    pasteContext = {text:text, start:selectionStart, end:selectionEnd, cursor:cursorPosition}
    pasteError = ""
    return clipboard.request()
  }
  function cancelPaste() { pasteContext = null; clipboard.cancel() }
  function finishPaste(value) {
    var context = pasteContext
    pasteContext = null
    if (!context || !visible || !enabled || readOnly || !activeFocus
        || context.text !== text || context.start !== selectionStart
        || context.end !== selectionEnd || context.cursor !== cursorPosition) return
    var result = SafeText.insertion(text, value, context.start, context.end, maximumLength)
    if (!result) { pasteError = "input"; return }
    text = result.text
    cursorPosition = result.cursor
    textEdited()
  }
  onVisibleChanged: if (!visible) cancelPaste()
  onEnabledChanged: if (!enabled) cancelPaste()
  onActiveFocusChanged: if (!activeFocus) cancelPaste()

  BoundedClipboard {
    id: clipboard
    onReceived: function(value) { field.finishPaste(value) }
    onRefused: { field.pasteContext = null; field.pasteError = "clipboard" }
  }
  Flickable {
    id: viewport
    anchors.fill: parent
    clip: true
    interactive: false
    boundsBehavior: Flickable.StopAtBounds
    Controls.TextArea.flickable: Controls.TextArea {
      id: editor
      focus: true
      textFormat: TextEdit.PlainText
      wrapMode: TextEdit.NoWrap
      selectByMouse: true
      persistentSelection: false
      color: field.foreground
      selectionColor: Style.selectionFillFor(field.foreground, Color.accent)
      selectedTextColor: field.foreground
      placeholderTextColor: Qt.darker(field.foreground, 1.6)
      font.family: Style.font.family
      font.pixelSize: Style.font.body
      leftPadding: Style.spacing.controlPaddingX + Style.normalBorderWidth
      rightPadding: Style.spacing.controlPaddingX + Style.normalBorderWidth
      topPadding: Style.spacing.inputPaddingY + Style.normalBorderWidth
      bottomPadding: Style.spacing.inputPaddingY + Style.normalBorderWidth
      // Disable every native menu activation (mouse, keyboard, touch).
      Controls.ContextMenu.menu: null
      Keys.priority: Keys.BeforeItem
      Keys.forwardTo: [field]
      Keys.onPressed: function(event) {
        if (event.matches(StandardKey.Paste)
            || (event.key === Qt.Key_V && (event.modifiers & Qt.ControlModifier))) {
          event.accepted = true
          field.requestPaste()
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
          event.accepted = true
          field.accepted()
        } else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) {
          // Attached navigation on the outer scope precedes TextArea's Tab.
          event.accepted = true
        }
      }
      onTextChanged: {
        if (!SafeText.valid(text, field.maximumLength)) {
          text = field.previousText
          field.pasteError = "input"
        } else field.previousText = text
      }
      onTextEdited: { field.pasteError = ""; field.textEdited() }
      background: BorderSurface {
        color: Style.controlFill(field.activeFocus, editor.hovered, field.foreground, Color.accent)
        borderSpec: Border.controlSpec(field.activeFocus ? "focus" : editor.hovered ? "hover-cursor" : "normal", field.foreground, Color.accent)
        radius: Style.cornerRadius
      }
    }
    // Do not silently substitute ordinary clipboard for primary selection.
    MouseArea {
      anchors.fill: parent
      acceptedButtons: Qt.MiddleButton
      onPressed: function(mouse) { mouse.accepted = true; field.pasteError = "primary" }
    }
  }
  Controls.ToolTip.visible: pasteError !== ""
  Controls.ToolTip.timeout: 4000
  Controls.ToolTip.text: I18n.translate(pasteError === "input" ? "input.error.invalid" : pasteError === "primary" ? "input.error.primary" : "input.error.clipboard", locale)
}
