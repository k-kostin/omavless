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
  property var baselineInput: null
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
  FloatingWindow {
    id: window
    visible: true
    implicitWidth: 700
    implicitHeight: 500
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
    Product.SubscriptionPrompt { id: subscription; anchors.fill: parent; accepted: true }
    TestCase {
      name: "SafeTextFields"
      when: review.ready
      onCompletedChanged: if (completed) console.log("SAFE_INPUT_RESULT " + JSON.stringify({passed:qtest_results.passCount, failed:qtest_results.failCount, skipped:qtest_results.skipCount}))
      function cleanup() { console.log("SAFE_INPUT_CASE " + qtest_results.functionName + " failed=" + qtest_results.failed) }
      function type(value) { for (var i = 0; i < value.length; i++) keyClick(value.charAt(i)) }
      function init() { subscription.dismiss(); field.text = ""; field.pasteError = ""; field.forceActiveFocus(); wait(30) }
      function test_typing_enter() {
        type("Safe name"); compare(field.text, "Safe name")
        keyClick(Qt.Key_Return); compare(field.text, "Safe name"); compare(field.accepts, 1)
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
      function test_subscription_reveal() {
        subscription.openWith("Synthetic", "https://synthetic.invalid/token")
        wait(30); compare(subscription.urlValue, "https://synthetic.invalid/token")
        subscription.dismiss(); compare(subscription.urlValue, "")
      }
    }
  }
  IpcHandler {
    target: "safeInputReview"
    function inspect(): string { return JSON.stringify({loaded:true, length:field.text.length, error:field.pasteError, focus:field.activeFocus}) }
    function finish() { Qt.quit() }
  }
}
