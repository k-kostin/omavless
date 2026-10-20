// SPDX-License-Identifier: MIT
// Synthetic provider assertions only; no HTTP/runtime/host/credential access.
const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const parser=vm.createContext({});vm.runInContext(fs.readFileSync(__dirname+'/../plugin/SubscriptionUsage.js','utf8'),parser);
const context={instanceId:'fixture-owner',revision:7};
const usage=()=>({uploadBytes:'1',downloadBytes:'2',totalBytes:'10',expiryUnixSeconds:'1700000000'});
const frame=(u=usage(),availability='reported')=>({api:'omavless.control',version:1,id:'fixed-read',ok:true,revision:7,result:{schemaVersion:1,scope:'private_provider_reported_usage',instanceId:'fixture-owner',availability,usage:u}});
let count=0;function test(name,fn){try{fn();count++}catch(e){e.message=name+': '+e.message;throw e}}
test('exact private shape and target epoch, no raw provider field retained',()=>{
 const out=parser.parse(JSON.stringify(frame()),context);assert(out);assert.equal(out.used,'3');assert.equal(out.remaining,'7');assert.equal(out.expiry,'2023-11-14 22:13 UTC');assert.equal(out.instanceId,undefined);
 for(const mutate of [p=>p.result.url='private-token',p=>p.result.usage.token='private-token',p=>p.id='',p=>p.revision++,p=>p.result.instanceId='new-owner',p=>p.result.scope='support',p=>p.ok=false,p=>p.result.schemaVersion=2]){const p=frame();mutate(p);assert.equal(parser.parse(JSON.stringify(p),context),null)}
 for(const raw of ['private-token','x'.repeat(4097),'null','[]'])assert.equal(parser.parse(raw,context),null);
});
test('not-provided is distinct from failed, and zero is never unlimited/no-expiry proof',()=>{
 assert.equal(parser.parse(JSON.stringify(frame(null,'not_provided')),context).status,'notProvided');
 assert.equal(parser.parse(JSON.stringify(frame(usage(),'not_provided')),context),null);
 const out=parser.parse(JSON.stringify(frame({...usage(),totalBytes:'0',expiryUnixSeconds:'0'})),context);
 assert.equal(out.total,null);assert.equal(out.remaining,null);assert.equal(out.expiry,null);
});
test('canonical decimal u64 without Number coercion, overflow and exceeded quota suppression',()=>{
 for(const value of [0,1,-1,'','01','+1','1e3',' 1','18446744073709551616','private-token',null]){
  for(const key of ['uploadBytes','downloadBytes','totalBytes'])assert.equal(parser.parse(JSON.stringify(frame({...usage(),[key]:value})),context),null);
 }
 assert.equal(parser.add('9007199254740993','1'),'9007199254740994');
 assert.equal(parser.add('18446744073709551615','0'),'18446744073709551615');
 assert.equal(parser.add('18446744073709551615','1'),null);
 assert.equal(parser.remaining('18446744073709551615','9007199254740993'),'18437736874454810622');
 assert.equal(parser.remaining('3','3'),'0');assert.equal(parser.remaining('2','3'),null);
 const out=parser.parse(JSON.stringify(frame({...usage(),uploadBytes:'18446744073709551615',downloadBytes:'1'})),context);assert.equal(out.used,null);assert.equal(out.remaining,null);
});
test('UTC boundary ignores local timezone, leap/calendar maximum and unknown expiry',()=>{
 for(const [value,wanted]of [['1','1970-01-01 00:00 UTC'],['951782400','2000-02-29 00:00 UTC'],['253402300799','9999-12-31 23:59 UTC']])assert.equal(parser.utcDate(value),wanted);
 assert.equal(parser.utcDate('0'),null);assert.equal(parser.utcDate('253402300800'),null);
 for(const value of ['-1','01','253402300800',1])assert.equal(parser.parse(JSON.stringify(frame({...usage(),expiryUnixSeconds:value})),context),null);
 assert.equal(parser.parse(JSON.stringify(frame({...usage(),expiryUnixSeconds:null})),context).expiry,null);
});
test('byte rendering is bounded, locale-specific and never coerces a large counter',()=>{
 for(const [n,en,ru]of [['0','0 B','0 Б'],['1023','1023 B','1023 Б'],['1024','1.00 KiB','1,00 КиБ'],['1536','≈ 1.50 KiB','≈ 1,50 КиБ'],['18446744073709551615','≈ 15.99 EiB','≈ 15,99 ЭиБ']]){
  assert.equal(parser.bytes(n,'en'),en);assert.equal(parser.bytes(n,'ru'),ru);
 }
 assert.equal(parser.bytes(null,'en'),null);assert.equal(parser.bytes('private-token','en'),null);
});
const service=fs.readFileSync(__dirname+'/../plugin/Service.qml','utf8');
function serviceContext(){
 const c=vm.createContext({SubscriptionUsage:parser,NativeSnapshot:{id:(s)=>typeof s==='string'&&s.length>0},panelVisible:true,nativeCanAct:true,nativeOutcomeUnknown:false,nativeActionRunning:false,nativeUsageSubscriptionId:'one',nativeSnapshot:{instanceId:'fixture-owner',revision:7,subscriptions:[{id:'one'},{id:'two'}]},nativeSubscriptionUsage:null,nativeSubscriptionUsageStatus:'',_nativeUsageRead:null,_nativeUsageContext:null,_nativeUsageGeneration:0,backendPath:'/synthetic/backend.sh',requests:[]});c.root=c;
 c.nativeSubscriptionUsageComponent={createObject:(_,p)=>{const process={...p,destroy(){this.destroyed=true}};c.requests.push(process);return process}};
 for(const name of ['clearNativeSubscriptionUsage','nativeSubscriptionUsageCurrent','requestNativeSubscriptionUsage','finishNativeSubscriptionUsage']){
  const start=service.indexOf('  function '+name+'('),end=service.indexOf('\n  }',start)+4;assert(start>=0);vm.runInContext(service.slice(start,end),c);
 }
 return c;
}
test('only explicit request starts one exact target read; busy activation refuses',()=>{
 const c=serviceContext();assert.equal(c.requests.length,0);assert(c.requestNativeSubscriptionUsage());assert.equal(c.requests.length,1);
 assert.equal(c.requests[0].command.join('|'),'bash|/synthetic/backend.sh|native-subscription-usage|one');
 assert.equal(c.requestNativeSubscriptionUsage(),false);assert.equal(c.requests.length,1);
 const process=c._nativeUsageRead;c.finishNativeSubscriptionUsage(process,0,JSON.stringify(frame()));
 assert.equal(c.nativeSubscriptionUsage.remaining,'7');assert.equal(c.nativeSubscriptionUsageStatus,'reported');assert(process.destroyed);
});
test('clear and late completion never replay a provider request or replace another target',()=>{
 const c=serviceContext();c.requestNativeSubscriptionUsage();const old=c._nativeUsageRead;c.clearNativeSubscriptionUsage();c.nativeUsageSubscriptionId='two';
 assert.equal(c.nativeSubscriptionUsage,null);assert.equal(c.requestNativeSubscriptionUsage(),false);
 c.finishNativeSubscriptionUsage(old,0,JSON.stringify(frame()));assert.equal(c.nativeSubscriptionUsage,null);assert.equal(c.requests.length,1);
 assert(c.requestNativeSubscriptionUsage());const newer=c._nativeUsageRead;
 c.finishNativeSubscriptionUsage(old,0,JSON.stringify(frame()));assert.equal(c._nativeUsageRead,newer);assert.equal(c.requests.length,2);
 c.finishNativeSubscriptionUsage(newer,0,JSON.stringify(frame(null,'not_provided')));assert.equal(c.nativeSubscriptionUsageStatus,'notProvided');assert.equal(c.nativeSubscriptionUsage,null);
});
test('owner revision target removal admission or panel close fences a reply without retry',()=>{
 for(const change of [c=>c.panelVisible=false,c=>c.nativeCanAct=false,c=>c.nativeOutcomeUnknown=true,c=>c.nativeActionRunning=true,c=>c.nativeSnapshot.revision++,c=>c.nativeSnapshot.instanceId='new',c=>c.nativeSnapshot.subscriptions=[],c=>c.nativeUsageSubscriptionId='two']){
  const c=serviceContext();c.requestNativeSubscriptionUsage();const p=c._nativeUsageRead;change(c);c.finishNativeSubscriptionUsage(p,0,JSON.stringify(frame()));assert.equal(c.nativeSubscriptionUsage,null);assert.equal(c.requests.length,1);assert(p.destroyed);
 }
});
test('failed requests discard raw values and do not fake not-provided or success',()=>{
 const c=serviceContext();c.requestNativeSubscriptionUsage();c.finishNativeSubscriptionUsage(c._nativeUsageRead,2,'private-token');
 assert.equal(c.nativeSubscriptionUsage,null);assert.equal(c.nativeSubscriptionUsageStatus,'failed');assert(!JSON.stringify(c.nativeSubscriptionUsage).includes('private-token'));
 c.clearNativeSubscriptionUsage();assert.equal(c.nativeSubscriptionUsageStatus,'');assert.equal(c._nativeUsageContext,null);
 const component=service.slice(service.indexOf('id: nativeSubscriptionUsageComponent'),service.indexOf('property string nativeDetailsProfileId'));
 assert(component.includes('property Timer watchdog'));assert(!component.includes('console.'));assert(!component.includes('refreshNative'));
 assert.match(service,/onNativeSubscriptionUsageEligibleChanged: if \(!nativeSubscriptionUsageEligible\) clearNativeSubscriptionUsage\(\)/);
 assert.match(service,/onNativeUsageSubscriptionIdChanged: clearNativeSubscriptionUsage\(\)/);
});
const panel=fs.readFileSync(__dirname+'/../plugin/Panel.qml','utf8');
function panelContext(){
 const later=[],c=vm.createContext({opened:true,page:'subscription',nativeSubscriptionId:'one',nativeSubscription:{id:'one'},nativeUsageTargetId:'',nativeUsageVisit:0,nativeUsageQueued:false,subscriptionUsagePrompt:{visible:false},Qt:{callLater:f=>later.push(f)},vless:{nativeCanAct:true,nativeSubscriptionUsageBusy:false,requestNativeSubscriptionUsage(){c.requests++;return true},clearNativeSubscriptionUsage(){c.cleared++}},requests:0,cleared:0,originFocus:0,fallbackFocus:0,nativeSubscriptionUsage:{visible:true,enabled:true,forceActiveFocus(){c.originFocus++}},keyCatcher:{forceActiveFocus(){c.fallbackFocus++}}});
 c.root=c;c.flush=()=>{while(later.length)later.shift()()};
 Object.defineProperty(c,'modalInputActive',{get:()=>c.otherModal||c.subscriptionUsagePrompt.visible});
 for(const name of ['openNativeUsage','requestNativeUsage','closeNativeUsage']){
  const start=panel.indexOf('  function '+name+'('),end=panel.indexOf('\n  }',start)+4;assert(start>=0);vm.runInContext(panel.slice(start,end),c);
 }
 return c;
}
test('panel opens only an explicit current subscription and queues one binding-settled read',()=>{
 const c=panelContext();assert.equal(c.requests,0);assert(c.openNativeUsage());assert.equal(c.nativeUsageTargetId,'one');assert(c.nativeUsageQueued);assert.equal(c.requests,0);
 assert.equal(c.openNativeUsage(),false);c.flush();assert.equal(c.requests,1);assert.equal(c.nativeUsageQueued,false);
 assert(c.requestNativeUsage());assert.equal(c.requests,2); // explicit Check again
 for(const change of [c=>c.opened=false,c=>c.page='main',c=>c.otherModal=true,c=>c.nativeSubscription=null,c=>c.vless.nativeCanAct=false,c=>c.vless.nativeSubscriptionUsageBusy=true]){
  const denied=panelContext();change(denied);assert.equal(denied.openNativeUsage(),false);denied.flush();assert.equal(denied.requests,0);
 }
});
test('queued open is fenced by close, page, target, removal and newer visits',()=>{
 for(const change of [c=>c.closeNativeUsage(),c=>c.opened=false,c=>c.page='main',c=>c.nativeSubscriptionId='two',c=>c.nativeSubscription=null]){
  const c=panelContext();c.openNativeUsage();change(c);c.flush();assert.equal(c.requests,0);
 }
 const c=panelContext();c.openNativeUsage();c.closeNativeUsage();c.openNativeUsage();c.flush();assert.equal(c.requests,1);
});
test('close erases transient state and restores eligible origin focus only in the same surface',()=>{
 const c=panelContext();c.openNativeUsage();c.flush();c.closeNativeUsage();c.flush();
 assert.equal(c.cleared,1);assert.equal(c.nativeUsageTargetId,'');assert.equal(c.nativeUsageQueued,false);assert.equal(c.subscriptionUsagePrompt.visible,false);assert.equal(c.originFocus,1);
 for(const change of [c=>c.page='main',c=>c.nativeSubscriptionUsage.enabled=false,c=>c.nativeSubscriptionUsage.visible=false]){
  const c=panelContext();change(c);c.closeNativeUsage();c.flush();assert.equal(c.originFocus,0);assert.equal(c.fallbackFocus,1);
 }
 for(const change of [c=>c.opened=false,c=>c.otherModal=true]){const c=panelContext();change(c);c.closeNativeUsage();c.flush();assert.equal(c.originFocus+c.fallbackFocus,0)}
 assert.match(panel.slice(panel.indexOf('  onPageChanged:'),panel.indexOf('  property string nativeUsageTargetId:')),/subscriptionUsagePrompt.visible.*closeNativeUsage\(\)/);
 assert.match(panel.slice(panel.indexOf('  onNativeSubscriptionIdChanged:'),panel.indexOf('  onNativeSubscriptionIdChanged:')+160),/subscriptionUsagePrompt.visible.*closeNativeUsage\(\)/);
});
test('QML Process var context may copy the object without invalidating a current reply',()=>{
 const c=serviceContext();c.requestNativeSubscriptionUsage();const p=c._nativeUsageRead;
 p.context={...p.context};c.finishNativeSubscriptionUsage(p,0,JSON.stringify(frame()));
 assert.equal(c.nativeSubscriptionUsageStatus,'reported');assert.equal(c.nativeSubscriptionUsage.remaining,'7');
});
test('modal keyboard scroll reads all rows without focus changes or provider requests',()=>{
 const modal=fs.readFileSync(__dirname+'/../plugin/SubscriptionUsagePrompt.qml','utf8');
 const keys=['Escape','Home','End','Up','Down','PageUp','PageDown','Tab','Backtab'];
 const qt={ShiftModifier:1024};keys.forEach((key,i)=>qt['Key_'+key]=i+1);
 const c=vm.createContext({Qt:qt,Style:{space:v=>v},scroll:{contentY:0,contentHeight:700,height:200},focusTargets:[],cancels:0,canceled:()=>c.cancels++});
 const start=modal.indexOf('  function navigate('),end=modal.indexOf('\n  }',start)+4;vm.runInContext(modal.slice(start,end),c);
 for(const [key,wanted]of [['Down',32],['PageDown',232],['End',500],['Down',500],['PageUp',300],['Up',268],['Home',0],['Up',0]]){
  const e={key:qt['Key_'+key],accepted:false};c.navigate(e);assert(e.accepted);assert.equal(c.scroll.contentY,wanted);
 }
 c.scroll.contentHeight=100;c.scroll.contentY=0;c.navigate({key:qt.Key_End});assert.equal(c.scroll.contentY,0);
 c.navigate({key:qt.Key_Escape});assert.equal(c.cancels,1);
 assert.match(modal,/anchors.bottom: actions.top/);assert.match(modal,/opacity: enabled \? 1 : 0.45/);
});
test('hosted gate includes this regression exactly once',()=>{
 const runner=fs.readFileSync(__dirname+'/run.sh','utf8');assert.equal(runner.split('node "$here/test-native-subscription-usage.js"').length-1,1);
});
console.log(`${count} native subscription usage tests passed`);
