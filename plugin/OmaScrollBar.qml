// SPDX-License-Identifier: MIT
// Copyright (c) 2026 OmaVLESS contributors
import QtQuick
import QtQuick.Controls
import qs.Commons

// The hit lane stays the same width in every state. Callers reserve a 16px
// scaled gutter so the thumb never covers text or shifts it when scrolling.
ScrollBar {
  id: control
  hoverEnabled: true
  implicitWidth: Style.space(12)
  minimumSize: Math.min(1, Style.space(24) / Math.max(1, availableHeight))
  padding: Style.space(4)
  contentItem: Rectangle {
    implicitWidth: Style.space(4)
    implicitHeight: Style.space(24)
    radius: 0
    color: control.pressed || control.hovered ? Color.accent : Color.foreground
    opacity: control.pressed || control.hovered ? 1 : (control.active ? 0.65 : 0.35)
  }
  background: Item {}
}
