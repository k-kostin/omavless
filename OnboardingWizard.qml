// SPDX-License-Identifier: MIT
// Copyright (c) 2026 OmaVLESS contributors

import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui

// A deliberately short first-run path: make the external core usable, pick
// an optional Routing policy, then hand the existing private import flow its
// first supported profile. Installation remains an explicit terminal action.
Item {
  id: wizard

  property var coreSetup: ({ installed: false, tunReady: false, path: "" })
  property var filePicker: ({ available: false, provider: "" })
  property var presets: []
  property var profiles: []
  property string routingPreset: ""
  property bool busy: false
  property string installCommand: ""
  property string capabilityCommand: ""
  property string verifyCommand: ""
  property color foreground: Color.foreground
  property color dim: Qt.darker(foreground, 1.55)
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family
  property int step: 1

  signal copyCommand(string command)
  signal refreshRequested()
  signal presetChosen(string preset)
  signal pasteRequested()
  signal fileRequested()
  signal finishRequested()
  signal canceled()

  visible: false
  focus: visible

  function openAt(value) {
    step = Math.max(1, Math.min(3, Number(value) || 1))
    visible = true
    Qt.callLater(function() { wizard.forceActiveFocus() })
  }

  function dismiss() { visible = false }

  Keys.onEscapePressed: canceled()

  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Color.background, 0.82)
    MouseArea { anchors.fill: parent; onClicked: wizard.canceled() }

    BorderSurface {
      id: card
      width: Math.min(parent.width - Style.space(24), Style.space(440))
      height: Math.min(parent.height - Style.space(24), content.implicitHeight
        + card.contentTopInset + card.contentBottomInset)
      anchors.centerIn: parent
      color: Color.background
      borderSpec: Border.flat(Color.accent, Style.normalBorderWidth)
      padding: Style.space(18)
      radius: Style.cornerRadius

      MouseArea { anchors.fill: parent; onClicked: {} }

      Flickable {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.topMargin: card.contentTopInset
        anchors.bottomMargin: card.contentBottomInset
        anchors.leftMargin: card.contentLeftInset
        anchors.rightMargin: card.contentRightInset
        contentWidth: width
        contentHeight: content.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        Column {
          id: content
          width: parent.width
          spacing: Style.space(10)

          Row {
            width: parent.width
            spacing: Style.space(8)
            PlainText {
              width: parent.width - stepText.width - parent.spacing
              text: "SET UP OMAVLESS"
              color: wizard.foreground
              font.family: wizard.fontFamily
              font.pixelSize: Style.font.title
              elide: Text.ElideRight
            }
            PlainText {
              id: stepText
              text: wizard.step + " / 3"
              color: wizard.dim
              font.family: wizard.fontFamily
              font.pixelSize: Style.font.caption
            }
          }

          PlainText {
            width: parent.width
            text: wizard.step === 1 ? "Mihomo core"
              : (wizard.step === 2 ? "Routing profile" : "First connection")
            color: Color.accent
            font.family: wizard.fontFamily
            font.pixelSize: Style.font.subtitle
            font.bold: true
          }

          Column {
            width: parent.width
            spacing: Style.space(9)
            visible: wizard.step === 1

            PlainText {
              width: parent.width
              text: !wizard.coreSetup.installed
                ? "OmaVLESS needs the Mihomo core. Install the package in a terminal, then check again."
                : (wizard.coreSetup.tunReady
                  ? "Mihomo is installed and has the permissions required for a TUN connection."
                  : "Mihomo is installed, but Linux has not granted it the TUN capabilities yet.")
              color: wizard.dim
              font.family: wizard.fontFamily
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.WordWrap
            }

            CommandRow {
              visible: !wizard.coreSetup.installed
              label: "Install Mihomo"
              command: wizard.installCommand
            }

            CommandRow {
              visible: wizard.coreSetup.installed && !wizard.coreSetup.tunReady
              label: "Grant TUN access"
              command: wizard.capabilityCommand
            }

            CommandRow {
              visible: wizard.coreSetup.installed
              label: "Verify installation"
              command: wizard.verifyCommand
            }

            PlainText {
              visible: wizard.coreSetup.path !== ""
              width: parent.width
              text: wizard.coreSetup.path
              color: wizard.dim
              font.family: wizard.fontFamily
              font.pixelSize: Style.font.caption
              elide: Text.ElideMiddle
            }
          }

          Column {
            width: parent.width
            spacing: Style.space(8)
            visible: wizard.step === 2

            PlainText {
              width: parent.width
              text: "Routing sends local destinations directly and the remaining traffic through the selected profile. You can skip this and use Full VPN first."
              color: wizard.dim
              font.family: wizard.fontFamily
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.WordWrap
            }

            Repeater {
              model: wizard.presets
              BorderSurface {
                id: presetCard
                required property var modelData
                readonly property bool selected: wizard.routingPreset === modelData.id
                width: content.width
                height: presetBody.implicitHeight + Style.space(16)
                color: selected ? Util.alpha(Color.foreground, 0.08) : "transparent"
                borderSpec: Border.flat(selected ? Color.accent
                  : Util.alpha(wizard.foreground, 0.30), Style.normalBorderWidth)
                radius: Style.cornerRadius

                Row {
                  id: presetBody
                  anchors.left: parent.left
                  anchors.right: parent.right
                  anchors.verticalCenter: parent.verticalCenter
                  anchors.leftMargin: Style.space(10)
                  anchors.rightMargin: Style.space(10)
                  spacing: Style.space(8)
                  Column {
                    width: parent.width - choosePreset.width - parent.spacing
                    PlainText {
                      width: parent.width
                      text: presetCard.modelData.country
                      color: presetCard.selected ? Color.accent : wizard.foreground
                      font.family: wizard.fontFamily
                      font.pixelSize: Style.font.body
                      font.bold: true
                    }
                    PlainText {
                      width: parent.width
                      text: presetCard.modelData.summary
                      color: wizard.dim
                      font.family: wizard.fontFamily
                      font.pixelSize: Style.font.caption
                      wrapMode: Text.WordWrap
                    }
                  }
                  Button {
                    id: choosePreset
                    text: presetCard.selected ? "Selected" : "Choose"
                    bordered: true
                    enabled: !presetCard.selected && !wizard.busy
                    foreground: enabled ? wizard.foreground : wizard.dim
                    fontFamily: wizard.fontFamily
                    onClicked: wizard.presetChosen(presetCard.modelData.id)
                  }
                }
              }
            }
          }

          Column {
            width: parent.width
            spacing: Style.space(9)
            visible: wizard.step === 3

            PlainText {
              width: parent.width
              text: wizard.profiles.length > 0
                ? (wizard.profiles.length === 1 ? "Your first connection is ready."
                  : wizard.profiles.length + " connections are ready.")
                : "Paste a VLESS, Trojan, Hysteria2 or TUIC link from the clipboard, or choose a file. The secret stays in OmaVLESS private storage."
              color: wizard.profiles.length > 0 ? Color.accent : wizard.dim
              font.family: wizard.fontFamily
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.WordWrap
            }

            PlainText {
              visible: !wizard.filePicker.available
              width: parent.width
              text: "File import unavailable — file picker missing. Run omarchy pkg add zenity in a terminal. Clipboard import still works."
              color: wizard.urgent
              font.family: wizard.fontFamily
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.WordWrap
            }

            Row {
              spacing: Style.space(8)
              Button {
                text: "Paste link"
                bordered: true
                enabled: !wizard.busy
                foreground: enabled ? wizard.foreground : wizard.dim
                fontFamily: wizard.fontFamily
                onClicked: wizard.pasteRequested()
              }
              Button {
                text: "Choose file"
                bordered: true
                enabled: wizard.filePicker.available && !wizard.busy
                foreground: enabled ? wizard.foreground : wizard.dim
                fontFamily: wizard.fontFamily
                onClicked: wizard.fileRequested()
              }
            }
          }

          Item {
            width: parent.width
            implicitHeight: navigation.implicitHeight
            Row {
              id: navigation
              anchors.right: parent.right
              spacing: Style.space(8)

              Button {
                text: wizard.step === 1 ? "Close" : "Back"
                bordered: true
                foreground: wizard.foreground
                fontFamily: wizard.fontFamily
                onClicked: {
                  if (wizard.step === 1) wizard.canceled()
                  else wizard.step--
                }
              }

              Button {
                visible: wizard.step === 1
                text: "Check again"
                bordered: true
                enabled: !wizard.busy
                foreground: enabled ? wizard.foreground : wizard.dim
                fontFamily: wizard.fontFamily
                onClicked: wizard.refreshRequested()
              }

              Button {
                visible: wizard.step === 1
                text: "Continue"
                bordered: true
                enabled: wizard.coreSetup.tunReady && !wizard.busy
                foreground: enabled ? wizard.foreground : wizard.dim
                fontFamily: wizard.fontFamily
                onClicked: wizard.step = 2
              }

              Button {
                visible: wizard.step === 2
                text: wizard.routingPreset === "" ? "Skip for now" : "Continue"
                bordered: true
                enabled: !wizard.busy
                foreground: enabled ? wizard.foreground : wizard.dim
                fontFamily: wizard.fontFamily
                onClicked: wizard.step = 3
              }

              Button {
                visible: wizard.step === 3
                text: wizard.profiles.length > 0 ? "Finish" : "Finish later"
                bordered: true
                enabled: !wizard.busy
                foreground: enabled ? wizard.foreground : wizard.dim
                fontFamily: wizard.fontFamily
                onClicked: wizard.finishRequested()
              }
            }
          }
        }
      }
    }
  }

  component CommandRow: BorderSurface {
    id: commandRow
    property string label: ""
    property string command: ""
    width: content.width
    height: commandContent.implicitHeight + Style.space(14)
    color: "transparent"
    borderSpec: Border.flat(Util.alpha(wizard.foreground, 0.28), Style.normalBorderWidth)
    radius: Style.cornerRadius

    Row {
      id: commandContent
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      anchors.leftMargin: Style.space(9)
      anchors.rightMargin: Style.space(9)
      spacing: Style.space(8)
      Column {
        width: parent.width - copyButton.width - parent.spacing
        PlainText {
          width: parent.width
          text: commandRow.label
          color: wizard.foreground
          font.family: wizard.fontFamily
          font.pixelSize: Style.font.bodySmall
        }
        PlainText {
          width: parent.width
          text: commandRow.command
          color: wizard.dim
          font.family: wizard.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideMiddle
        }
      }
      Button {
        id: copyButton
        text: "Copy"
        tooltipText: "Copy command for the terminal"
        bordered: true
        foreground: wizard.foreground
        fontFamily: wizard.fontFamily
        onClicked: wizard.copyCommand(commandRow.command)
      }
    }
  }
}
