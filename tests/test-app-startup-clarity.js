// SPDX-License-Identifier: MIT
const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const service=fs.readFileSync(__dirname+'/../plugin/Service.qml','utf8');
const panel=fs.readFileSync(__dirname+'/../plugin/Panel.qml','utf8');
const i18n=require('../plugin/I18n.js');
const parser=vm.createContext({});vm.runInContext(fs.readFileSync(__dirname+'/../plugin/NativeSnapshot.js','utf8'),parser);
function report(enabled){return JSON.stringify({api:'omavless.control',version:1,id:'test',ok:true,revision:7,result:{
 schemaVersion:3,scope:'native_support',runtime:{implementation:'rust',version:'0.9.9',lastKnownState:'disconnected',routingTransactionPending:false},
 configuration:{inventory:{profiles:0,favorites:0,subscriptions:0,customRules:0},routing:{preset:'',configured:false,lastManualRuleUpdate:0},
 startup:{configured:true,enabled:false,target:'last',mode:'rule'},updates:{latestSubscription:0},onboardingComplete:true},
 localObservation:{availability:'unavailable',desired:{connected:false,mode:'global'},facts:null,
 verification:{serviceOwnership:false,tunOwnership:false,routes:false,dns:false,internet:false}},
 host:{core:null,runtimeService:{loaded:true,active:true,enabled,ownsCurrentProcess:true},
 loginService:{loaded:true,active:true,enabled:!enabled,ownsCurrentProcess:null},files:null,configuredPolicy:null},
 coverage:{privateStoreValidated:true,liveHostObservation:false,controllerQuery:false,loginActivationVerified:false,
 coreSetupVerified:false,serviceEnablementVerified:true,loadedPolicyCounts:false,fileReadiness:false}}});}
