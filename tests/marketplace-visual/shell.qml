// SPDX-License-Identifier: MIT
// Render unmodified product QML against the read-only fixture transport.
import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import "plugin" as Product

ShellRoot {
  id: review
  property string result: "loading"
  function find(parent, predicate) {
    if (predicate(parent)) return parent
    var children = parent.data || parent.children || []
    for (var i = 0; i < children.length; i++) {
      var found = find(children[i], predicate)
      if (found) return found
    }
    return null
  }
  Product.Panel {
    id: product
    width: Style.bar.iconSlot
    height: Style.bar.iconSlot
    settings: ({locale:"en", showExitIp:false, showBarThroughput:false})
  }
  IpcHandler {
    target: "marketplaceReview"
    function scene(name: string): string {
      if (["main", "subscription", "settings"].indexOf(name) < 0) return "refused"
      product.open()
      product.nativeSelectedProfile = ""
      product.nativeExpanded = {}
      if (name === "settings") product.openSettings()
      else {
        product.page = "main"
        if (name === "subscription") product.nativeExpanded = {"demo-subscription":true}
      }
      return "ready"
    }
    function capture(slug: string): string {
      if (!/^[a-z0-9-]{1,40}$/.test(slug)) return "refused"
      var popup = review.find(product, function(o) { return "cardOrigin" in o && "contentWidth" in o })
      if (!popup || !product.opened || product.nativeView.state !== "disconnected") return "not-ready"
      var card = popup.contentItem[0].parent.parent
      review.result = "capturing"
      card.grabToImage(function(image) {
        review.result = image.saveToFile(Quickshell.env("OMAVLESS_CAPTURE_DIR") + "/" + slug + ".png") ? "captured" : "failed"
      })
      return "started"
    }
    function inspect(): string {
      return JSON.stringify({state:product.nativeView.state, profiles:product.nativeView.profiles.length,
        page:product.page, connected:product.nativeView.connected, opened:product.opened})
    }
    function result(): string { return review.result }
    function finish() { Qt.quit() }
  }
}
