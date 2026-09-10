// SPDX-License-Identifier: MIT
// Execute actual Service and parser functions, no network/private fixture.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const source=fs.readFileSync(path.join(__dirname,'../plugin/Service.qml'),'utf8');
const parser=vm.createContext({});vm.runInContext(fs.readFileSync(path.join(__dirname,'../plugin/NativeSnapshot.js'),'utf8'),parser);
const url='https://synthetic.invalid/private-token';
function frame(result){return JSON.stringify({api:'omavless.control',version:1,id:'synthetic',ok:true,revision:4,result});}
function context(){
  const c=vm.createContext({NativeSnapshot:parser,nativeOwner:true,nativeCanAct:true,nativeFactsCurrent:true,
    nativeSnapshot:{instanceId:'instance',revision:4,subscriptions:[{id:'record',name:'Synthetic'}]},
    nativePending:null,nativeActionRunning:false,nativeOutcomeUnknown:false,nativeActionCode:'',nativeActionProcess:{},
    nativeSubscriptionDraft:null,nativeSubscriptionLoading:false,nativeSubscriptionReadProcess:null,
    nativeSubscriptionReadComponent:{createObject:(_parent,properties)=>({...properties,running:false})},
    nativeSubscriptionCode:'',_nativeOperationSerial:0,backendPath:'/synthetic/backend.sh',ready:[],saved:0});
  c.nativeSubscriptionReady=(...args)=>c.ready.push(args);c.nativeSubscriptionSaved=()=>c.saved++;
  for(const name of ['cancelNativeSubscription','startNativeSubscription','nativeSubscriptionCurrent','finishNativeSubscriptionRead',
    'requestNativeSubscriptionAction','finishNativeSubscriptionAction','reconcileNativeAction','acceptRefreshedNativeState','isValidName']){
    const start=source.indexOf('  function '+name+'('),end=source.indexOf('\n  }',start)+4;assert(start>=0&&end>start,name);vm.runInContext(source.slice(start,end),c);
  }
  c.root=c;c.nativeActionStdout={text:''};c.finishNativeEditorAction=()=>{};c.finishNativeRoutingAction=()=>{};c.refreshAfterChange=()=>{};
  const actionStart=source.indexOf('    id: nativeActionProcess');
  const handlerStart=source.indexOf('    onExited: function(exitCode) {',actionStart);
  const handlerEnd=source.indexOf('\n    }',handlerStart)+6;
  assert(handlerStart>actionStart&&handlerEnd>handlerStart);
  vm.runInContext('this.actionExited = '+source.slice(handlerStart,handlerEnd).replace('    onExited: ',''),c);
  return c;
}
let count=0;function test(name,f){try{f();count++;}catch(e){e.message=name+': '+e.message;throw e;}}
test('editor parser exact envelope bounds and no diagnostic projection',()=>{
  const raw=frame({name:'Synthetic',url});assert.equal(parser.parseSubscriptionEditor(raw,4).url,url);
  for(const mutate of [p=>p.result.extra='secret',p=>p.result.name='',p=>p.result.url='\ud800',p=>p.result.url='x'.repeat(8193),p=>p.result.url='я'.repeat(4097),p=>p.result.url='x\ny',p=>p.revision=5,p=>p.ok=false,p=>p.api='other']){
    const p=JSON.parse(raw);mutate(p);assert.equal(parser.parseSubscriptionEditor(JSON.stringify(p),4),null);
  }
  assert.equal(parser.parseSubscriptionEditor('private-token',4),null);
});
test('new add opens confirmation without dispatch, explicit confirmation uses private stdin',()=>{
  const c=context(),before=JSON.stringify(c.nativeSnapshot);assert(c.startNativeSubscription('','Synthetic',url,'clipboard'));
  assert.equal(c.nativePending,null);assert.equal(c.nativeSubscriptionReadProcess,null);assert.deepEqual(c.ready[0],['Synthetic',url,'clipboard',false]);
  assert(c.requestNativeSubscriptionAction('subscription-add','','Synthetic',url));
  assert.equal(c.nativePending.input,'Synthetic\n'+url);assert.equal(c.nativePending.command[2],'native-subscription-add');
  assert(!c.nativePending.command.join(' ').includes('private-token'));assert.equal(JSON.stringify(c.nativeSnapshot),before);
});
test('existing edit reads fixed opaque id and updates original fenced target',()=>{
  const c=context();assert(c.startNativeSubscription('record','','','manual'));assert.equal(c.ready.length,0);
  assert.deepEqual(Array.from(c.nativeSubscriptionReadProcess.command),['bash','/synthetic/backend.sh','native-subscription-edit-input','record']);
  c.finishNativeSubscriptionRead({...c.nativeSubscriptionReadProcess.context},0,frame({name:'Synthetic',url}));assert.deepEqual(c.ready[0],['Synthetic',url,'manual',true]);
  assert(c.requestNativeSubscriptionAction('subscription-update','record','New',url));assert.equal(c.nativePending.input,'record\nNew\n'+url);
});
test('cancel and replacement context invalidate late private read',()=>{
  const c=context();c.startNativeSubscription('record');const old={...c.nativeSubscriptionReadProcess.context};c.cancelNativeSubscription();c.finishNativeSubscriptionRead(old,0,frame({name:'Synthetic',url}));
  assert.equal(c.ready.length,0);assert.equal(c.nativeSubscriptionDraft,null);
  c.startNativeSubscription('record');c.finishNativeSubscriptionRead(old,0,frame({name:'Synthetic',url}));assert.equal(c.ready.length,0);
  const d=context();d.startNativeSubscription('record');const replaced={...d.nativeSubscriptionDraft};d.nativeSubscriptionDraft={};d.finishNativeSubscriptionRead(replaced,0,frame({name:'Synthetic',url}));assert.equal(d.ready.length,0);
});
test('instance/revision or unavailable ownership refuses stale edit read and save',()=>{
  for(const mutate of [c=>c.nativeSnapshot.revision++,c=>c.nativeSnapshot.instanceId='restart',c=>c.nativeCanAct=false]){
    const c=context();c.startNativeSubscription('record');mutate(c);c.finishNativeSubscriptionRead({...c.nativeSubscriptionReadProcess.context},0,frame({name:'Synthetic',url}));assert.equal(c.ready.length,0);assert.equal(c.nativeSubscriptionCode,'unavailable');
    const d=context();d.startNativeSubscription('','Synthetic',url);mutate(d);assert.equal(d.requestNativeSubscriptionAction('subscription-add','','Synthetic',url),false);assert.equal(d.nativePending,null);
  }
});
test('delete and refresh are fixed known-record operations, invalid framing refuses',()=>{
  for(const action of ['subscription-delete','subscription-refresh']){
    const c=context();assert.equal(c.requestNativeSubscriptionAction(action,'missing','',''),false);
    assert(c.requestNativeSubscriptionAction(action,'record','','',{id:'record',instanceId:'instance',revision:4}));assert.equal(c.nativePending.input,'record');assert.equal(c.nativePending.command[2],'native-'+action);
  }
  for(const [name,value] of [['','x'],['two\nlines',url],['x',''],['x','a\nb'],['x','я'.repeat(4097)]]){
    const c=context();c.startNativeSubscription('','Synthetic',url);assert.equal(c.requestNativeSubscriptionAction('subscription-add','',name,value),false);assert.equal(c.nativePending,null);
  }
});
test('unknown exact retry preserves identity revision bytes; fixed exit74 is known rejection',()=>{
  for(const action of ['subscription-add','subscription-update','subscription-delete','subscription-refresh']){
    const c=context(),id=action==='subscription-add'?'':'record';if(action==='subscription-add'||action==='subscription-update')c.startNativeSubscription(id,'Synthetic',url);
    assert(c.requestNativeSubscriptionAction(action,id,'Synthetic',url,{id,instanceId:'instance',revision:4}));const saved=JSON.stringify(c.nativePending);
    c.finishNativeSubscriptionAction(null,true);c.nativeOutcomeUnknown=true;assert.equal(c.nativeSubscriptionCode,'unknown');
    assert(c.reconcileNativeAction());assert.equal(JSON.stringify(c.nativePending),saved);assert.equal(c.nativeActionProcess.stdinEnabled,true);
    const rejection=parser.parseActionExit('private-token',c.nativePending,74);assert.equal(rejection.ok,false);assert.equal(rejection.code,'invalid_argument');
    c.finishNativeSubscriptionAction(rejection,false);assert.equal(c.nativeSubscriptionCode,'rejected');assert.equal(c.saved,0);
  }
});
test('delete confirmation requires its captured identity and revision',()=>{
  for(const fence of [undefined,{id:'other',instanceId:'instance',revision:4},{id:'record',instanceId:'old',revision:4},{id:'record',instanceId:'instance',revision:3}]) {
    const c=context();assert.equal(c.requestNativeSubscriptionAction('subscription-delete','record','','',fence),false);assert.equal(c.nativePending,null);assert.equal(c.nativeSubscriptionCode,'stale');
  }
});
test('copied read context accepts same token but mismatched token or fences never releases input',()=>{
  for(const mutate of [c=>c.token='old',c=>c.id='other',c=>c.instanceId='old',c=>c.revision++]) {
    const c=context();c.startNativeSubscription('record');const copied={...c.nativeSubscriptionDraft};mutate(copied);
    c.finishNativeSubscriptionRead(copied,0,frame({name:'Synthetic',url}));assert.equal(c.ready.length,0);
  }
});
test('reviewed unknown draft cannot start a new save under any refreshed revision',()=>{
  const c=context();c.startNativeSubscription('','Synthetic',url);c.requestNativeSubscriptionAction('subscription-add','','Synthetic',url);
  c.nativeActionStdout.text='';c.actionExited(73);assert(c.acceptRefreshedNativeState());assert.equal(c.nativePending,null);
  assert.equal(c.requestNativeSubscriptionAction('subscription-add','','Synthetic',url),false);assert.equal(c.nativePending,null);
  c.nativeSnapshot.revision++;assert.equal(c.requestNativeSubscriptionAction('subscription-add','','Synthetic',url),false);
});
test('dynamic read collector is destroyed after success failure and cancellation',()=>{
  const start=source.indexOf('id: nativeSubscriptionReadComponent'),from=source.indexOf('      onExited: function(code) {',start),to=source.indexOf('\n      }',from)+8;
  assert(from>start&&to>from);
  for(const code of [0,2])for(const cancelled of [false,true]) {
    const c=context();c.startNativeSubscription('record');c.context={...c.nativeSubscriptionDraft};c.output={text:frame({name:'Synthetic',url})};let destroyed=0;c.process={destroy:()=>destroyed++};
    if(cancelled)c.cancelNativeSubscription();
    vm.runInContext('this.readExited = '+source.slice(from,to).replace('      onExited: ',''),c);c.readExited(code);
    assert.equal(destroyed,1);assert.equal(c.nativeSubscriptionReadProcess,null);
    if(cancelled)assert.equal(c.ready.length,0);
  }
});
test('action response identity bounds and error message never become public UI code',()=>{
  const c=context();c.startNativeSubscription('','Synthetic',url);c.requestNativeSubscriptionAction('subscription-add','','Synthetic',url);
  const result={schemaVersion:1,instanceId:'instance',operationId:c.nativePending.operationId,action:'subscription-add',applied:true};
  assert(parser.parseAction(frame(result),c.nativePending));result.instanceId='other';assert.equal(parser.parseAction(frame(result),c.nativePending),null);
  const raw=JSON.stringify({api:'omavless.control',version:1,id:'synthetic',ok:false,revision:4,error:{code:'conflict',message:'private-token',retryable:false}});
  const parsed=parser.parseAction(raw,c.nativePending);assert.equal(parsed.code,'conflict');assert(!JSON.stringify(parsed).includes('private-token'));
  c.finishNativeSubscriptionAction(parsed,false);assert.equal(c.nativeSubscriptionCode,'rejected');
  c.finishNativeSubscriptionAction({ok:true},false);assert.equal(c.nativeSubscriptionDraft,null);assert.equal(c.saved,1);
});
test('actual process completion treats lost reply as pending, local74 as terminal refusal',()=>{
  for(const exit of [73,74]){
    const c=context();c.startNativeSubscription('','Synthetic',url);c.requestNativeSubscriptionAction('subscription-add','','Synthetic',url);
    const pending=c.nativePending;c.nativeActionStdout.text='private-invalid-output';c.actionExited(exit);
    assert.equal(c.nativeOutcomeUnknown,exit===73);assert.equal(c.nativePending,exit===73?pending:null);
    assert.equal(c.nativeSubscriptionCode,exit===73?'unknown':'rejected');assert(c.nativeSubscriptionDraft);
    assert(!JSON.stringify({code:c.nativeSubscriptionCode,action:c.nativeActionCode}).includes('private-invalid-output'));
  }
});
console.log('native subscriptions: '+count+' passed');
