// SPDX-License-Identifier: MIT
// Copyright (c) 2026 OmaVLESS contributors

import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui
import "I18n.js" as I18n

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
  property string locale: "en"
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

  function textFor(key, values) {
    return I18n.translate(key, locale, values || {})
  }

  function localizedCount(baseKey, count) {
    return I18n.plural(baseKey, count, locale)
  }

  function presetCountry(preset) {
    if (!preset) return ""
    if (preset.id === "roscomvpn-default") return textFor("routing.source.russia")
    if (preset.id === "china-cn-direct") return textFor("routing.source.china")
    if (preset.id === "iran-ir-direct") return textFor("routing.source.iran")
    return preset.country
  }

  function presetSummary(preset) {
    if (!preset) return ""
    if (preset.id === "roscomvpn-default") return textFor("routing.preset.russia")
    if (preset.id === "china-cn-direct") return textFor("routing.preset.china")
    if (preset.id === "iran-ir-direct") return textFor("routing.preset.iran")
    return preset.summary
  }

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
              text: wizard.textFor("onboarding.title")
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
            text: wizard.step === 1 ? wizard.textFor("onboarding.step.core")
              : (wizard.step === 2 ? wizard.textFor("onboarding.step.routing")
                : wizard.textFor("onboarding.step.connection"))
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
              text: wizard.textFor(!wizard.coreSetup.installed
                ? "onboarding.core.missing" : (wizard.coreSetup.tunReady
                  ? "onboarding.core.ready" : "onboarding.core.tun_missing"))
              color: wizard.dim
              font.family: wizard.fontFamily
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.WordWrap
            }

            CommandRow {
              visible: !wizard.coreSetup.installed
              label: wizard.textFor("onboarding.install_mihomo")
              command: wizard.installCommand
            }

            CommandRow {
              visible: wizard.coreSetup.installed && !wizard.coreSetup.tunReady
              label: wizard.textFor("onboarding.grant_tun")
              command: wizard.capabilityCommand
            }

            CommandRow {
              visible: wizard.coreSetup.installed
              label: wizard.textFor("onboarding.verify_installation")
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
              text: wizard.textFor("onboarding.routing_help")
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
                      text: wizard.presetCountry(presetCard.modelData)
                      color: presetCard.selected ? Color.accent : wizard.foreground
                      font.family: wizard.fontFamily
                      font.pixelSize: Style.font.body
                      font.bold: true
                    }
                    PlainText {
                      width: parent.width
                      text: wizard.presetSummary(presetCard.modelData)
                      color: wizard.dim
                      font.family: wizard.fontFamily
                      font.pixelSize: Style.font.caption
                      wrapMode: Text.WordWrap
                    }
                  }
                  Button {
                    id: choosePreset
                    text: presetCard.selected
                      ? wizard.textFor("common.selected") : wizard.textFor("common.choose")
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
                ? wizard.textFor("onboarding.connections_ready", {
                    count: wizard.localizedCount("connection", wizard.profiles.length)
                  })
                : wizard.textFor("onboarding.import_help")
              color: wizard.profiles.length > 0 ? Color.accent : wizard.dim
              font.family: wizard.fontFamily
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.WordWrap
            }

            PlainText {
              visible: !wizard.filePicker.available
              width: parent.width
              text: wizard.textFor("onboarding.file_picker_missing")
              color: wizard.urgent
              font.family: wizard.fontFamily
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.WordWrap
            }

            CommandRow {
              visible: !wizard.filePicker.available
              width: parent.width
              label: wizard.textFor("onboarding.install_file_picker")
              command: "omarchy pkg add zenity"
            }

            Row {
              spacing: Style.space(8)
              Button {
                text: wizard.textFor("onboarding.paste_link")
                bordered: true
                enabled: !wizard.busy
                foreground: enabled ? wizard.foreground : wizard.dim
                fontFamily: wizard.fontFamily
                onClicked: wizard.pasteRequested()
              }
              Button {
                text: wizard.textFor("onboarding.choose_file")
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
                text: wizard.step === 1
                  ? wizard.textFor("common.close") : wizard.textFor("common.back")
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
                text: wizard.textFor("common.check_again")
                bordered: true
                enabled: !wizard.busy
                foreground: enabled ? wizard.foreground : wizard.dim
                fontFamily: wizard.fontFamily
                onClicked: wizard.refreshRequested()
              }

              Button {
                visible: wizard.step === 1
                text: wizard.textFor("common.continue")
                bordered: true
                enabled: wizard.coreSetup.tunReady && !wizard.busy
                foreground: enabled ? wizard.foreground : wizard.dim
                fontFamily: wizard.fontFamily
                onClicked: wizard.step = 2
              }

              Button {
                visible: wizard.step === 2
                text: wizard.routingPreset === ""
                  ? wizard.textFor("onboarding.skip_for_now") : wizard.textFor("common.continue")
                bordered: true
                enabled: !wizard.busy
                foreground: enabled ? wizard.foreground : wizard.dim
                fontFamily: wizard.fontFamily
                onClicked: wizard.step = 3
              }

              Button {
                visible: wizard.step === 3
                text: wizard.profiles.length > 0
                  ? wizard.textFor("onboarding.finish") : wizard.textFor("onboarding.finish_later")
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
        text: wizard.textFor("common.copy")
        tooltipText: wizard.textFor("onboarding.copy_command_tooltip")
        bordered: true
        foreground: wizard.foreground
        fontFamily: wizard.fontFamily
        onClicked: wizard.copyCommand(commandRow.command)
      }
    }
  }
}
