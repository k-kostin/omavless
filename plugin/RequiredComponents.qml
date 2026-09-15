// SPDX-License-Identifier: MIT
import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Ui
import "SetupState.js" as SetupState
import "I18n.js" as I18n

// Presentation only. Installed programs are never offered for reinstallation.
ColumnLayout {
  id: card
  property string locale: "en"
  property var facts: ({state:"checking", coreInstalled:null})
  property bool busy: false
  property bool terminalOpened: false
  readonly property bool appMissing: SetupState.appMissing(facts)
  readonly property bool coreMissing: facts.coreInstalled === false
  readonly property string action: SetupState.missingAction(facts)
  readonly property var focusTargets: [installButton, prepareButton, retryButton, checkButton, guideButton]
  signal installRequested(string action)
  signal checkRequested()
  signal terminalClosed()
  signal guideRequested()
  function tr(key) { return I18n.translate("setup." + key, locale, {}) }
  visible: SetupState.needsAttention(facts)
  spacing: Style.space(12)
  BorderSurface {
    Layout.fillWidth: true
    implicitHeight: missingContent.implicitHeight + Style.space(20)
    visible: card.appMissing || card.coreMissing
    color: "transparent"; radius: 0
    borderSpec: Border.flat(Util.alpha(Color.foreground, 0.38), Style.normalBorderWidth)
    ColumnLayout {
      id: missingContent
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      anchors.margins: Style.space(10)
      spacing: Style.space(10)
      PlainText { Layout.fillWidth: true; text: card.tr("components"); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Color.foreground; wrapMode: Text.Wrap }
      PlainText { Layout.fillWidth: true; visible: card.appMissing; text: card.tr("app_missing"); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Color.foreground; wrapMode: Text.Wrap }
      PlainText { Layout.fillWidth: true; visible: card.coreMissing; text: card.tr("core_missing"); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Color.foreground; wrapMode: Text.Wrap }
      PlainText { Layout.fillWidth: true; visible: card.facts.state === "release_unavailable"; text: card.tr("release_unavailable"); font.family: Style.font.family; font.pixelSize: Style.font.caption; color: Qt.darker(Color.foreground, 1.55); wrapMode: Text.Wrap }
      Button {
        id: installButton
        Layout.fillWidth: true
        text: card.tr(card.appMissing ? (card.coreMissing ? "install_all" : "install_app") : "install_core")
        visible: card.action !== ""
        enabled: visible && !card.busy && !card.terminalOpened
        opacity: enabled ? 1 : 0.45
        bordered: true; focusable: true
        onClicked: card.installRequested(card.action)
      }
    }
  }
  BorderSurface {
    Layout.fillWidth: true
    implicitHeight: preparation.implicitHeight + Style.space(20)
    visible: !card.appMissing && (card.facts.state !== "ready" || card.facts.coreInstalled === null)
    color: "transparent"; radius: 0
    borderSpec: Border.flat(Util.alpha(Color.foreground, 0.38), Style.normalBorderWidth)
    ColumnLayout {
      id: preparation
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      anchors.margins: Style.space(10)
      spacing: Style.space(10)
      PlainText { Layout.fillWidth: true; text: card.tr("prepare_title"); font.family: Style.font.family; font.pixelSize: Style.font.body; color: Color.foreground; wrapMode: Text.Wrap }
      PlainText { Layout.fillWidth: true; text: card.tr(card.facts.coreInstalled === null && card.facts.state === "ready" ? "needs_attention" : card.facts.state); font.family: Style.font.family; font.pixelSize: Style.font.caption; color: Color.foreground; wrapMode: Text.Wrap }
      Button {
        id: prepareButton
        Layout.fillWidth: true
        text: card.tr("prepare")
        visible: card.facts.state === "needs_activation"
        enabled: visible && card.facts.coreInstalled === true && !card.busy && !card.terminalOpened
        opacity: enabled ? 1 : 0.45
        bordered: true; focusable: true
        onClicked: card.installRequested("install")
      }
    }
  }
  PlainText { Layout.fillWidth: true; visible: card.terminalOpened; text: card.tr("terminal"); font.family: Style.font.family; font.pixelSize: Style.font.caption; color: Color.foreground; wrapMode: Text.Wrap }
  Button { id: retryButton; Layout.fillWidth: true; visible: card.terminalOpened; text: card.tr("terminal_closed"); bordered: true; focusable: true; onClicked: card.terminalClosed() }
  RowLayout {
    Layout.fillWidth: true
    spacing: Style.space(8)
    Button { id: checkButton; Layout.fillWidth: true; text: card.tr("check"); bordered: true; focusable: true; enabled: !card.busy; onClicked: card.checkRequested() }
    Button { id: guideButton; Layout.fillWidth: true; text: card.tr("guide_short"); bordered: true; focusable: true; onClicked: card.guideRequested() }
  }
}
