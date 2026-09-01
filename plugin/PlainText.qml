// SPDX-License-Identifier: MIT
import QtQuick

// One local primitive prevents provider-controlled labels from being parsed
// as rich text. Length and control-character bounds live in Service.qml.
Text {
  textFormat: Text.PlainText
}
