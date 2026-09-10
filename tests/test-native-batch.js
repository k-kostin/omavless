// SPDX-License-Identifier: MIT
const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm'),path=require('node:path');
const source=fs.readFileSync(path.join(__dirname,'../plugin/Service.qml'),'utf8');
const parser=vm.createContext({});vm.runInContext(fs.readFileSync(path.join(__dirname,'../plugin/NativeSnapshot.js'),'utf8'),parser);
const frame=(result,revision=4)=>JSON.stringify({api:'omavless.control',version:1,id:'test',ok:true,revision,result});
const operation=(job,patch={})=>({instanceId:job.instanceId,operationId:job.operationId,method:job.kind==='subscriptions'?'subscriptions.refresh_all':'routing.refresh_providers',state:'running',baseRevision:4,outcomeRevision:null,progress:{completed:0,total:2},cancelRequested:false,cancellable:true,error:null,...patch});
function context(){
 const c=vm.createContext({NativeSnapshot:parser,nativeOwner:true,nativeCanAct:true,nativeFactsCurrent:true,nativeSnapshot:{instanceId:'instance',revision:4},nativePending:null,
 nativeBatchJob:null,nativeBatchUnknown:false,nativeBatchErrorCode:'',_nativeBatchProcess:null,_nativeBatchFailures:0,_nativeBatchPolls:0,_nativeOperationSerial:0,
 backendPath:'/synthetic/backend.sh',nativeBatchComponent:{createObject:(_,p)=>({...p,running:false})},nativeBatchPoll:{running:false,interval:2000,start(){this.running=true},stop(){this.running=false}},
 refreshes:0,diagnosticsPageVisible:false});
 Object.defineProperty(c,'nativeBatchBusy',{get(){return c.nativeBatchJob!==null&&!c.nativeBatchJob.terminal}});
 Object.defineProperty(c,'nativeBatchRequestRunning',{get(){return c._nativeBatchProcess!==null}});
 Object.defineProperty(c,'nativeBatchAbandonable',{get(){return c.nativeBatchUnknown&&c.nativeBatchJob!==null&&!c.nativeBatchJob.terminal&&!c.nativeBatchRequestRunning&&c.nativeFactsCurrent&&c.nativeSnapshot.instanceId!==c.nativeBatchJob.instanceId}});
 c.refreshAfterChange=()=>c.refreshes++;
 for(const name of ['startNativeBatch','runNativeBatchRequest','finishNativeBatchRequest','nativeBatchPublicError','retryNativeBatch','cancelNativeBatch','dismissNativeBatch','abandonNativeBatch','refreshAllSubscriptions','refreshRuleProviders']){
  const start=source.indexOf('  function '+name+'('),end=source.indexOf('\n  }',start)+4;assert(start>=0);vm.runInContext(source.slice(start,end),c);
 }c.root=c;return c;
}
function reply(c,result,code=0,kind){const req=c._nativeBatchProcess;c._nativeBatchProcess=null;c.finishNativeBatchRequest(kind||req.requestKind,req.operation,code,result)}
let count=0;function test(name,f){try{f();count++}catch(e){e.message=name+': '+e.message;throw e}}
test('exact start/get/cancel projection schema with method bounds',()=>{
 for(const kind of ['subscriptions','providers']){
  const c=context();c.startNativeBatch(kind);const o=operation(c.nativeBatchJob);assert(parser.parseOperation(frame({operation:o}),c.nativeBatchJob,'start'));
  assert(parser.parseOperation(frame({accepted:false,operation:o}),c.nativeBatchJob,'cancel'));
  for(const patch of [{extra:1},{operationId:'other'},{instanceId:'other'},{method:'arbitrary'},{baseRevision:3},{progress:{completed:3,total:2}},{progress:{completed:0,total:kind==='subscriptions'?65:257}},{state:'guess'},{outcomeRevision:4}])
   assert.equal(parser.parseOperation(frame({operation:{...o,...patch}}),c.nativeBatchJob,'get'),null);
 }
});
test('terminal invariants and error message never retained',()=>{
 const c=context();c.startNativeBatch('subscriptions');const job=c.nativeBatchJob;
 const succeeded=operation(job,{state:'succeeded',outcomeRevision:5,progress:{completed:2,total:2},cancellable:false});
 assert.equal(parser.parseOperation(frame({operation:succeeded},5),job,'get').terminal,true);
 for(const patch of [{outcomeRevision:4},{cancellable:true},{progress:{completed:1,total:2}},{error:{code:'conflict',message:'private-secret',retryable:false}}])assert.equal(parser.parseOperation(frame({operation:{...succeeded,...patch}},5),job,'get'),null);
 const failed=operation(job,{state:'failed',outcomeRevision:4,cancellable:false,error:{code:'conflict',message:'private-secret',retryable:false}});
 const result=parser.parseOperation(frame({operation:failed}),job,'get');assert.equal(result.errorCode,'conflict');assert(!JSON.stringify(result).includes('private-secret'));
 assert.equal(parser.parseOperation('private-token',job,'get'),null);assert.equal(parser.parseOperation('x'.repeat(8193),job,'get'),null);
});
test('start fixed argv fences, no general mutation slot or optimistic state writes',()=>{
 for(const kind of ['subscriptions','providers']){const c=context(),before=JSON.stringify(c.nativeSnapshot);assert(c.startNativeBatch(kind));
 assert.deepEqual(Array.from(c._nativeBatchProcess.command.slice(0,4)),['bash','/synthetic/backend.sh',kind==='subscriptions'?'native-subscriptions-refresh-all':'native-providers-refresh','instance']);
 assert.equal(c._nativeBatchProcess.command[5],'4');assert.equal(c.nativePending,null);assert.equal(JSON.stringify(c.nativeSnapshot),before);assert.equal(c.nativeCanAct,true);assert.equal(c.startNativeBatch(kind),false)}
});
test('unknown start only retries same identity and original revision',()=>{
 const c=context();c.startNativeBatch('subscriptions');const before=Array.from(c._nativeBatchProcess.command);reply(c,'lost',73);
 assert(c.nativeBatchUnknown);assert(c.nativeBatchBusy);assert.equal(c.startNativeBatch('providers'),false);assert.equal(c.nativeBatchPoll.running,false);
 c.nativeSnapshot.revision=8;assert(c.retryNativeBatch());assert.deepEqual(Array.from(c._nativeBatchProcess.command),before);
 const rejected=JSON.stringify({api:'omavless.control',version:1,id:'test',ok:false,revision:8,error:{code:'conflict',message:'secret',retryable:false}});reply(c,rejected,1);
 assert.equal(c.nativeBatchJob.terminal,false);assert(c.nativeBatchUnknown);assert.equal(c.dismissNativeBatch(),false);
});
test('first proven start rejection releases slot without exposing error text',()=>{
 const c=context();c.startNativeBatch('subscriptions');reply(c,JSON.stringify({api:'omavless.control',version:1,id:'test',ok:false,revision:4,error:{code:'busy',message:'private-token',retryable:true}}),1);
 assert(c.nativeBatchJob.terminal);assert.equal(c.nativeBatchJob.errorCode,'busy');assert(!JSON.stringify(c.nativeBatchJob).includes('private-token'));assert(c.dismissNativeBatch());
});
test('successful admission polls, cancellation uses same id, terminal refreshes once',()=>{
 const c=context();c.startNativeBatch('subscriptions');reply(c,frame({operation:operation(c.nativeBatchJob)}));assert.equal(c.nativeBatchPoll.interval,2000);assert(c.nativeBatchPoll.running);
 assert(c.cancelNativeBatch());assert.equal(c._nativeBatchProcess.command[2],'native-operation-cancel');
 reply(c,frame({accepted:true,operation:operation(c.nativeBatchJob,{cancelRequested:true})}));assert.equal(c.cancelNativeBatch(),false);
 c.runNativeBatchRequest('get');reply(c,frame({operation:operation(c.nativeBatchJob,{state:'cancelled',cancelRequested:true,cancellable:false,outcomeRevision:4})}));
 assert(c.nativeBatchJob.terminal);assert.equal(c.refreshes,1);assert.equal(c.nativeBatchPoll.running,false);assert(c.dismissNativeBatch());
});
test('poll failures bounded backoff then explicit retry, restart cannot retarget',()=>{
 const c=context();c.startNativeBatch('providers');reply(c,frame({operation:operation(c.nativeBatchJob)}));
 for(let i=0;i<5;i++){assert(c.runNativeBatchRequest('get'));reply(c,'lost',1);assert(c.nativeBatchPoll.interval<=30000)}
 assert.equal(c.nativeBatchPoll.running,false);assert(c.nativeBatchUnknown);assert(c.retryNativeBatch());
 const request=c._nativeBatchProcess;c.nativeSnapshot.instanceId='new';reply(c,frame({operation:operation(c.nativeBatchJob)}));assert(c.nativeBatchUnknown);assert.equal(c.retryNativeBatch(),false);
 assert.equal(request.command[3],'instance');assert.equal(c.startNativeBatch('subscriptions'),false);
});
test('closing UI does not cancel daemon job, bounded process watchdog present',()=>{
 assert(source.includes('Timer { interval: 15000; running: process.running; onTriggered: process.running = false }'));
 assert(source.includes('_nativeBatchPolls >= 300'));
 const c=context();assert(c.refreshAllSubscriptions());reply(c,frame({operation:operation(c.nativeBatchJob)}));c.nativeRoutingToolsVisible=false;assert(c.nativeBatchBusy);assert(c.nativeBatchPoll.running);
 const d=context();assert(d.refreshRuleProviders());assert.equal(d.nativeBatchJob.kind,'providers');
});
test('known progress and cancellation cannot regress',()=>{
 const c=context();c.startNativeBatch('subscriptions');const job={...c.nativeBatchJob,acknowledged:true,state:'running',completed:1,total:2,cancelRequested:true};
 for(const patch of [{progress:{completed:0,total:2},cancelRequested:true},{progress:{completed:1,total:1},cancelRequested:true},{progress:{completed:1,total:2},cancelRequested:false}])
  assert.equal(parser.parseOperation(frame({operation:operation(job,patch)}),job,'get'),null);
});
test('explicit stale epoch acknowledgement requires coherent new facts and no reader',()=>{
 const c=context();c.startNativeBatch('subscriptions');assert.equal(c.abandonNativeBatch(),false);reply(c,'lost',73);assert.equal(c.abandonNativeBatch(),false);
 c.nativeSnapshot.instanceId='new';c.nativeFactsCurrent=false;assert.equal(c.abandonNativeBatch(),false);c.nativeFactsCurrent=true;
 assert(c.abandonNativeBatch());assert.equal(c.nativeBatchJob,null);assert.equal(c.refreshes,0);assert(c.startNativeBatch('providers'));
});
test('Process watchdog uses an explicit QML property',()=>{
 const component=source.slice(source.indexOf('id: nativeBatchComponent'),source.indexOf('onExited: function(code)',source.indexOf('id: nativeBatchComponent')));
 assert(component.includes('property Timer timeout: Timer'));
 assert(!/\n\s+Timer \{/.test(component));
});
test('provider status and errors are visible beside its action',()=>{
 const panel=fs.readFileSync(path.join(__dirname,'../plugin/Panel.qml'),'utf8');
 const start=panel.indexOf('  function nativeProviderUpdateDescription('),end=panel.indexOf('\n  }',start)+4;
 const c=vm.createContext({vless:{nativeBatchJob:{kind:'providers',state:'running',completed:2,total:23,errorCode:''},nativeBatchUnknown:false,nativeBatchErrorCode:''},textFor:(key,v)=>v?`${v.completed}/${v.total}`:key});c.root=c;
 vm.runInContext(panel.slice(start,end),c);
 assert.equal(c.nativeProviderUpdateDescription(),'native.batch.running · 2/23');
 c.vless.nativeBatchJob.state='failed';c.vless.nativeBatchJob.errorCode='core_rejected';
 assert(c.nativeProviderUpdateDescription().includes('\nerror.core_rejected'));
 assert(panel.includes('description: root.nativeProviderUpdateDescription()'));
});
console.log('native batch: '+count+' passed');
