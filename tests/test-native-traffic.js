// SPDX-License-Identifier: MIT
const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm'),path=require('node:path');
const source=fs.readFileSync(path.join(__dirname,'../plugin/Service.qml'),'utf8'),panel=fs.readFileSync(path.join(__dirname,'../plugin/Panel.qml'),'utf8');
const parser=vm.createContext({});vm.runInContext(fs.readFileSync(path.join(__dirname,'../plugin/NativeSnapshot.js'),'utf8'),parser);
const token='a'.repeat(64),fence={instanceId:'instance',revision:4,generation:0};
const frame=(sample,availability='observed')=>JSON.stringify({api:'omavless.control',version:1,id:'traffic',ok:true,revision:4,result:{schemaVersion:1,scope:'controller_attributed_tun_counters',instanceId:'instance',availability,sample}});
const sample=(time=1000,rx=100,tx=200)=>({identity:token,sampledAtMs:time,rxBytes:rx,txBytes:tx});
function context(){
 const c=vm.createContext({NativeSnapshot:parser,nativeTrafficEligible:true,nativeTrafficSample:null,nativeRxHistory:[],nativeTxHistory:[],_nativeTrafficRead:null,_nativeTrafficFence:null,_nativeTrafficGeneration:0,
 _nativeTrafficReceivedAt:0,_nativeTrafficClock:0,nativeSnapshot:{instanceId:'instance',revision:4},backendPath:'/synthetic/backend.sh',historyMaxPoints:30,now:1000,
 nativeTrafficComponent:{createObject:(_,p)=>({...p,running:false})},fmtRate:v=>String(v)+'/s',fmtSize:v=>String(v)});
 c.Date={now:()=>c.now};Object.defineProperty(c,'nativeTrafficFresh',{get(){return c.nativeTrafficEligible&&c.nativeTrafficSample!==null&&c._nativeTrafficClock>=c._nativeTrafficReceivedAt&&c._nativeTrafficClock-c._nativeTrafficReceivedAt<=10000}});
 for(const name of ['clearNativeTraffic','nativeTrafficCurrent','sampleNativeTraffic','finishNativeTraffic','nativeTrafficValue']){
 const start=source.indexOf('  function '+name+'('),end=source.indexOf('\n  }',start)+4;assert(start>=0);vm.runInContext(source.slice(start,end),c)}c.root=c;return c;
}
let count=0;function test(name,f){try{f();count++}catch(e){e.message=name+': '+e.message;throw e}}
test('strict owner revision bounded response and unavailable is not zero',()=>{
 assert.equal(parser.parseTraffic(frame(sample()),fence).rx,100);assert.equal(parser.parseTraffic(frame(null,'unavailable'),fence).available,false);
 for(const mutate of [p=>p.revision++,p=>p.result.instanceId='other',p=>p.result.extra='private',p=>p.result.sample.identity='bad',p=>p.result.sample.rxBytes=-1,p=>p.result.sample.txBytes=9007199254740992,p=>p.result.sample.sampledAtMs='1',p=>p.result.sample.path='private']){const p=JSON.parse(frame(sample()));mutate(p);assert.equal(parser.parseTraffic(JSON.stringify(p),fence),null)}
 assert.equal(parser.parseTraffic(frame(sample(),'unavailable'),fence),null);assert.equal(parser.parseTraffic('private-token',fence),null);
});
test('same shared corpus agrees with retained production applyTraffic oracle',()=>{
 const cases=JSON.parse(fs.readFileSync(path.join(__dirname,'traffic-cases.json'),'utf8'));let previous=null;
 for(const c of cases){const next=parser.trafficDelta({identity:token,rx:c.rx,tx:c.tx,sampledAtMs:c.at},previous);
 for(const key of ['rx','tx','rated','rxRate','txRate'])assert.equal(next[key],c[key],c.id+': '+key);previous=next}
});
test('first sample unknown rate then monotonic interval independent of wall clock',()=>{
 const c=context();c.finishNativeTraffic(fence,0,frame(sample()));assert.equal(c.nativeTrafficValue('rxRate',true),'--');assert.equal(c.nativeTrafficValue('rx',false),'100');
 c.now=500;c.finishNativeTraffic(fence,0,frame(sample(3000,300,600)));assert.equal(c.nativeTrafficValue('rxRate',true),'100/s');assert.equal(c.nativeRxHistory.length,1);
});
test('counter or identity reset clears graph and no invented idle sample',()=>{
 const c=context();c.finishNativeTraffic(fence,0,frame(sample()));c.finishNativeTraffic(fence,0,frame(sample(3000,300,600)));assert.equal(c.nativeRxHistory.length,1);
 c.finishNativeTraffic(fence,0,frame(sample(4000,1,2)));assert.equal(c.nativeRxHistory.length,0);assert.equal(c.nativeTrafficValue('rxRate',true),'--');
 c.finishNativeTraffic(fence,0,frame({...sample(5000,300,600),identity:'b'.repeat(64)}));assert.equal(c.nativeTrafficSample.rated,false);
});
test('unavailable malformed and error clear all former samples',()=>{
 for(const [code,raw] of [[0,frame(null,'unavailable')],[1,'private-error'],[0,'malformed']]){const c=context();c.finishNativeTraffic(fence,0,frame(sample()));c.finishNativeTraffic(fence,code,raw);assert.equal(c.nativeTrafficSample,null);assert.equal(c.nativeRxHistory.length,0);assert.equal(c.nativeTrafficValue('rx',false),'--')}
});
test('late read after close disconnect revision or owner change is discarded',()=>{
 for(const mutate of [c=>c.clearNativeTraffic(),c=>c.nativeTrafficEligible=false,c=>c.nativeSnapshot.instanceId='new',c=>c.nativeSnapshot.revision++]){const c=context();mutate(c);c.finishNativeTraffic(fence,0,frame(sample()));assert.equal(c.nativeTrafficSample,null)}
});
test('one fixed read, stale TTL clears baseline and history bounded',()=>{
 const c=context();assert(c.sampleNativeTraffic());assert.equal(c._nativeTrafficRead.command[2],'native-traffic');assert.equal(c.sampleNativeTraffic(),false);c._nativeTrafficRead=null;
 for(let i=0;i<40;i++)c.finishNativeTraffic(fence,0,frame(sample(1000+i*2000,100+i,200+i)));
 assert.equal(c.nativeRxHistory.length,30);c.now=20000;c.sampleNativeTraffic();assert.equal(c.nativeTrafficSample,null);assert.equal(c.nativeRxHistory.length,0);
});
test('panel old graph components reused and opt-in bar only, no invented device',()=>{
 assert(panel.includes('nativeTrafficMonitoring: vless.nativeOwner && ((root.opened && root.page === "main") || vless.showBarThroughput)'));
 assert(panel.includes('rxValues: vless.nativeRxHistory'));assert(panel.includes('DetailPair { Layout.fillWidth: true; label: root.textFor("metric.receiving")'));
 assert(!source.includes('primaryDevice = "native"'));assert(source.includes('interval: 2000'));assert(source.includes('finally { process.destroy() }'));
 assert(source.includes('property Timer watchdog: Timer'));
});
console.log('native traffic: '+count+' passed');
