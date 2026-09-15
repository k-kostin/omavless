// SPDX-License-Identifier: MIT
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "SetupState.js" as SetupState
import "I18n.js" as I18n

// This page must work before /usr/bin/omavless exists. It owns provisioning UI,
// never VPN state. Normal backend commands retain their native-owner guard.
Item {
  id: setup
  property string locale: "en"
  property string state: "checking"
  property bool panelOpen: false
  property bool launching: false
  property bool terminalOpened: false
  readonly property string script: String(Qt.resolvedUrl("setup-runtime.sh")).replace(/^file:\/\//, "")
  readonly property var focusTargets: [installButton, checkButton, retryButton, guideButton, closeButton]
  implicitHeight: content.implicitHeight
  signal ready()
  signal closeRequested()
  function tr(key) { return I18n.translate("setup." + key, locale, {}) }
  function check() { if (!probe.running && !launch.running) probe.running = true }
  function install() {
    if (!SetupState.canInstall(state) || launching || terminalOpened) return
    launching = true
    launch.running = true
  }
  Component.onCompleted: check()
  onPanelOpenChanged: if (panelOpen) check()
  Process {
    id: probe
    command: ["/bin/bash", setup.script, "status"]
    stdout: StdioCollector { id: statusOutput; waitForEnd: true }
    stderr: StdioCollector { waitForEnd: true }
    onExited: function(code) {
      setup.state = SetupState.parse(statusOutput.text, code)
      if (setup.state === "ready") setup.ready()
    }
  }
  Process {
    id: launch
    command: ["omarchy", "launch", "terminal", "/bin/bash", setup.script, "install", setup.locale === "ru" ? "ru" : "en"]
    stdout: StdioCollector { waitForEnd: true }
    stderr: StdioCollector { waitForEnd: true }
    onExited: function(code) {
      setup.launching = false
      setup.terminalOpened = code === 0
      if (code !== 0) setup.state = "needs_attention"
    }
  }
  ColumnLayout {
    id: content
    width: parent.width
    spacing: Style.space(12)
    PlainText { Layout.fillWidth: true; text: "OmaVLESS"; font.family: Style.font.family; font.pixelSize: Style.font.heading; color: Color.foreground }
    PlainText { Layout.fillWidth: true; text: setup.tr("title"); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Color.foreground; wrapMode: Text.Wrap }
    PlainText { Layout.fillWidth: true; text: setup.tr(setup.state); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Color.foreground; wrapMode: Text.Wrap }
    PlainText { Layout.fillWidth: true; text: setup.tr("explanation"); font.family: Style.font.family; font.pixelSize: Style.font.caption; color: Qt.darker(Color.foreground, 1.55); wrapMode: Text.Wrap }
    PlainText { Layout.fillWidth: true; visible: setup.terminalOpened; text: setup.tr("terminal"); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Color.foreground; wrapMode: Text.Wrap }
    Button {
      id: installButton
      Layout.fillWidth: true
      text: setup.tr(setup.state === "needs_activation" ? "prepare" : "install")
      visible: SetupState.canInstall(setup.state)
      bordered: true
      focusable: true
      enabled: SetupState.canInstall(setup.state) && !setup.launching && !setup.terminalOpened && !probe.running
      opacity: enabled ? 1 : 0.45
      onClicked: setup.install()
    }
    Button {
      id: checkButton
      Layout.fillWidth: true
      text: setup.tr("check")
      bordered: true
      focusable: true
      enabled: !probe.running && !setup.launching
      // Rechecking never retries installation or opens another terminal.
      onClicked: setup.check()
    }
    Button {
      id: retryButton
      Layout.fillWidth: true
      visible: setup.terminalOpened
      text: setup.tr("terminal_closed")
      bordered: true
      focusable: true
      // Explicit human acknowledgement, not inference from a detached launcher
      // exiting. This never launches a second installer by itself.
      onClicked: { setup.terminalOpened = false; setup.check() }
    }
    Button {
      id: guideButton
      Layout.fillWidth: true
      text: setup.tr("guide")
      bordered: true
      focusable: true
      onClicked: Qt.openUrlExternally("https://github.com/k-kostin/omavless/blob/main/docs/user/NATIVE_INSTALL.md")
    }
    Button { id: closeButton; Layout.fillWidth: true; text: setup.tr("later"); bordered: true; focusable: true; onClicked: setup.closeRequested() }
  }
}
