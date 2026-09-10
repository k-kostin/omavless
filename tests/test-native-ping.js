// SPDX-License-Identifier: MIT
const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const source=fs.readFileSync(__dirname+'/../plugin/Service.qml','utf8');
const panel=fs.readFileSync(__dirname+'/../plugin/Panel.qml','utf8');
const parser=vm.createContext({});vm.runInContext(fs.readFileSync(__dirname+'/../plugin/NativeSnapshot.js','utf8'),parser);
const fence={instanceId:'instance',revision:4,generation:0,host:'1.1.1.1'};
const frame=(outcome='reply',latency=4.5)=>JSON.stringify({api:'omavless.control',version:1,id:'ping',ok:true,revision:4,result:{
  schemaVersion:1,scope:'controller_attributed_tun_icmp',instanceId:'instance',availability:outcome==='unavailable'?'unavailable':'observed',
  sample:outcome==='unavailable'?null:{outcome,latencyMs:latency},code:outcome==='reply'?'ok':outcome==='loss'?'timeout':'probe_unavailable'}});
function context(){
 const c=vm.createContext({NativeSnapshot:parser,nativePingEligible:true,nativePingSamples:[],nativePingStatus:'',_nativePingRead:null,
  _nativePingFence:null,_nativePingGeneration:0,_nativePingReceivedAt:0,_nativePingClock:1000,now:1000,pingHost:'1.1.1.1',
  nativeSnapshot:{instanceId:'instance',revision:4},backendPath:'/synthetic/backend.sh',nativePingComponent:{createObject:(_,p)=>({...p,running:false})}});
 c.Date={now:()=>c.now};c.root=c;
 for(const name of ['clearNativePing','nativePingCurrent','sampleNativePing','testNativePing','finishNativePing']){
  const start=source.indexOf('  function '+name+'('),end=source.indexOf('\n  }',start)+4;assert(start>=0);vm.runInContext(source.slice(start,end),c)
 }return c;
}
let count=0;function test(name,f){try{f();count++}catch(e){e.message=name+': '+e.message;throw e}}
test('strict envelope, finite reply, loss and unavailable',()=>{
 assert.equal(parser.parsePing(frame(),fence).value,4.5);
 assert.equal(parser.parsePing(frame('loss',null),fence).value,-1);
 assert.equal(parser.parsePing(frame('unavailable'),fence).available,false);
 for(const mutate of [p=>p.version=2,p=>p.revision++,p=>p.result.instanceId='other',p=>p.result.extra='private',p=>p.result.sample.latencyMs=-1,
  p=>p.result.sample.latencyMs=3001,p=>p.result.sample.latencyMs='3',p=>p.result.sample.latencyMs=null,p=>p.result.sample.host='private.invalid',
  p=>p.result.code='timeout',p=>p.result.availability='unavailable',p=>p.result.sample.outcome='unknown']){
  const p=JSON.parse(frame());mutate(p);assert.equal(parser.parsePing(JSON.stringify(p),fence),null)
 }
 assert.equal(parser.parsePing(frame('loss',3),fence),null);
 assert.equal(parser.parsePing('https://private.invalid/password=synthetic',fence),null);
 assert.equal(parser.parsePing('x'.repeat(2049),fence),null);
});
test('ten-sample mean and rounded loss match actual legacy getters',()=>{
 const c=vm.createContext({pingSamples:[]});
 for(const [name,next] of [['pingLatency','pingLoss'],['pingLoss','function']]){
  const start=source.indexOf('readonly property real '+name+': {')>=0?source.indexOf('readonly property real '+name+': {'):source.indexOf('readonly property int '+name+': {');
  assert(start>=0,name);const brace=source.indexOf('{',start),end=source.indexOf('\n  }',brace)+4;
  vm.runInContext('function '+name+'() '+source.slice(brace,end),c)
 }
 for(const samples of [[],[3],[1,2,-1],[-1,-1],[0,0.5,100,3000,-1],[1,2,3,4,5,6,7,8,9,-1]]){
  c.pingSamples=samples;const actual=parser.pingWindow(samples);
  assert.equal(actual.loss,samples.length?c.pingLoss():null);
  const successes=samples.filter(x=>x>=0);assert.equal(actual.latency,successes.length?c.pingLatency():null)
 }
 assert.equal(parser.pingWindow(new Array(11).fill(1)),null);
 assert.equal(parser.pingWindow([NaN]),null);assert.equal(parser.pingWindow([-2]),null);
});
test('only observed reply/loss enters history, bounded to ten',()=>{
 const c=context();for(let n=0;n<12;n++)c.finishNativePing(fence,0,frame('reply',n));
 assert.equal(c.nativePingSamples.length,10);assert.equal(c.nativePingSamples[0],2);
 c.finishNativePing(fence,0,frame('loss',null));assert.equal(c.nativePingSamples.at(-1),-1);
 c.finishNativePing(fence,0,frame('unavailable'));assert.equal(c.nativePingSamples.length,10);assert.equal(c.nativePingStatus,'unavailable');
 c.finishNativePing(fence,1,'private raw error');assert.equal(c.nativePingStatus,'unavailable');
});
test('one fixed request, target never placed in argv',()=>{
 const c=context();assert(c.sampleNativePing());assert.equal(c._nativePingRead.command[2],'native-ping');
 assert(!c._nativePingRead.command.includes(c.pingHost));assert.equal(c.sampleNativePing(),false);
 assert(source.includes('write(context.host + "\\n")'));assert(source.includes('stdinEnabled = false'));
});
test('manual test clears window but joins same in-flight request',()=>{
 const c=context();c.sampleNativePing();const active=c._nativePingRead;c.nativePingSamples=[4];assert(c.testNativePing());
 assert.equal(c.nativePingSamples.length,0);assert.equal(c._nativePingRead,active);
 c._nativePingRead=null;assert(c.testNativePing());assert.notEqual(c._nativePingRead,active);
});
test('late close, owner, revision, host and generation results discarded',()=>{
 for(const mutate of [c=>c.nativePingEligible=false,c=>c.nativeSnapshot.instanceId='other',c=>c.nativeSnapshot.revision++,c=>c.pingHost='8.8.8.8',c=>c.clearNativePing()]){
  const c=context();mutate(c);c.finishNativePing(fence,0,frame());assert.equal(c.nativePingSamples.length,0)
 }
});
test('no probe while disabled; stale in-flight manual result not adopted',()=>{
 const c=context();c.nativePingEligible=false;assert.equal(c.sampleNativePing(),false);assert.equal(c.testNativePing(),false);
 c.nativePingEligible=true;c.sampleNativePing();c.pingHost='8.8.8.8';assert.equal(c.testNativePing(),false);
});
test('stale and backwards clock clear old window',()=>{
 for(const now of [0,11001]){const c=context();c.finishNativePing(fence,0,frame());c.now=now;c.sampleNativePing();assert.equal(c.nativePingSamples.length,0)}
});
test('visible-main only, native controls and localized number sink',()=>{
 assert(panel.includes('nativePingMonitoring: vless.nativeOwner && root.opened && root.page === "main"'));
 assert(panel.includes('onClicked: vless.testNativePing()'));assert(panel.includes('toLocaleString(Qt.locale(root.uiLocale), "f"'));
 assert(source.includes('interval: 3000'));assert(source.includes('pingHost !== ""'));
 assert(source.includes('property Timer watchdog: Timer'));assert(source.includes('finally { process.destroy() }'));
});
console.log('native ping: '+count+' checks passed');
