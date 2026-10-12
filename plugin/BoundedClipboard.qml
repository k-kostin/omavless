// SPDX-License-Identifier: MIT
import QtQuick
import Quickshell.Io

// Explicit fixed-purpose read in an isolated, byte/time-bounded Rust helper.
// No QClipboard, native paste(), generic command property or passive polling.
QtObject {
  id: reader
  property var pending: null
  property int generation: 0
  readonly property bool running: pending !== null
  signal received(string value)
  signal refused()
  function request() {
    if (pending) return false
    var process = exchange.createObject(reader, {ticket:++generation})
    pending = process
    process.running = true
    return true
  }
  // Cancellation invalidates the result, not the one bounded running process.
  // Keep the slot until exit so repeated close/focus cannot allocate readers.
  function cancel() { generation++ }
  function complete(ticket, code, value) {
    if (!pending || ticket !== pending.ticket) return
    pending = null
    if (ticket !== generation) return
    if (code === 0) received(value)
    else refused()
  }
  property Component exchange: Component {
    Process {
      id: process
      required property int ticket
      command: ["/usr/bin/timeout", "6s", "/usr/bin/omavless", "desktop", "clipboard-read"]
      stdout: StdioCollector { id: output }
      // Never retain or display raw private helper diagnostics.
      onExited: function(code) {
        reader.complete(ticket, code, String(output.text || ""))
        process.destroy()
      }
    }
  }
}
