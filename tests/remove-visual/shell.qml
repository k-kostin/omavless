// SPDX-License-Identifier: MIT
// Synthetic production-component rendering, not installed removal acceptance.
// Stage RemoveReview.qml and I18n.js ONLY: no executable removal entry/helper.
import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Commons as Commons

ShellRoot {
  id: review
  property string state: "loading"
  property string output: Quickshell.env("OMAVLESS_REMOVE_REVIEW_DIR")
  property int viewportHeight: 600
  FloatingWindow {
    implicitWidth: Style.space(460)
    implicitHeight: Style.space(review.viewportHeight)
    title: "OmaVLESS removal review — synthetic"
    color: Commons.Color.background
    Loader {
      id: loader
      anchors.fill: parent
      source: Quickshell.env("OMAVLESS_REMOVE_REVIEW_ENTRY")
      onLoaded: {
        item.canceled.connect(function() { item.dismiss(); review.state = "closed" })
        review.state = "loaded"
      }
    }
  }
  IpcHandler {
    target: "removeReview"
    function scenario(locale: string, height: int, fixture: string): string {
      if (!loader.item || ["en", "ru"].indexOf(locale) < 0 || [360, 600].indexOf(height) < 0
          || ["ready", "native", "setup", "setup-terminal", "terminal", "failed"].indexOf(fixture) < 0) {
        review.state = "refused"; return "refused"
      }
      // Fixture assignment only; never presented as product completion/recovery.
      loader.item.dismiss()
      loader.item.locale = locale
      loader.item.nativeEffectBlocked = fixture === "native"
      loader.item.setupBusy = fixture === "setup"
      loader.item.setupTerminalOpened = fixture === "setup-terminal"
      loader.item.terminalOpened = fixture === "terminal" || fixture === "failed"
      loader.item.launchFailed = fixture === "failed"
      review.viewportHeight = height
      loader.item.openReview()
      review.state = "ready"
      return "ready"
    }
    function inspect(): string {
      if (!loader.item) return "loading"
      return JSON.stringify({state:review.state,locale:loader.item.locale,visible:loader.item.visible,
        blocked:loader.item.blockReason(),canLaunch:loader.item.canLaunch,
        scrollY:loader.item.flickable.contentY,contentHeight:loader.item.flickable.contentHeight,
        height:loader.item.flickable.height,controls:loader.item.focusTargets.map(function(control) {
          return {name:control.objectName,enabled:control.enabled,focused:control.activeFocus}
        })})
    }
    function capture(slug: string): string {
      if (review.state !== "ready" || !/^[a-z0-9-]{1,48}$/.test(slug)
          || !review.output.startsWith(Quickshell.env("HOME") + "/.cache/ovt/remove.")) return "refused"
      review.state = "capturing"
      loader.grabToImage(function(image) { review.state = image.saveToFile(review.output + "/" + slug + ".png") ? "ready" : "failed" })
      return "started"
    }
    function result(): string { return review.state }
    function finish(): string { Qt.callLater(function() { Qt.quit() }); return "closing" }
  }
}
