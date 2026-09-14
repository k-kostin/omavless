// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict'), fs = require('node:fs'), vm = require('node:vm');
const parser = vm.createContext({});
vm.runInContext(fs.readFileSync(__dirname + '/../plugin/NativeSnapshot.js', 'utf8'), parser);
const source = fs.readFileSync(__dirname + '/../plugin/Service.qml', 'utf8');
const fixture = () => ({schemaVersion:1, scope:'native_configuration', runtime:{implementation:'rust', version:'0.1.0', lastKnownState:'disconnected', routingTransactionPending:false}, configuration:{inventory:{profiles:2,favorites:1,subscriptions:1,customRules:0},routing:{preset:'custom',configured:true,lastManualRuleUpdate:0},startup:{configured:true,enabled:false,target:'last',mode:'rule'},updates:{latestSubscription:0},onboardingComplete:true},coverage:{privateStoreValidated:true,liveHostObservation:false,controllerQuery:false,loginActivationVerified:false}});
const frame = r => JSON.stringify({api:'omavless.control',version:1,id:'test',ok:true,revision:7,result:r});
const modernFixture = (observed = true) => {
 const r = fixture(); r.schemaVersion = 2; r.scope = 'native_support';
 r.localObservation = {availability:observed ? 'observed' : 'unavailable', desired:{connected:observed,mode:'global'},
  facts:observed ? {ownedCoreRunning:true,visibleMihomoCount:1,ownedAuxiliaryMihomoCount:0,visibleTunCount:1,ownedControllerConfigVerified:true,desiredProfileMatchesOwned:true} : null,
  verification:{serviceOwnership:false,tunOwnership:false,routes:false,dns:false,internet:false}};
 Object.assign(r.coverage,{liveHostObservation:observed,controllerQuery:observed,coreSetupVerified:false,serviceEnablementVerified:false,loadedPolicyCounts:false,fileReadiness:false});
 return r;
};
let count=0;
const hostFixture = () => {
 const r = modernFixture(); r.schemaVersion=3;
 r.host={core:{installed:true,fileNetworkCapabilities:true,tunDevicePresent:true},
  runtimeService:{loaded:true,active:true,enabled:true,ownsCurrentProcess:true},
  loginService:{loaded:true,active:false,enabled:true,ownsCurrentProcess:null},
  files:{store:true,template:true,generatedConfig:true,runtimeUnit:true,loginUnit:true},
  configuredPolicy:{basis:'active_config',rules:42,providers:3}};
 Object.assign(r.coverage,{coreSetupVerified:true,serviceEnablementVerified:true,fileReadiness:true});
 return r;
};
function test(name, fn) { try { fn(); count++; } catch(e) { e.message = name + ': ' + e.message; throw e; } }
test('reports accept stable and release-candidate runtime versions across supported schemas',()=>{
 const cargo=fs.readFileSync(__dirname+'/../Cargo.toml','utf8');
 const current=cargo.match(/^version = "([^"]+)"$/m)[1];
 for(const version of ['0.1.0','0.8.0','0.8.0-rc.1','0.8.0-rc.12',current]) {
  for(const make of [fixture,modernFixture,hostFixture]) {
   const r=make();r.runtime.version=version;
   const result=parser.configurationReport(frame(r),7);assert(result,version);
   assert.equal(JSON.parse(result).runtime.version,version);
  }
 }
});
test('runtime version remains bounded machine vocabulary, not arbitrary release text',()=>{
 for(const version of ['0.8.0-rc.0','0.8.0-rc.01','0.8.0-rc.','0.8.0-RC.1','0.8.0-beta.1',
  '0.8.0-rc.1+private','0.8.0\n','0.8.0-rc.1/private','v0.8.0','0.8.0-rc.1 password=secret',
  '1'.repeat(33)+'.0.0',null,800]) {
  const r=hostFixture();r.runtime.version=version;
  assert.equal(parser.configurationReport(frame(r),7),null);
 }
});
test('v3 host facts preserve bounded setup and configured counts without network claims',()=>{
 const r=hostFixture(), result=JSON.parse(parser.configurationReport(frame(r),7));
 assert.equal(result.schemaVersion,3); assert.equal(result.host.configuredPolicy.rules,42);
 assert.equal(result.host.runtimeService.ownsCurrentProcess,true);
 assert.equal(result.coverage.loadedPolicyCounts,false);
 assert(Object.values(result.localObservation.verification).every(v=>v===false));
 assert(JSON.stringify(result).length<4096);
});
test('v3 missing host facts remain unavailable not healthy zero',()=>{
 const r=hostFixture(); Object.keys(r.host).forEach(k=>r.host[k]=null);
 Object.assign(r.coverage,{coreSetupVerified:false,serviceEnablementVerified:false,fileReadiness:false});
 const result=JSON.parse(parser.configurationReport(frame(r),7));
 assert(Object.values(result.host).every(v=>v===null));
});
test('v3 rejects unknown nested private data and incorrect coverage',()=>{
 for(const section of ['host','host.core','host.runtimeService','host.loginService','host.files','host.configuredPolicy']) {
  const r=hostFixture(); let node=r; for(const p of section.split('.')) node=node[p]; node.private='private-token';
  assert.equal(parser.configurationReport(frame(r),7),null);
 }
 for(const [path,value] of [['host.configuredPolicy.rules',100001],['host.configuredPolicy.providers',1025],
  ['host.configuredPolicy.basis','private-token'],['host.runtimeService.active',false],['host.loginService.ownsCurrentProcess',true],
  ['host.core.installed',false],['host.core.fileNetworkCapabilities','yes'],['host.files.store','private-path'],
  ['coverage.coreSetupVerified',false],['coverage.serviceEnablementVerified',false],['coverage.fileReadiness',false],
  ['coverage.loadedPolicyCounts',true],['schemaVersion',4]]) {
  const r=hostFixture(); const parts=path.split('.'); let node=r; for(const p of parts.slice(0,-1)) node=node[p]; node[parts.at(-1)]=value;
  assert.equal(parser.configurationReport(frame(r),7),null);
 }
});
test('v3 negatively observed core or files do not mean missing observation',()=>{
 const r=hostFixture(); Object.assign(r.host.core,{installed:false,fileNetworkCapabilities:false,tunDevicePresent:false});
 Object.keys(r.host.files).forEach(k=>r.host.files[k]=false);
 const result=JSON.parse(parser.configurationReport(frame(r),7));
 assert.equal(result.coverage.coreSetupVerified,true); assert.equal(result.host.core.installed,false);
 assert.equal(result.coverage.fileReadiness,true); assert.equal(result.host.files.store,false);
 r.host.files.store=null; r.coverage.fileReadiness=false;
 assert(parser.configurationReport(frame(r),7));
});
test('configuration scope and no live claims', () => {
 const report = parser.configurationReport(frame(fixture()),7); assert(report);
 assert.equal(JSON.parse(report).coverage.liveHostObservation,false);
 assert(!report.includes('instanceId')); assert(!report.includes('revision'));
});
test('v2 observed facts remain separate from cached state and unverified network', () => {
 const r = modernFixture(); r.runtime.lastKnownState = 'failed';
 const result = JSON.parse(parser.configurationReport(frame(r),7));
 assert.equal(result.schemaVersion,2); assert.equal(result.runtime.lastKnownState,'failed');
 assert.equal(result.localObservation.facts.visibleMihomoCount,1);
 assert(Object.values(result.localObservation.verification).every(v => v === false));
 assert.equal(result.coverage.loginActivationVerified,false);
 for(const forbidden of ['instanceId','profileId','generation','revision', '://']) assert(!JSON.stringify(result).includes(forbidden));
 assert(JSON.stringify(result).length < 4096);
});
test('v2 unavailable sample stays null instead of healthy zero counts', () => {
 const result = JSON.parse(parser.configurationReport(frame(modernFixture(false)),7));
 assert.equal(result.localObservation.availability,'unavailable');
 assert.equal(result.localObservation.facts,null); assert.equal(result.coverage.liveHostObservation,false);
});
test('v2 rejects unexpected nested fields and incompatible schema', () => {
 for(const section of ['', 'localObservation', 'localObservation.desired', 'localObservation.facts', 'localObservation.verification', 'coverage']) {
  const r=modernFixture(); let node=r; for(const p of section.split('.').filter(Boolean)) node=node[p]; node.secret='private-token';
  assert.equal(parser.configurationReport(frame(r),7),null);
 }
 for(const n of [0,3,'2',null]) { const r=modernFixture(); r.schemaVersion=n; assert.equal(parser.configurationReport(frame(r),7),null); }
 assert.equal(parser.configurationReport(frame(modernFixture()),8),null);
});
test('v2 rejects inconsistent facts inflated counts and unearned verification', () => {
 for(const [path,value] of [['localObservation.availability','private-token'],['localObservation.desired.mode','private-token'],
  ['localObservation.desired.connected',false],['localObservation.facts.ownedCoreRunning',false],
  ['localObservation.facts.visibleMihomoCount',65],['localObservation.facts.visibleTunCount',9],
  ['localObservation.facts.ownedAuxiliaryMihomoCount',2],['localObservation.facts.ownedControllerConfigVerified','yes'],
  ['localObservation.verification.dns',true],['localObservation.verification.serviceOwnership',true],
  ['coverage.liveHostObservation',false],['coverage.controllerQuery',false],['coverage.coreSetupVerified',true],
  ['coverage.loadedPolicyCounts',true],['coverage.fileReadiness',true],['coverage.serviceEnablementVerified',true]]) {
  const r=modernFixture(); const parts=path.split('.'); let node=r; for(const p of parts.slice(0,-1)) node=node[p]; node[parts.at(-1)]=value;
  assert.equal(parser.configurationReport(frame(r),7),null);
 }
 const r=modernFixture(false); r.localObservation.facts=modernFixture().localObservation.facts;
 assert.equal(parser.configurationReport(frame(r),7),null);
});
test('strict shape across all nested fields', () => {
 for (const section of ['', 'runtime','configuration','coverage','configuration.inventory','configuration.routing','configuration.startup','configuration.updates']) {
  const r=fixture(); let node=r; for(const key of section.split('.').filter(Boolean)) node=node[key]; node.private='private-token';
  assert.equal(parser.configurationReport(frame(r),7),null);
 }
});
test('private preset is replaced with category', () => {
 const r=fixture(); r.configuration.routing.preset='private-provider-name';
 const report=parser.configurationReport(frame(r),7); assert(report); assert(!report.includes('private-provider-name'));
});
test('bounds enum and type rejection', () => {
 for(const [path,value] of [['runtime.version','private-token'],['runtime.lastKnownState','private-token'],['coverage.liveHostObservation',true],['configuration.inventory.profiles',257],['configuration.inventory.favorites',3],['configuration.startup.mode','direct'],['configuration.startup.target','private-token'],['configuration.updates.latestSubscription',-1],['configuration.onboardingComplete','yes']]) {
  const r=fixture(), parts=path.split('.'); let target=r; for(const p of parts.slice(0,-1)) target=target[p]; target[parts.at(-1)]=value;
  assert.equal(parser.configurationReport(frame(r),7),null);
 }
 assert.equal(parser.configurationReport(frame(fixture()),8),null);
 assert.equal(parser.configurationReport('password=private-token',7),null);
 assert.equal(parser.configurationReport('x'.repeat(262145),7),null);
});
function context() {
 const c=vm.createContext({NativeSnapshot:parser,nativeFactsCurrent:true,panelVisible:true,nativeSnapshot:{instanceId:'instance',revision:7},nativeSupportBusy:false,copying:false,nativeSupportStatus:'',_nativeSupportRead:null,_nativeSupportCopy:false,_nativeSupportGeneration:2,backendPath:'/synthetic/backend.sh',nativeSupportComponent:{createObject:(_,p)=>({...p})},copied:[]});
 c.root=c; c.copyText=value=>{ c.copied.push(value); return true; };
 for(const name of ['copyNativeConfigurationReport','finishNativeConfigurationReport']) {
  const start=source.indexOf('  function '+name+'('),end=source.indexOf('\n  }',start)+4; assert(start>=0); vm.runInContext(source.slice(start,end),c);
 } return c;
}
test('fixed read command and explicit successful clipboard handoff',()=>{
 const c=context(); assert(c.copyNativeConfigurationReport()); assert.equal(c._nativeSupportRead.command.join('|'),'bash|/synthetic/backend.sh|native-support-report');
 assert(c.finishNativeConfigurationReport('instance',7,2,0,frame(fixture()))); assert.equal(c.copied.length,1); assert(c._nativeSupportCopy);
});
test('no copying on stale owner revision close or failed malformed output',()=>{
 for(const change of [c=>c.nativeSnapshot.instanceId='new',c=>c.nativeSnapshot.revision=8,c=>c.panelVisible=false,c=>c.nativeFactsCurrent=false,c=>c._nativeSupportGeneration++]) {
  const c=context(); change(c); assert(!c.finishNativeConfigurationReport('instance',7,2,0,frame(fixture()))); assert.equal(c.copied.length,0);
 }
 for(const [code,raw] of [[1,frame(fixture())],[0,'private-token']]) { const c=context(); assert(!c.finishNativeConfigurationReport('instance',7,2,code,raw)); assert.equal(c.copied.length,0); }
});
test('busy clipboard and report refusal',()=>{
 for(const name of ['copying','nativeSupportBusy']) { const c=context(); c[name]=true; assert(!c.copyNativeConfigurationReport()); }
});
test('native clipboard and UI bounded lifecycle contract',()=>{
 assert(source.includes('"native-clipboard-copy"] : ["wl-copy"]'));
 assert(source.includes('if (_nativeSupportRead) _nativeSupportRead.running = false'));
 assert(source.includes('interval: 15000; running: process.running'));
 const panel=fs.readFileSync(__dirname+'/../plugin/Panel.qml','utf8');
 assert(panel.slice(panel.indexOf('function panelTabTargets()'),panel.indexOf('function availablePanelTabTargets()')).includes('nativeSupportSetting.focusTarget'));
 assert(panel.includes('onClicked: vless.copyNativeConfigurationReport()'));
});
console.log(`${count} native support-report tests passed`);
