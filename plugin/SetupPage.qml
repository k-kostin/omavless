// SPDX-License-Identifier: MIT
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Commons as Commons
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
  readonly property bool appMissing: SetupState.appMissing(facts)
  readonly property bool needsAttention: SetupState.needsAttention(facts)
  readonly property bool coreMissing: facts.coreInstalled === false
  property bool panelOpen: false
  property bool componentReview: false
  property string reviewCoreState: "attention"
  property bool launching: false
  property bool terminalOpened: false
  property string launchAction: "install"
  property bool nativeEffectBlocked: false
  property bool probing: true
  readonly property bool busy: probing || probe.running || launching || startApp.running
  readonly property bool reviewReady: setup.state === "ready" && setup.reviewCoreState === "ready"
    && !setup.busy && !setup.terminalOpened && !setup.nativeEffectBlocked
  readonly property string script: String(Qt.resolvedUrl("setup-runtime.sh")).replace(/^file:\/\//, "")
  readonly property var focusTargets: requirements.focusTargets.concat([reviewTerminalButton, reviewCheckButton, reviewGuideButton, closeButton])
  implicitHeight: content.implicitHeight
  signal ready()
  signal closeRequested()
  signal factsRefreshRequested()
  signal helpRequested(string section)
  function tr(key) { return I18n.translate("setup." + key, locale, {}) }
  function check() {
    if (!probe.running && !launch.running) {
      probing = true
      probe.running = true
    }
  }
  function acknowledgeTerminalClosed() { terminalOpened = false; check() }
  function guide() { helpRequested(SetupState.hasUpdate(facts) ? "updates" : "install") }
  function install(action) {
    var allowed = SetupState.canRunAction(facts, action)
    if (!allowed || busy || launching || terminalOpened) return
    if (action === "update-help") { guide(); return }
    if (nativeEffectBlocked) return
    if (action === "start-app") { startApp.running = true; return }
    launchAction = action
    launching = true
    launch.running = true
  }
  Component.onCompleted: check()
  Process {
    id: startApp
    command: ["/usr/bin/omavless", "app", "start"]
    stdout: StdioCollector { id: startOutput; waitForEnd: true }
    stderr: StdioCollector { waitForEnd: true }
    onExited: function(code) {
      if (code === 0 && startOutput.text === "ready\n") setup.check()
      else setup.facts = {state:"needs_attention", coreInstalled:setup.facts.coreInstalled}
    }
  }
  onPanelOpenChanged: if (panelOpen) check()
  Process {
    id: probe
    command: ["/bin/bash", setup.script, "components"]
    stdout: StdioCollector { id: statusOutput; waitForEnd: true }
    stderr: StdioCollector { waitForEnd: true }
    onExited: function(code) {
      setup.facts = SetupState.inventory(statusOutput.text, code)
      setup.probing = false
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
    PlainText { Layout.fillWidth: true; text: "OmaVLESS"; font.family: Style.font.family; font.pixelSize: Style.font.title; color: Commons.Color.foreground }
    PlainText { Layout.fillWidth: true; text: setup.tr(setup.probing ? "checking" : setup.componentReview ? "review_components" : "panel_unavailable"); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Commons.Color.foreground; wrapMode: Text.Wrap }
    PlainText {
      objectName: "setupOtherVpnAdvice"
      Layout.fillWidth: true
      visible: !setup.probing && !setup.componentReview && !setup.terminalOpened
        && (setup.state === "needs_package" || setup.state === "needs_activation")
      text: setup.tr("other_vpn_advice")
      font.family: Style.font.family; font.pixelSize: Style.font.caption
      color: Qt.darker(Commons.Color.foreground, 1.55); wrapMode: Text.Wrap
    }
    BorderSurface {
      Layout.fillWidth: true
      implicitHeight: profilesPlaceholder.implicitHeight + Style.space(20)
      visible: !setup.probing && !setup.componentReview
      color: "transparent"; radius: 0
      borderSpec: Border.flat(Util.alpha(Commons.Color.foreground, 0.38), Style.normalBorderWidth)
      ColumnLayout {
        id: profilesPlaceholder
        anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
        anchors.margins: Style.space(10)
        spacing: Style.space(8)
        PlainText { Layout.fillWidth: true; text: I18n.translate("profiles.title", setup.locale, {}); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Commons.Color.foreground }
        PlainText { Layout.fillWidth: true; text: setup.tr("profiles_unavailable"); font.family: Style.font.family; font.pixelSize: Style.font.caption; color: Qt.darker(Commons.Color.foreground, 1.55); wrapMode: Text.Wrap }
      }
    }
    RequiredComponents {
      id: requirements
      Layout.fillWidth: true
      locale: setup.locale; facts: setup.facts; busy: setup.busy; effectBlocked: setup.nativeEffectBlocked; terminalOpened: setup.terminalOpened
      busyMessage: startApp.running ? "starting_app" : setup.launching ? "opening_terminal" : "checking"
      onInstallRequested: function(action) { setup.install(action) }
      onCheckRequested: { setup.check(); setup.factsRefreshRequested() }
      onTerminalClosed: setup.acknowledgeTerminalClosed()
      onGuideRequested: setup.guide()
    }
    PlainText {
      Layout.fillWidth: true
      visible: setup.componentReview && !setup.needsAttention
      text: I18n.translate("native.onboarding.core_" + (setup.terminalOpened ? "terminal" : setup.reviewCoreState), setup.locale, {})
      font.family: Style.font.family; font.pixelSize: Style.font.body; color: Commons.Color.foreground; wrapMode: Text.Wrap
    }
    PlainText {
      Layout.fillWidth: true
      visible: setup.componentReview && !setup.needsAttention
        && (setup.terminalOpened || (setup.reviewCoreState !== "ready" && setup.reviewCoreState !== "checking"))
      text: setup.tr(setup.terminalOpened ? "terminal" : "review_core_attention")
      font.family: Style.font.family; font.pixelSize: Style.font.caption; color: Commons.Color.foreground; wrapMode: Text.Wrap
    }
    Button {
      id: reviewTerminalButton
      Layout.fillWidth: true
      visible: setup.componentReview && !setup.needsAttention && setup.terminalOpened
      text: setup.tr("terminal_closed"); bordered: true; focusable: true
      onClicked: { setup.acknowledgeTerminalClosed(); setup.factsRefreshRequested() }
    }
    Button {
      id: reviewCheckButton
      Layout.fillWidth: true
      visible: setup.componentReview && !setup.needsAttention && !setup.reviewReady
      enabled: !setup.busy
      text: setup.tr("check"); bordered: true; focusable: true
      onClicked: { setup.check(); setup.factsRefreshRequested() }
    }
    Button {
      id: reviewGuideButton
      Layout.fillWidth: true
      visible: setup.componentReview && !setup.needsAttention && !setup.reviewReady
      text: setup.tr("guide_short"); bordered: false; focusable: true
      onClicked: setup.guide()
    }
    Button { id: closeButton; Layout.fillWidth: true; text: setup.tr(setup.componentReview ? "back_to_panel" : "later"); foreground: setup.componentReview && setup.reviewReady ? Commons.Color.accent : Commons.Color.foreground; bordered: true; focusable: true; onClicked: setup.closeRequested() }
  }
}
