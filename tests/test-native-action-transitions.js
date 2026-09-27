// SPDX-License-Identifier: MIT
// Transitional UI is not optimistic connection or DNS/Internet proof.
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const vm = require('node:vm')
const plugin = path.join(__dirname, '../plugin')
const service = fs.readFileSync(path.join(plugin, 'Service.qml'), 'utf8')
const panel = fs.readFileSync(path.join(plugin, 'Panel.qml'), 'utf8')
const presentation = vm.createContext({})
const parser = vm.createContext({})
vm.runInContext(fs.readFileSync(path.join(plugin, 'NativePresentation.js'), 'utf8'), presentation)
vm.runInContext(fs.readFileSync(path.join(plugin, 'NativeSnapshot.js'), 'utf8'), parser)

const name = 'finishNativeActionTransitionObservation'
const start = service.indexOf('  function ' + name + '(')
const end = service.indexOf('\n  }', start) + 4
assert(start >= 0 && end > start)
function context(action, actual, revision, activeId) {
  const timer = {running:true,stop(){this.running=false}}
  const c = vm.createContext({NativePresentation:presentation,NativeSnapshot:parser,
    nativeConnectionTransitionTimeout:timer,nativeMetadataTransitionTimeout:{stop(){}},
    nativePending:null,nativeActionRunning:false,nativeSnapshotFailed:false,
    nativeConnectionTransition:{instanceId:'fixture',revision:4,action,profileId:'new'},
    nativeMetadataTransition:null,
    nativeSnapshot:{instanceId:'fixture',revision,lastKnownActual:actual,
      desired:{connected:actual!=='disconnected',profileId:activeId,mode:'rule',generation:2},
      profiles:[{id:'new',name:'New',favorite:false}],subscriptions:[],lastProfileId:'new'},
    nativeObservation:{instanceId:'fixture',revision,lastKnownActual:actual,availability:'observed',
      desired:{connected:actual!=='disconnected',mode:'rule',generation:2},
      facts:{ownedCoreRunning:actual!=='disconnected',desiredProfileMatchesOwned:true,
        ownedControllerConfigVerified:true,visibleMihomoCount:actual==='disconnected'?0:1,
        visibleTunCount:actual==='disconnected'?0:1}}})
  vm.runInContext(service.slice(start,end),c)
  return c
}

const old=context('connect','connected',4,'new')
old.finishNativeActionTransitionObservation()
assert(old.nativeConnectionTransition,'old observation must not confirm new connection')
const wrong=context('connect','connected',5,'other')
wrong.finishNativeActionTransitionObservation()
assert(wrong.nativeConnectionTransition,'different server is not requested connection proof')
const connected=context('connect','connected',5,'new')
connected.finishNativeActionTransitionObservation()
assert.equal(connected.nativeConnectionTransition,null)
const stopping=context('disconnect','stopping',5,'new')
stopping.finishNativeActionTransitionObservation()
assert(stopping.nativeConnectionTransition,'stopping is not disconnected')
const disconnected=context('disconnect','disconnected',5,'')
disconnected.finishNativeActionTransitionObservation()
assert.equal(disconnected.nativeConnectionTransition,null)
const failed=context('connect','failed',5,'new')
failed.finishNativeActionTransitionObservation()
assert.equal(failed.nativeConnectionTransition,null,'actual failure must not be hidden')
const stale=context('connect','connected',5,'new')
stale.nativeObservation.revision=4
stale.finishNativeActionTransitionObservation()
assert(stale.nativeConnectionTransition,'stale observation is not proof')

