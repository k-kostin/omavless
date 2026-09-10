// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict'), fs = require('node:fs'), vm = require('node:vm');
const parser = vm.createContext({});
vm.runInContext(fs.readFileSync(__dirname + '/../plugin/NativeSnapshot.js', 'utf8'), parser);
const source = fs.readFileSync(__dirname + '/../plugin/Service.qml', 'utf8');
const fixture = () => ({schemaVersion:1, scope:'native_configuration', runtime:{implementation:'rust', version:'0.1.0', lastKnownState:'disconnected', routingTransactionPending:false}, configuration:{inventory:{profiles:2,favorites:1,subscriptions:1,customRules:0},routing:{preset:'custom',configured:true,lastManualRuleUpdate:0},startup:{configured:true,enabled:false,target:'last',mode:'rule'},updates:{latestSubscription:0},onboardingComplete:true},coverage:{privateStoreValidated:true,liveHostObservation:false,controllerQuery:false,loginActivationVerified:false}});
const frame = r => JSON.stringify({api:'omavless.control',version:1,id:'test',ok:true,revision:7,result:r});
let count=0;
function test(name, fn) { try { fn(); count++; } catch(e) { e.message = name + ': ' + e.message; throw e; } }
test('configuration scope and no live claims', () => {
 const report = parser.configurationReport(frame(fixture()),7); assert(report);
 assert.equal(JSON.parse(report).coverage.liveHostObservation,false);
 assert(!report.includes('instanceId')); assert(!report.includes('revision'));
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
 assert(panel.includes('targets.push(nativeSupportSetting.focusTarget)'));
 assert(panel.includes('onAction: vless.copyNativeConfigurationReport()'));
});
console.log(`${count} native support-report tests passed`);
