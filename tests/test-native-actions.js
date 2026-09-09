// SPDX-License-Identifier: MIT
// Actual QML JS boundary; synthetic data only, no socket or private store.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const parser = vm.createContext({});
vm.runInContext(fs.readFileSync(path.join(__dirname, '../plugin/NativeSnapshot.js'), 'utf8'), parser);
let count = 0;
function test(name, fn) { try { fn(); count++; } catch (e) { e.message = name + ': ' + e.message; throw e; } }
function frame(result) { return {api:'omavless.control', version:1, id:'synthetic-request', ok:true, revision:4, result}; }
function observation() {
  return frame({schemaVersion:1, scope:'local_runtime_observation', instanceId:'instance-one', transition:null,
    availability:'observed', desired:{connected:true, mode:'rule', generation:3}, lastKnownActual:'connected',
    manualRecoveryRequired:false, facts:{ownedCoreRunning:true, visibleMihomoCount:1, visibleTunCount:1,
      ownedControllerConfigVerified:true, desiredProfileMatchesOwned:true},
    verification:{serviceOwnership:false, tunOwnership:false, routes:false, dns:false, internet:false}});
}
function pending() { return {instanceId:'instance-one', operationId:'operation-one', action:'connect', revision:4}; }
function action() { return frame({schemaVersion:1, instanceId:'instance-one', operationId:'operation-one', action:'connect', applied:true}); }
function parseObservation(p) { return parser.parseObservation(JSON.stringify(p)); }
function parseAction(p, q=pending()) { return parser.parseAction(JSON.stringify(p), q); }
test('observed facts stay local and cached state remains separate', () => {
  const p=observation(); p.result.lastKnownActual='failed';
  const r=parseObservation(p);
  assert(r); assert.equal(r.lastKnownActual,'failed'); assert.equal(r.facts.ownedCoreRunning,true);
  assert.equal(r.revision,4); assert.equal(r.desired.generation,3);
  assert.equal(r.active,undefined); assert.equal(r.internet,undefined);
});
test('unavailable is null, not an empty/disconnected inventory', () => {
  const p=observation(); p.result.availability='unavailable'; p.result.facts=null;
  p.result.lastKnownActual='manualRecoveryRequired'; p.result.manualRecoveryRequired=true;
  const r=parseObservation(p); assert(r); assert.equal(r.facts,null);
  assert.equal(r.manualRecoveryRequired,true); assert.equal(r.desired.connected,true);
  p.result.facts={}; assert.equal(parseObservation(p),null);
});
test('exact observation schema, types, bounds and contradictory facts refuse', () => {
  const cases=[
    p=>p.version=2, p=>p.result.schemaVersion=2, p=>p.result.privateSecret='value',
    p=>p.result.verification.internet=true, p=>p.result.verification.extra=false,
    p=>p.result.facts.visibleMihomoCount=65, p=>p.result.facts.visibleTunCount=9,
    p=>p.result.facts.visibleTunCount=-1, p=>p.result.facts.visibleMihomoCount=0.5,
    p=>p.result.facts.ownedCoreRunning=false, p=>p.result.facts.desiredProfileMatchesOwned=false,
    p=>{p.result.facts.ownedControllerConfigVerified=false;p.result.facts.ownedCoreRunning=false;},
    p=>{p.result.facts.ownedControllerConfigVerified=false;p.result.desired.connected=false;},
    p=>p.result.manualRecoveryRequired=true, p=>p.result.lastKnownActual='unknown-state',
    p=>p.result.transition='cutoverPreparing', p=>p.result.desired.profileId='unrequested-private-id',
    p=>p.result.desired.mode='invalid', p=>p.result.desired.generation=9007199254740992,
    p=>p.result.availability='unknown', p=>p.result.facts=null, p=>p.result.instanceId='',
    p=>p.result.facts.ownedCoreRunning='true', p=>p.revision=-1, p=>delete p.result.verification
  ];
  for(const mutate of cases){const p=observation();mutate(p);assert.equal(parseObservation(p),null);}
});
test('visible inventories never imply named-core or TUN ownership', () => {
  const p=observation();p.result.facts.visibleMihomoCount=0;p.result.facts.visibleTunCount=0;
  assert(parseObservation(p)); // Owned child may be renamed; config may omit TUN.
  p.result.facts.ownedCoreRunning=false;p.result.facts.ownedControllerConfigVerified=false;
  p.result.facts.desiredProfileMatchesOwned=false;p.result.facts.visibleMihomoCount=3;p.result.facts.visibleTunCount=2;
  assert(parseObservation(p)); // Unrelated visible processes/interfaces are not ours.
});
test('metadata/observation join requires exact instance revision intent and generation', () => {
  const r=parseObservation(observation());
  const snapshot={instanceId:r.instanceId,revision:r.revision,lastKnownActual:r.lastKnownActual,desired:{...r.desired,profileId:'synthetic-profile'}};
  assert.equal(parser.coherent(snapshot,r),true);
  for(const mutate of [s=>s.instanceId='new-instance',s=>s.revision--,s=>s.revision++,
    s=>s.desired.generation++,s=>s.desired.mode='global',s=>s.desired.connected=false,
    s=>s.lastKnownActual='manualRecoveryRequired']) {
    const copy=JSON.parse(JSON.stringify(snapshot));mutate(copy);assert.equal(parser.coherent(copy,r),false);
  }
  assert.equal(parser.coherent(null,r),false);assert.equal(parser.coherent(snapshot,null),false);
});
test('action success requires exact pending identity and nonstale revision', () => {
  assert.equal(parseAction(action()).ok,true);
  const after=action();after.revision=5;assert.equal(parseAction(after).revision,5);
  for(const mutate of [p=>p.result.instanceId='new-instance',p=>p.result.operationId='other-operation',
    p=>p.result.action='disconnect',p=>p.result.applied=false,p=>p.result.schemaVersion=2,
    p=>p.result.secret='private',p=>p.revision=3,p=>p.ok=false,p=>delete p.result.operationId]) {
    const p=action();mutate(p);assert.equal(parseAction(p),null);
  }
  assert.equal(parseAction(action(),null),null);
  const q=pending();delete q.revision;assert.equal(parseAction(action(),q),null);
});
test('all canonical public error codes retain code and discard raw private message', () => {
  const source=fs.readFileSync(path.join(__dirname,'../crates/omavless-control-protocol/src/lib.rs'),'utf8');
  const start=source.indexOf('pub const fn as_str(');
  const body=source.slice(start,source.indexOf('\n    }',start));
  const codes=[...body.matchAll(/=> "([a-z_]+)"/g)].map(m=>m[1]);
  assert.equal(codes.length,14);
  for(const code of codes){
    const p={api:'omavless.control',version:1,id:'request',ok:false,revision:4,
      error:{code,message:'https://private.invalid/password?key=private-token',retryable:false}};
    const r=parseAction(p);assert(r,code);assert.equal(r.ok,false);assert.equal(r.code,code);
    assert(!JSON.stringify(r).includes('private'));assert.equal(r.message,undefined);
  }
  const p={api:'omavless.control',version:1,id:'request',ok:false,revision:4,error:{code:'private_fake_code',message:'secret',retryable:false}};
  assert.equal(parseAction(p),null);
});
test('malformed oversized and invalid Unicode inputs fail without throwing or echo', () => {
  const before=JSON.stringify(pending());
  for(const raw of ['', 'private-malformed', 'null', '[]', '{}', JSON.stringify(observation())+'{}',
    ' '.repeat(8193), '\uD800', JSON.stringify(action()).replace('connect','\uD800'),
    JSON.stringify(observation()).replace('instance-one','😀'.repeat(3000))]) {
    assert.equal(parser.parseObservation(raw),null);
    assert.equal(parser.parseAction(raw,pending()),null);
  }
  assert.equal(JSON.stringify(pending()),before);
});
function serviceHarness() {
  const source=fs.readFileSync(path.join(__dirname,'../plugin/Service.qml'),'utf8');
  const context=vm.createContext({NativeSnapshot:parser,nativeOwner:true,nativeSnapshotFailed:false,
    nativeSnapshot:{instanceId:'instance-one',revision:4,lastKnownActual:'disconnected',desired:{connected:false,mode:'rule',generation:3},
      profiles:[{id:'profile-one',missing:false,subscriptionId:'',favorite:false},{id:'profile-missing',missing:true,subscriptionId:''},
        {id:'profile-managed',missing:false,subscriptionId:'subscription-one',favorite:false}]},
    nativeObservation:{instanceId:'instance-one',revision:4,desired:{connected:false,mode:'rule',generation:3},
      availability:'observed',lastKnownActual:'disconnected',manualRecoveryRequired:false},nativePending:null,nativeOutcomeUnknown:false,
    nativeActionCode:'',_nativeOperationSerial:0,backendPath:'/synthetic/backend.sh',
    nativeActionProcess:{command:[],running:false},profiles:[{id:'legacy-profile',active:false}]});
  for(const name of ['nativeActionRunning','nativeFactsCurrent','nativeCanAct']) {
    const match=source.match(new RegExp('readonly property bool '+name+': ([\\s\\S]*?)(?=\\n  (?:readonly )?property|\\n  function)'));
    assert(match,name);
    vm.runInContext('Object.defineProperty(this,"'+name+'",{get:function(){return ('+match[1].trim()+');}});',context);
  }
  for(const name of ['requestNativeAction','requestNativeProfileAction','isValidName','reconcileNativeAction','acceptRefreshedNativeState']) {
    const start=source.indexOf('  function '+name+'(');
    const end=source.indexOf('\n  }',start)+4;
    assert(start>=0 && end>start,name);
    vm.runInContext(source.slice(start,end),context);
  }
  return context;
}
test('actual Service actions are pending, do not optimistically change state, and reject doubleclick', () => {
  for(const action of ['connect','disconnect','mode']) {
    const c=serviceHarness(), before=JSON.stringify([c.nativeSnapshot,c.nativeObservation,c.profiles]);
    assert.equal(c.requestNativeAction(action,'profile-one','global'),true);
    assert.equal(c.nativeActionProcess.running,true);
    assert.equal(c.nativePending.action,action);
    assert.equal(c.nativePending.instanceId,'instance-one');
    assert.equal(c.nativePending.revision,4);
    assert.equal(c.nativeActionProcess.command[2],'native-'+action);
    assert.equal(c.nativeActionProcess.command[5],c.nativePending.operationId);
    assert.equal(c.requestNativeAction('disconnect','',''),false);
    c.nativeActionProcess.running=false;
    assert.equal(c.requestNativeAction('disconnect','',''),false); // Pending persists after transport ends.
    assert.equal(JSON.stringify([c.nativeSnapshot,c.nativeObservation,c.profiles]),before);
  }
});
test('actual Service refuses unsupported, missing or stale action admission', () => {
  for(const [action,profile,mode] of [['raw','profile-one','rule'],['connect','unknown','rule'],
    ['connect','profile-missing','rule'],['mode','','invalid']]) {
    const c=serviceHarness();assert.equal(c.requestNativeAction(action,profile,mode),false);
    assert.equal(c.nativePending,null);assert.equal(c.nativeActionProcess.running,false);
  }
  for(const mutate of [c=>c.nativeOwner=false,c=>c.nativeSnapshotFailed=true,
    c=>c.nativeObservation.revision++,c=>c.nativeObservation.instanceId='new-instance',
    c=>c.nativeObservation.desired.generation++,c=>c.nativeObservation.availability='unavailable',
    c=>c.nativeObservation.manualRecoveryRequired=true]) {
    const c=serviceHarness();mutate(c);assert.equal(c.requestNativeAction('connect','profile-one','rule'),false);
    assert.equal(c.nativePending,null);
  }
});
test('actual Service unknown-outcome retry preserves the entire original command identity', () => {
  const c=serviceHarness();assert.equal(c.requestNativeAction('connect','profile-one','global'),true);
  const pendingBefore=JSON.stringify(c.nativePending), commandBefore=JSON.stringify(c.nativeActionProcess.command);
  assert.equal(c.reconcileNativeAction(),false); // Cannot overlap running request.
  c.nativeActionProcess.running=false;c.nativeOutcomeUnknown=true;
  c.nativeSnapshot.instanceId='new-instance';c.nativeSnapshot.revision=0;
  assert.equal(c.reconcileNativeAction(),true);
  assert.equal(JSON.stringify(c.nativePending),pendingBefore);
  assert.equal(JSON.stringify(c.nativeActionProcess.command),commandBefore);
  assert.equal(c._nativeOperationSerial,1);
});
test('same revision and instance never hide a cached manual-recovery mismatch', () => {
  const c=serviceHarness();
  c.nativeSnapshot.lastKnownActual='manualRecoveryRequired';
  assert.equal(c.nativeFactsCurrent,false);
  assert.equal(c.requestNativeAction('connect','profile-one','rule'),false);
  c.nativeObservation.lastKnownActual='manualRecoveryRequired';
  // Even contradictory cached boolean input cannot override snapshot refusal.
  assert.equal(c.nativeFactsCurrent,true);
  assert.equal(c.nativeCanAct,false);
  assert.equal(c.requestNativeAction('disconnect','',''),false);
  assert.equal(c.nativePending,null);
});
test('actual Service acknowledgement requires reviewed coherent facts and no running action', () => {
  const c=serviceHarness();c.requestNativeAction('connect','profile-one','global');
  c.nativeOutcomeUnknown=true;
  assert.equal(c.acceptRefreshedNativeState(),false);
  c.nativeActionProcess.running=false;c.nativeSnapshotFailed=true;
  assert.equal(c.acceptRefreshedNativeState(),false);
  c.nativeSnapshotFailed=false;c.nativeObservation.revision++;
  assert.equal(c.acceptRefreshedNativeState(),false);
  c.nativeObservation.revision--;c.nativeOutcomeUnknown=false;
  assert.equal(c.acceptRefreshedNativeState(),false);
  assert(c.nativePending);
  c.nativeOutcomeUnknown=true;c.nativeActionCode='conflict';
  const before=JSON.stringify([c.nativeSnapshot,c.nativeObservation]);
  assert.equal(c.acceptRefreshedNativeState(),true);
  assert.equal(c.nativePending,null);assert.equal(c.nativeOutcomeUnknown,false);assert.equal(c.nativeActionCode,'');
  assert.equal(JSON.stringify([c.nativeSnapshot,c.nativeObservation]),before);
});
test('profile mutations keep private input out of argv and preserve exact retry', () => {
  for(const [action,value,input] of [['profile-rename','Private <b>name</b>','profile-one\nPrivate <b>name</b>'],
    ['profile-favorite',true,'profile-one\non'],['profile-delete',null,'profile-one']]) {
    const c=serviceHarness(), before=JSON.stringify(c.nativeSnapshot);
    assert.equal(c.requestNativeProfileAction(action,'profile-one',value),true);
    assert.equal(c.nativePending.input,input);
    assert.equal(c.nativeActionProcess.command.length,6);
    assert(!JSON.stringify(c.nativeActionProcess.command).includes('profile-one'));
    assert(!JSON.stringify(c.nativeActionProcess.command).includes('Private'));
    assert.equal(c.nativeActionProcess.stdinEnabled,true);
    assert.equal(c.requestNativeProfileAction(action,'profile-one',value),false);
    const saved=JSON.stringify(c.nativePending);
    c.nativeActionProcess.running=false;c.nativeActionProcess.stdinEnabled=false;c.nativeOutcomeUnknown=true;
    assert.equal(c.reconcileNativeAction(),true);
    assert.equal(c.nativeActionProcess.stdinEnabled,true);
    assert.equal(JSON.stringify(c.nativePending),saved);
    assert.equal(JSON.stringify(c.nativeSnapshot),before);
    const reply=action === 'profile-delete' ? frame({schemaVersion:1,instanceId:'instance-one',operationId:c.nativePending.operationId,action,applied:true}) : null;
    if(reply) assert.equal(parseAction(reply,c.nativePending).ok,true);
  }
});
test('profile validation refuses managed rename/delete and malformed or stale intent', () => {
  for(const [action,id,value] of [['profile-rename','profile-managed','name'],['profile-delete','profile-managed',null],
    ['profile-delete','unknown',null],['profile-rename','profile-one',''],['profile-rename','profile-one','x'.repeat(81)],
    ['profile-rename','profile-one','two\nlines'],['profile-favorite','profile-one','true'],['raw','profile-one',null]]) {
    const c=serviceHarness();assert.equal(c.requestNativeProfileAction(action,id,value),false);assert.equal(c.nativePending,null);
  }
  const managed=serviceHarness();assert.equal(managed.requestNativeProfileAction('profile-favorite','profile-managed',true),true);
  const stale=serviceHarness();stale.nativeObservation.revision++;
  assert.equal(stale.requestNativeProfileAction('profile-delete','profile-one',null),false);
});
console.log(`${count} native action/observation tests passed`);
