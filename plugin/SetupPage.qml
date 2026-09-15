// SPDX-License-Identifier: MIT
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "SetupState.js" as SetupState
import "I18n.js" as I18n

// Runtime-independent panel shell and shared provisioning controller. Never a
// modal onboarding step. The card remains on reopen until the facts change.
Item {
  id: setup
  property string locale: "en"
  property var facts: ({state:"checking", coreInstalled:null})
  readonly property string state: facts.state
  readonly property bool needsAttention: SetupState.needsAttention(facts)
  readonly property bool coreMissing: facts.coreInstalled === false
  property bool panelOpen: false
  property bool launching: false
  property bool terminalOpened: false
  property string launchAction: "install"
  readonly property bool busy: probe.running || launching
  readonly property string script: String(Qt.resolvedUrl("setup-runtime.sh")).replace(/^file:\/\//, "")
  readonly property var focusTargets: requirements.focusTargets.concat([closeButton])
  implicitHeight: content.implicitHeight
  signal ready()
  signal closeRequested()
  function tr(key) { return I18n.translate("setup." + key, locale, {}) }
  function check() { if (!probe.running && !launch.running) probe.running = true }
  function acknowledgeTerminalClosed() { terminalOpened = false; check() }
  function guide() { Qt.openUrlExternally("https://github.com/k-kostin/omavless/blob/main/docs/user/NATIVE_INSTALL.md") }
  function install(action) {
    var allowed = action === SetupState.missingAction(facts) && action !== ""
      || action === "install" && facts.state === "needs_activation" && facts.coreInstalled === true
    if (!allowed || busy || launching || terminalOpened) return
    launchAction = action
    launching = true
    launch.running = true
  }
  Component.onCompleted: check()
  onPanelOpenChanged: if (panelOpen) check()
  Process {
    id: probe
    command: ["/bin/bash", setup.script, "components"]
    stdout: StdioCollector { id: statusOutput; waitForEnd: true }
    stderr: StdioCollector { waitForEnd: true }
    onExited: function(code) {
      setup.facts = SetupState.inventory(statusOutput.text, code)
      if (setup.state === "ready") setup.ready()
    }
  }
  Process {
    id: launch
    command: ["omarchy", "launch", "terminal", "/bin/bash", setup.script, setup.launchAction, setup.locale === "ru" ? "ru" : "en"]
    stdout: StdioCollector { waitForEnd: true }
    stderr: StdioCollector { waitForEnd: true }
    onExited: function(code) {
      setup.launching = false
      setup.terminalOpened = code === 0
      if (code !== 0) setup.facts = {state:"needs_attention", coreInstalled:setup.facts.coreInstalled}
    }
  }
  ColumnLayout {
    id: content
    width: parent.width
    spacing: Style.space(12)
    PlainText { Layout.fillWidth: true; text: "OmaVLESS"; font.family: Style.font.family; font.pixelSize: Style.font.title; color: Color.foreground }
    PlainText { Layout.fillWidth: true; text: setup.tr("panel_unavailable"); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Color.foreground; wrapMode: Text.Wrap }
    BorderSurface {
      Layout.fillWidth: true
      implicitHeight: profilesPlaceholder.implicitHeight + Style.space(20)
      color: "transparent"; radius: 0
      borderSpec: Border.flat(Util.alpha(Color.foreground, 0.38), Style.normalBorderWidth)
      ColumnLayout {
        id: profilesPlaceholder
        anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
        anchors.margins: Style.space(10)
        spacing: Style.space(8)
        PlainText { Layout.fillWidth: true; text: I18n.translate("profiles.title", setup.locale, {}); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Color.foreground }
        PlainText { Layout.fillWidth: true; text: setup.tr("profiles_unavailable"); font.family: Style.font.family; font.pixelSize: Style.font.caption; color: Qt.darker(Color.foreground, 1.55); wrapMode: Text.Wrap }
      }
    }
    RequiredComponents {
      id: requirements
      Layout.fillWidth: true
      locale: setup.locale; facts: setup.facts; busy: setup.busy; terminalOpened: setup.terminalOpened
      onInstallRequested: function(action) { setup.install(action) }
      onCheckRequested: setup.check()
      onTerminalClosed: setup.acknowledgeTerminalClosed()
      onGuideRequested: setup.guide()
    }
    Button { id: closeButton; Layout.fillWidth: true; text: setup.tr("later"); bordered: true; focusable: true; onClicked: setup.closeRequested() }
  }
}
