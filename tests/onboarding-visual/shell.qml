// SPDX-License-Identifier: MIT
// Opt-in synthetic rendering ONLY. Never creates Service, store, core or socket clients.
import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Commons as Commons

ShellRoot {
    id: review
    property string output: Quickshell.env("OMAVLESS_ONBOARDING_REVIEW_DIR")
    property string result: "loading"
    property int copyCount: 0
    property string lastAction: ""
    property int viewportHeight: 720
    function control(name, parent) {
        if (parent.objectName === name) return parent
        var children = parent.children || []
        for (var i = 0; i < children.length; i++) {
            var found = control(name, children[i])
            if (found) return found
        }
        return null
    }
    function scenario(language, stage, helpers, core) {
        if (!loader.item || ["en", "ru"].indexOf(language) < 0
            || stage < 1 || stage > 3 || ["ready", "missing", "clipboard", "picker", "unknown"].indexOf(helpers) < 0
            || ["ready", "missing", "permissions", "unknown", "update"].indexOf(core) < 0) { result = "refused"; return "refused" }
        var w = loader.item
        w.locale = language
        w.nativeCoreFacts = core === "unknown" ? null : {
            installed:core !== "missing", version:core === "missing" ? null : "1.19.31",
            tunDevice:"present", fileNetworkCapabilities:core === "missing" ? "not_applicable" : core === "permissions" ? "missing" : "present"
        }
        w.nativeCoreDescription = core === "unknown" ? w.textFor("native.core.notChecked") : w.textFor(core === "missing" ? "native.core.missing" : "native.core.version", {version:"1.19.31"})
        w.nativeSetupFacts = {state:core === "update" ? "needs_update" : core === "missing" ? "needs_package" : "ready", coreInstalled:core !== "missing" && core !== "update"}
        w.nativeSetupTerminalOpened = false
        w.profiles = []
        w.busy = false
        w.routingSavePending = false
        w.routingSaveUnknown = false
        if ("completionPending" in w) w.completionPending = false
        w.nativeStatus = ""
        w.nativeCanContinue = true
        if ("firstRun" in w) w.firstRun = true
        // Supports inspecting the unchanged installed baseline before the new property exists.
        if ("nativeDesktopFacts" in w) w.nativeDesktopFacts = helpers === "unknown" ? null : {
            filePicker:helpers === "ready" || helpers === "picker" ? "zenity" : null,
            clipboardReadAvailable:helpers === "ready" || helpers === "clipboard",
            clipboardWriteAvailable:helpers === "ready" || helpers === "clipboard"
        }
        w.openAt(stage)
        window.visible = true
        result = "ready"
        return "ready"
    }
    FloatingWindow {
        id: window
        title: "OmaVLESS onboarding review — synthetic"
        implicitWidth: 760
        implicitHeight: 720
        color: Commons.Color.background
        Loader {
            id: loader
            anchors.centerIn: parent
            width: parent.width
            height: Math.min(parent.height, review.viewportHeight)
            source: Quickshell.env("OMAVLESS_ONBOARDING_REVIEW_ENTRY")
            onLoaded: {
                item.nativeContext = true
                item.presets = [
                    {id:"roscomvpn-default", country:"", summary:""},
                    {id:"china-cn-direct", country:"", summary:""},
                    {id:"iran-ir-direct", country:"", summary:""}
                ]
                item.copyCommand.connect(function(_) { review.copyCount++; review.lastAction = "copy" })
                item.pasteRequested.connect(function() { review.lastAction = "clipboard" })
                item.fileRequested.connect(function() { review.lastAction = "file" })
                item.finishRequested.connect(function() { review.lastAction = "finish"; item.dismiss() })
                item.canceled.connect(function() { review.lastAction = "cancel"; item.dismiss() })
                item.componentSetupRequested.connect(function() { review.lastAction = "component-review"; item.dismiss() })
                review.scenario("en", 3, "missing", "ready")
            }
        }
        Text {
            anchors.top: parent.top
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.topMargin: 12
            text: "UI TEST — synthetic data; nothing is saved"
            textFormat: Text.PlainText
            color: Commons.Color.foreground
        }
    }
    IpcHandler {
        target: "onboardingReview"
        function state(language: string, stage: int, helpers: string, core: string): string {
            return review.scenario(language, stage, helpers, core)
        }
        function terminal(open: bool) { loader.item.nativeSetupTerminalOpened = open }
        function firstRun(value: bool) { loader.item.firstRun = value }
        function profiles(count: int): string {
            if (count !== 0 && count !== 1) return "refused"
            loader.item.profiles = count === 1 ? [{id:"synthetic-profile",name:"UI fixture"}] : []
            review.result = "ready"
            return "ready"
        }
        function pending(value: bool): string {
            if (loader.item.step !== 2) return "refused"
            loader.item.routingSavePending = value
            loader.item.busy = value
            review.result = "ready"
            return "ready"
        }
        function unknown(value: bool): string {
            if (loader.item.step !== 2) return "refused"
            loader.item.routingSaveUnknown = value
            loader.item.nativeStatus = value ? loader.item.textFor("native.unknownOutcome") : ""
            loader.item.nativeCanContinue = !value
            loader.item.busy = false
            review.result = "ready"
            return "ready"
        }
        function delivery(value: string): string {
            if (!loader.item || ["current", "stale", "stale_manual"].indexOf(value) < 0) return "refused"
            var facts = loader.item.nativeSetupFacts
            loader.item.nativeSetupFacts = {state:facts.state, coreInstalled:facts.coreInstalled, delivery:value}
            review.result = "ready"
            return "ready"
        }
        function completion(value: string): string {
            if (!loader.item || loader.item.step !== 3 || !("completionPending" in loader.item)
                || ["waiting", "unconfirmed", "refused", "unknown", "ready"].indexOf(value) < 0) {
                review.result = "refused"; return "refused"
            }
            // Synthetic presentation only: no Service, native request or store.
            loader.item.completionPending = value === "waiting"
            loader.item.nativeCanContinue = ["waiting", "unknown"].indexOf(value) < 0
            loader.item.busy = false
            loader.item.nativeStatus = value === "unconfirmed" ? loader.item.textFor("onboarding.finish_unconfirmed")
                : value === "refused" ? loader.item.textFor("error.conflict")
                : value === "unknown" ? loader.item.textFor("native.unknownOutcome") : ""
            review.result = "ready"
            return "ready"
        }
        function capture(slug: string): string {
            if (["ready", "captured"].indexOf(review.result) < 0 || !/^[a-z0-9-]{1,48}$/.test(slug) || !review.output.startsWith(Quickshell.env("HOME") + "/.cache/ovt/onboarding.")) return "refused"
            review.result = "capturing"
            loader.grabToImage(function(image) {
                review.result = image.saveToFile(review.output + "/" + slug + ".png") ? "captured" : "failed"
            })
            return "started"
        }
        function result(): string { return review.result }
        function action(): string { return review.lastAction }
        function hide() { window.visible = false }
        function viewport(height: int): string {
            if ([480, 720].indexOf(height) < 0) return "refused"
            review.viewportHeight = height
            return "ready"
        }
        function scroll(where: string): string {
            var flick = review.control("onboardingScroll", loader.item)
            if (!flick || ["top", "bottom"].indexOf(where) < 0) return "refused"
            flick.contentY = where === "top" ? 0 : Math.max(0, flick.contentHeight - flick.height)
            return "ready"
        }
        function press(action: string): string {
            var names = {paste:"onboardingPaste", file:"onboardingFile", finish:"onboardingFinish", copy:"onboardingCopyCommand", refresh:"onboardingHelpersRefresh", components:"onboardingComponentSetup", next:"onboardingCoreContinue",back:"onboardingBack",routingNext:"onboardingRoutingContinue"}
            if (!Object.prototype.hasOwnProperty.call(names, action)) return "refused"
            var button = review.control(names[action], loader.item)
            if (!button || !button.visible || !button.enabled) return "unavailable"
            button.clicked()
            return "pressed"
        }
        function inspect(): string {
            var result = {visible:loader.item.visible,step:loader.item.step,coreState:loader.item.nativeCoreState,lastAction:review.lastAction}
            for (var name of ["onboardingPaste", "onboardingFile", "onboardingFinish", "onboardingHelpersRefresh", "onboardingComponentSetup", "onboardingCoreContinue", "onboardingBack", "onboardingRoutingContinue"]) {
                var button = review.control(name, loader.item)
                result[name] = button ? {text:button.text,visible:button.visible,enabled:button.enabled,primary:button.primary === true} : null
            }
            return JSON.stringify(result)
        }
        function finish() { Qt.quit() }
    }
}
