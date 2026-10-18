// SPDX-License-Identifier: MIT
import QtQuick
import QtQuick.Controls as Controls
import qs.Commons
import qs.Commons as Commons
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
  property color foreground: Commons.Color.foreground
  property string locale: "en"
  property int maximumLength: 8192
  property bool multiline: false
  property string pasteError: ""
  property string previousText: ""
  property var pasteContext: null
  property bool externalPaste: false
  property bool externalPasteBusy: false
  readonly property bool canPaste: !readOnly && !(externalPaste ? externalPasteBusy : clipboard.running)
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
  function cut() { if (!readOnly) { editSnapshot = snapshot(); editKind = "cut"; editor.cut() } }
  function snapshot() { return {text:text, cursor:cursorPosition, start:selectionStart, end:selectionEnd} }
  function resetHistory() { undoStates = []; redoStates = []; lastEditKind = ""; lastEditAfter = null; editSnapshot = null }
  function remember(before, kind) {
    seedReset.stop()
    if (!before || before.text === text) return
    var now = Date.now()
    var merge = (kind === "typing" || kind === "delete") && kind === lastEditKind
      && now >= lastEditAt && now - lastEditAt < 1500 && lastEditAfter
      && before.text === lastEditAfter.text && before.cursor === lastEditAfter.cursor
      && before.start === before.end
    if (!merge) undoStates = undoStates.concat([before]).slice(-32)
    redoStates = []
    lastEditKind = kind; lastEditAt = now; lastEditAfter = snapshot()
  }
  function restore(value) {
    cancelPaste()
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
    var result = SafeText.insertion(text, value, start, end, maximumLength, multiline)
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
    if (!visible || !enabled || !canPaste) return false
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
    var result = SafeText.insertion(text, value, context.start, context.end, maximumLength, multiline)
    if (!result) { pasteError = "input"; return }
    insertBounded(value, context.start, context.end)
  }
  onVisibleChanged: if (!visible) { cancelPaste(); resetHistory() }
  onEnabledChanged: if (!enabled) cancelPaste()
  onActiveFocusChanged: if (!activeFocus) cancelPaste()
  onTextChanged: if (!reverting) cancelPaste()
  onSelectionStartChanged: if (!reverting && pasteContext) cancelPaste()
  onSelectionEndChanged: if (!reverting && pasteContext) cancelPaste()
  // Programmatic seeds/reopens are not undoable edits of a previous record.
  Timer { id: seedReset; interval: 0; onTriggered: field.resetHistory() }

  BoundedClipboard {
    id: clipboard
    onReceived: function(value) { field.finishPaste(value) }
    onRefused: { field.pasteContext = null; field.pasteError = "clipboard" }
  }
  Controls.Menu {
    id: editMenu
    objectName: "omavlessSafeInputMenu"
    font: field.font
    Controls.MenuItem { text: I18n.translate("common.copy", field.locale); enabled: editor.selectedText.length > 0; onTriggered: editor.copy() }
    Controls.MenuItem { text: I18n.translate("input.cut", field.locale); enabled: !field.readOnly && editor.selectedText.length > 0; onTriggered: field.cut() }
    Controls.MenuItem { objectName: "omavlessSafePaste"; text: I18n.translate("common.paste", field.locale); enabled: field.canPaste; onTriggered: field.requestPaste() }
    Controls.MenuSeparator {}
    Controls.MenuItem { text: I18n.translate("input.undo", field.locale); enabled: field.undoStates.length > 0 && !field.readOnly; onTriggered: field.undo() }
    Controls.MenuItem { text: I18n.translate("input.redo", field.locale); enabled: field.redoStates.length > 0 && !field.readOnly; onTriggered: field.redo() }
    Controls.MenuItem { text: I18n.translate("input.select_all", field.locale); enabled: field.text.length > 0; onTriggered: field.selectAll() }
  }
  Flickable {
    id: viewport
    anchors.fill: parent
    clip: true
    interactive: field.multiline
    boundsBehavior: Flickable.StopAtBounds
    Controls.TextArea.flickable: Controls.TextArea {
      id: editor
      focus: true
      textFormat: TextEdit.PlainText
      wrapMode: field.multiline ? TextEdit.WrapAnywhere : TextEdit.NoWrap
      selectByMouse: true
      persistentSelection: false
      color: field.foreground
      selectionColor: Style.selectionFillFor(field.foreground, Commons.Color.accent)
      selectedTextColor: field.foreground
      placeholderTextColor: Qt.darker(field.foreground, 1.6)
      font.family: Style.font.family
      font.pixelSize: Style.font.body
      leftPadding: Style.spacing.controlPaddingX + Style.normalBorderWidth
      rightPadding: Style.spacing.controlPaddingX + Style.normalBorderWidth + (field.multiline ? Style.space(12) : 0)
      topPadding: Style.spacing.inputPaddingY + Style.normalBorderWidth
      bottomPadding: Style.spacing.inputPaddingY + Style.normalBorderWidth
      // Every activation path uses our bounded Paste, never editor.paste().
      Controls.ContextMenu.menu: editMenu
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
          if (field.multiline && (event.modifiers & Qt.ShiftModifier)) {
            if (!field.readOnly) field.insertBounded("\n", selectionStart, selectionEnd)
          }
          else field.accepted()
        } else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) {
          // Attached navigation on the outer scope precedes TextArea's Tab.
          event.accepted = true
        } else if (event.text && !(event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
                   && !SafeText.insertion(text, event.text, selectionStart, selectionEnd, field.maximumLength, field.multiline)) {
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
        field.cancelPaste()
        if (!SafeText.valid(text, field.maximumLength, field.multiline)) {
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
        field.cancelPaste()
        field.remember(field.editSnapshot, field.editKind || "typing")
        field.editSnapshot = null
        field.pasteError = ""; field.textEdited()
      }
      background: BorderSurface {
        color: Style.controlFill(field.activeFocus, editor.hovered, field.foreground, Commons.Color.accent)
        borderSpec: Border.controlSpec(field.activeFocus ? "focus" : editor.hovered ? "hover-cursor" : "normal", field.foreground, Commons.Color.accent)
        radius: Style.cornerRadius
      }
    }
    Controls.ScrollBar.vertical: Controls.ScrollBar {
      policy: field.multiline ? Controls.ScrollBar.AsNeeded : Controls.ScrollBar.AlwaysOff
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
  Controls.ToolTip.text: I18n.translate(pasteError === "input" ? (multiline ? "input.error.multiline" : "input.error.invalid") : pasteError === "primary" ? "input.error.primary" : "input.error.clipboard", locale)
}
