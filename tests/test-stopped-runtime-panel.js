// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const state = require('../plugin/SetupState.js');
const i18n = require('../plugin/I18n.js');
const panel = fs.readFileSync(__dirname + '/../plugin/Panel.qml', 'utf8');
const page = fs.readFileSync(__dirname + '/../plugin/SetupPage.qml', 'utf8');
const service = fs.readFileSync(__dirname + '/../plugin/Service.qml', 'utf8');
let n = 0;
function test(name, run) { try { run(); n++; } catch (e) { e.message = name + ': ' + e.message; throw e; } }
const stopped = {state:'needs_runtime_start', coreInstalled:true};
function method(source, name) {
  const from=source.indexOf('  function '+name+'(');
  assert(from>=0,name);
  return source.slice(from,source.indexOf('\n  }',from)+4);
}
function exitHandler(source, id) {
  const from=source.indexOf('onExited: function(',source.indexOf('id: '+id+'\n'));
  assert(from>=0,id);
  return source.slice(from,source.indexOf('\n    }',from)+6).replace('onExited: function','function exited');
}
function setupHarness() {
  const c=vm.createContext({SetupState:state, facts:{...stopped},runtimeAttempt:'',
    nativeEffectBlocked:false,terminalOpened:false,launching:false,probing:false,
    probe:{running:false},launch:{running:false},startApp:{running:false},
    startOutput:{text:'ready\n'},statusOutput:{text:'ready\tpresent\n'},readyCount:0,ready(){this.readyCount++}});
  c.setup=c;
  Object.defineProperty(c,'state',{get:()=>c.facts.state});
  Object.defineProperty(c,'busy',{get:()=>c.probing||c.probe.running||c.launching||c.startApp.running||c.runtimeAttempt==='starting'});
  Object.defineProperty(c,'runtimePanelState',{get:()=>state.runtimePanelState(c.facts,c.runtimeAttempt,c.nativeEffectBlocked||c.terminalOpened)});
  for(const name of ['check','install']) vm.runInContext(method(page,name),c);
  vm.runInContext(exitHandler(page,'startApp').replace('function exited','function startExited'),c);
  vm.runInContext(exitHandler(page,'probe').replace('function exited','function probeExited'),c);
  return c;
}
test('actual controller Start remains single-submission across running-false notification',()=>{
  const c=setupHarness(); c.install('start-app');
  assert.equal(c.startApp.running,true);assert.equal(c.runtimePanelState,'appStarting');
  c.startApp.running=false; // Process.running changes before onExited publication.
  c.install('start-app');c.check();
  assert.equal(c.startApp.running,false);assert.equal(c.probe.running,false);
  c.startExited(0);
  assert.equal(c.runtimeAttempt,'verifying');assert.equal(c.runtimePanelState,'appStarting');
  assert.equal(c.probe.running,true);assert.equal(c.readyCount,0);
  c.probe.running=false;c.probeExited(0);
  assert.equal(c.runtimePanelState,'');assert.equal(c.readyCount,1);
  assert.equal(c.facts.state,'ready');
});
test('actual controller refusal/malformed success stays ordinary failure until explicit fresh check',()=>{
  for(const [code,raw] of [[1,''],[0,''],[0,'ready\nextra'],[2,'ready\n']]) {
    const c=setupHarness();c.startOutput.text=raw;c.install('start-app');
    c.startApp.running=false;c.startExited(code);
    assert.equal(c.runtimePanelState,'appStartFailed');assert.equal(c.readyCount,0);
    c.install('start-app');assert.equal(c.startApp.running,false); // no blind resend
    c.check();c.probe.running=false;c.statusOutput.text='needs_runtime_start\tpresent\n';c.probeExited(0);
    assert.equal(c.runtimePanelState,'appStopped');c.install('start-app');assert.equal(c.startApp.running,true);
  }
});
test('actual Start refuses wrong targets, blocked native work and unsettled terminal',()=>{
  for(const block of ['terminalOpened','nativeEffectBlocked','probing']) {
    const c=setupHarness();c[block]=true;c.install('start-app');
    assert.equal(c.startApp.running,false);assert.equal(c.launch.running,false);
  }
  for(const value of ['needs_attention','needs_package','needs_pair_mismatch','needs_selection','ready']) {
    const c=setupHarness();c.facts.state=value;c.install('start-app');
    assert.equal(c.startApp.running,false);assert.equal(c.launch.running,false);
  }
});
test('actual availability handler discards prior Connected metadata, never pending/replay tokens',()=>{
  const from=service.indexOf('onNativeRuntimeUnavailableChanged: {');
  const body=service.slice(from+'onNativeRuntimeUnavailableChanged: {'.length,service.indexOf('\n  }',from));
  const c=vm.createContext({nativeRuntimeUnavailable:true,nativeAvailabilityGeneration:0,
    nativeRuntimeRefreshing:false,nativeSnapshot:{lastKnownActual:'connected'},nativeObservation:{},
    nativeSnapshotFailed:false,nativePending:null,nativeOutcomeUnknown:false});
  vm.runInContext('(function(){'+body+'\n})()',c);
  assert.equal(c.nativeSnapshot,null);assert.equal(c.nativeObservation,null);
  assert.equal(c.nativeSnapshotFailed,true);assert.equal(c.nativeAvailabilityGeneration,1);
  c.nativeRuntimeUnavailable=false;vm.runInContext('(function(){'+body+'\n})()',c);
  assert.equal(c.nativeAvailabilityGeneration,2);assert.equal(c.nativeRuntimeRefreshing,false);
  assert.equal(c.nativeSnapshot,null);assert.equal(c.nativeOutcomeUnknown,false);
});
test('actual late status and observation replies across stopped/start epochs cannot restore caches',()=>{
  for(const id of ['statusProcess','nativeObservationProcess']) {
    let queued=0,parsed=0;
    const c=vm.createContext({nativeAvailabilityGeneration:2,nativeRuntimeUnavailable:false,
      nativeSnapshot:null,nativeObservation:null,nativeRuntimeRefreshing:true,
      _refreshAfterStatus:true,Qt:{callLater(){queued++}},
      refreshAfterChange(){},refreshNativeObservation(){},
      applyStatus(){parsed++;return true},NativeSnapshot:{parseObservation(){parsed++;return {}}},
      statusProcess:{availabilityGeneration:0},nativeObservationProcess:{availabilityGeneration:1}});
    c.root=c;vm.runInContext(exitHandler(service,id),c);c.exited(0);
    assert.equal(parsed,0);assert.equal(queued,1);
    assert.equal(c.nativeSnapshot,null);assert.equal(c.nativeObservation,null);
    assert.equal(c.nativeRuntimeRefreshing,true);
  }
  assert(!service.includes('onStarted: availabilityGeneration = root.nativeAvailabilityGeneration'));
  for(const [name,process] of [['refresh','statusProcess'],['refreshNativeObservation','nativeObservationProcess']]) {
    const body=method(service,name);
    assert(body.indexOf(process+'.availabilityGeneration = nativeAvailabilityGeneration')<body.indexOf(process+'.running = true'));
  }
});
test('completed verification failure is unavailable, not endless starting or Start retry',()=>{
  for(const [code,raw] of [[1,''],[0,'bad'],[0,'ready\tpresent\nextra']]) {
    const c=setupHarness();c.install('start-app');c.startApp.running=false;c.startExited(0);
    c.statusOutput.text=raw;c.probe.running=false;c.probeExited(code);
    assert.equal(c.runtimeAttempt,'verifyFailed');assert.equal(c.runtimePanelState,'unavailable');
    assert.equal(c.busy,false);assert.equal(c.readyCount,0);
    c.install('start-app');assert.equal(c.startApp.running,false);
    c.check();c.probe.running=false;c.statusOutput.text='ready\tpresent\n';c.probeExited(0);
    assert.equal(c.runtimePanelState,'');assert.equal(c.readyCount,1);
  }
});
test('fresh-RPC progress cannot hide current failed/recovery/unknown state',()=>{
  const marker='  readonly property string nativeTransitionKind: {';
  const from=panel.indexOf(marker)+marker.length,to=panel.indexOf('\n  }',from);
  const code='(function(){'+panel.slice(from,to)+'\n})()';
  for(const actual of ['failed','manualRecoveryRequired']) {
    const c=vm.createContext({runtimePanelState:'',nativeRealFailure:true,
      vless:{nativeRuntimeRefreshing:true,nativeOutcomeUnknown:false,nativeSnapshot:{lastKnownActual:actual}}});
    assert.equal(vm.runInContext(code,c),'');
  }
  const c=vm.createContext({runtimePanelState:'',nativeRealFailure:true,
    vless:{nativeRuntimeRefreshing:true,nativeOutcomeUnknown:true,nativeSnapshot:null}});
  assert.equal(vm.runInContext(code,c),'');
  c.vless.nativeOutcomeUnknown=false;assert.equal(vm.runInContext(code,c),'refreshState');
});
test('late observation cannot launch a new observation ahead of fresh post-Start status',()=>{
  const c=vm.createContext({nativeOwner:true,nativeRuntimeUnavailable:false,nativeSnapshot:null,
    nativeSnapshotFailed:true,nativeActionRunning:false,nativeAvailabilityGeneration:2,
    nativeRuntimeRefreshing:true,nativeObservationProcess:{availabilityGeneration:0,running:false},
    Qt:{callLater(f){f()}},NativeSnapshot:{parseObservation(){throw new Error('stale parsed')}},
    nativeObservation:null});
  c.root=c;vm.runInContext(method(service,'refreshNativeObservation'),c);
  vm.runInContext(exitHandler(service,'nativeObservationProcess'),c);c.exited(0);
  assert.equal(c.nativeObservationProcess.running,false);
  assert.equal(c.nativeRuntimeRefreshing,true);assert.equal(c.nativeObservation,null);
  c.nativeSnapshot={instanceId:'fresh',revision:1};c.nativeSnapshotFailed=false;
  assert.equal(c.refreshNativeObservation(),true);
  assert.equal(c.nativeObservationProcess.availabilityGeneration,2);
  assert.equal(c.nativeObservationProcess.running,true);
});
test('confirmed compatible stopped app opens the ordinary panel, even on cold boot', () => {
  for (const nativeOwner of [false,true]) for (const snapshot of [false,true]) {
    assert.equal(state.bootstrapRequired(stopped,nativeOwner,snapshot,false,false,false), false);
    assert.equal(state.runtimePanelState(stopped,'',false), 'appStopped');
  }
  for (const delivery of ['current','stale','stale_manual'])
    assert.equal(state.runtimePanelState({...stopped,delivery},'',false), 'appStopped');
});
test('unknown, incomplete and in-flight states are not classified as stopped', () => {
  for (const status of ['checking','needs_attention','needs_package','needs_activation',
      'needs_pair_mismatch','needs_selection','needs_broker_stopped','needs_dns_setup','ready'])
    assert.equal(state.runtimePanelState({state:status,coreInstalled:true},'',false), '');
  for (const coreInstalled of [false,null,undefined])
    assert.equal(state.runtimePanelState({...stopped,coreInstalled},'',false), '');
  assert.equal(state.runtimePanelState(stopped,'',true), '');
});
test('Start progress and failure remain distinct, without claiming stopped or ready', () => {
  assert.equal(state.runtimePanelState(stopped,'starting',false),'appStarting');
  assert.equal(state.runtimePanelState({state:'needs_attention'},'failed',false),'appStartFailed');
  assert.equal(state.runtimePanelState(stopped,'verifying',false),'appStarting');
  assert.equal(state.runtimePanelState(stopped,'failed',true),'');
});
test('Start reuses only the fixed native command, retains duplicate/terminal/action fences', () => {
  assert(page.includes('command: ["/usr/bin/omavless", "app", "start"]'));
  assert(page.includes('runtimeAttempt = "starting"'));
  assert(page.includes('runtimeAttempt = "verifying"'));
  assert(page.includes('runtimeAttempt = "failed"'));
  assert(page.includes('busy || launching || terminalOpened'));
  assert(page.includes('if (nativeEffectBlocked) return'));
  assert(!page.includes('systemctl'));
});
test('ordinary panel Start is visible, bordered, keyboard-reachable and has one target', () => {
  const from=panel.indexOf('id: nativeStartApp');
  assert(from>0);
  const button=panel.slice(from,panel.indexOf('\n            }',from));
  assert(button.includes('bordered: true'));
  assert(button.includes('focusable: true'));
  assert(button.includes('setupPage.install("start-app")'));
  for (const id of ['nativeStartApp','nativeStartCheck','nativeStartHelp']) {
    const from=panel.indexOf('id: '+id+'\n');
    const control=panel.slice(from,panel.indexOf('\n            }',from));
    assert(control.includes('Keys.onPressed: function(event) { root.handlePanelControlKey(event) }'),id);
  }
  assert(panel.includes('targets = [nativeStartApp, nativeStartCheck, nativeStartHelp].concat(targets)'));
  assert(panel.includes('root.runtimePanelState !== ""'));
});
test('stopped runtime invalidates stale RPC caches and blocks all semantic actions', () => {
  assert(service.includes('property bool nativeRuntimeUnavailable: false'));
  const from=service.indexOf('onNativeRuntimeUnavailableChanged:');
  const handler=service.slice(from,service.indexOf('\n  readonly property',from));
  assert(handler.includes('nativeSnapshot = null'));
  assert(handler.includes('nativeObservation = null'));
  assert(service.includes('nativeOwner && !nativeRuntimeUnavailable && !nativeSnapshotFailed'));
  assert(service.includes('if (nativeRuntimeUnavailable) { runtimeInventoryRequested(); return false }'));
  assert(panel.includes('root.runtimePanelState !== "" ? null : vless.nativeSnapshot'));
  assert(panel.includes('return textFor("native.profiles.after_start")'));
});
test('EN/RU describe app state, not installation or VPN connection', () => {
  for (const locale of ['en','ru']) for (const key of ['native.state.appStopped',
      'native.state.appStarting','native.state.appStartFailed','native.app.start',
      'native.profiles.after_start']) {
    const text=i18n.translate(key,locale,{});
    assert(!text.startsWith('Missing translation:'));
    assert(!/setup|установ|VPN connected/i.test(text));
  }
});
test('unavailable familiar controls are visibly disabled without changing ready styling', () => {
  const modes=panel.slice(panel.indexOf('id: nativeModeButtons'),panel.indexOf('id: nativeRoutingPresetSetting'));
  assert(modes.includes('opacity: root.runtimePanelState !== "" ? 0.45 : 1'));
  for (const id of ['nativeStartApp','nativeStartCheck','nativeSearch','nativeOpenAppButton']) {
    const from=panel.indexOf('id: '+id+'\n');
    const control=panel.slice(from,panel.indexOf('\n            }',from));
    assert(control.includes('opacity: enabled ? 1 : 0.45'),id);
  }
  const search=panel.slice(panel.indexOf('id: nativeSearch'),panel.indexOf('id: nativeProfiles\n'));
  assert(search.includes('enabled: root.runtimePanelState === ""'));
});
test('stopped Settings counts do not fabricate zero rules or subscriptions', () => {
  for(const id of ['nativeRoutingPresetSetting','nativeRoutingToolsSetting','nativeSubscriptionsSetting']) {
    const from=panel.indexOf('id: '+id);
    const row=panel.slice(from,panel.indexOf('onAction:',from));
    assert(row.includes('root.runtimePanelState !== "" ? root.runtimeDataUnavailableText()'),id);
  }
  const c=vm.createContext({runtimePanelState:'appStopped',textFor:key=>key});c.root=c;
  vm.runInContext(method(panel,'runtimeDataUnavailableText'),c);
  for(const state of ['appStopped','appStarting','appStartFailed']) {
    c.runtimePanelState=state;assert.equal(c.runtimeDataUnavailableText(),'native.profiles.after_start');
  }
  c.runtimePanelState='unavailable';assert.equal(c.runtimeDataUnavailableText(),'native.state.unavailable');
  vm.runInContext(method(panel,'nativeProviderUpdateDescription'),c);
  assert.equal(c.nativeProviderUpdateDescription(),'native.state.unavailable');
});
console.log('stopped runtime panel: '+n+' passed');
