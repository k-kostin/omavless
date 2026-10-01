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
  property string reviewKind: ""
  property var reviewSnapshot: null
  function find(parent, predicate) {
    if (predicate(parent)) return parent
    var children = parent.data || parent.children || []
    for (var i = 0; i < children.length; i++) {
      var found = find(children[i], predicate)
      if (found) return found
    }
    return null
  }
  function stopFixtureTimers(parent) {
    if ("interval" in parent && "repeat" in parent && "running" in parent)
      parent.running = false
    var children = parent.data || parent.children || []
    for (var i = 0; i < children.length; i++) stopFixtureTimers(children[i])
  }
  function captureCard(slug) {
    if (!/^[a-z0-9-]{1,40}$/.test(slug)) return "refused"
    var popup = review.find(product, function(o) { return "cardOrigin" in o && "contentWidth" in o })
    if (!popup || !product.opened || product.nativeView.connected) return "not-ready"
    var card = popup.contentItem[0].parent.parent
    review.result = "capturing"
    card.grabToImage(function(image) {
      review.result = image.saveToFile(Quickshell.env("OMAVLESS_CAPTURE_DIR") + "/" + slug + ".png") ? "captured" : "failed"
    })
    return "started"
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
      if (product.nativeView.state !== "disconnected") return "not-ready"
      return review.captureCard(slug)
    }
    // Development-only state review. The isolated transport still refuses
    // every mutation; these states must not become marketing/live evidence.
    function reviewState(locale: string, kind: string, width: int): string {
      if (["en", "ru"].indexOf(locale) < 0 || [360, 460].indexOf(width) < 0
          || ["normal", "empty", "unavailable", "missing", "subscription-empty", "search"].indexOf(kind) < 0)
        return "refused"
      var service = review.find(product, function(o) { return o.objectName === "omavlessService" })
      if (!service || !service.nativeSnapshot || service.nativeSnapshot.desired.connected) return "not-ready"
      if (!review.reviewSnapshot) review.reviewSnapshot = JSON.parse(JSON.stringify(service.nativeSnapshot))
      review.stopFixtureTimers(product)
      var snapshot = JSON.parse(JSON.stringify(review.reviewSnapshot))
      product.settings = {locale:locale, showExitIp:false, showBarThroughput:false}
      product.open()
      product.profileFilter = kind === "search" ? "Helsinki" : ""
      product.nativeExpanded = {"demo-subscription":false}
      product.nativeSelectedProfile = ""
      if (["empty", "unavailable", "subscription-empty"].indexOf(kind) >= 0) snapshot.profiles = []
      if (kind === "empty" || kind === "unavailable") snapshot.subscriptions = []
      service.nativeSnapshot = snapshot
      service.nativeSnapshotFailed = kind === "unavailable"
      // Assign the page after snapshot-change navigation has settled.
      product.nativeSubscriptionId = kind === "missing" ? "absent-fixture" : "demo-subscription"
      product.page = ["missing", "subscription-empty"].indexOf(kind) >= 0 ? "subscription" : "main"
      var popup = review.find(product, function(o) { return "cardOrigin" in o && "contentWidth" in o })
      if (!popup) return "not-ready"
      popup.contentWidth = width
      review.reviewKind = kind
      return "ready"
    }
    function reviewInspect(): string {
      var group = review.find(product, function(o) { return "pointerHovered" in o && "iconText" in o })
      var hint = group && review.find(group, function(o) { return "panelBackground" in o && "delay" in o && o.text !== "" })
      return JSON.stringify({kind:review.reviewKind, locale:product.uiLocale,
        rows:product.nativeRows.length, storedExpanded:product.nativeExpanded["demo-subscription"] === true,
        expanded:product.nativeRows.some(function(row) { return row.kind === "subscription" && row.expanded }),
        hintVisible:!!(hint && hint.visible), hintWidth:hint ? hint.width : 0,
        rowWidth:group ? group.parent.width : 0, focused:!!(group && group.activeFocus),
        pointerHovered:!!(group && group.pointerHovered), state:product.nativeView.state})
    }
    function reviewFocus(): string {
      var group = review.find(product, function(o) { return "pointerHovered" in o && "iconText" in o })
      if (!group) return "not-ready"
      group.forceActiveFocus()
      return "focused"
    }
    function reviewPointer(): string {
      var group = review.find(product, function(o) { return "pointerHovered" in o && "iconText" in o })
      if (!group) return "not-ready"
      var point = group.mapToGlobal(group.width / 2, group.height / 2)
      return JSON.stringify({x:Math.round(point.x), y:Math.round(point.y)})
    }
    function reviewToggle(): string {
      product.toggleNativeSubscription("demo-subscription")
      return "toggled"
    }
    function reviewClearSearch(): string { product.profileFilter = ""; return "cleared" }
    function reviewCapture(slug: string): string {
      if (review.reviewKind === "") return "not-ready"
      return review.captureCard(slug)
    }
    function inspect(): string {
      return JSON.stringify({state:product.nativeView.state, profiles:product.nativeView.profiles.length,
        page:product.page, connected:product.nativeView.connected, opened:product.opened})
    }
    function result(): string { return review.result }
    function finish() { Qt.quit() }
  }
}
