// SPDX-License-Identifier: MIT
// Synthetic, separate-process GUI conformance. No runtime/store/mutations.
import QtQuick
import Quickshell
import Quickshell.Io
import QtTest
import "plugin" as Product

ShellRoot {
  id: review
  property bool ready: false
  readonly property bool visual: Quickshell.env("OMAVLESS_INPUT_VISUAL") === "1"
  property string captureResult: "idle"
  property string visualKind: ""
  property int visualWidth: 460
  property int visualHeight: 520
  property var baselineInput: null
  property string modelText: ""
  property var lateDialog: null
  function find(item, name) {
    if (item.objectName === name) return item
    var children = item.children || item.data || []
    for (var i = 0; i < children.length; i++) { var result = find(children[i], name); if (result) return result }
    return null
  }
  Timer {
    interval: 200; running: true
    onTriggered: {
      if (Quickshell.env("OMAVLESS_PASSIVE_PROBE") === "1") {
        subscription.openWith("Synthetic", "https://synthetic.invalid/token")
        if (Quickshell.env("OMAVLESS_PROBE_BASELINE") === "1")
          review.baselineInput = Qt.createQmlObject('import QtQuick.Controls; TextField { objectName: "unboundedBaseline"; text: "baseline" }', window.contentItem)
        console.log("SAFE_BASELINE_CREATED " + !!review.baselineInput)
      } else review.ready = true
    }
  }
  Timer {
    interval: 1100
    running: Quickshell.env("OMAVLESS_PROBE_LATE") === "1"
    onTriggered: {
      review.lateDialog = lateDialogComponent.createObject(window.contentItem)
      review.lateDialog.openWith("Late fixture", "https://synthetic.invalid/late")
    }
  }
  Component { id: lateDialogComponent; Product.SubscriptionPrompt { width:700; height:500; accepted:true } }
  Component {
    id: narrowSubscriptionComponent
    Product.SubscriptionPrompt {
      width: 360; height: 520; accepted: true
      hint: textFor("native.subscription.private")
    }
  }
  FloatingWindow {
    id: window
    visible: true
    implicitWidth: 700
    implicitHeight: review.visual ? 700 : 500
    color: "#20202e"
    Product.SafeTextField {
      id: field
      x: 30; y: 30; width: 400
      maximumLength: 80
      property int accepts: 0
      property int edits: 0
      onAccepted: accepts++
      onTextEdited: edits++
      KeyNavigation.tab: second
      KeyNavigation.backtab: second
    }
    Product.SafeTextField { id: second; x: 30; y: 100; width: 400; KeyNavigation.tab: field; KeyNavigation.backtab: field }
    Product.SafeTextField { id: bound; x: 30; y: 170; width: 400; text: review.modelText; onTextChanged: review.modelText = text }
    Product.SubscriptionPrompt { id: subscription; anchors.fill: parent; accepted: true }
    Item { id: visualCanvas; width: review.visualWidth; height: review.visualHeight }
    Product.ImportEntryPrompt {
      id: importEntry
      parent: review.visual ? visualCanvas : window.contentItem
      anchors.fill: parent
      available: true
      property int submissions: 0
      property int cancellations: 0
      onSubmitted: function(value) { submissions++; dismiss() }
      onCanceled: { cancellations++; dismiss() }
    }
    Product.BundleImportPrompt {
      id: bundlePrompt
      parent: review.visual ? visualCanvas : window.contentItem
      anchors.fill: parent
      available: true
      counts: ({accepted:2, duplicates:1, skipped:1})
      property int confirmations: 0
      property int cancellations: 0
      onConfirmed: { confirmations++; dismiss() }
      onCanceled: { cancellations++; dismiss() }
    }
    TestCase {
      name: "SafeTextFields"
      when: review.ready && !review.visual
      onCompletedChanged: if (completed) console.log("SAFE_INPUT_RESULT " + JSON.stringify({passed:qtest_results.passCount, failed:qtest_results.failCount, skipped:qtest_results.skipCount}))
      function cleanup() { console.log("SAFE_INPUT_CASE " + qtest_results.functionName + " failed=" + qtest_results.failed) }
      function type(value) { for (var i = 0; i < value.length; i++) keyClick(value.charAt(i)) }
      function init() { bundlePrompt.dismiss(); importEntry.dismiss(); importEntry.available = true; subscription.dismiss(); field.text = ""; field.pasteError = ""; field.forceActiveFocus(); wait(30) }
      function test_import_unavailable_keeps_cancel_and_blocks_review() {
        importEntry.open(); wait(30)
        var editor = findChild(importEntry, "omavlessImportEntry")
        var reviewButton = findChild(importEntry, "omavlessImportReview")
        var cancel = findChild(importEntry, "omavlessImportCancel")
        editor.text = "synthetic fixture"; importEntry.available = false
        verify(!reviewButton.enabled); compare(reviewButton.opacity, 0.45)
        verify(findChild(importEntry, "omavlessImportUnavailable").visible)
        verify(cancel.enabled)
        var before = importEntry.submissions
        editor.forceActiveFocus(); keyClick(Qt.Key_Return)
        compare(importEntry.submissions, before)
        keyClick(Qt.Key_Escape); verify(!importEntry.visible); compare(editor.text, "")
      }
      function test_import_multiline_and_undo() {
        importEntry.open(); wait(30)
        var editor = findChild(importEntry, "omavlessImportEntry")
        type("first"); keyClick(Qt.Key_Return, Qt.ShiftModifier); type("second")
        compare(editor.text, "first\nsecond")
        editor.selectAll(); verify(editor.insertBounded("wrapped\nBase64", 0, editor.text.length))
        compare(editor.text, "wrapped\nBase64")
        editor.undo(); compare(editor.text, "first\nsecond")
        editor.readOnly = true; keyClick(Qt.Key_Return, Qt.ShiftModifier)
        compare(editor.text, "first\nsecond"); editor.readOnly = false
        importEntry.dismiss(); compare(editor.text, ""); compare(editor.undoStates.length, 0)
        compare(field.multiline, false)
      }
      function test_bundle_confirmation_navigation_cancel_and_no_new() {
        bundlePrompt.counts = {accepted:2,duplicates:1,skipped:1}
        bundlePrompt.open(); wait(30)
        var cancel = findChild(bundlePrompt, "omavlessBundleCancel")
        var save = findChild(bundlePrompt, "omavlessBundleSave")
        verify(cancel.activeFocus); verify(save.enabled)
        var before = bundlePrompt.confirmations
        keyClick(Qt.Key_Tab); verify(save.activeFocus)
        keyClick(Qt.Key_Return); compare(bundlePrompt.confirmations, before + 1)
        bundlePrompt.counts = {accepted:0,duplicates:3,skipped:0}
        bundlePrompt.open(); wait(30); verify(!save.enabled)
        before = bundlePrompt.cancellations
        keyClick(Qt.Key_Escape); compare(bundlePrompt.cancellations, before + 1)
        compare(bundlePrompt.visible, false)
      }
      function test_import_entry_typing_review_clear() {
        importEntry.open(); wait(30)
        var editor = findChild(importEntry, "omavlessImportEntry")
        verify(editor !== null); verify(editor.activeFocus)
        compare(importEntry.revealed, false); compare(editor.opacity, 0)
        var before = importEntry.submissions
        type("https://synthetic.invalid/token")
        compare(editor.text, "https://synthetic.invalid/token")
        keyClick(Qt.Key_Return)
        compare(importEntry.submissions, before + 1)
        compare(importEntry.visible, false); compare(editor.text, "")
        compare(editor.previousText, ""); compare(editor.undoStates.length, 0)
        importEntry.open(); wait(30); compare(editor.text, "")
        compare(importEntry.revealed, false); compare(editor.opacity, 0)
      }
      function test_import_entry_cancel_navigation() {
        importEntry.open(); wait(30)
        var editor = findChild(importEntry, "omavlessImportEntry")
        type("private-fixture")
        var paste = findChild(importEntry, "omavlessImportPaste")
        var reveal = findChild(importEntry, "omavlessImportReveal")
        keyClick(Qt.Key_Tab); verify(reveal.activeFocus, "Tab reaches Show input")
        keyClick(Qt.Key_Return); compare(importEntry.revealed, true)
        compare(editor.opacity, 1); verify(editor.activeFocus)
        keyClick(Qt.Key_Tab); verify(reveal.activeFocus)
        keyClick(Qt.Key_Tab); verify(paste.activeFocus, "Tab reaches Paste")
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier); verify(reveal.activeFocus)
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier); verify(editor.activeFocus, "Shift+Tab returns to entry")
        var before = importEntry.cancellations
        keyClick(Qt.Key_Escape)
        compare(importEntry.cancellations, before + 1)
        compare(importEntry.visible, false); compare(editor.text, "")
        compare(editor.undoStates.length, 0); compare(editor.redoStates.length, 0)
        compare(importEntry.revealed, false); compare(editor.opacity, 0)
      }
      function test_import_entry_explicit_paste() {
        importEntry.open(); wait(30)
        var editor = findChild(importEntry, "omavlessImportEntry")
        keyClick(Qt.Key_V, Qt.ControlModifier)
        if (Quickshell.env("OMAVLESS_FIELD_CLIPBOARD_MODE") === "normal")
          tryCompare(editor, "text", "replacement", 6500)
        else {
          tryCompare(editor, "pasteError", "clipboard", 6500)
          compare(editor.text, "")
        }
        compare(importEntry.visible, true)
        importEntry.dismiss(); compare(editor.text, ""); compare(editor.pasteContext, null)
      }
      function test_import_entry_reverse_from_editor_data() {
        var rows = []
        for (var key of [Qt.Key_Tab, Qt.Key_Backtab])
          for (var state of ["empty", "unavailable", "review"])
            for (var revealed of [false, true])
              rows.push({tag:key + "-" + state + "-" + revealed,
                key:key, state:state, revealed:revealed})
        return rows
      }
      function test_import_entry_reverse_from_editor(data) {
        importEntry.open(); wait(30)
        var editor = findChild(importEntry, "omavlessImportEntry")
        var cancel = findChild(importEntry, "omavlessImportCancel")
        var reviewButton = findChild(importEntry, "omavlessImportReview")
        if (data.state !== "empty") type("synthetic-cancel-only")
        importEntry.available = data.state !== "unavailable"
        importEntry.revealed = data.revealed
        editor.forceActiveFocus(); verify(editor.activeFocus)
        var submissions = importEntry.submissions
        var cancellations = importEntry.cancellations
        keyClick(data.key, Qt.ShiftModifier)
        if (data.state === "review") {
          verify(reviewButton.activeFocus, "reverse reaches enabled Review before activation")
          keyClick(Qt.Key_Escape)
        } else {
          verify(cancel.activeFocus, "reverse reaches Cancel before activation")
          keyClick(Qt.Key_Return)
        }
        compare(importEntry.cancellations, cancellations + 1)
        compare(importEntry.submissions, submissions)
        compare(importEntry.visible, false); compare(editor.text, "")
        compare(editor.previousText, ""); compare(editor.undoStates.length, 0)
        compare(editor.redoStates.length, 0); compare(importEntry.revealed, false)
      }
      function test_typing_enter() {
        type("Safe name"); compare(field.text, "Safe name")
        keyClick(Qt.Key_Return); compare(field.text, "Safe name"); compare(field.accepts, 1)
      }
      function test_actual_paste() {
        if (Quickshell.env("OMAVLESS_FIELD_CLIPBOARD_MODE") !== "normal") return
        keyClick(Qt.Key_V, Qt.ControlModifier)
        tryCompare(field, "text", "replacement", 6500)
      }
      function test_helper_refusal() {
        var mode = Quickshell.env("OMAVLESS_FIELD_CLIPBOARD_MODE")
        if (mode !== "oversized" && mode !== "slow") return
        field.text = "unchanged"; field.selectAll(); keyClick(Qt.Key_Insert, Qt.ShiftModifier)
        tryCompare(field, "pasteError", "clipboard", 6500)
        compare(field.text, "unchanged"); verify(field.activeFocus)
      }
      function test_menu_paste() {
        if (Quickshell.env("OMAVLESS_FIELD_CLIPBOARD_MODE") !== "normal") return
        var menu = findChild(field, "omavlessSafeInputMenu")
        var paste = findChild(field, "omavlessSafePaste")
        console.log("SAFE_MENU_LOOKUP " + !!menu + " " + !!paste)
        verify(menu !== null && paste !== null)
        menu.popup(); wait(30); paste.triggered(); menu.close()
        tryCompare(field, "text", "replacement", 6500)
      }
      function test_selection_and_bounds() {
        field.text = "abc"; field.selectAll(); type("Y"); compare(field.text, "Y")
        field.text = "Иван"; compare(field.text, "Иван")
        field.text = "x".repeat(80); type("z"); compare(field.text.length, 80)
        field.text = "<b>plain</b>"; compare(field.text, "<b>plain</b>")
      }
      function test_tab() {
        keyClick(Qt.Key_Tab); verify(second.activeFocus)
        keyClick(Qt.Key_Tab, Qt.ShiftModifier); verify(field.activeFocus)
      }
      function test_late_paste() {
        field.text = "abc"; field.selectAll()
        field.pasteContext = {text:"abc", start:0, end:3, cursor:3}
        second.forceActiveFocus(); field.finishPaste("discarded")
        compare(field.text, "abc")
      }
      function test_paste_undo() {
        type("abc"); field.select(1, 2)
        field.pasteContext = {text:field.text, start:field.selectionStart, end:field.selectionEnd, cursor:field.cursorPosition}
        field.finishPaste("X"); compare(field.text, "aXc")
        keyClick(Qt.Key_Z, Qt.ControlModifier)
        compare(field.text, "abc")
        keyClick(Qt.Key_Z, Qt.ControlModifier | Qt.ShiftModifier); compare(field.text, "aXc")
      }
      function test_append_undo() {
        type("abc")
        field.pasteContext = {text:field.text, start:field.selectionStart, end:field.selectionEnd, cursor:field.cursorPosition}
        field.finishPaste("X"); compare(field.text, "abcX")
        keyClick(Qt.Key_Z, Qt.ControlModifier)
        compare(field.text, "abc")
      }
      function test_binding_survives_paste() {
        review.modelText = "abc"; wait(20); bound.forceActiveFocus(); bound.selectAll()
        bound.pasteContext = {text:bound.text, start:bound.selectionStart, end:bound.selectionEnd, cursor:bound.cursorPosition}
        bound.finishPaste("new"); compare(review.modelText, "new")
        review.modelText = ""; wait(20); compare(bound.text, "")
        review.modelText = "next"; wait(20); compare(bound.text, "next")
      }
      function test_excess_typing_keeps_undo() {
        type("abc"); field.maximumLength = 3; type("x"); compare(field.text, "abc")
        keyClick(Qt.Key_Z, Qt.ControlModifier); compare(field.text, "")
        field.maximumLength = 80
      }
      function test_subscription_reveal() {
        subscription.openWith("Synthetic", "https://synthetic.invalid/token")
        wait(30); compare(subscription.urlValue, "https://synthetic.invalid/token")
        subscription.dismiss(); compare(subscription.urlValue, "")
      }
      function test_subscription_private_hint_uses_full_width() {
        var prompt = createTemporaryObject(narrowSubscriptionComponent, window.contentItem)
        verify(prompt !== null)
        for (var locale of ["en", "ru"]) for (var width of [360, 460]) {
          prompt.locale = locale; prompt.width = width
          prompt.openWith("Synthetic", "https://synthetic.invalid/token"); wait(30)
          var hint = review.find(prompt, "omavlessSubscriptionPrivateHint")
          verify(hint !== null); verify(hint.visible)
          verify(hint.width >= width - 100)
          verify(hint.lineCount <= 4)
          var url = review.find(prompt, "omavlessMaskedSubscriptionUrl")
          compare(url.readOnly, true); compare(url.echoMode, TextInput.Password)
          prompt.dismiss(); compare(prompt.urlValue, "")
        }
      }
      function test_subscription_reopen_binding() {
        subscription.openWith("Synthetic", "https://synthetic.invalid/first"); wait(30)
        var url = review.find(subscription, "omavlessSubscriptionUrl")
        var editor = review.find(subscription, "omavlessSubscriptionUrlEditor")
        var hidden = review.find(subscription, "omavlessMaskedSubscriptionUrl")
        verify(url !== null && editor !== null && hidden !== null)
        compare(hidden.readOnly, true); compare(hidden.echoMode, TextInput.Password)
        // Exercise the document mutation used by revealed paste without ever
        // asking the real Wayland clipboard for data.
        editor.insertBounded("https://synthetic.invalid/changed", 0, editor.text.length)
        compare(subscription.urlValue, "https://synthetic.invalid/changed")
        subscription.dismiss(); subscription.openWith("New", "https://synthetic.invalid/next"); wait(30)
        compare(editor.text, "https://synthetic.invalid/next")
        compare(hidden.readOnly, true)
      }
    }
  }
  IpcHandler {
    target: "safeInputReview"
    // Rendering only: fixed invented text/counts, no classifier/store/action.
    function visualScene(locale: string, kind: string, width: int, height: int): string {
      if (!review.visual || ["en", "ru"].indexOf(locale) < 0
          || [360, 460].indexOf(width) < 0 || [400, 520, 600].indexOf(height) < 0
          || ["empty", "masked", "revealed", "blocked", "bundle", "no-new", "stale"].indexOf(kind) < 0)
        return "refused"
      importEntry.dismiss(); bundlePrompt.dismiss(); subscription.dismiss()
      // Fixed backing window; bind the actual QML canvas rather than claiming
      // that an offscreen platform honored a native window-resize request.
      review.visualWidth = width; review.visualHeight = height
      importEntry.locale = locale; bundlePrompt.locale = locale
      importEntry.available = kind !== "blocked"
      bundlePrompt.available = kind !== "stale"
      bundlePrompt.errorText = kind === "stale" ? importEntry.tr("native.importError.stale") : ""
      if (["bundle", "no-new", "stale"].indexOf(kind) >= 0) {
        bundlePrompt.counts = {accepted:kind === "no-new" ? 0 : 37, duplicates:3, skipped:2}
        bundlePrompt.open()
      } else {
        importEntry.open()
        var editor = review.find(importEntry, "omavlessImportEntry")
        if (kind !== "empty") editor.text = "trojan://synthetic-not-a-real-key@fixture.invalid:443#Synthetic"
        importEntry.revealed = kind === "revealed"
      }
      review.visualKind = kind
      return "ready"
    }
    function visualInspect(): string {
      if (!review.visual) return "refused"
      var prompt = bundlePrompt.visible ? bundlePrompt : importEntry
      var ids = bundlePrompt.visible ? ["omavlessBundleCancel", "omavlessBundleSave"]
        : ["omavlessImportEntry", "omavlessImportReveal", "omavlessImportPaste", "omavlessImportCancel", "omavlessImportReview"]
      return JSON.stringify({kind:review.visualKind, width:visualCanvas.width, height:visualCanvas.height,
        masked:importEntry.visible && !importEntry.revealed,
        controls:ids.map(function(id) {
          var item = review.find(prompt, id)
          var point = item ? item.mapToItem(visualCanvas, 0, 0) : {x:-1,y:-1}
          return {id:id, present:!!item, enabled:!!(item && item.enabled), focus:!!(item && item.activeFocus),
            x:point.x, y:point.y, width:item ? item.width : 0, height:item ? item.height : 0}
        })})
    }
    function visualCapture(slug: string): string {
      if (!review.visual || !/^[a-z0-9-]{1,60}$/.test(slug)) return "refused"
      review.captureResult = "capturing"
      var surface = bundlePrompt.visible ? bundlePrompt : importEntry
      surface.grabToImage(function(image) {
        review.captureResult = image.saveToFile(Quickshell.env("OMAVLESS_CAPTURE_DIR") + "/" + slug + ".png") ? "captured" : "failed"
      })
      return "started"
    }
    function visualResult(): string { return review.captureResult }
    function inspect(): string { return JSON.stringify({loaded:true, length:field.text.length, error:field.pasteError, focus:field.activeFocus}) }
    function finish() { Qt.quit() }
  }
}
