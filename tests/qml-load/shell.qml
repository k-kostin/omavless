// SPDX-License-Identifier: MIT
import QtQuick
import Quickshell

ShellRoot {
  Component.onCompleted: {
    // Compile the entire imported component graph without creating the plugin:
    // no Service startup, private-store reads, tunnel or clipboard effects.
    const component = Qt.createComponent(Quickshell.env("OMAVLESS_QML_ENTRY"), Component.PreferSynchronous)
    console.log(component.status === Component.Ready ? "OMAVLESS_QML_LOAD_PASS" : "OMAVLESS_QML_LOAD_FAIL")
    Qt.quit()
  }
}
