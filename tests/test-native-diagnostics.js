// SPDX-License-Identifier: MIT
// Execute production parser/Service methods; no controller/network/private data.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const parser=vm.createContext({});vm.runInContext(fs.readFileSync(path.join(__dirname,'../plugin/NativeSnapshot.js'),'utf8'),parser);
const source=fs.readFileSync(path.join(__dirname,'../plugin/Service.qml'),'utf8');
function summary(){return {version:1,rules:{total:1,shown:1,truncated:false,items:[{type:'DomainSuffix',payload:'example.invalid',target:'VPN'}]},providers:{total:1,shown:1,truncated:false,items:[{name:'Synthetic',behavior:'domain',ruleCount:1,updatedAt:'',status:'loaded',refreshable:true}]}};}
function frame(result=summary()){return JSON.stringify({api:'omavless.control',version:1,id:'diagnostics',ok:true,revision:4,result});}
function context(){
  const c=vm.createContext({NativeSnapshot:parser,nativeOwner:true,diagnosticsPageVisible:true,nativeSnapshot:{instanceId:'instance',revision:4},
    nativeObservation:{synthetic:'unchanged'},nativePending:null,_advancedDiagnosticsGeneration:2,_nativeDiagnosticsRequestGeneration:0,_nativeDiagnosticsOwner:true,_nativeDiagnosticsInstance:'instance',
    _nativeDiagnosticsProcess:null,_advancedDiagnosticsRefreshPending:false,advancedDiagnosticsErrorCode:'',advancedDiagnosticsError:'',
    loadedRules:[],loadedRuleTotal:0,loadedRulesTruncated:false,loadedRuleProviders:[],loadedRuleProviderTotal:0,loadedRuleProvidersTruncated:false,advancedDiagnosticsLoadedAt:0,
    nativeDiagnosticsComponent:{createObject:(_root,properties)=>({...properties,running:false})},backendPath:'/synthetic/backend.sh',queued:[],rejected:0});
  c.root=c;c.Qt={callLater:fn=>c.queued.push(fn)};c.rejectNativeAction=()=>{c.rejected++;return false;};
  for(const name of ['clearNativeDiagnosticsSample','invalidateNativeDiagnosticsIdentity','refreshNativeDiagnostics','finishNativeDiagnostics','refreshAdvancedDiagnostics','applyAdvancedDiagnostics','plainText','refreshRuleProviders']){
    const start=source.indexOf('  function '+name+'('),end=source.indexOf('\n  }',start)+4;
    assert(start>=0&&end>start,name);vm.runInContext(source.slice(start,end),c);
  }
  const start=source.indexOf('  onDiagnosticsPageVisibleChanged: {'),end=source.indexOf('\n  }',start)+4;
  vm.runInContext('this.visibilityChanged = function() '+source.slice(start,end).replace('  onDiagnosticsPageVisibleChanged: ',''),c);
  return c;
}
let count=0;function test(name,f){try{f();count++;}catch(e){e.message=name+': '+e.message;throw e;}}
test('independent diagnostics sample carries no health state and providers cannot mutate',()=>{
  const r=parser.parseDiagnosticsSummary(frame());assert(r);assert.equal(r.rules.items[0].target,'VPN');
  assert.equal(r.providers.items[0].refreshable,false);assert.equal(r.connected,undefined);assert.equal(r.instanceId,undefined);
  const p=JSON.parse(frame());p.revision=900;assert(parser.parseDiagnosticsSummary(JSON.stringify(p)));
});
test('unknown envelope/result/row fields and malformed JSON reject wholesale',()=>{
  for(const mutate of [p=>p.extra='private',p=>p.result.secret='private',p=>p.result.rules.extra=true,p=>p.result.rules.items[0].secret='private',p=>p.result.providers.items[0].secret='private',p=>p.api='other',p=>p.version=2,p=>p.ok=false,p=>p.revision=-1,p=>p.result.version=2]){
    const p=JSON.parse(frame());mutate(p);assert.equal(parser.parseDiagnosticsSummary(JSON.stringify(p)),null);
  }
  for(const raw of ['',null,'private-token','x'.repeat(262145)])assert.equal(parser.parseDiagnosticsSummary(raw),null);
});
test('row counts truncation and string limits are exact',()=>{
  for(const mutate of [r=>r.rules.total=65537,r=>r.rules.shown=0,r=>r.rules.truncated=true,r=>r.providers.total=257,r=>r.providers.shown=2,
    r=>r.rules.items[0].type='x'.repeat(81),r=>r.rules.items[0].payload='x'.repeat(513),r=>r.rules.items[0].target='private-group',
    r=>r.providers.items[0].name='x'.repeat(161),r=>r.providers.items[0].behavior='x'.repeat(81),r=>r.providers.items[0].updatedAt='x'.repeat(81),
    r=>r.providers.items[0].ruleCount=1.5,r=>r.providers.items[0].ruleCount=-2,r=>r.providers.items[0].status='bad',r=>r.providers.items[0].refreshable='yes',
    r=>r.rules.items[0].payload='\ud800',r=>r.providers.items[0].name='x\ny']){
    const r=summary();mutate(r);assert.equal(parser.parseDiagnosticsSummary(frame(r)),null);
  }
  const r=summary();r.rules.total=2;r.rules.truncated=true;assert(parser.parseDiagnosticsSummary(frame(r)));
});
test('all permitted terminal categories, provider unknown/empty and maxima remain bounded',()=>{
  for(const target of ['VPN','DIRECT','REJECT']){const r=summary();r.rules.items[0].target=target;assert(parser.parseDiagnosticsSummary(frame(r)));}
  for(const [ruleCount,status] of [[-1,'unknown'],[0,'empty'],[1000000000,'loaded']]){const r=summary();Object.assign(r.providers.items[0],{ruleCount,status});assert(parser.parseDiagnosticsSummary(frame(r)));}
  const r=summary();r.rules.items=Array.from({length:2048},()=>({type:'Match',payload:'',target:'DIRECT'}));r.rules.total=2048;r.rules.shown=2048;
  r.providers.items=Array.from({length:256},(_,i)=>({name:'synthetic-'+i,behavior:'',ruleCount:-1,updatedAt:'',status:'unknown',refreshable:false}));r.providers.total=256;r.providers.shown=256;
  assert(parser.parseDiagnosticsSummary(frame(r)));r.rules.items.push(r.rules.items[0]);r.rules.shown++;r.rules.total++;assert.equal(parser.parseDiagnosticsSummary(frame(r)),null);
});
test('actual Service fixed read updates display only, never current health or mutations',()=>{
  const c=context(),snapshot=JSON.stringify(c.nativeSnapshot),observation=JSON.stringify(c.nativeObservation);
  assert(c.refreshAdvancedDiagnostics());assert.deepEqual(Array.from(c._nativeDiagnosticsProcess.command),['bash','/synthetic/backend.sh','native-diagnostics-summary']);
  c._nativeDiagnosticsProcess=null;c.finishNativeDiagnostics(2,'instance',0,frame());
  assert.equal(c.loadedRules.length,1);assert.equal(c.loadedRuleProviders[0].refreshable,false);assert(c.advancedDiagnosticsLoadedAt>0);
  assert.equal(JSON.stringify(c.nativeSnapshot),snapshot);assert.equal(JSON.stringify(c.nativeObservation),observation);assert.equal(c.nativePending,null);
  assert.equal(c.refreshRuleProviders(),false);assert.equal(c.rejected,1);
});
test('private errors and malformed samples become fixed unavailable with empty rows',()=>{
  for(const [code,output] of [[2,'private-token'],[0,'private-token']]){
    const c=context();c.finishNativeDiagnostics(2,'instance',0,frame());c.finishNativeDiagnostics(2,'instance',code,output);
    assert.equal(c.loadedRules.length,0);assert.equal(c.loadedRuleProviders.length,0);assert.equal(c.advancedDiagnosticsLoadedAt,0);
    assert.equal(c.advancedDiagnosticsErrorCode,'unavailable');assert(!c.advancedDiagnosticsError.includes('private-token'));
  }
});
test('closed page old generation and known daemon change discard late samples',()=>{
  for(const mutate of [c=>c.diagnosticsPageVisible=false,c=>c._advancedDiagnosticsGeneration++,c=>c.nativeSnapshot.instanceId='restarted',c=>c.nativeOwner=false]){
    const c=context();mutate(c);c.finishNativeDiagnostics(2,'instance',0,frame());assert.equal(c.loadedRules.length,0);assert.equal(c.advancedDiagnosticsLoadedAt,0);assert.equal(c.advancedDiagnosticsErrorCode,'');
  }
});
test('page dismissal clears previous rows; reopen coalesces old running request',()=>{
  const c=context();c.finishNativeDiagnostics(2,'instance',0,frame());c.refreshNativeDiagnostics();
  c.diagnosticsPageVisible=false;c.visibilityChanged();assert.equal(c.loadedRules.length,0);assert.equal(c.advancedDiagnosticsLoadedAt,0);
  c.diagnosticsPageVisible=true;c.visibilityChanged();assert.equal(c._advancedDiagnosticsRefreshPending,true);
  const process=c._nativeDiagnosticsProcess;c._nativeDiagnosticsProcess=null;c.finishNativeDiagnostics(process.generation,process.instance,0,frame());
  assert.equal(c.loadedRules.length,0);assert.equal(c.queued.length,1);c.queued[0]();assert.equal(c._nativeDiagnosticsProcess.generation,4);
});
test('native provider mutation control and keyboard loop remain guarded',()=>{
  const advanced=fs.readFileSync(path.join(__dirname,'../plugin/AdvancedDiagnostics.qml'),'utf8');
  assert.match(advanced,/visible: !page.readOnlyNative/);assert.match(advanced,/enabled: !page.readOnlyNative && service/);
  assert.match(advanced,/KeyNavigation.backtab: page.readOnlyNative \? searchField : providersRefreshButton/);
  const panel=fs.readFileSync(path.join(__dirname,'../plugin/Panel.qml'),'utf8');
  assert.match(panel,/onRefreshProvidersRequested:.*!vless.nativeOwner/);
});
test('byte cap cannot be bypassed with multibyte values',()=>{
  const r=summary();r.rules.items=Array.from({length:1500},()=>({type:'Match',payload:'я'.repeat(80),target:'DIRECT'}));r.rules.total=1500;r.rules.shown=1500;
  const raw=frame(r);assert(raw.length<262144);assert(Buffer.byteLength(raw)>262144);assert.equal(parser.parseDiagnosticsSummary(raw),null);
});
test('actual disposable sample collector destroyed for success error and late dismissal',()=>{
  const from=source.indexOf('      onExited: function(code) {',source.indexOf('id: nativeDiagnosticsComponent')),to=source.indexOf('\n      }',from)+8;
  assert(from>0&&to>from);
  for(const code of [0,2])for(const cancelled of [false,true]){
    const c=context();c.refreshNativeDiagnostics();c.generation=2;c.instance='instance';c.sampleOutput={text:frame()};let destroyed=0;c.sample={destroy:()=>destroyed++};
    if(cancelled)c.diagnosticsPageVisible=false;
    vm.runInContext('this.sampleExited = '+source.slice(from,to).replace('      onExited: ',''),c);c.sampleExited(code);
    assert.equal(destroyed,1);assert.equal(c._nativeDiagnosticsProcess,null);if(cancelled)assert.equal(c.loadedRules.length,0);
  }
});
test('known owner or daemon identity changes invalidate already displayed sample and queue refresh',()=>{
  for(const mutate of [c=>c.nativeOwner=false,c=>c.nativeSnapshot.instanceId='new-instance',c=>c.nativeSnapshot=null]){
    const c=context();c.finishNativeDiagnostics(2,'instance',0,frame());assert(c.loadedRules.length);mutate(c);
    assert(c.invalidateNativeDiagnosticsIdentity());assert.equal(c.loadedRules.length,0);assert.equal(c.loadedRuleProviders.length,0);assert.equal(c.advancedDiagnosticsLoadedAt,0);assert.equal(c._advancedDiagnosticsGeneration,3);
    c.finishNativeDiagnostics(2,'instance',0,frame());assert.equal(c.loadedRules.length,0);
  }
  const c=context();c.refreshNativeDiagnostics();c.nativeSnapshot.instanceId='new';c.invalidateNativeDiagnosticsIdentity();assert.equal(c._advancedDiagnosticsRefreshPending,true);
});
test('ordinary revision polling never triggers another sample or clears independent sample',()=>{
  const c=context();c.finishNativeDiagnostics(2,'instance',0,frame());c.nativeSnapshot.revision++;
  assert.equal(c.invalidateNativeDiagnosticsIdentity(),false);assert.equal(c._nativeDiagnosticsProcess,null);assert.equal(c.loadedRules.length,1);assert.equal(c._advancedDiagnosticsGeneration,2);
});
test('failed sample creation clears prior display instead of retaining stale results',()=>{
  const c=context();c.finishNativeDiagnostics(2,'instance',0,frame());c.nativeDiagnosticsComponent.createObject=()=>null;
  assert.equal(c.refreshNativeDiagnostics(),false);assert.equal(c.loadedRules.length,0);assert.equal(c.advancedDiagnosticsLoadedAt,0);assert.equal(c.advancedDiagnosticsErrorCode,'unavailable');
});
console.log('native diagnostics: '+count+' passed');
