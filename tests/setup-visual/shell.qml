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
  property int viewportHeight: 576
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
      height: Math.min(parent.height - Style.space(24), Style.space(review.viewportHeight))
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
          if (item.closeRequested) item.closeRequested.connect(function() { window.visible = false; review.result = "closed" })
          review.result = "loaded"
        }
      }
    }
  }
  IpcHandler {
    target: "setupReview"
    function surface(kind: string): string {
      if (["panel", "card"].indexOf(kind) < 0) return "refused"
      var entry = Quickshell.env("OMAVLESS_SETUP_REVIEW_ENTRY")
      loader.source = kind === "panel" ? entry : entry.replace(/SetupPage.qml$/, "RequiredComponents.qml")
      return "loading"
    }
    function scenario(locale: string, state: string, core: string, terminal: bool): string {
      // Do not race the panel's initial real read-only inventory process.
      if (loader.item && loader.item.busy) return "busy"
      if (!loader.item || ["en", "ru"].indexOf(locale) < 0
          || ["checking", "ready", "needs_package", "needs_activation", "needs_attention", "release_unavailable"].indexOf(state) < 0
          || ["present", "missing", "unknown"].indexOf(core) < 0) return "refused"
      loader.item.locale = locale
      loader.item.facts = {state:state, coreInstalled:core === "unknown" ? null : core === "present"}
      loader.item.terminalOpened = terminal
      flick.contentY = 0
      window.visible = true
      return "ready"
    }
    function inspect(): string {
      if (!loader.item) return "loading"
      return JSON.stringify({state:loader.item.facts.state, visible:loader.item.visible, height:loader.item.implicitHeight,
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
    function viewport(height: int): string {
      if ([320, 576].indexOf(height) < 0) return "refused"
      review.viewportHeight = height
      return "ready"
    }
    function later(): string {
      if (!loader.item.closeRequested) return "refused"
      loader.item.focusTargets[loader.item.focusTargets.length-1].clicked(); return review.result
    }
    function result(): string { return review.result }
    function finish() { Qt.quit() }
  }
}
