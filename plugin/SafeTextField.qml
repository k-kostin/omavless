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
  property bool externalPaste: false
  property bool reverting: false
  property var editSnapshot: null
  property var undoStates: []
  property var redoStates: []
  property string editKind: ""
  property string lastEditKind: ""
  property var lastEditAfter: null
  property double lastEditAt: 0
  property int previousCursor: 0
  signal accepted()
  signal textEdited()
  signal pasteRequested()
  implicitWidth: editor.implicitWidth
  implicitHeight: editor.implicitHeight
  KeyNavigation.priority: KeyNavigation.BeforeItem
  function selectAll() { editor.selectAll() }
  function select(start, end) { editor.select(start, end) }
  function deselect() { editor.deselect() }
  function snapshot() { return {text:text, cursor:cursorPosition, start:selectionStart, end:selectionEnd} }
  function resetHistory() { undoStates = []; redoStates = []; lastEditKind = ""; editSnapshot = null }
  function remember(before, kind) {
    seedReset.stop()
    if (!before || before.text === text) return
    var now = Date.now()
    var merge = (kind === "typing" || kind === "delete") && kind === lastEditKind
      && now - lastEditAt < 1500 && lastEditAfter
      && before.text === lastEditAfter.text && before.cursor === lastEditAfter.cursor
      && before.start === before.end
    if (!merge) undoStates = undoStates.concat([before]).slice(-32)
    redoStates = []
    lastEditKind = kind; lastEditAt = now; lastEditAfter = snapshot()
  }
  function restore(value) {
    reverting = true
    editor.remove(0, text.length)
    editor.insert(0, value.text)
    if (value.start !== value.end) editor.select(value.cursor === value.start ? value.end : value.start,
                                                value.cursor === value.start ? value.start : value.end)
    else cursorPosition = value.cursor
    previousText = text; previousCursor = cursorPosition
    reverting = false
    lastEditKind = ""; editSnapshot = null
    textEdited()
  }
  function undo() {
    if (!undoStates.length || readOnly) return
    var value = undoStates[undoStates.length - 1]
    undoStates = undoStates.slice(0, -1)
    redoStates = redoStates.concat([snapshot()]).slice(-32)
    restore(value)
  }
  function redo() {
    if (!redoStates.length || readOnly) return
    var value = redoStates[redoStates.length - 1]
    redoStates = redoStates.slice(0, -1)
    undoStates = undoStates.concat([snapshot()]).slice(-32)
    restore(value)
  }
  function insertBounded(value, start, end) {
    var result = SafeText.insertion(text, value, start, end, maximumLength)
    if (!result) { pasteError = "input"; return false }
    var before = snapshot()
    reverting = true
    editor.remove(start, end)
    editor.insert(start, value)
    cursorPosition = result.cursor
    reverting = false
    previousText = text; previousCursor = cursorPosition
    remember(before, "paste")
    textEdited()
    return true
  }
  function paste() { return requestPaste() }
  function requestPaste() {
    if (!visible || !enabled || readOnly || clipboard.running) return false
    forceActiveFocus()
    if (externalPaste) { pasteRequested(); return true }
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
    insertBounded(value, context.start, context.end)
  }
  onVisibleChanged: if (!visible) { cancelPaste(); resetHistory() }
  onEnabledChanged: if (!enabled) cancelPaste()
  onActiveFocusChanged: if (!activeFocus) cancelPaste()
  // Programmatic seeds/reopens are not undoable edits of a previous record.
  Timer { id: seedReset; interval: 0; onTriggered: field.resetHistory() }

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
        if (event.matches(StandardKey.Undo)) { event.accepted = true; field.undo() }
        else if (event.matches(StandardKey.Redo)) { event.accepted = true; field.redo() }
        else if (event.matches(StandardKey.Paste)
            || (event.key === Qt.Key_V && (event.modifiers & Qt.ControlModifier))) {
          event.accepted = true
          field.requestPaste()
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
          event.accepted = true
          field.accepted()
        } else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) {
          // Attached navigation on the outer scope precedes TextArea's Tab.
          event.accepted = true
        } else if (event.text && !(event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
                   && !SafeText.insertion(text, event.text, selectionStart, selectionEnd, field.maximumLength)) {
          // Reject ordinary excess typing before it joins Qt's Undo group.
          event.accepted = true
          field.pasteError = "input"
        } else {
          field.editSnapshot = field.snapshot()
          field.editKind = event.matches(StandardKey.Cut) ? "cut"
            : event.key === Qt.Key_Backspace || event.key === Qt.Key_Delete ? "delete" : "typing"
        }
      }
      onTextChanged: {
        if (field.reverting) return
        if (!SafeText.valid(text, field.maximumLength)) {
          field.reverting = true
          remove(0, length)
          insert(0, field.previousText)
          cursorPosition = Math.min(field.previousCursor, length)
          field.reverting = false
          field.pasteError = "input"
        } else {
          if (!field.editSnapshot) field.editSnapshot = {text:field.previousText,
            cursor:field.previousCursor, start:field.previousCursor, end:field.previousCursor}
          field.previousText = text
          seedReset.restart()
        }
      }
      onCursorPositionChanged: field.previousCursor = cursorPosition
      onTextEdited: {
        field.remember(field.editSnapshot, field.editKind || "typing")
        field.editSnapshot = null
        field.pasteError = ""; field.textEdited()
      }
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
