// SPDX-License-Identifier: MIT
// Opt-in synthetic render of the REAL setup component. Initial status is a
// read-only installed-owner probe; this harness NEVER invokes install().
import QtQuick
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import qs.Commons

ShellRoot {
  id: review
  property string result: "loading"
  property string output: Quickshell.env("OMAVLESS_SETUP_REVIEW_DIR")
  FloatingWindow {
    id: window
    title: "OmaVLESS setup review — synthetic"
    implicitWidth: Style.space(460)
    implicitHeight: Style.space(600)
    color: Color.background
    Flickable {
      id: flick
      anchors.centerIn: parent
      width: Math.min(parent.width - Style.space(24), Style.space(436))
      height: Math.min(parent.height - Style.space(24), Style.space(576))
      contentWidth: width
      contentHeight: loader.item ? loader.item.implicitHeight : 0
      clip: true
      boundsBehavior: Flickable.StopAtBounds
      ScrollBar.vertical: ScrollBar {}
      Loader {
        id: loader
        width: parent.width - Style.space(16)
        source: Quickshell.env("OMAVLESS_SETUP_REVIEW_ENTRY")
        onLoaded: {
          item.closeRequested.connect(function() { window.visible = false; review.result = "closed" })
          review.result = "loaded"
        }
      }
    }
  }
  IpcHandler {
    target: "setupReview"
    function scenario(locale: string, state: string, terminal: bool): string {
      if (!loader.item || ["en", "ru"].indexOf(locale) < 0
          || ["checking", "ready", "needs_package", "needs_activation", "needs_attention", "release_unavailable"].indexOf(state) < 0) return "refused"
      loader.item.locale = locale
      loader.item.state = state
      loader.item.terminalOpened = terminal
      flick.contentY = 0
      window.visible = true
      return "ready"
    }
    function inspect(): string {
      if (!loader.item) return "loading"
      return JSON.stringify({state:loader.item.state, height:loader.item.implicitHeight,
        controls:loader.item.focusTargets.map(function(b) { return {text:b.text, visible:b.visible, enabled:b.enabled} })})
    }
    function capture(slug: string): string {
      if (!/^[a-z0-9-]{1,48}$/.test(slug) || !review.output.startsWith("/tmp/omavless-setup-review.")) return "refused"
      review.result = "capturing"
      flick.grabToImage(function(image) { review.result = image.saveToFile(review.output + "/" + slug + ".png") ? "captured" : "failed" })
      return "started"
    }
    function scroll(where: string): string {
      if (["top", "bottom"].indexOf(where) < 0) return "refused"
      flick.contentY = where === "top" ? 0 : Math.max(0, flick.contentHeight - flick.height)
      return "ready"
    }
    function later(): string { loader.item.focusTargets[4].clicked(); return review.result }
    function result(): string { return review.result }
    function finish() { Qt.quit() }
  }
}
