// SPDX-License-Identifier: MIT
import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "I18n.js" as I18n
import "SubscriptionUsage.js" as SubscriptionUsage

// Presentation only. The owning panel admits explicit requests and erases data.
Item {
  id: prompt
  property string locale: "en"
  property string targetName: ""
  property string status: ""
  property var usage: null
  property bool requestEnabled: false
  readonly property var focusTargets: [closeButton, retryButton].filter(function(c) { return c.visible && c.enabled })
  signal canceled()
  signal requested()
  visible: false
  focus: visible
  function tr(key) { return I18n.translate("usage." + key, locale, {}) }
  function byteText(value) { return SubscriptionUsage.bytes(value, locale) || tr("unavailable_value") }
  function navigate(event) {
    if (event.key === Qt.Key_Escape) { canceled(); event.accepted = true; return }
    var last = Math.max(0, scroll.contentHeight - scroll.height), next = scroll.contentY
    if (event.key === Qt.Key_Home) next = 0
    else if (event.key === Qt.Key_End) next = last
    else if (event.key === Qt.Key_Up) next -= Style.space(32)
    else if (event.key === Qt.Key_Down) next += Style.space(32)
    else if (event.key === Qt.Key_PageUp) next -= scroll.height
    else if (event.key === Qt.Key_PageDown) next += scroll.height
    else next = null
    if (next !== null) {
      scroll.contentY = Math.max(0, Math.min(next, last))
      event.accepted = true
      return
    }
    if (event.key !== Qt.Key_Tab && event.key !== Qt.Key_Backtab) return
    var controls = focusTargets, current = -1
    if (!controls.length) return
    for (var i = 0; i < controls.length; i++) if (controls[i].activeFocus) current = i
    var direction = event.key === Qt.Key_Backtab || (event.modifiers & Qt.ShiftModifier) ? -1 : 1
    controls[current < 0 ? (direction < 0 ? controls.length - 1 : 0) : (current + direction + controls.length) % controls.length].forceActiveFocus()
    event.accepted = true
  }
  onVisibleChanged: if (visible) {
    scroll.contentY = 0
    Qt.callLater(function() { if (prompt.visible) closeButton.forceActiveFocus() })
  }
  Keys.onPressed: function(event) { navigate(event) }
  component DataRow: Column {
    property string label
    property string value
    width: parent.width
    spacing: Style.space(3)
    PlainText { width: parent.width; text: parent.label; color: Qt.darker(Commons.Color.foreground, 1.55); font.family: Style.font.family; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap }
    PlainText { width: parent.width; text: parent.value; color: Commons.Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body; wrapMode: Text.Wrap }
  }
  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Commons.Color.background, 0.82)
    MouseArea { anchors.fill: parent; onClicked: prompt.canceled() }
    BorderSurface {
      id: card
      anchors.centerIn: parent
      width: Math.min(parent.width - Style.space(24), Style.space(440))
      height: Math.min(parent.height - Style.space(24), content.implicitHeight + actions.implicitHeight + Style.space(10) + card.contentTopInset + card.contentBottomInset)
      color: Commons.Color.background
      borderSpec: Border.flat(Commons.Color.accent, Style.normalBorderWidth)
      padding: Style.space(18)
      radius: Style.cornerRadius
      MouseArea { anchors.fill: parent; onClicked: {} }
      Item {
        anchors.fill: parent
        anchors.topMargin: card.contentTopInset; anchors.bottomMargin: card.contentBottomInset
        anchors.leftMargin: card.contentLeftInset; anchors.rightMargin: card.contentRightInset
        Flickable {
        id: scroll
        objectName: "subscriptionUsageScroll"
        anchors.top: parent.top; anchors.left: parent.left; anchors.right: parent.right
        anchors.bottom: actions.top; anchors.bottomMargin: Style.space(10)
        contentWidth: width
        contentHeight: content.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height
        ScrollBar.vertical: OmaScrollBar { policy: ScrollBar.AsNeeded }
        Column {
          id: content
          width: Math.max(0, parent.width - Style.space(16))
          spacing: Style.space(10)
          PlainText { width: parent.width; text: prompt.tr("title"); color: Commons.Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.title; wrapMode: Text.Wrap }
          PlainText { width: parent.width; text: prompt.targetName; color: Commons.Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body; wrapMode: Text.Wrap }
          PlainText { width: parent.width; text: prompt.tr("scope"); color: Qt.darker(Commons.Color.foreground, 1.55); font.family: Style.font.family; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap }
          PlainText { width: parent.width; visible: prompt.status !== "reported"; text: prompt.tr(prompt.status === "loading" ? "loading" : prompt.status === "notProvided" ? "not_provided" : "failed"); color: Qt.darker(Commons.Color.foreground, 1.55); font.family: Style.font.family; font.pixelSize: Style.font.body; wrapMode: Text.Wrap }
          Column {
            visible: prompt.status === "reported" && prompt.usage !== null
            width: parent.width
            spacing: Style.space(8)
            DataRow { label: prompt.tr("used"); value: prompt.byteText(prompt.usage ? prompt.usage.used : null) }
            DataRow { label: prompt.tr("limit"); value: prompt.byteText(prompt.usage ? prompt.usage.total : null) }
            DataRow { label: prompt.tr("remaining"); value: prompt.byteText(prompt.usage ? prompt.usage.remaining : null) }
            DataRow { label: prompt.tr("expiry"); value: prompt.usage && prompt.usage.expiry !== null ? prompt.usage.expiry : prompt.tr("unavailable_value") }
            DataRow { label: prompt.tr("upload"); value: prompt.byteText(prompt.usage ? prompt.usage.upload : null) }
            DataRow { label: prompt.tr("download"); value: prompt.byteText(prompt.usage ? prompt.usage.download : null) }
          }
        }
        }
        // Actions stay visible when loading grows into a scrollable result.
        Flow {
          id: actions
          anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: parent.bottom
          spacing: Style.space(8)
          Button { id: closeButton; objectName: "subscriptionUsageClose"; text: I18n.translate("common.close", prompt.locale, {}); focusable: true; bordered: true; onClicked: prompt.canceled(); Keys.onPressed: function(event) { prompt.navigate(event) } }
          Button { id: retryButton; objectName: "subscriptionUsageCheck"; text: prompt.tr("again"); focusable: true; bordered: true; enabled: prompt.requestEnabled; opacity: enabled ? 1 : 0.45; onClicked: prompt.requested(); Keys.onPressed: function(event) { prompt.navigate(event) } }
        }
      }
    }
  }
}
