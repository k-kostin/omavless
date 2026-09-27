// SPDX-License-Identifier: MIT
// The mode transition is a presentation hold, never optimistic tunnel proof.
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const vm = require('node:vm')
const plugin = path.join(__dirname, '../plugin')
const service = fs.readFileSync(path.join(plugin, 'Service.qml'), 'utf8')
const panel = fs.readFileSync(path.join(plugin, 'Panel.qml'), 'utf8')
const snapshotParser = vm.createContext({})
const presentation = vm.createContext({})
vm.runInContext(fs.readFileSync(path.join(plugin, 'NativeSnapshot.js'), 'utf8'), snapshotParser)
vm.runInContext(fs.readFileSync(path.join(plugin, 'NativePresentation.js'), 'utf8'), presentation)

function serviceFunction(source, name, context) {
  const start = source.indexOf('  function ' + name + '(')
  const end = source.indexOf('\n  }', start) + 4
  assert(start >= 0 && end > start, name)
  vm.runInContext(source.slice(start, end), context)
}

function context() {
  const timer = {running: false, restart() { this.running = true }, stop() { this.running = false }}
  const c = vm.createContext({NativeSnapshot: snapshotParser, NativePresentation: presentation,
    nativeModeTransitionTimeout: timer,
    nativeModeTransition: {instanceId: 'synthetic', revision: 4, targetMode: 'global', operationId: 'mode-one'},
    nativeSnapshotFailed: false, nativePending: null, nativeActionRunning: false,
    nativeObservation: null,
    nativeSnapshot: {instanceId: 'synthetic', revision: 4, lastKnownActual: 'connected',
      desired: {connected: true, profileId: 'profile-one', generation: 2, mode: 'rule'},
      profiles: [{id: 'profile-one', name: 'Synthetic', favorite: false}], subscriptions: [], lastProfileId: 'profile-one'}})
  serviceFunction(service, 'finishNativeModeTransitionObservation', c)
  return c
}

function observed(c, revision, mode, state = 'connected') {
  c.nativeSnapshot.revision = revision
  c.nativeSnapshot.lastKnownActual = state
  c.nativeSnapshot.desired.mode = mode
  c.nativeObservation = {instanceId: 'synthetic', revision, lastKnownActual: state,
    availability: 'observed', desired: {connected: true, generation: 2, mode},
    facts: {ownedCoreRunning: true, desiredProfileMatchesOwned: true,
      ownedControllerConfigVerified: true, visibleMihomoCount: 1, visibleTunCount: 1}}
}

// An old coherent observation is still old evidence: it cannot terminate the
// animation or turn the requested destination into a confirmed mode.
const old = context()
observed(old, 4, 'rule')
old.finishNativeModeTransitionObservation()
assert(old.nativeModeTransition)
assert.equal(presentation.project(old.nativeSnapshot, old.nativeObservation, false,
  {action: 'mode'}, false).modeConfirmed, false)

const success = context()
observed(success, 5, 'global')
success.finishNativeModeTransitionObservation()
assert.equal(success.nativeModeTransition, null)

const restored = context()
observed(restored, 5, 'rule')
restored.finishNativeModeTransitionObservation()
assert.equal(restored.nativeModeTransition, null)
assert.equal(presentation.project(restored.nativeSnapshot, restored.nativeObservation, false).mode, 'rule')

const unavailable = context()
observed(unavailable, 5, 'global')
unavailable.nativeObservation.facts.ownedCoreRunning = false
unavailable.finishNativeModeTransitionObservation()
assert.equal(unavailable.nativeModeTransition, null)

const stale = context()
observed(stale, 5, 'global')
stale.nativeObservation.revision = 4
stale.finishNativeModeTransitionObservation()
assert(stale.nativeModeTransition)
stale.nativeSnapshot.instanceId = 'new-instance'
stale.finishNativeModeTransitionObservation()
assert.equal(stale.nativeModeTransition, null)

const pending = context()
observed(pending, 5, 'global')
pending.nativePending = {action: 'mode'}
pending.finishNativeModeTransitionObservation()
assert(pending.nativeModeTransition)

function completeMode(result, exitCode) {
  const c = context()
  c.root = c
  c.nativePending = {action: 'mode', instanceId: 'synthetic', revision: 4, operationId: 'mode-one'}
  c.nativeObservation = {older: true}
  c.nativeActionStdout = {text: ''}
  c.NativeSnapshot = {parseActionExit: () => result}
  c.finishNativeEditorAction = () => {}
  c.finishNativeSubscriptionAction = () => {}
  c.finishNativeRoutingAction = () => {}
  c.refreshAfterChange = () => { c.refreshCount++ }
  c.refreshCount = 0
  const block = service.slice(service.indexOf('    id: nativeActionProcess'))
  const start = block.indexOf('    onExited: function(exitCode) {')
  const end = block.indexOf('\n    }', start) + 6
  assert(start >= 0 && end > start)
  vm.runInContext('this.complete = ' + block.slice(start, end).replace('    onExited: ', ''), c)
  c.complete(exitCode)
  assert.equal(c.refreshCount, 1)
  assert.equal(c.nativeObservation, null)
  return c
}

const actionSuccess = completeMode({ok: true}, 0)
assert(actionSuccess.nativeModeTransition)
assert.equal(actionSuccess.nativeModeTransitionTimeout.running, true)
assert.equal(actionSuccess.nativePending, null)
const actionFailure = completeMode({ok: false, code: 'core_rejected'}, 74)
assert.equal(actionFailure.nativeModeTransition, null)
assert.equal(actionFailure.nativeActionCode, 'core_rejected')
const actionUnknown = completeMode(null, 73)
assert.equal(actionUnknown.nativeModeTransition, null)
assert.equal(actionUnknown.nativeOutcomeUnknown, true)
assert(actionUnknown.nativePending)

assert(service.includes('if (action === "mode") nativeModeTransition = {instanceId:nativeSnapshot.instanceId'))
assert(service.includes('if (result && result.ok) nativeModeTransitionTimeout.restart()'))
assert(service.includes('onTriggered: if (!root.nativePending && !root.nativeActionRunning) root.nativeModeTransition = null'))
assert(panel.includes('id: nativeModeTransitionCard'))
assert(panel.includes('visible: !vless.nativeModeSwitching && root.nativeView.state !== "disconnected"'))
assert(panel.includes('vless.nativeModeSwitching ? transitionIcon : nativeView.connected'))
assert(panel.includes('vless.nativeModeSwitching ? "native.modeSwitch.traffic" : "traffic.native_unavailable"'))
assert(panel.includes('visible: vless.nativeTrafficFresh && !vless.nativeModeSwitching'))
console.log('native mode transition: pending, settled, rollback, failure and unknown states passed')
