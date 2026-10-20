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
  property var protocols: []
  property bool expanded: false
  property var _values: []
  readonly property var values: _values
  readonly property bool hasChoices: values.length > 2
  readonly property string selectedLabel: label(value)
  visible: hasChoices || canReset
  onProtocolsChanged: updateChoices()
  onVisibleChanged: if (!visible) collapse(false)
  Component.onCompleted: updateChoices()
  function updateChoices() {
    var next = protocols.length > 1 ? [""].concat(protocols) : []
    if (next.join(",") === values.join(",")) return
    // Move focus BEFORE removing its delegate/control. An unrelated search or
    // dock keeps focus. Equal metadata refreshes leave the Repeater untouched.
    var ownedFocus = disclosure.activeFocus || reset.activeFocus
    for (var i = 0; i < choices.count; i++) {
      var item = choices.itemAt(i)
      if (item && item.activeFocus) ownedFocus = true
    }
    if (ownedFocus) {
      if (next.length > 2) disclosure.forceActiveFocus()
      else searchFocusRequested()
    }
    expanded = false
    _values = next
  }
  function label(protocol) {
    var names = {vless:"VLESS", trojan:"Trojan", hysteria2:"Hysteria2", tuic:"TUIC", wireguard:"WireGuard", amneziawg:"AmneziaWG"}
    return protocol === "" ? I18n.translate("profiles.protocol_all", locale) : (names[protocol] || "")
  }
  function collapse(restoreFocus) {
    var hadFocus = false
    // Child buttons do not make Flow.activeFocus true. Check the actual stops.
    for (var i = 0; i < choices.count; i++) {
      var item = choices.itemAt(i)
      if (item && item.activeFocus) hadFocus = true
    }
    expanded = false
    if (restoreFocus && hadFocus && disclosure.visible && disclosure.enabled)
      disclosure.forceActiveFocus()
  }
  function tabTargets() {
    var result = []
    if (!visible) return result
    if (disclosure.visible) result.push(disclosure)
    if (reset.visible) result.push(reset)
    if (expanded && hasChoices) for (var i = 0; i < choices.count; i++) {
      var item = choices.itemAt(i)
      if (item) result.push(item)
    }
    return result
  }
  function handleKey(event) {
    if (event.key === Qt.Key_Escape && expanded) {
      collapse(true)
      event.accepted = true
    } else controlKey(event)
  }
  signal chosen(string protocol)
  signal resetRequested()
  signal controlKey(var event)
  signal searchFocusRequested()
  spacing: Style.space(6)
  Flow {
    Layout.fillWidth: true
    spacing: Style.space(6)
    Button {
      id: disclosure
      visible: root.hasChoices
      text: I18n.translate("profiles.protocol_choice", root.locale, {protocol:root.selectedLabel}) + (root.expanded ? " ▾" : " ▸")
      Accessible.name: I18n.translate(root.expanded ? "profiles.protocol_hide" : "profiles.protocol_show", root.locale, {protocol:root.selectedLabel})
      bordered: true
      focusable: true
      enabled: root.canFilter
      opacity: enabled ? 1 : 0.45
      Keys.onPressed: function(event) { root.handleKey(event) }
      onClicked: if (enabled) root.expanded = !root.expanded
    }
    Button {
      id: reset
      visible: root.canReset
      text: I18n.translate("profiles.filters_reset", root.locale)
      bordered: true
      focusable: true
      enabled: root.canFilter
      opacity: enabled ? 1 : 0.45
      Keys.onPressed: function(event) { root.handleKey(event) }
      onClicked: if (enabled) { root.collapse(false); root.resetRequested() }
    }
  }
  Flow {
    id: choicesFlow
    visible: root.expanded && root.hasChoices
    Layout.fillWidth: true
    spacing: Style.space(6)
    Repeater {
      id: choices
      model: root.values
      Button {
        required property int index
        required property string modelData
        text: root.label(modelData)
        selected: root.value === modelData
        bordered: true
        focusable: true
        enabled: root.canFilter
        opacity: enabled ? 1 : 0.45
        Keys.onPressed: function(event) { root.handleKey(event) }
        onClicked: if (enabled) { root.chosen(modelData); root.collapse(true) }
      }
    }
  }
}