let count=0;
function test(name,fn){try{fn();count++}catch(e){e.message=name+': '+e.message;throw e}}
function context(){
 const c=vm.createContext({nativeFactsCurrent:true,nativeAppStartupVisible:true,nativePending:null,nativeQuitting:false,
  nativeOutcomeUnknown:false,nativeActionRunning:false,nativeAvailabilityGeneration:2,_nativeAppStartupGeneration:3,
  _nativeAppStartupRead:null,nativeAppStartupFact:null,nativeAppStartupFailed:false,backendPath:'/synthetic/backend.sh',
  nativeSnapshot:{instanceId:'instance',revision:7},nativeAppStartupComponent:{createObject:(_,p)=>({...p})},
  NativeSnapshot:{appStartupEnabled:(raw,rev)=>raw==='valid'&&rev===7?true:raw==='disabled'&&rev===7?false:null}});
 c.root=c;
 for(const name of ['refreshNativeAppStartup','completeNativeAppStartupRead']){
  const start=service.indexOf('  function '+name+'(');assert(start>=0,'missing '+name);
  const end=service.indexOf('\n  }',start)+4;vm.runInContext(service.slice(start,end),c);
 }return c;
}
test('actual service wiring: app enablement is not the static login-intent dependency',()=>{
 const runtime=fs.readFileSync(__dirname+'/../packaging/systemd/omavless-runtime.service','utf8');
 const login=fs.readFileSync(__dirname+'/../packaging/systemd/omavless-login-prepare.service','utf8');
 assert(runtime.includes('WantedBy=default.target'));assert(runtime.includes('Requires=omavless-login-prepare.service'));
 assert(!login.includes('[Install]'));assert(login.includes('ExecStart=/usr/bin/omavless login-prepare'));
});
test('actual strict host projection uses runtime service despite opposite login intent flags',()=>{
 for(const enabled of [true,false])assert.equal(parser.appStartupEnabled(report(enabled),7),enabled);
 for(const mutate of [p=>{p.result.host.runtimeService=null;p.result.coverage.serviceEnablementVerified=false},p=>p.result.host.runtimeService.enabled='yes',
  p=>p.result.host.runtimeService.private='secret',p=>p.result.coverage.serviceEnablementVerified=false,
  p=>p.result.host.runtimeService.loaded=false]){
  const p=JSON.parse(report(true));mutate(p);assert.equal(parser.appStartupEnabled(JSON.stringify(p),7),null);
 }
 assert.equal(parser.appStartupEnabled(report(true),8),null);
 assert.equal(parser.appStartupEnabled('private failure',7),null);
});
test('presentation invalidates a previously checked fact after context changes',()=>{
 const c=context(),start=service.indexOf('  readonly property string nativeAppStartupState: {');
 const body=service.slice(start,service.indexOf('\n  }',start)+4).replace('  readonly property string nativeAppStartupState:','function state()');
 vm.runInContext(body,c);assert.equal(c.state(),'not_checked');assert(c.refreshNativeAppStartup());assert.equal(c.state(),'loading');
 assert(c.completeNativeAppStartupRead(c._nativeAppStartupRead,0,'valid'));assert.equal(c.state(),'on');
 c.nativeSnapshot.revision++;assert.equal(c.state(),'not_checked');c.nativeSnapshot.revision--;
 c.nativeAvailabilityGeneration++;assert.equal(c.state(),'not_checked');c.nativeAvailabilityGeneration--;
 c._nativeAppStartupGeneration++;assert.equal(c.state(),'not_checked');c._nativeAppStartupGeneration--;
 c.nativeFactsCurrent=false;assert.equal(c.state(),'unavailable');
});
test('explicit local read has a fixed command and clears prior facts',()=>{
 const c=context();c.nativeAppStartupFact={enabled:false};assert(c.refreshNativeAppStartup());
 assert.equal(c.nativeAppStartupFact,null);assert.equal(c._nativeAppStartupRead.command.join('|'),'bash|/synthetic/backend.sh|native-support-report');
 assert(!c.refreshNativeAppStartup());const p=c._nativeAppStartupRead;
 assert(c.completeNativeAppStartupRead(p,0,'valid'));assert.equal(c.nativeAppStartupFact.enabled,true);
 assert.equal(c.nativeAppStartupFact.revision,7);assert.equal(c._nativeAppStartupRead,null);
});
test('unavailable and mutation states refuse without dispatch',()=>{
 for(const change of [c=>c.nativeFactsCurrent=false,c=>c.nativeAppStartupVisible=false,c=>c.nativePending={},
  c=>c.nativeQuitting=true,c=>c.nativeOutcomeUnknown=true,c=>c.nativeActionRunning=true]){
  const c=context();change(c);assert(!c.refreshNativeAppStartup());assert.equal(c._nativeAppStartupRead,null);
 }
});
test('late replies from old visits, epochs, revisions or runtimes cannot publish',()=>{
 for(const change of [c=>c.nativeAppStartupVisible=false,c=>c.nativeFactsCurrent=false,c=>c._nativeAppStartupGeneration++,
  c=>c.nativeAvailabilityGeneration++,c=>c.nativeSnapshot.instanceId='other',c=>c.nativeSnapshot.revision++,
  c=>c.nativePending={},c=>c.nativeQuitting=true,c=>c.nativeOutcomeUnknown=true]){
  const c=context();assert(c.refreshNativeAppStartup());const p=c._nativeAppStartupRead;change(c);
  assert(!c.completeNativeAppStartupRead(p,0,'valid'));assert.equal(c.nativeAppStartupFact,null);
 }
});
test('only original process may complete; errors never become disabled',()=>{
 const c=context();assert(c.refreshNativeAppStartup());const p=c._nativeAppStartupRead;
 assert(!c.completeNativeAppStartupRead({},0,'valid'));assert.equal(c._nativeAppStartupRead,p);
 assert(!c.completeNativeAppStartupRead(p,1,'private failure'));assert.equal(c.nativeAppStartupFact,null);assert(c.nativeAppStartupFailed);
 assert(c.refreshNativeAppStartup());assert(!c.completeNativeAppStartupRead(c._nativeAppStartupRead,0,'invalid'));
 assert(c.refreshNativeAppStartup());assert(c.completeNativeAppStartupRead(c._nativeAppStartupRead,0,'disabled'));
 assert.equal(c.nativeAppStartupFact.enabled,false);
});
test('separate localized application and VPN rows retain explicit read and edit roles',()=>{
 assert(panel.includes('id: nativeAppStartupRow'));assert(panel.includes('nativeAppStartupRow.focusTarget'));
 assert(panel.includes('onAction: vless.refreshNativeAppStartup()'));
 assert(panel.includes('title: root.textFor("native.startup.vpn_title")'));
 for(const locale of ['en','ru'])for(const key of ['native.startup.app_title','native.startup.vpn_title',
  'native.startup.app_scope','native.startup.app_not_checked','native.startup.app_on','native.startup.app_off','native.startup.app_unavailable']){
  const value=i18n.translate(key,locale);assert(value&&!value.includes('Missing translation'));assert(value.length<220);
 }
});
console.log(`${count} application startup clarity checks passed`);
