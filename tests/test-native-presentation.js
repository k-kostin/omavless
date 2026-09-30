// SPDX-License-Identifier: MIT
const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm'),path=require('node:path');
const p=vm.createContext({});vm.runInContext(fs.readFileSync(path.join(__dirname,'../plugin/NativePresentation.js'),'utf8'),p);
function fixture(){
  const snapshot={instanceId:'test',revision:1,lastKnownActual:'connected',lastProfileId:'one',
    desired:{connected:true,profileId:'one',mode:'global',generation:1},
    profiles:[{id:'one',name:'Synthetic <b>name</b>',favorite:false}],subscriptions:[]};
  const observation={instanceId:'test',revision:1,lastKnownActual:'connected',availability:'observed',
    desired:{connected:true,mode:'global',generation:1},facts:{ownedCoreRunning:true,desiredProfileMatchesOwned:true,
      ownedControllerConfigVerified:true,visibleMihomoCount:1,visibleTunCount:1}};
  return {snapshot,observation};
}
let n=0;function test(name,f){try{f();n++;}catch(e){e.message=name+': '+e.message;throw e;}}
test('connected is observed ownership, not desired intent alone',()=>{
  const {snapshot:s,observation:o}=fixture();const r=p.project(s,o,false);
  assert.equal(r.connected,true);assert.equal(r.activeId,'one');assert.equal(r.state,'connected');
  assert.equal(r.profiles[0].name,'Synthetic <b>name</b>');assert(!('internet' in r));
  assert.equal(p.project(s,null,false).connected,false);assert.equal(p.project(s,o,true).state,'unavailable');
});
test('stale identity/revision/generation and inconsistent modes never imply connected',()=>{
  for(const change of [o=>o.instanceId='other',o=>o.revision++,o=>o.desired.generation++,o=>o.desired.mode='rule',o=>o.desired.connected=false,o=>o.availability='unavailable',o=>o.lastKnownActual='disconnected']) {
    const {snapshot:s,observation:o}=fixture();change(o);assert.equal(p.project(s,o,false).state,'unavailable');
  }
});
test('missing ownership or duplicate core/tun is unknown, not healthy',()=>{
  for(const [key,val] of [['ownedCoreRunning',false],['desiredProfileMatchesOwned',false],['ownedControllerConfigVerified',false],['visibleMihomoCount',2],['visibleTunCount',0]]) {
    const {snapshot:s,observation:o}=fixture();o.facts[key]=val;const r=p.project(s,o,false);assert.equal(r.connected,false);assert.equal(r.activeId,'');
  }
});
test('disconnected requires absent runtime resources',()=>{
  const {snapshot:s,observation:o}=fixture();s.lastKnownActual=o.lastKnownActual='disconnected';s.desired.connected=o.desired.connected=false;
  Object.assign(o.facts,{ownedCoreRunning:false,visibleMihomoCount:0,visibleTunCount:0});
  assert.equal(p.project(s,o,false).state,'disconnected');o.facts.visibleTunCount=1;assert.equal(p.project(s,o,false).state,'unavailable');
});
test('only an attributed no-TUN auxiliary core is excluded from tunnel health',()=>{
  const {snapshot:s,observation:o}=fixture();
  o.facts.visibleMihomoCount=2; o.facts.ownedAuxiliaryMihomoCount=1;
  assert.equal(p.project(s,o,false).connected,true);
  assert.equal(o.facts.visibleMihomoCount,2);
  o.facts.visibleMihomoCount=3; assert.equal(p.project(s,o,false).state,'unavailable');
  s.lastKnownActual=o.lastKnownActual='disconnected'; s.desired.connected=o.desired.connected=false;
  Object.assign(o.facts,{ownedCoreRunning:false,visibleMihomoCount:1,visibleTunCount:0});
  assert.equal(p.project(s,o,false).state,'disconnected');
  o.facts.ownedAuxiliaryMihomoCount=2; assert.equal(p.project(s,o,false).state,'unavailable');
});
test('search is bounded, preserves names and does not mutate source order',()=>{
  const list=[{name:'First',favorite:false},{name:'Тест',favorite:true},{name:'<b>First</b>',favorite:false}];
  assert.equal(p.filtered(list,'тЕСТ').length,1);assert.equal(p.filtered(list,'').at(0).name,'Тест');assert.equal(list[0].name,'First');
  assert.equal(p.filtered(Array(300).fill(list[0]),'').length,256);assert.equal(p.filtered(list,'<b>')[0].name,'<b>First</b>');
});
test('failure and transition states stay distinct from connected',()=>{
  for(const state of ['starting','reconnecting','stopping','failed','manualRecoveryRequired']) {
    const {snapshot:s,observation:o}=fixture();s.lastKnownActual=o.lastKnownActual=state;
    const result=p.project(s,o,false);assert.equal(result.state,state);assert.equal(result.connected,false);assert.equal(result.activeId,'');
  }
  const {snapshot:s,observation:o}=fixture();o.facts=null;assert.equal(p.project(s,o,false).state,'unavailable');
  assert.equal(p.project(null,null,false).state,'unavailable');
});
test('active profile is never inferred from selection, name, last-used or unavailable state',()=>{
  const {snapshot:s,observation:o}=fixture();s.profiles.push({id:'two',name:s.profiles[0].name});s.lastProfileId='two';
  let view=p.project(s,o,false);assert.equal(p.activeProfile(view).id,'one');
  view.activeId='removed';assert.equal(p.activeProfile(view),null);
  for(const state of ['disconnected','unavailable','starting','failed']) {
    view={...view,activeId:'one',state};assert.equal(p.activeProfile(view),null);
  }
  assert.equal(p.activeProfile(null),null);
});
test('foreign TUNs do not become our recovery state or connected device',()=>{
  const {snapshot:s,observation:o}=fixture();
  Object.assign(o.facts,{visibleTunCount:2,managedTunCount:1});
  assert.equal(p.project(s,o,false).state,'connected');
  o.facts.managedTunCount=0;assert.equal(p.project(s,o,false).state,'unavailable');
  s.lastKnownActual=o.lastKnownActual='disconnected';s.desired.connected=o.desired.connected=false;
  Object.assign(o.facts,{ownedCoreRunning:false,visibleMihomoCount:0,managedTunCount:0});
  assert.equal(p.project(s,o,false).state,'disconnected');
  for(const invalid of [-1,3,0.5,null,'0']) {
    o.facts.managedTunCount=invalid; assert.equal(p.project(s,o,false).state,'unavailable');
  }
  o.facts.managedTunCount=1;assert.equal(p.project(s,o,false).state,'unavailable');
});
test('desired mode is never confirmed by pending, unknown, stale or recovery state',()=>{
  const {snapshot:s,observation:o}=fixture();
  assert.equal(p.project(s,o,false).modeConfirmed,true);
  for(const pending of [{action:'mode'},{action:'connect'},{action:'disconnect'}]) {
    const result=p.project(s,o,false,pending,false);
    assert.equal(result.modeConfirmed,false);assert.equal(result.mode,'global');
  }
  assert.equal(p.project(s,o,false,null,true).modeConfirmed,false);
  for(const state of ['starting','reconnecting','stopping','failed','manualRecoveryRequired']) {
    s.lastKnownActual=o.lastKnownActual=state;
    assert.equal(p.project(s,o,false).modeConfirmed,false);
  }
  s.lastKnownActual=o.lastKnownActual='connected';o.revision++;
  assert.equal(p.project(s,o,false).modeConfirmed,false);
  assert.equal(p.project(s,null,false).modeConfirmed,false);
});
test('settled rollback confirms the restored mode, not the attempted desired mode',()=>{
  const {snapshot:s,observation:o}=fixture();
  s.desired.mode='direct';s.revision++;
  assert.equal(p.project(s,o,false).modeConfirmed,false);
  s.desired.mode=o.desired.mode='rule';o.revision=s.revision;
  const result=p.project(s,o,false);assert.equal(result.mode,'rule');assert.equal(result.modeConfirmed,true);
  assert.equal(result.dnsVerified,undefined);assert.equal(result.routesVerified,undefined);
  s.lastKnownActual=o.lastKnownActual='disconnected';s.desired.connected=o.desired.connected=false;
  Object.assign(o.facts,{ownedCoreRunning:false,visibleMihomoCount:0,visibleTunCount:0});
  assert.equal(p.project(s,o,false).modeConfirmed,true); // saved disconnected preference only
});
test('production selector binds confirmation as well as desired mode without new actions',()=>{
  const panel=fs.readFileSync(path.join(__dirname,'../plugin/Panel.qml'),'utf8');
  assert(panel.includes('vless.nativeSnapshotFailed, vless.nativePending, vless.nativeOutcomeUnknown)'));
  for(const mode of ['global','rule','direct']) {
    assert(panel.includes('foreground: root.nativeView.modeConfirmed && root.nativeView.mode === "'+mode+'"'));
  }
});
console.log('native presentation: '+n+' passed');
