// SPDX-License-Identifier: MIT
// Copyright (c) 2026 OmaVLESS contributors
import QtQuick
import qs.Commons
import qs.Ui

// Header/navigation actions have one size and one hover/focus treatment.
// Inline row actions intentionally retain the shell's quieter presentation.
PanelActionButton {
  bordered: true
  size: Math.max(Style.space(32), fontSize + Style.spacing.sm * 2)
  fontSize: Style.font.icon
  foreground: Color.foreground
  hoverColor: Color.accent
}
