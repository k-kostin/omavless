// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const root = path.join(__dirname, '..');
const parser = vm.createContext({});
vm.runInContext(fs.readFileSync(path.join(root, 'plugin/NativeSnapshot.js'), 'utf8'), parser);
let count = 0;
function test(name, action) { action(); count++; }
function fixture() {
  return {api:'omavless.control', version:1, id:'test', ok:true, revision:7, result:{
    schemaVersion:1, scope:'private_ui_metadata', instanceId:'instance-one', transition:null,
    desired:{connected:true, profileId:'profile-one', mode:'global', generation:8},
    lastKnownActual:'manualRecoveryRequired', healthFresh:false, liveHealth:'unavailable',
    profiles:[{id:'profile-one', name:'<b>Untranslated</b>', protocol:'vless', subscriptionId:'subscription-one', missing:false, favorite:true}],
    subscriptions:[{id:'subscription-one', name:'Provider display', updatedAt:1, profileCount:1, staleCount:0}],
    lastProfileId:'profile-one', startup:{configured:true, enabled:false, target:'last', profileId:'', mode:'rule'},
    onboardingComplete:true, routing:{storedPreset:'custom', customRuleCount:0}
  }};
}
function parse(value, previous) { return parser.parse(JSON.stringify(value), previous); }
test('native state is metadata, never live connected or disconnected inference', () => {
  const r = parse(fixture());
  assert.equal(r.healthFresh, false);
  assert.equal(r.liveHealth, 'unavailable');
  assert.equal(r.desired.connected, true);
  assert.equal(r.lastKnownActual, 'manualRecoveryRequired');
  assert.equal(r.profiles[0].name, '<b>Untranslated</b>');
  assert.equal(r.profiles[0].active, undefined);
});
test('invalid fields fail without echoing input or modifying previous snapshot', () => {
  const previous = parse(fixture()), before = JSON.stringify(previous);
  const mutations = [
    p => p.version = 2, p => p.result.schemaVersion = 2, p => p.ok = false,
    p => p.result.healthFresh = true, p => p.result.liveHealth = 'healthy',
    p => p.result.lastKnownActual = 'arbitrary-private-error',
    p => p.result.transition = 'cutoverPreparing', p => p.result.password = 'secret',
    p => p.result.profiles[0].uri = 'vless://private-secret',
    p => p.result.subscriptions[0].url = 'https://private.invalid/key',
    p => p.result.profiles.push(p.result.profiles[0]),
    p => p.result.profiles[0].subscriptionId = 'unknown',
    p => p.result.profiles[0].name = '\uD800', p => p.result.lastProfileId = 'unknown',
    p => p.result.desired.connected = false, p => p.revision = 9007199254740992,
    p => p.result.startup.enabled = 'false', p => p.result.routing.customRuleCount = 129,
    p => p.result.profiles[0].name = 'x'.repeat(81), p => p.result.instanceId = '',
    p => p.result.subscriptions[0].staleCount = 2, p => p.revision = -1
  ];
  for (const mutate of mutations) { const p=fixture(); mutate(p); assert.equal(parse(p, previous), null); }
  for (const raw of ['', '{private secret', 'null', '[]', ' '.repeat(262145)]) assert.equal(parser.parse(raw, previous), null);
  assert.equal(JSON.stringify(previous), before);
});
test('same-instance stale revisions rejected; daemon restart accepted', () => {
  const old = parse(fixture()), p=fixture(); p.revision = 6;
  assert.equal(parse(p, old), null);
  p.result.instanceId = 'instance-two';
  assert.equal(parse(p, old).revision, 6);
});
test('maximum records and Unicode scalar names remain bounded', () => {
  const p=fixture(); p.result.profiles=[]; p.result.subscriptions=[]; p.result.lastProfileId='';
  for(let i=0;i<64;i++) p.result.subscriptions.push({id:'s'+i,name:'😀'.repeat(80),updatedAt:0,profileCount:4,staleCount:0});
  for(let i=0;i<256;i++) p.result.profiles.push({id:'p'+i,name:'😀'.repeat(80),protocol:'vless',subscriptionId:'s'+(i%64),missing:false,favorite:false});
  assert.equal(parse(p).profiles.length,256);
  p.result.profiles.push(p.result.profiles[0]); assert.equal(parse(p),null);
});
test('actual Service applyStatus does not replace snapshot on failed parse', () => {
  const source=fs.readFileSync(path.join(root,'plugin/Service.qml'),'utf8');
  const start=source.indexOf('  function applyStatus(raw) {');
  const end=source.indexOf('  function rejectStatus()',start);
  const context=vm.createContext({NativeSnapshot:parser,nativeOwner:false,nativeSnapshot:null,nativeSnapshotFailed:false,
    lastError:'',_pollError:false, rejectStatus(){return false;}});
  vm.runInContext('function enterNativeReadOnly() { nativeOwner=true; }',context);
  vm.runInContext(source.slice(start,end),context);
  assert.equal(context.applyStatus(JSON.stringify(fixture())),true);
  const before=JSON.stringify(context.nativeSnapshot), p=fixture(); p.result.password='private';
  assert.equal(context.applyStatus(JSON.stringify(p)),false);
  assert.equal(context.nativeOwner,true);
  assert.equal(context.nativeSnapshotFailed,true);
  assert.equal(JSON.stringify(context.nativeSnapshot),before);
  assert(source.includes('exitCode === 70 || exitCode === 71'));
  for(const name of ['connectTo','disconnectAll','setRoutingMode','runControl','sampleTraffic','samplePing','refreshExitIp','pickConfigFile','pasteConfig']) {
    assert(new RegExp('function '+name+'\\([^\\n]*\\) \\{\\n    if \\(nativeOwner\\) return rejectNativeAction\\(\\)').test(source),name);
  }
  for(const name of ['_flushDrops','_flushMarkActive','_flushPendingSave']) {
    assert(new RegExp('function '+name+'\\([^\\n]*\\) \\{\\n    if \\(nativeOwner\\) return').test(source),name);
  }
});
test('native view keeps data plain and hides old interactive pages', () => {
  const source=fs.readFileSync(path.join(root,'plugin/Panel.qml'),'utf8');
  for(const page of ['main','settings','subscriptions','diagnostics']) assert(source.includes('visible: !vless.nativeOwner && root.page === "'+page+'"'));
  const view=source.slice(source.indexOf('id: nativeFlick'),source.indexOf('      AdvancedDiagnostics {'));
  assert(view.includes('textFormat: Text.PlainText'));
  assert(view.includes('PlainText {'));
  assert(!/(?:^|\s)Text \{/.test(view));
  assert(!view.includes('Text.AutoText'));
  assert(view.includes('focusable: true; bordered: true'));
  assert(source.includes('liveHealth: "unavailable", metadataUnavailable: vless.nativeSnapshotFailed'));
});
console.log(`${count} native snapshot tests passed`);
