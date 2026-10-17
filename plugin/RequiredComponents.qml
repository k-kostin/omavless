// SPDX-License-Identifier: MIT
import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "SetupState.js" as SetupState
import "I18n.js" as I18n

// Presentation only. Installed programs are never offered for reinstallation.
ColumnLayout {
  id: card
  property string locale: "en"
  property var facts: ({state:"checking", coreInstalled:null})
  property bool busy: false
  property bool effectBlocked: false
  readonly property bool effectBusy: busy || effectBlocked
  property bool terminalOpened: false
  property string busyMessage: "checking"
  readonly property bool checking: facts.state === "checking" || (busy && busyMessage === "checking")
  property bool recoveryExpanded: false
  readonly property bool appMissing: SetupState.appMissing(facts)
  readonly property bool coreMissing: facts.coreInstalled === false
  readonly property string action: SetupState.missingAction(facts)
  readonly property bool pairUpdate: SetupState.canOfferPairUpdate(card.facts) || ["needs_pair_install", "needs_pair_update"].indexOf(card.facts.state) >= 0
  readonly property bool updateReview: SetupState.hasUpdate(card.facts) || ["needs_update", "needs_pair_update_manual"].indexOf(card.facts.state) >= 0 || card.pairUpdate
  readonly property bool operationalStep: ["needs_activation", "needs_selection", "needs_broker_stopped", "needs_runtime_start", "needs_dns_setup"].indexOf(facts.state) >= 0
  readonly property bool showPreparation: !appMissing && ["needs_update", "needs_pair_update", "needs_pair_update_manual", "needs_pair_install", "needs_pair_mismatch", "needs_companion"].indexOf(facts.state) < 0 && (facts.state !== "ready" || facts.coreInstalled === null)
  readonly property bool optionalUpdate: (SetupState.hasUpdate(facts) && ["needs_update", "needs_pair_install", "needs_pair_mismatch", "needs_companion"].indexOf(facts.state) < 0) || ["needs_pair_update", "needs_pair_update_manual"].indexOf(facts.state) >= 0
  readonly property var focusTargets: [installButton, prepareButton, recoveryToggle, restoreButton, updateButton, pairedUpdateButton, updateHelpButton, retryButton, checkButton, guideButton]
  signal installRequested(string action)
  signal checkRequested()
  signal terminalClosed()
  signal guideRequested()
  function tr(key) { return I18n.translate("setup." + key, locale, {}) }
  onFactsChanged: if (facts.state !== "needs_broker_stopped") recoveryExpanded = false
  visible: SetupState.needsAttention(facts)
  spacing: Style.space(12)
  component SetupButton: Button {
    property bool primary: false
    bordered: true; focusable: true
    foreground: enabled && primary ? Commons.Color.accent : Commons.Color.foreground
    background: enabled && primary ? Util.alpha(Commons.Color.accent, 0.08) : "transparent"
    opacity: enabled ? 1 : 0.45
  }
  PlainText {
    objectName: "setupDnsHelperIntro"
    Layout.fillWidth: true
    visible: !card.checking && !card.terminalOpened
    text: card.tr("dns_helper_intro")
    font.family: Style.font.family; font.pixelSize: Style.font.caption
    color: Qt.darker(Commons.Color.foreground, 1.55); wrapMode: Text.Wrap
  }
  BorderSurface {
    Layout.fillWidth: true
    implicitHeight: missingContent.implicitHeight + Style.space(20)
    visible: !card.checking && (card.appMissing || card.coreMissing)
    color: "transparent"; radius: 0
    borderSpec: Border.flat(Util.alpha(Commons.Color.foreground, 0.38), Style.normalBorderWidth)
    ColumnLayout {
      id: missingContent
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      anchors.margins: Style.space(10)
      spacing: Style.space(10)
      PlainText { Layout.fillWidth: true; text: card.tr("components"); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Commons.Color.foreground; wrapMode: Text.Wrap }
      PlainText { Layout.fillWidth: true; visible: card.appMissing; text: card.tr("app_missing"); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Commons.Color.foreground; wrapMode: Text.Wrap }
      PlainText { Layout.fillWidth: true; visible: card.coreMissing; text: card.tr("core_missing"); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Commons.Color.foreground; wrapMode: Text.Wrap }
      PlainText { Layout.fillWidth: true; visible: card.facts.state === "release_unavailable"; text: card.tr("release_unavailable"); font.family: Style.font.family; font.pixelSize: Style.font.caption; color: Qt.darker(Commons.Color.foreground, 1.55); wrapMode: Text.Wrap }
      SetupButton {
        id: installButton
        objectName: "setupInstall"
        Layout.fillWidth: true
        primary: true
        text: card.tr(card.coreMissing ? "install_all" : "install_app")
        visible: card.appMissing && card.action === "install"
        enabled: visible && !card.effectBusy && !card.terminalOpened
        onClicked: card.installRequested(card.action)
      }
    }
  }
  BorderSurface {
    Layout.fillWidth: true
    implicitHeight: preparation.implicitHeight + Style.space(20)
    visible: !card.checking && card.showPreparation
    color: "transparent"; radius: 0
    borderSpec: Border.flat(Util.alpha(Commons.Color.foreground, 0.38), Style.normalBorderWidth)
    ColumnLayout {
      id: preparation
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      anchors.margins: Style.space(10)
      spacing: Style.space(10)
      PlainText { Layout.fillWidth: true; text: card.tr("prepare_title"); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Commons.Color.foreground; wrapMode: Text.Wrap }
      PlainText {
        objectName: "setupOperationalReason"
        Layout.fillWidth: true
        text: card.tr(card.facts.coreInstalled === null && card.facts.state === "ready" ? "needs_attention" : card.facts.state)
        font.family: Style.font.family; font.pixelSize: Style.font.caption
        color: Commons.Color.foreground; wrapMode: Text.Wrap
      }
      SetupButton {
        id: prepareButton
        objectName: "setupPrepare"
        Layout.fillWidth: true
        primary: true
        visible: card.operationalStep
        text: card.tr(card.facts.state === "needs_dns_setup" ? "resume_dns" : card.facts.state === "needs_runtime_start" ? "start_app" : card.facts.state === "needs_selection" ? "select_pair" : card.facts.state === "needs_broker_stopped" ? "start_broker" : "prepare")
        enabled: visible && card.facts.coreInstalled === true && !card.effectBusy && !card.terminalOpened
        onClicked: card.installRequested(card.action)
      }
      SetupButton {
        id: recoveryToggle
        objectName: "setupRecoveryOptions"
        Layout.fillWidth: true
        visible: card.facts.state === "needs_broker_stopped"
        text: card.tr(card.recoveryExpanded ? "hide_recovery" : "other_options")
        bordered: false
        onClicked: card.recoveryExpanded = !card.recoveryExpanded
      }
      PlainText {
        Layout.fillWidth: true
        visible: recoveryToggle.visible && card.recoveryExpanded
        text: card.tr("restore_help")
        font.family: Style.font.family; font.pixelSize: Style.font.caption
        color: Commons.Color.foreground; wrapMode: Text.Wrap
      }
      SetupButton {
        id: restoreButton
        objectName: "setupRestoreAccess"
        Layout.fillWidth: true
        visible: recoveryToggle.visible && card.recoveryExpanded
        text: card.tr("restore_enrollment")
        enabled: visible && card.facts.coreInstalled === true && !card.effectBusy && !card.terminalOpened
        onClicked: card.installRequested("restore-enrollment")
      }
    }
  }
  BorderSurface {
    Layout.fillWidth: true
    implicitHeight: updates.implicitHeight + Style.space(20)
    visible: !card.checking && !card.appMissing && (card.updateReview || card.facts.state === "needs_pair_mismatch" || card.facts.state === "needs_companion")
    color: "transparent"; radius: 0
    borderSpec: Border.flat(Util.alpha(Commons.Color.foreground, 0.38), Style.normalBorderWidth)
    ColumnLayout {
      id: updates
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      anchors.margins: Style.space(10)
      spacing: Style.space(10)
      PlainText { Layout.fillWidth: true; text: card.tr(card.optionalUpdate ? "optional_update_title" : card.facts.state === "needs_pair_install" ? "prepare_title" : "update_title"); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Commons.Color.foreground; wrapMode: Text.Wrap }
      PlainText { Layout.fillWidth: true; text: card.tr(card.facts.coreInstalled === null && card.facts.state === "ready" ? "needs_attention" : SetupState.updateMessage(card.facts)); font.family: Style.font.family; font.pixelSize: Style.font.caption; color: Commons.Color.foreground; wrapMode: Text.Wrap }
      SetupButton {
        id: updateButton
        objectName: "setupUpdate"
        Layout.fillWidth: true
        primary: !card.optionalUpdate && !card.operationalStep
        visible: card.facts.state === "needs_update"
        text: card.tr("update_components")
        enabled: visible && !card.effectBusy && !card.terminalOpened
        onClicked: card.installRequested("upgrade")
      }
      SetupButton {
        id: pairedUpdateButton
        objectName: "setupPairedUpdate"
        Layout.fillWidth: true
        primary: !card.optionalUpdate && !card.operationalStep
        visible: card.pairUpdate
        text: card.tr(card.facts.state === "needs_pair_install" ? "complete_pair" : "prepare_update")
        enabled: visible && !card.effectBusy && !card.terminalOpened
        onClicked: card.installRequested("update-pair")
      }
      SetupButton {
        id: updateHelpButton
        objectName: "setupUpdateHelp"
        Layout.fillWidth: true
        visible: card.updateReview || card.facts.state === "needs_pair_mismatch" || card.facts.state === "needs_companion"
        text: card.tr("update_help")
        bordered: false
        enabled: visible && !card.busy && !card.terminalOpened
        onClicked: card.installRequested("update-help")
      }
    }
  }
  PlainText { Layout.fillWidth: true; visible: (card.busy || card.checking) && !card.terminalOpened; text: card.tr(card.busyMessage); font.family: Style.font.family; font.pixelSize: Style.font.caption; color: Commons.Color.foreground; wrapMode: Text.Wrap }
  PlainText { Layout.fillWidth: true; visible: card.effectBlocked && !card.busy && !card.terminalOpened; text: card.tr("action_blocked"); font.family: Style.font.family; font.pixelSize: Style.font.caption; color: Commons.Color.foreground; wrapMode: Text.Wrap }
  PlainText { Layout.fillWidth: true; visible: card.terminalOpened; text: card.tr("terminal"); font.family: Style.font.family; font.pixelSize: Style.font.caption; color: Commons.Color.foreground; wrapMode: Text.Wrap }
  SetupButton { id: retryButton; objectName: "setupTerminalClosed"; primary: true; Layout.fillWidth: true; visible: card.terminalOpened; text: card.tr("terminal_closed"); onClicked: card.terminalClosed() }
  RowLayout {
    visible: !card.checking
    Layout.fillWidth: true
    spacing: Style.space(8)
    Button { id: checkButton; Layout.fillWidth: true; text: card.tr("check"); bordered: true; focusable: true; enabled: !card.busy; onClicked: card.checkRequested() }
    Button { id: guideButton; Layout.fillWidth: true; text: card.tr("guide_short"); bordered: false; focusable: true; onClicked: card.guideRequested() }
  }
}
