// SPDX-License-Identifier: MIT
import QtQuick
import QtQuick.Controls
import Quickshell.Io
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "I18n.js" as I18n

// Public, runtime-independent help. No profile/report/clipboard reads, native
// commands or automatic external navigation. Copy only the public template.
Item {
  id: help
  property string locale: "en"
  property string guideSection: "help"
  property string copyState: ""
  property bool requestExpanded: false
  property string pendingText: ""
  property int generation: 0
  property int copyGeneration: 0
  readonly property string agentPrompt: tr("agent_prompt")
  readonly property var focusTargets: [copyButton, requestToggle, requestText, guideButton, issueButton, closeButton].filter(function(control) { return control.visible && control.enabled })
  readonly property alias flickable: helpScroll
  signal canceled()
  signal guideRequested(string section)
  signal issueRequested()
  visible: false
  focus: visible

  function tr(key) { return I18n.translate("help." + key, locale, {}) }
  function openHelp(section) {
    generation++
    guideSection = ["install", "updates", "help"].indexOf(section) >= 0 ? section : "help"
    copyState = ""
    requestExpanded = false
    requestText.deselect()
    helpScroll.contentY = 0
    visible = true
    Qt.callLater(function() { if (help.visible) copyButton.forceActiveFocus() })
  }
  function dismiss() { generation++; visible = false }
  function copyRequest() {
    if (!visible || clipboardWriter.running) return false
    pendingText = agentPrompt
    copyGeneration = generation
    copyState = "copying"
    clipboardWriter.stdinEnabled = true
    clipboardWriter.running = true
    return true
  }
  function finishCopy(code) {
    if (!visible || copyGeneration !== generation) return
    copyState = code === 0 ? "copied" : "copy_failed"
    if (code !== 0) requestExpanded = true
    // Disclosure can move a lower action out of view. Reveal the currently
    // focused control after layout without moving focus to another action.
    for (var i = 0; i < focusTargets.length; i++) {
      if (focusTargets[i].activeFocus) { reveal(focusTargets[i]); break }
    }
  }
  function toggleRequest() {
    if (!visible) return
    if (requestExpanded) {
      if (requestText.activeFocus) requestToggle.forceActiveFocus()
      requestText.deselect()
    }
    requestExpanded = !requestExpanded
    clampScroll()
    reveal(requestToggle)
  }
  function clampScroll() {
    Qt.callLater(function() {
      if (help.visible) helpScroll.contentY = Math.max(0, Math.min(helpScroll.contentY, Math.max(0, helpScroll.contentHeight - helpScroll.height)))
    })
  }
  function openGuide() {
    guideRequested(guideSection)
  }
  function openIssue() { issueRequested() }
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
      if (!help.visible || !control.activeFocus) return
      var top = control.mapToItem(helpScroll.contentItem, 0, 0).y
      var bottom = top + control.height
      var next = helpScroll.contentY
      if (top < next) next = top
      else if (bottom > next + helpScroll.height) next = bottom - helpScroll.height
      helpScroll.contentY = Math.max(0, Math.min(next, Math.max(0, helpScroll.contentHeight - helpScroll.height)))
    })
  }
  Keys.onPressed: function(event) { navigate(event) }

  Process {
    id: clipboardWriter
    command: ["/usr/bin/timeout", "5s", "wl-copy"]
    stdinEnabled: false
    onStarted: {
      write(help.pendingText)
      help.pendingText = ""
      stdinEnabled = false
    }
    onExited: function(code) { help.pendingText = ""; help.finishCopy(code) }
  }

  component HelpButton: Button {
    focusable: true
    width: parent.width
    onActiveFocusChanged: if (activeFocus) help.reveal(this)
    Keys.onPressed: function(event) { help.navigate(event) }
  }
  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Commons.Color.background, 0.82)
    MouseArea { anchors.fill: parent; onClicked: help.canceled() }
    BorderSurface {
      id: card
      width: Math.min(parent.width - Style.space(24), Style.space(440))
      height: Math.min(parent.height - Style.space(24), content.implicitHeight + card.contentTopInset + card.contentBottomInset)
      anchors.centerIn: parent
      color: Commons.Color.background
      borderSpec: Border.flat(Commons.Color.accent, Style.normalBorderWidth)
      padding: Style.space(18)
      radius: Style.cornerRadius
      MouseArea { anchors.fill: parent; onClicked: {} }
      Flickable {
        id: helpScroll
        objectName: "helpScroll"
        anchors.fill: parent
        anchors.topMargin: card.contentTopInset
        anchors.bottomMargin: card.contentBottomInset
        anchors.leftMargin: card.contentLeftInset
        anchors.rightMargin: card.contentRightInset
        contentWidth: width
        contentHeight: content.implicitHeight
        onContentHeightChanged: help.clampScroll()
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height
        ScrollBar.vertical: OmaScrollBar { policy: ScrollBar.AsNeeded }
        Column {
          id: content
          width: Math.max(0, parent.width - Style.space(16))
          spacing: Style.space(10)
          PlainText { width: parent.width; text: help.tr("title"); color: Commons.Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.title; wrapMode: Text.Wrap }
          PlainText { width: parent.width; text: help.tr("assistant"); color: Commons.Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body; wrapMode: Text.Wrap }
          PlainText { width: parent.width; text: help.tr("intro"); color: Qt.darker(Commons.Color.foreground, 1.55); font.family: Style.font.family; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap }
          HelpButton {
            id: copyButton
            objectName: "helpCopyRequest"
            text: help.tr(clipboardWriter.running ? "copying" : "copy_request")
            foreground: Commons.Color.accent
            background: Util.alpha(Commons.Color.accent, 0.08)
            bordered: true
            onClicked: help.copyRequest()
          }
          PlainText { width: parent.width; text: help.copyState === "" || help.copyState === "copying" ? " " : help.tr(help.copyState); color: help.copyState === "copy_failed" ? Commons.Color.urgent : Qt.darker(Commons.Color.foreground, 1.55); font.family: Style.font.family; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap }
          HelpButton { id: requestToggle; objectName: "helpShowRequest"; text: help.tr(help.requestExpanded ? "hide_request" : "show_request"); bordered: false; onClicked: help.toggleRequest() }
          BorderSurface {
            visible: help.requestExpanded
            width: parent.width
            implicitHeight: requestText.contentHeight + Style.space(20)
            color: "transparent"
            borderSpec: Border.flat(requestText.activeFocus ? Commons.Color.accent : Util.alpha(Commons.Color.foreground, 0.28), Style.normalBorderWidth)
            radius: Style.cornerRadius
            TextEdit {
              id: requestText
              objectName: "helpAgentPrompt"
              anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
              anchors.margins: Style.space(10)
              height: contentHeight
              text: help.agentPrompt
              textFormat: TextEdit.PlainText
              readOnly: true
              selectByMouse: true
              activeFocusOnTab: true
              color: Commons.Color.foreground
              selectionColor: Commons.Color.accent
              selectedTextColor: Commons.Color.background
              font.family: Style.font.family
              font.pixelSize: Style.font.caption
              wrapMode: TextEdit.Wrap
              onActiveFocusChanged: if (activeFocus) help.reveal(this)
              Keys.onPressed: function(event) { help.navigate(event) }
            }
          }
          HelpButton { id: guideButton; objectName: "helpUserGuide"; text: help.tr("guide"); bordered: false; onClicked: help.openGuide() }
          PlainText { width: parent.width; text: help.tr("issue_hint"); color: Qt.darker(Commons.Color.foreground, 1.55); font.family: Style.font.family; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap }
          HelpButton { id: issueButton; objectName: "helpIssue"; text: help.tr("issue"); bordered: false; onClicked: help.openIssue() }
          HelpButton { id: closeButton; objectName: "helpClose"; text: I18n.translate("common.close", help.locale, {}); bordered: true; onClicked: help.canceled() }
        }
      }
    }
  }
}
