// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync(__dirname+'/../plugin/Service.qml','utf8');
const panel = fs.readFileSync(__dirname+'/../plugin/Panel.qml','utf8');
const parser = vm.createContext({});
vm.runInContext(fs.readFileSync(__dirname+'/../plugin/NativeSnapshot.js','utf8'),parser);
const presentation=vm.createContext({});
vm.runInContext(fs.readFileSync(__dirname+'/../plugin/NativePresentation.js','utf8'),presentation);
function property(name) {
  const start=source.indexOf('  readonly property bool '+name+':');
  assert(start>=0);
  const rest=source.slice(start).split('\n');
  let expr=rest[0].split(':').slice(1).join(':');
  for(let i=1;i<rest.length && /^    (&&|\|\|)/.test(rest[i]);i++) expr+='\n'+rest[i];
  return expr;
}
function context() {
  const c=vm.createContext({NativeSnapshot:parser, NativePresentation:presentation, nativeSnapshotFailed:false, nativeFactsCurrent:true,nativePending:null,nativeQuitting:false,
    nativeObservation:{manualRecoveryRequired:false,availability:'observed',instanceId:'fixture-runtime',revision:7,lastKnownActual:'disconnected',desired:{connected:false,generation:0,mode:'rule'},facts:{ownedCoreRunning:false,visibleMihomoCount:0,ownedAuxiliaryMihomoCount:0,visibleTunCount:0,managedTunCount:0}},nativeActionRunning:false,nativeOutcomeUnknown:false,nativeBatchBusy:false,
    nativeSnapshot:{instanceId:'fixture-runtime',revision:7,lastKnownActual:'disconnected',profiles:[],subscriptions:[],lastProfileId:'',
      desired:{connected:false,generation:0,mode:'rule'},dns:{preset:'preset-default',source:'bundled'}},
    _nativeOperationSerial:0,backendPath:'/fixture/backend.sh',nativeActionProcess:{}});
  for(const name of ['nativeCanAct','nativeCanSaveDns'])
    vm.runInContext('Object.defineProperty(this,"'+name+'",{get:function(){return '+property(name)+'}})',c);
  const start=source.indexOf('  function requestNativeDns('),end=source.indexOf('\n  }',start)+4;
  vm.runInContext(source.slice(start,end),c);
  return c;
}
for(const preset of ['google','cloudflare']) {
  const c=context(),before=JSON.stringify(c.nativeSnapshot);
  assert(c.requestNativeDns(preset));
  assert.equal(c.nativePending.action,'dns-configure');
  assert.equal(c.nativePending.input,preset);
  assert.equal(c.nativePending.command[2],'native-dns-configure');
  assert.equal(JSON.stringify(c.nativeSnapshot),before);
  assert(!c.requestNativeDns('cloudflare'),'pending save must not be dispatched twice');
  const pending=c.nativePending;
  const frame={api:'omavless.control',version:1,id:'dns',ok:true,revision:8,result:{schemaVersion:1,
    instanceId:pending.instanceId,operationId:pending.operationId,action:'dns-configure',applied:true}};
  assert(parser.parseAction(JSON.stringify(frame),pending).ok);
  frame.result.operationId='other'; assert.equal(parser.parseAction(JSON.stringify(frame),pending),null);
  assert.equal(parser.parseActionExit('',pending,74).code,'invalid_argument');
  assert.equal(parser.parseActionExit('',pending,73),null);
}
for(const change of [c=>c.nativeFactsCurrent=false,c=>c.nativeSnapshot.desired.connected=true,
  c=>c.nativeSnapshot.lastKnownActual='starting',c=>c.nativeSnapshot.lastKnownActual='manualRecoveryRequired',
  c=>c.nativeSnapshot.dns=null,c=>c.nativeSnapshot.dns.source='custom',
  c=>c.nativeObservation.facts.ownedCoreRunning=true,
  c=>{c.nativeObservation.facts.managedTunCount=1;c.nativeObservation.facts.visibleTunCount=1},
  c=>c.nativeObservation.facts.visibleMihomoCount=1,
  c=>c.nativeOutcomeUnknown=true,c=>c.nativeActionRunning=true,c=>c.nativeBatchBusy=true,c=>c.nativeQuitting=true]) {
  const c=context();change(c);assert(!c.requestNativeDns('google'));assert.equal(c.nativePending,null);
}
for(const invalid of ['preset-default','quad9','https://private.invalid/token','',null]) {
  const c=context();assert(!c.requestNativeDns(invalid));assert.equal(c.nativePending,null);
}
for(const id of ['nativeDnsDefault','nativeDnsGoogle','nativeDnsCloudflare']) {
  assert(panel.includes(id+',') || panel.includes(id+']'));
  assert(panel.includes('id: '+id+';'));
}
assert(panel.includes('dns.custom') && panel.includes('dns.disconnect_first') && panel.includes('dns.next_connection'));
console.log('native DNS UI: fixed IDs, Off/fresh/pending fences, no optimistic state, exact outcome checked');
