// SPDX-License-Identifier: MIT
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "I18n.js" as I18n

// Counts only. Private links and the preview binding stay in Service until
// explicit confirmation; this surface cannot fetch or connect anything.
Item {
  id: prompt
  objectName: "omavlessBundleImportPrompt"
  property string locale: "en"
  property var counts: null
  property bool available: false
  property string errorText: ""
  readonly property alias flickable: detailsScroll
  signal confirmed()
  signal canceled()
  visible: false
  focus: visible
  function tr(key, values) { return I18n.translate(key, locale, values) }
  function open() {
    detailsScroll.contentY = 0
    visible = true
    Qt.callLater(function() { if (prompt.visible) cancel.forceActiveFocus() })
  }
  function dismiss() { visible = false }
  function scrollDetails(delta) {
    detailsScroll.contentY = Math.max(0, Math.min(detailsScroll.contentY + delta,
      Math.max(0, detailsScroll.contentHeight - detailsScroll.height)))
  }
  Keys.onEscapePressed: canceled()
  Keys.onPressed: function(event) {
    if (event.key === Qt.Key_PageDown || event.key === Qt.Key_PageUp) {
      scrollDetails((event.key === Qt.Key_PageDown ? 1 : -1) * detailsScroll.height * 0.8)
      event.accepted = true
    }
  }
  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Commons.Color.background, 0.78)
    MouseArea { anchors.fill: parent; onClicked: prompt.canceled() }
    BorderSurface {
      id: card
      width: Math.min(parent.width - Style.space(24), Style.space(420))
      height: Math.min(Math.max(0, parent.height - Style.space(24)),
        card.contentTopInset + card.contentBottomInset + title.implicitHeight
          + detailsColumn.implicitHeight + actions.implicitHeight + body.spacing * 2)
      anchors.centerIn: parent
      color: Commons.Color.background
      borderSpec: Border.flat(Commons.Color.accent, Style.normalBorderWidth)
      padding: Style.space(18)
      radius: Style.cornerRadius
      MouseArea { anchors.fill: parent; onClicked: {} }
      ColumnLayout {
        id: body
        anchors.fill: parent
        anchors.topMargin: card.contentTopInset
        anchors.bottomMargin: card.contentBottomInset
        anchors.leftMargin: card.contentLeftInset
        anchors.rightMargin: card.contentRightInset
        spacing: Style.space(10)
        PlainText {
          id: title
          Layout.fillWidth: true
          text: prompt.tr("import.bundle.title")
          color: Commons.Color.foreground
          font.family: Style.font.family
          font.pixelSize: Style.font.title
          wrapMode: Text.Wrap
        }
        Flickable {
          id: detailsScroll
          objectName: "omavlessBundleDetails"
          Layout.fillWidth: true
          Layout.fillHeight: true
          Layout.preferredHeight: detailsColumn.implicitHeight
          contentWidth: width
          contentHeight: detailsColumn.implicitHeight
          clip: true
          boundsBehavior: Flickable.StopAtBounds
          interactive: contentHeight > height
          ScrollBar.vertical: OmaScrollBar { policy: ScrollBar.AsNeeded }
          Column {
            id: detailsColumn
            width: Math.max(0, detailsScroll.width - Style.space(16))
            spacing: Style.space(10)
            PlainText {
              width: parent.width
              text: prompt.tr("import.bundle.counts", {
                accepted: prompt.counts ? prompt.counts.accepted : 0,
                duplicates: prompt.counts ? prompt.counts.duplicates : 0,
                skipped: prompt.counts ? prompt.counts.skipped : 0})
              color: Commons.Color.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.body
              wrapMode: Text.Wrap
            }
            PlainText {
              width: parent.width
              text: prompt.tr("import.bundle.explanation")
              color: Commons.Color.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.Wrap
            }
            PlainText {
              width: parent.width
              visible: text !== ""
              text: I18n.importReasons(prompt.counts ? prompt.counts.skipReasons : null, prompt.locale)
              color: Commons.Color.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.Wrap
            }
            PlainText {
              width: parent.width
              visible: !!prompt.counts && prompt.counts.accepted === 0
              text: prompt.tr("import.bundle.no_new")
              color: Commons.Color.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.Wrap
            }
            PlainText {
              width: parent.width
              visible: prompt.errorText !== ""
              text: prompt.errorText
              color: Commons.Color.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.Wrap
            }
          }
        }
        RowLayout {
          id: actions
          Layout.fillWidth: true
          spacing: Style.space(8)
          Item { Layout.fillWidth: true }
          Button {
            id: cancel
            objectName: "omavlessBundleCancel"
            KeyNavigation.priority: KeyNavigation.BeforeItem
            text: prompt.tr("common.cancel")
            bordered: true
            foreground: Commons.Color.foreground
            accent: Commons.Color.accent
            focusable: true
            KeyNavigation.tab: save.enabled ? save : cancel
            KeyNavigation.backtab: save.enabled ? save : cancel
            onClicked: prompt.canceled()
          }
          Button {
            id: save
            objectName: "omavlessBundleSave"
            KeyNavigation.priority: KeyNavigation.BeforeItem
            text: prompt.tr("import.bundle.save")
            bordered: true
            foreground: Commons.Color.foreground
            accent: Commons.Color.accent
            focusable: true
            enabled: prompt.available && !!prompt.counts && prompt.counts.accepted > 0
            opacity: enabled ? 1 : 0.45
            KeyNavigation.tab: cancel
            KeyNavigation.backtab: cancel
            onClicked: prompt.confirmed()
          }
        }
      }
    }
  }
}
