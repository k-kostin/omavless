// SPDX-License-Identifier: MIT
import QtQuick
import QtQuick.Controls
import Quickshell.Io
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "I18n.js" as I18n

// Local review is independent of native availability. Hiding the sheet is
// never evidence that a terminal, authorization or removal has completed.
Item {
  id: removal
  property string locale: "en"
  property bool nativeEffectBlocked: false
  property bool setupBusy: false
  property bool setupTerminalOpened: false
  property bool terminalOpened: false
  property bool launching: false
  property bool launchFailed: false
  readonly property string script: String(Qt.resolvedUrl("remove-entry.sh")).replace(/^file:\/\//, "")
  readonly property bool canLaunch: visible && blockReason() === ""
  readonly property alias flickable: removeScroll
  readonly property var focusTargets: [launchButton, terminalClosedButton, closeButton].filter(function(control) { return control.visible && control.enabled })
  signal canceled()
  signal terminalRequested()
  signal recheckRequested()
  visible: false
  focus: visible

  function tr(key) { return I18n.translate("remove." + key, locale, {}) }
  function openReview() {
    if (visible) return
    removeScroll.contentY = 0
    visible = true
    Qt.callLater(function() { if (removal.visible) closeButton.forceActiveFocus() })
  }
  function dismiss() { visible = false }
  function blockReason() {
    if (launching || launch.running) return "opening"
    if (terminalOpened) return "terminal"
    if (nativeEffectBlocked) return "native_blocked"
    if (setupTerminalOpened) return "setup_terminal"
    if (setupBusy) return "setup_busy"
    return ""
  }
  function startRemoval() {
    if (!visible || blockReason() !== "") return false
    launchFailed = false
    launching = true
    terminalOpened = true
    launch.running = true
    return true
  }
  function finishLaunch(code) {
    launching = false
    launchFailed = code !== 0
    // No completion signal: a detached terminal may still own authorization.
  }
  function acknowledgeTerminalClosed() {
    if (!visible || !terminalOpened || launching || launch.running) return false
    terminalOpened = false
    recheckRequested()
    return true
  }
  function navigate(event) {
    if (event.key === Qt.Key_Escape) { canceled(); event.accepted = true; return }
    if (event.key !== Qt.Key_Tab && event.key !== Qt.Key_Backtab) return
    if (focusTargets.length === 0) return
    var current = -1
    for (var i = 0; i < focusTargets.length; i++) if (focusTargets[i].activeFocus) current = i
    var direction = event.key === Qt.Key_Backtab || (event.modifiers & Qt.ShiftModifier) ? -1 : 1
    var next = current < 0 ? (direction < 0 ? focusTargets.length - 1 : 0)
      : (current + direction + focusTargets.length) % focusTargets.length
    focusTargets[next].forceActiveFocus()
    event.accepted = true
  }
  function reveal(control) {
    Qt.callLater(function() {
      if (!removal.visible || !control.activeFocus) return
      var top = control.mapToItem(removeScroll.contentItem, 0, 0).y
      var bottom = top + control.height
      var next = removeScroll.contentY
      if (top < next) next = top
      else if (bottom > next + removeScroll.height) next = bottom - removeScroll.height
      removeScroll.contentY = Math.max(0, Math.min(next, Math.max(0, removeScroll.contentHeight - removeScroll.height)))
    })
  }
  Keys.onPressed: function(event) { navigate(event) }
  Process {
    id: launch
    command: ["omarchy", "launch", "terminal", "/bin/bash", removal.script]
    stdout: StdioCollector { waitForEnd: true }
    stderr: StdioCollector { waitForEnd: true }
    onStarted: if (removal.launching && removal.terminalOpened) removal.terminalRequested()
    onExited: function(code) { removal.finishLaunch(code) }
  }
  component ReviewButton: Button {
    focusable: true
    width: parent.width
    onActiveFocusChanged: if (activeFocus) removal.reveal(this)
    Keys.onPressed: function(event) { removal.navigate(event) }
  }
  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Commons.Color.background, 0.82)
    MouseArea { anchors.fill: parent; onClicked: removal.canceled() }
    BorderSurface {
      id: card
      width: Math.min(parent.width - Style.space(24), Style.space(440))
      height: Math.min(parent.height - Style.space(24), content.implicitHeight + card.contentTopInset + card.contentBottomInset)
      anchors.centerIn: parent
      color: Commons.Color.background
      borderSpec: Border.flat(Commons.Color.urgent, Style.normalBorderWidth)
      padding: Style.space(18)
      radius: Style.cornerRadius
      MouseArea { anchors.fill: parent; onClicked: {} }
      Flickable {
        id: removeScroll
        objectName: "removeScroll"
        anchors.fill: parent
        anchors.topMargin: card.contentTopInset
        anchors.bottomMargin: card.contentBottomInset
        anchors.leftMargin: card.contentLeftInset
        anchors.rightMargin: card.contentRightInset
        contentWidth: width
        contentHeight: content.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height
        ScrollBar.vertical: OmaScrollBar { policy: ScrollBar.AsNeeded }
        Column {
          id: content
          width: Math.max(0, parent.width - Style.space(16))
          spacing: Style.space(12)
          PlainText { width: parent.width; text: removal.tr("title"); color: Commons.Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.title; wrapMode: Text.Wrap }
          PlainText { width: parent.width; text: removal.tr("scope"); color: Commons.Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body; wrapMode: Text.Wrap }
          PlainText { width: parent.width; text: removal.tr("data"); color: Commons.Color.urgent; font.family: Style.font.family; font.pixelSize: Style.font.body; wrapMode: Text.Wrap }
          PlainText { width: parent.width; text: removal.tr("kept"); color: Qt.darker(Commons.Color.foreground, 1.55); font.family: Style.font.family; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap }
          PlainText { width: parent.width; text: removal.tr("continuation"); color: Qt.darker(Commons.Color.foreground, 1.55); font.family: Style.font.family; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap }
          PlainText {
            width: parent.width
            visible: removal.blockReason() !== ""
            text: visible ? removal.tr(removal.blockReason()) : ""
            color: Qt.darker(Commons.Color.foreground, 1.55)
            font.family: Style.font.family; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap
          }
          PlainText {
            width: parent.width
            visible: removal.launchFailed && removal.terminalOpened
            text: removal.tr("launch_failed")
            color: Commons.Color.urgent
            font.family: Style.font.family; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap
          }
          ReviewButton {
            id: launchButton
            objectName: "removeLaunch"
            visible: !removal.terminalOpened
            enabled: removal.canLaunch
            text: removal.tr("launch")
            foreground: enabled ? Commons.Color.urgent : Qt.darker(Commons.Color.foreground, 1.55)
            bordered: true
            onClicked: removal.startRemoval()
          }
          ReviewButton {
            id: terminalClosedButton
            objectName: "removeTerminalClosed"
            visible: removal.terminalOpened
            enabled: !removal.launching && !launch.running
            text: I18n.translate("setup.terminal_closed", removal.locale, {})
            bordered: true
            onClicked: removal.acknowledgeTerminalClosed()
          }
          ReviewButton {
            id: closeButton
            objectName: "removeClose"
            text: I18n.translate(removal.terminalOpened ? "common.close" : "common.back", removal.locale, {})
            bordered: true
            onClicked: removal.canceled()
          }
        }
      }
    }
  }
}
