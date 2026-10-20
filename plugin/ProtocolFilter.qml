// SPDX-License-Identifier: MIT
import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "I18n.js" as I18n

// Local list selection only. No process, stored preference or runtime action.
ColumnLayout {
  id: root
  property string locale: "en"
  property string value: ""
  property bool canFilter: true
  property bool canReset: false
  readonly property var values: ["", "vless", "trojan", "hysteria2", "tuic", "wireguard", "amneziawg"]
  readonly property var labels: [I18n.translate("profiles.protocol_all", locale), "VLESS", "Trojan", "Hysteria2", "TUIC", "WireGuard", "AmneziaWG"]
  function tabTargets() {
    var result = []
    for (var i = 0; i < choices.count; i++) {
      var item = choices.itemAt(i)
      if (item) result.push(item)
    }
    if (reset.visible) result.push(reset)
    return result
  }
  signal chosen(string protocol)
  signal resetRequested()
  signal controlKey(var event)
  spacing: Style.space(6)
  PlainText {
    Layout.fillWidth: true
    text: I18n.translate("profiles.protocol_filter", root.locale)
    color: Commons.Color.foreground
    font.family: Style.font.family
    font.pixelSize: Style.font.caption
    wrapMode: Text.Wrap
  }
  Flow {
    Layout.fillWidth: true
    spacing: Style.space(6)
    Repeater {
      id: choices
      model: root.values
      Button {
        required property int index
        required property string modelData
        text: root.labels[index]
        selected: root.value === modelData
        bordered: true
        focusable: true
        enabled: root.canFilter
        opacity: enabled ? 1 : 0.45
        Keys.onPressed: function(event) { root.controlKey(event) }
        onClicked: if (enabled) root.chosen(modelData)
      }
    }
    Button {
      id: reset
      visible: root.canReset
      text: I18n.translate("profiles.filters_reset", root.locale)
      bordered: true
      focusable: true
      enabled: root.canFilter
      opacity: enabled ? 1 : 0.45
      Keys.onPressed: function(event) { root.controlKey(event) }
      onClicked: if (enabled) root.resetRequested()
    }
  }
}
