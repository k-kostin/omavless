// SPDX-License-Identifier: MIT
// Execute production functions; synthetic text only, no host/editor/store access.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const parser=vm.createContext({});
vm.runInContext(fs.readFileSync(path.join(__dirname,'../plugin/NativeSnapshot.js'),'utf8'),parser);
const source=fs.readFileSync(path.join(__dirname,'../plugin/Service.qml'),'utf8');
const seed='vless://synthetic-original',edited='vless://synthetic-edited';
function frame(input=seed){return JSON.stringify({api:'omavless.control',version:1,id:'editor',ok:true,revision:4,result:{name:'Synthetic',input}});}
function context(){
  const component={createObject:(_parent,properties)=>({...properties,running:false})};
  const c=vm.createContext({NativeSnapshot:parser, nativeOwner:true,nativeFactsCurrent:true,nativeCanAct:true,
    nativeSnapshot:{instanceId:'instance',revision:4,profiles:[{id:'record',subscriptionId:''}]},
    nativeEditorDraft:null,nativeEditorRunning:false,nativeEditorReadProcess:null,nativeEditorProcess:null,
    nativeEditorReadComponent:component,nativeEditorComponent:component,nativeEditorCode:'',_nativeEditorSeed:'',
    nativePending:null,nativeActionRunning:false,nativeOutcomeUnknown:false,nativeActionCode:'',nativeActionProcess:{},
    _nativeOperationSerial:0,backendPath:'/synthetic/backend.sh',attention:0,finished:0});
  c.root=c;c.nativeEditorAttention=()=>c.attention++;c.editFinished=()=>c.finished++;
  for(const name of ['nativeEditorContextCurrent','nativeEditorFence','startNativeEditor','finishNativeEditorRead','reopenNativeEditor',
    'discardNativeEditor','finishNativeEditor','finishNativeEditorAction','reconcileNativeAction','acceptRefreshedNativeState']) {
    const start=source.indexOf('  function '+name+'('),end=source.indexOf('\n  }',start)+4;
    assert(start>=0&&end>start);vm.runInContext(source.slice(start,end),c);
  }
  return c;
}
function opened(){const c=context();assert(c.startNativeEditor({uuid:'record'}));c.nativeEditorReadProcess=null;c.finishNativeEditorRead(c.nativeEditorDraft,0,frame());return c;}
function saved(){const c=opened();c.nativeEditorProcess=null;c.finishNativeEditor(c.nativeEditorDraft,0,edited,'');return c;}
let count=0;function test(name,f){try{f();count++;}catch(e){e.message=name+': '+e.message;throw e;}}
test('pre-dispatch rejection is known; lost reply stays unknown',()=>{
  const c=saved();
  const result=parser.parseActionExit('',c.nativePending,74);
  assert.equal(result.code,'invalid_argument');
  c.finishNativeEditorAction(result,false);
  assert.equal(c.nativeEditorDraft.input,edited);assert.equal(c.nativeEditorCode,'rejected');
  assert.equal(parser.parseActionExit('',c.nativePending,73),null);
  assert.equal(parser.parseActionExit('',{...c.nativePending,action:'connect'},74),null);
  assert.match(source,/NativeSnapshot.parseActionExit\(nativeActionStdout.text, root.nativePending, exitCode\)/);
});
test('QML copied process context matches token, not object identity',()=>{
  const c=context();c.startNativeEditor({uuid:'record'});
  const copy=JSON.parse(JSON.stringify(c.nativeEditorDraft));
  c.nativeEditorReadProcess=null;c.finishNativeEditorRead(copy,0,frame());
  assert.equal(c._nativeEditorSeed,seed);
  const editorCopy=JSON.parse(JSON.stringify(c.nativeEditorDraft));
  c.nativeEditorProcess=null;c.finishNativeEditor(editorCopy,0,edited,'');
  assert.equal(c.nativePending.input,'record\nSynthetic\n'+edited);
  for(const field of ['token','instanceId','revision','profileId']) {
    const stale={...editorCopy,[field]:'other'};
    assert.equal(c.nativeEditorContextCurrent(stale),false);
  }
});
test('strict editor read shape, bounds and original revision',()=>{
  assert.equal(parser.parseEditorInput(frame(),4).input,seed);
  for(const input of ['\u0000','\ud800','é'.repeat(16385)])assert.equal(parser.parseEditorInput(frame(input),4),null);
  for(const change of [p=>p.revision++,p=>p.result.extra=true,p=>p.result.name='',p=>p.result.name='x\ny',p=>p.ok=false,p=>p.result.input=42]){
    const p=JSON.parse(frame());change(p);assert.equal(parser.parseEditorInput(JSON.stringify(p),4),null);
  }
});
test('managed/unknown selection and concurrent draft refuse, read is fixed',()=>{
  const c=context();c.nativeSnapshot.profiles[0].subscriptionId='managed';assert.equal(c.startNativeEditor({uuid:'record'}),false);
  c.nativeSnapshot.profiles[0].subscriptionId='';assert.equal(c.startNativeEditor({uuid:'unknown'}),false);
  assert(c.startNativeEditor({uuid:'record'}));assert.equal(c.startNativeEditor({uuid:'record'}),false);
  assert.deepEqual(Array.from(c.nativeEditorReadProcess.command),['bash','/synthetic/backend.sh','native-profile-edit-input','record']);
});
test('editor seed and saved replacement are stdin only; no optimistic state',()=>{
  const c=opened();assert.equal(c._nativeEditorSeed,seed);
  assert.deepEqual(Array.from(c.nativeEditorProcess.command),['bash','/synthetic/backend.sh','native-profile-editor']);
  c.nativeEditorProcess=null;c.finishNativeEditor(c.nativeEditorDraft,0,edited,'');
  assert.equal(c.nativePending.input,'record\nSynthetic\n'+edited);
  assert.equal(c.nativePending.action,'profile-replace');assert.equal(c.nativePending.revision,4);
  assert(!c.nativePending.command.join(' ').includes(edited));assert.equal(c.nativeSnapshot.revision,4);
});
test('cancel and unchanged input clear draft without save',()=>{
  for(const [code,out] of [[3,''],[0,seed]]){const c=opened();c.nativeEditorProcess=null;c.finishNativeEditor(c.nativeEditorDraft,code,out,'');assert.equal(c.nativeEditorDraft,null);assert.equal(c.nativePending,null);assert.equal(c._nativeEditorSeed,'');}
});
test('returned oversized or empty edited text is retained without truncation',()=>{
  for(const value of ['','é'.repeat(20000)]){const c=opened();c.nativeEditorProcess=null;c.finishNativeEditor(c.nativeEditorDraft,0,value,'');assert.equal(c.nativeEditorDraft.input,value);assert.equal(c.nativeEditorCode,'rejected');assert.equal(c.nativePending,null);}
});
test('acknowledged rejection retains draft and explicit reopen keeps original fence',()=>{
  const c=saved(),operation=c.nativePending.operationId;
  c.finishNativeEditorAction({ok:false,code:'invalid_argument'},false);c.nativePending=null;
  assert.equal(c.nativeEditorDraft.input,edited);assert.equal(c.nativeEditorDraft.revision,4);assert.equal(c.nativeEditorCode,'rejected');
  assert(c.reopenNativeEditor());assert.equal(c._nativeEditorSeed,edited);
  assert.equal(c.nativePending,null);assert.equal(typeof operation,'string');
});
test('unknown preserves exact replay and accepting state does not authorize fresh save',()=>{
  const c=saved(),pending=c.nativePending;c.finishNativeEditorAction(null,true);c.nativeOutcomeUnknown=true;
  assert(c.reconcileNativeAction());assert.equal(c.nativePending,pending);assert.equal(c.nativeActionProcess.command,pending.command);
  assert(c.acceptRefreshedNativeState());assert.equal(c.nativePending,null);assert.equal(c.nativeEditorDraft.input,edited);
  assert(c.reopenNativeEditor());c.nativeEditorProcess=null;
  c.finishNativeEditor(c.nativeEditorDraft,0,edited+'-recovery','');
  assert.equal(c.nativePending,null);assert.equal(c.nativeEditorCode,'unknown');
  assert(c.discardNativeEditor());assert.equal(c.nativeEditorDraft,null);
});
test('stale save retains text and never silently adopts new revision or instance',()=>{
  for(const mutate of [c=>c.nativeSnapshot.revision++,c=>c.nativeSnapshot.instanceId='restart',c=>c.nativeOwner=false]) {
    const c=opened();c.nativeEditorProcess=null;mutate(c);c.finishNativeEditor(c.nativeEditorDraft,0,edited,'');
    assert.equal(c.nativePending,null);assert.equal(c.nativeEditorDraft.input,edited);assert.equal(c.nativeEditorDraft.revision,4);
    assert(c.reopenNativeEditor());c.nativeEditorProcess=null;
    c.finishNativeEditor(c.nativeEditorDraft,0,edited+'-recovery','');
    assert.equal(c.nativePending,null);assert.equal(c.nativeEditorCode,'stale');
  }
});
test('helper failures never echo private errors or erase earlier returned draft',()=>{
  for(const [error,expected] of [['Profile editor unavailable: install zenity','missing'],['private-token','unavailable']]){
    const c=opened();c.nativeEditorProcess=null;c.finishNativeEditor(c.nativeEditorDraft,2,'private-token',error);
    assert.equal(c.nativeEditorCode,expected);assert.equal(c.nativeEditorDraft.input,seed);assert.equal(c.nativePending,null);
  }
});
test('cancel from recovery editor preserves the previous rejected text',()=>{
  const c=saved();c.finishNativeEditorAction({ok:false},false);c.nativePending=null;
  assert(c.reopenNativeEditor());c.nativeEditorProcess=null;
  c.finishNativeEditor(c.nativeEditorDraft,3,'','');
  assert.equal(c.nativeEditorDraft.input,edited);assert.equal(c.nativeEditorCode,'rejected');assert.equal(c.nativePending,null);
});
test('success clears retained draft and replacement response is accepted',()=>{
  const c=saved();const p={api:'omavless.control',version:1,id:'result',ok:true,revision:5,result:{schemaVersion:1,instanceId:'instance',operationId:c.nativePending.operationId,action:'profile-replace',applied:true}};
  assert(parser.parseAction(JSON.stringify(p),c.nativePending));c.finishNativeEditorAction({ok:true},false);assert.equal(c.nativeEditorDraft,null);
});
test('production private collectors destroyed, QML recovery uses plain text',()=>{
  assert.match(source,/finally \{ process.destroy\(\) \}/);assert.match(source,/write\(root\._nativeEditorSeed\)/);
  const panel=fs.readFileSync(path.join(__dirname,'../plugin/Panel.qml'),'utf8');
  assert.match(panel,/nativeEditorCode[^\n]*textFormat: Text.PlainText/);
  assert(!panel.includes('text: vless.nativeEditorDraft.input'));
});
console.log('native editor: '+count+' passed');
