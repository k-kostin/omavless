// SPDX-License-Identifier: MIT
const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm'),path=require('node:path');
const source=fs.readFileSync(path.join(__dirname,'../plugin/Service.qml'),'utf8');
const parser=vm.createContext({});vm.runInContext(fs.readFileSync(path.join(__dirname,'../plugin/NativeSnapshot.js'),'utf8'),parser);
const rule={id:'00000000-0000-4000-8000-000000000001',kind:'domain',action:'direct',value:'example.invalid'};
const frame=result=>JSON.stringify({api:'omavless.control',version:1,id:'test',ok:true,revision:4,result});
function context(){
 const c=vm.createContext({NativeSnapshot:parser,nativeOwner:true,nativeCanAct:true,nativeFactsCurrent:true,nativeRoutingToolsVisible:true,nativeRoutingBusy:false,
 nativeSnapshot:{instanceId:'instance',revision:4},nativePending:null,nativeActionRunning:false,nativeOutcomeUnknown:false,nativeActionProcess:{},
 _nativeRoutingRead:null,_nativeRoutingGeneration:0,_nativeRulesFence:null,_nativeRouteFence:null,customRules:[],routeCheckResult:null,nativeRoutingErrorCode:'',_nativeOperationSerial:0,backendPath:'/synthetic/backend.sh',
 nativeRoutingReadComponent:{createObject:(_,p)=>({...p,running:false})},routingPresetById:p=>p==='ru'?{}:null});
 for(const name of ['clearNativeRouting','nativeRoutingCurrent','startNativeRoutingRead','finishNativeRoutingRead','requestNativeRoutingAction','finishNativeRoutingAction','loadCustomRules','addCustomRule','deleteCustomRule','useRoutingPreset','checkRoute','reconcileNativeAction']){
  const start=source.indexOf('  function '+name+'('),end=source.indexOf('\n  }',start)+4;assert(start>=0);vm.runInContext(source.slice(start,end),c);
 }c.root=c;return c;
}
let count=0;function test(name,f){try{f();count++}catch(e){e.message=name+': '+e.message;throw e}}
test('strict bounded custom rule response',()=>{
 assert.equal(parser.parseCustomRules(frame({version:1,rules:[rule]}),4)[0].value,rule.value);
 for(const rules of [[rule,rule],[{...rule,extra:'secret'}],[{...rule,value:'x'.repeat(1025)}],[{...rule,id:'bad'}],[{...rule,action:'shell'}]])assert.equal(parser.parseCustomRules(frame({version:1,rules}),4),null);
 assert.equal(parser.parseCustomRules(frame({version:1,rules:[rule]}),5),null);
 assert.equal(parser.parseCustomRules('secret',4),null);
});
test('strict route projection no raw error',()=>{
 const r={version:1,query:'example.invalid',outcome:'direct',ruleType:'DOMAIN',rulePayload:'example.invalid',target:'DIRECT',source:'custom'};
 assert.equal(parser.parseRouteCheck(frame(r),4).outcome,'direct');
 for(const patch of [{extra:'secret'},{source:'guess'},{query:'x\ny'},{target:[]},{rulePayload:'x'.repeat(1025)}])assert.equal(parser.parseRouteCheck(frame({...r,...patch}),4),null);
 assert.equal(parser.parseRouteCheck('password=secret',4),null);
});
test('private route query only stdin and stale or closed results ignored',()=>{
 for(const mutate of [c=>c.clearNativeRouting(),c=>c.nativeSnapshot.revision++,c=>c.nativeSnapshot.instanceId='restart',c=>c.nativeRoutingToolsVisible=false,c=>c.nativeFactsCurrent=false]){
  const c=context();assert(c.checkRoute('example.invalid'));assert(!c._nativeRoutingRead.command.join(' ').includes('example.invalid'));assert.equal(c._nativeRoutingRead.input,'example.invalid');
  const old={...c._nativeRoutingRead.context};mutate(c);c.finishNativeRoutingRead(old,0,frame({version:1,rules:[rule]}));assert.equal(c.routeCheckResult,null);
 }
});
test('list permits delete only from same fenced revision',()=>{
 const c=context();assert(c.loadCustomRules());const read=c._nativeRoutingRead.context;c._nativeRoutingRead=null;c.finishNativeRoutingRead(read,0,frame({version:1,rules:[rule]}));
 assert.equal(c.customRules.length,1);assert(c.deleteCustomRule(rule));assert.equal(c.nativePending.input,rule.id);assert.equal(c.nativePending.action,'custom-rule-delete');
 const d=context();assert.equal(d.deleteCustomRule(rule),false);assert.equal(d.nativeRoutingErrorCode,'error.conflict');
});
test('rule add bounds and exact replay preserve private input',()=>{
 const c=context();assert(c.addCustomRule('domain','direct',' example.invalid '));assert.equal(c.nativePending.input,'domain\ndirect\nexample.invalid');assert(!c.nativePending.command.join(' ').includes('example.invalid'));
 const original=JSON.stringify(c.nativePending);c.nativeOutcomeUnknown=true;assert(c.reconcileNativeAction());assert.equal(JSON.stringify(c.nativePending),original);
 for(const value of ['', 'x\ny','я'.repeat(513)])assert.equal(context().addCustomRule('domain','direct',value),false);
 assert.equal(context().addCustomRule('shell','direct','example.invalid'),false);
});
test('preset framing and no optimistic snapshot changes',()=>{
 for(const keep of [true,false]){const c=context(),before=JSON.stringify(c.nativeSnapshot);assert(c.useRoutingPreset('ru',keep));assert.equal(c.nativePending.input,'ru\n'+(keep?'on':'off'));assert.equal(JSON.stringify(c.nativeSnapshot),before)}
 assert.equal(context().useRoutingPreset('shell',false),false);
});
test('routing acknowledgements and local rejection use known action bounds',()=>{
 for(const action of ['routing-preset','custom-rule-add','custom-rule-delete']){
  const c=context();c.requestNativeRoutingAction(action,'synthetic');const pending=c.nativePending;
  assert(parser.parseAction(frame({schemaVersion:1,instanceId:'instance',operationId:pending.operationId,action,applied:true}),pending));
  assert.equal(parser.parseActionExit('private-error',pending,74).code,'invalid_argument');
  c.finishNativeRoutingAction(null,true);assert.equal(c.nativeRoutingErrorCode,'error.daemon_restarting');assert.equal(c.nativePending,pending);
  c.finishNativeRoutingAction({ok:false,code:'timeout'},false);assert.equal(c.nativeRoutingErrorCode,'error.capability_unavailable');
 }
});
test('busy fences refuse reads and mutations, clear erases private values',()=>{
 const c=context();c.nativeRoutingBusy=true;assert.equal(c.loadCustomRules(),false);assert.equal(c.addCustomRule('domain','direct','example.invalid'),false);
 c.customRules=[rule];c.routeCheckResult={query:'example.invalid'};c.clearNativeRouting();assert.equal(c.customRules.length,0);assert.equal(c.routeCheckResult,null);
 assert(source.includes('_nativeRoutingRead.input = ""; _nativeRoutingRead.running = false'));
 assert(source.includes('finally { process.destroy() }'));
});
console.log('native routing: '+count+' passed');
