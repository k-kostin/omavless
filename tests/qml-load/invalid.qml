import QtQuick
import Quickshell.Io

// Regression: Process has no default property for an unnamed Timer child.
Process {
  Timer { interval: 1000 }
}