function panelStateExpression(property) {
  const marker = '  readonly property ' + property + ': {'
  const from = panel.indexOf(marker)
  const to = panel.indexOf('\n  }\n', from)
  assert(from >= 0 && to > from, property)
  return panel.slice(from + marker.length, to)
}
const realFailureExpression = panelStateExpression('bool nativeRealFailure')
const transitionExpression = panelStateExpression('string nativeTransitionKind')
function visibleState(actual, overrides={}) {
  const vless = Object.assign({nativeOutcomeUnknown:false,nativeSnapshotFailed:false,
    nativeSnapshot:{lastKnownActual:actual,revision:5},nativeModeSwitching:false,
    nativeConnectionSwitching:false,nativeConnectionTransition:null}, overrides)
  const c = vm.createContext({vless,nativeRealFailure:false})
  c.nativeRealFailure = vm.runInContext('(function(){' + realFailureExpression + '\n})()',c)
  const kind = vm.runInContext('(function(){' + transitionExpression + '\n})()',c)
  return {failure:c.nativeRealFailure,kind}
}
for (const [actual,kind] of [['starting','starting'],['reconnecting','reconnecting'],['stopping','stopping']])
  assert.deepEqual(visibleState(actual),{failure:false,kind})
assert.deepEqual(visibleState('failed'),{failure:true,kind:''})
assert.deepEqual(visibleState('manualRecoveryRequired'),{failure:true,kind:''})
assert.deepEqual(visibleState('reconnecting',{nativeOutcomeUnknown:true}),{failure:true,kind:''})
assert.deepEqual(visibleState('reconnecting',{nativeSnapshotFailed:true}),{failure:true,kind:''})
assert.deepEqual(visibleState('failed',{nativeConnectionSwitching:true,
  nativeConnectionTransition:{revision:5,action:'connect',switchingProfile:false}}),
  {failure:false,kind:'connect'})
assert.deepEqual(visibleState('failed',{nativeConnectionSwitching:true,
  nativeConnectionTransition:{revision:4,action:'connect',switchingProfile:false}}),
  {failure:true,kind:''})

for(const kind of ['connect','server','disconnect','starting','reconnecting','stopping'])
  assert(panel.includes('native.progress.' + kind) || panel.includes('"' + kind + '"'),kind)
assert(panel.includes('if (nativeRealFailure) return ""'))
assert(panel.includes('if (vless.nativeSnapshot.lastKnownActual === "manualRecoveryRequired") return true'))
assert(panel.includes('nativeStatusWaiting ? transitionIcon'))
assert(panel.includes('root.nativeMetadataAction.indexOf("subscription-") === 0'))
assert(panel.includes('root.nativeMetadataAction.indexOf("profile-") === 0'))
assert(service.includes('action:action, targetId:id, command:args, input:input'))
assert(panel.includes('root.nativeMetadataTargetId === nativeRow.modelData.subscription.id'))
assert(panel.includes('vless.nativeSubscriptionStatusId === nativeRow.modelData.subscription.id'))
assert(panel.includes('["saved", "refreshFailed", "fetchFailed"].indexOf(vless.nativeSubscriptionCode) >= 0'))
assert(panel.includes('readonly property bool nativeStatusWaiting: nativeTransitionKind !== ""'))
assert(panel.includes('readonly property var nativeLifecyclePending: vless.nativePending'))
assert(panel.includes('vless.nativeSnapshotFailed, nativeLifecyclePending, vless.nativeOutcomeUnknown)'))
assert(panel.includes('root.nativeMetadataTargetId !== ""'))
assert(service.includes('id: nativeSubscriptionSuccessTimeout'))
assert(service.includes('var contextualMetadataAction = subscriptionAction'))
assert(service.includes('contextualMetadataAction && result.code !== "manual_recovery_required"'))
assert(service.includes('root.nativeMetadataErrorAction = pendingAction.action'))
assert.equal((service.match(/\bonNativePendingChanged:/g) || []).length, 1)
assert(panel.includes('vless.nativeMetadataErrorAction.indexOf("profile-") === 0'))
assert(panel.includes('vless.nativeMetadataErrorAction.indexOf("subscription-") !== 0'))
assert(panel.includes('root.nativeTransitionKind !== "disconnect"\n              && root.nativeTransitionKind !== "stopping"'))
assert(!panel.includes('visible: vless.nativeActionRunning && !vless.nativeModeSwitching; text: root.textFor("native.pending")'))
console.log('native action transitions: observation and reconnect/failure states plus contextual UI contracts passed')
