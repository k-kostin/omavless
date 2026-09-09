// SPDX-License-Identifier: MIT
// Opt-in installed Omarchy Qt test; see R5_NATIVE_MAIN_PANEL.md.
import QtQuick
import QtTest
import "file:///usr/share/omarchy/shell/Ui" as ShellUi

Item {
  width: 400; height: 300
  property int moves: 0
  ShellUi.PanelKeyCatcher {
    id: catcher
    anchors.fill: parent
    blocked: control.activeFocus || search.activeFocus
    onMoveRequested: function(dx, dy) { moves += dy }
    Flickable {
      anchors.fill: parent
      contentHeight: 400
      Keys.onPressed: function(event) {
        if (search.activeFocus) return
        if (event.key === Qt.Key_Down || event.key === Qt.Key_Up) {
          moves += event.key === Qt.Key_Down ? 1 : -1
          catcher.forceActiveFocus()
          event.accepted = true
        }
      }
      Item { id: control; width: 50; height: 50; activeFocusOnTab: true }
      TextInput { id: search; y: 60; width: 200; height: 30; text: "synthetic" }
    }
  }
  TestCase {
    name: "NativeKeyBubbling"
    when: windowShown
    function test_focused_control_then_catcher() {
      moves = 0
      control.forceActiveFocus()
      verify(control.activeFocus)
      keyClick(Qt.Key_Down)
      compare(moves, 1)
      verify(catcher.activeFocus)
      keyClick(Qt.Key_Down)
      compare(moves, 2)
      keyClick(Qt.Key_Up)
      compare(moves, 1)
    }
    function test_search_keeps_arrow_keys() {
      moves = 0
      search.forceActiveFocus()
      keyClick(Qt.Key_Down)
      compare(moves, 0)
      verify(search.activeFocus)
    }
  }
}
