// SPDX-License-Identifier: MIT
const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const source=fs.readFileSync(__dirname+'/../plugin/Service.qml','utf8');
const panel=fs.readFileSync(__dirname+'/../plugin/Panel.qml','utf8');
const parser=vm.createContext({});vm.runInContext(fs.readFileSync(__dirname+'/../plugin/NativeSnapshot.js','utf8'),parser);
const id='00000000-0000-4000-8000-000000000001';
const fence={instanceId:'instance',revision:7};
const frame=()=>({api:'omavless.control',version:1,id:'test',ok:true,revision:7,
 result:{runtimeOwnership:true,mutations:true,methods:['system.hello','startup.configure']}});
function context(){
 const c=vm.createContext({NativeSnapshot:parser,nativeOwner:true,nativeCanAct:true,nativeFactsCurrent:true,nativeStartupSettingsVisible:true,
  nativeStartupAvailable:true,nativeStartupCapability:null,nativeSnapshot:{...fence,desired:{connected:true,mode:'global'},
  startup:{enabled:false,target:'last',profileId:'',mode:'rule'},profiles:[{id,name:'Private <unchanged>',missing:false}]},
  _nativeOperationSerial:0,backendPath:'/synthetic/backend.sh',nativeActionProcess:{},nativePending:null,nativeActionCode:'',nativeOutcomeUnknown:false});
 c.root=c;
 for(const name of ['requestNativeStartup','configureStartup','reconcileNativeAction','finishNativeStartupCapability']){
  const start=source.indexOf('  function '+name+'('),end=source.indexOf('\n  }',start)+4;assert(start>=0);vm.runInContext(source.slice(start,end),c)
 }return c;
}
let count=0;function test(name,f){try{f();count++}catch(e){e.message=name+': '+e.message;throw e}}
test('capability projection is bounded and version/revision gated',()=>{
 assert(parser.startupCapability(JSON.stringify(frame()),fence).available);
 const full=frame();full.result=require('./fixtures/native_product_capabilities.json');
 assert(parser.startupCapability(JSON.stringify(full),fence).available);
 for(const mutate of [p=>p.version=2,p=>p.revision++,p=>p.result.extra=true,p=>p.result.methods.push('startup.configure'),
  p=>p.result.methods=['https://private.invalid/key'],p=>p.result.methods=new Array(129).fill('x'),p=>p.result.mutations='true']){
  const p=frame();mutate(p);assert.equal(parser.startupCapability(JSON.stringify(p),fence),null)
 }
 for(const mutate of [p=>p.result.methods=[],p=>p.result.mutations=false,p=>p.result.runtimeOwnership=false]){
  const p=frame();mutate(p);assert.equal(parser.startupCapability(JSON.stringify(p),fence).available,false)
 }
 assert.equal(parser.startupCapability('x'.repeat(16385),fence),null);
});
test('native save uses fixed stdin and leaves current/saved state untouched',()=>{
 const c=context(),before=JSON.stringify(c.nativeSnapshot);
 assert(c.configureStartup(true,'profile',id,'rule'));
 assert.equal(c.nativePending.action,'startup-configure');assert.equal(c.nativePending.input,'on\nprofile\n'+id+'\nrule');
 assert.equal(c.nativePending.command[2],'native-startup-configure');assert(!c.nativePending.command.includes(id));
 assert.equal(JSON.stringify(c.nativeSnapshot),before);assert(c.nativeActionProcess.stdinEnabled);
 const pending=JSON.stringify(c.nativePending);c.nativeActionRunning=false;assert(c.reconcileNativeAction());assert.equal(JSON.stringify(c.nativePending),pending);
});
test('invalid, missing and unavailable requests do not dispatch',()=>{
 for(const args of [[true,'bad',id,'rule'],[true,'profile','missing','rule'],[true,'last','', 'direct'],['on','last','','rule']]){
  const c=context();assert(!c.requestNativeStartup(...args));assert.equal(c.nativePending,null)
 }
 const c=context();c.nativeStartupAvailable=false;assert(!c.requestNativeStartup(false,'last','','rule'));assert.equal(c.nativePending,null);
});
test('last selection clears hidden profile; disabling can retain a missing record',()=>{
 const c=context();assert(c.requestNativeStartup(false,'last',id,'global'));assert.equal(c.nativePending.input,'off\nlast\n\nglobal');
 c.nativeSnapshot.profiles=[];assert(c.requestNativeStartup(false,'profile',id,'rule'));
 assert(!c.requestNativeStartup(true,'profile',id,'rule'));
});
test('closed, stale and failed capability reads never enable controls',()=>{
 for(const change of [c=>c.nativeStartupSettingsVisible=false,c=>c.nativeFactsCurrent=false,c=>c.nativeSnapshot.instanceId='other',c=>c.nativeSnapshot.revision++]){
  const c=context();change(c);c.finishNativeStartupCapability(fence,0,JSON.stringify(frame()));assert.equal(c.nativeStartupCapability,null)
 }
 const c=context();c.finishNativeStartupCapability(fence,1,'private raw failure');assert.equal(c.nativeStartupCapability,null);
});
test('success and local rejection use existing exact action outcome fence',()=>{
 const c=context();c.requestNativeStartup(false,'last','','rule');
 const pending=c.nativePending;
 const p={api:'omavless.control',version:1,id:'test',ok:true,revision:8,result:{schemaVersion:1,instanceId:pending.instanceId,
  operationId:pending.operationId,action:'startup-configure',applied:true}};
 assert(parser.parseAction(JSON.stringify(p),pending).ok);p.result.instanceId='other';assert.equal(parser.parseAction(JSON.stringify(p),pending),null);
 assert.equal(parser.parseActionExit('',pending,74).code,'invalid_argument');
});
test('shared dialog, native metadata, focus and localization retain legacy path',()=>{
 assert(panel.includes('actionVisible: vless.nativeStartupAvailable'));
 assert(panel.includes('nativeStartupSummaryRow.focusTarget'));
 assert(panel.includes('nativeContext: vless.nativeOwner'));
 assert(panel.includes('startupPrompt.openWith(vless.nativeSnapshot.startup)'));
 const prompt=fs.readFileSync(__dirname+'/../plugin/StartupPrompt.qml','utf8');
 assert(prompt.includes('prompt.nativeContext ? "native.startup.prompt_help" : "startup_prompt.help"'));
 const i18n=require('../plugin/I18n.js');for(const locale of ['en','ru'])for(const key of ['native.startup.configure_scope','native.startup.prompt_help']){
  const value=i18n.translate(key,locale);assert(value&&!value.includes('Missing translation'));assert(value.length<=512)
 }
});
console.log(`${count} native startup UI checks passed`);
