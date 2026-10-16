// SPDX-License-Identifier: MIT
// Explicit synthetic rendering of the production HelpPrompt. No native owner,
// package installer or private facts. Clipboard effects require an actual click.
import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons

ShellRoot {
  id: review
  property string state: "loading"
  property string output: Quickshell.env("OMAVLESS_HELP_REVIEW_DIR")
  property int viewportHeight: 600
  FloatingWindow {
    id: window
    title: "OmaVLESS help review — synthetic"
    implicitWidth: Style.space(460)
    implicitHeight: Style.space(review.viewportHeight)
    color: Color.background
    Loader {
      id: loader
      anchors.fill: parent
      source: Quickshell.env("OMAVLESS_HELP_REVIEW_ENTRY")
      onLoaded: {
        item.canceled.connect(function() { item.dismiss(); review.state = "closed" })
        review.state = "loaded"
      }
    }
  }
  IpcHandler {
    target: "helpReview"
    function scenario(locale: string, height: int, section: string): string {
      if (!loader.item || ["en", "ru"].indexOf(locale) < 0 || [360, 600].indexOf(height) < 0
          || ["install", "updates", "help"].indexOf(section) < 0) {
        review.state = "refused"; return "refused"
      }
      loader.item.locale = locale
      review.viewportHeight = height
      loader.item.openHelp(section)
      review.state = "ready"
      return "ready"
    }
    function inspect(): string {
      if (!loader.item) return "loading"
      return JSON.stringify({state:review.state,visible:loader.item.visible,locale:loader.item.locale,
        copyState:loader.item.copyState,prompt:loader.item.agentPrompt,
        scrollY:loader.item.flickable.contentY,contentHeight:loader.item.flickable.contentHeight,
        height:loader.item.flickable.height,
        controls:loader.item.focusTargets.map(function(control) {
          return {name:control.objectName,visible:control.visible,enabled:control.enabled,focused:control.activeFocus}
        })})
    }
    function capture(slug: string): string {
      if (review.state !== "ready" || !/^[a-z0-9-]{1,48}$/.test(slug)
          || !review.output.startsWith(Quickshell.env("HOME") + "/.cache/ovt/help.")) return "refused"
      review.state = "capturing"
      loader.grabToImage(function(image) { review.state = image.saveToFile(review.output + "/" + slug + ".png") ? "ready" : "failed" })
      return "started"
    }
    function result(): string { return review.state }
    function finish(): string { Qt.callLater(function() { Qt.quit() }); return "closing" }
  }
}
