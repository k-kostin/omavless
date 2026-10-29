// SPDX-License-Identifier: MIT
const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const service=fs.readFileSync(__dirname+'/../plugin/Service.qml','utf8');
const panel=fs.readFileSync(__dirname+'/../plugin/Panel.qml','utf8');
const parser=vm.createContext({});vm.runInContext(fs.readFileSync(__dirname+'/../plugin/NativeSnapshot.js','utf8'),parser);
const presentation=vm.createContext({});vm.runInContext(fs.readFileSync(__dirname+'/../plugin/NativePresentation.js','utf8'),presentation);
const fence={instanceId:'runtime',revision:7};
const result={schemaVersion:1,scope:'private_recorded_tun_bytes',availability:'recorded',instanceId:'runtime',enabled:true,
 periodStartedAtUtc:100,receivedBytes:'18446744073709551615',sentBytes:'9007199254740993',partial:true,controlRevision:2};
const frame=(r=result)=>JSON.stringify({api:'omavless.control',version:1,id:'statistics',ok:true,revision:7,result:r});
function expression(name){const a=service.indexOf('  readonly property bool '+name+':');assert(a>=0);const lines=service.slice(a).split('\n');let x=lines[0].split(':').slice(1).join(':');for(let i=1;i<lines.length&&/^    (&&|\|\|)/.test(lines[i]);i++)x+='\n'+lines[i];return x;}
function context(){
 const c=vm.createContext({NativeSnapshot:parser,NativePresentation:presentation,nativeFactsCurrent:true,nativeSnapshotFailed:false,nativeOwner:true,
 nativePending:null,nativeQuitting:false,nativeActionRunning:false,nativeOutcomeUnknown:false,nativeBatchBusy:false,nativeStatisticsEligible:true,
 nativeStatisticsFact:{available:true,instanceId:'runtime',ownerRevision:7,enabled:false,since:0,revision:0},nativeStatisticsPending:null,nativeStatisticsError:'',_nativeStatisticsRead:null,
 _nativeStatisticsGeneration:0,_nativeOperationSerial:0,backendPath:'/fixture/backend.sh',
 nativeSnapshot:{instanceId:'runtime',revision:7,lastKnownActual:'disconnected',profiles:[],subscriptions:[],lastProfileId:'',desired:{connected:false,generation:0,mode:'rule'}},
 nativeObservation:{manualRecoveryRequired:false,availability:'observed',instanceId:'runtime',revision:7,lastKnownActual:'disconnected',desired:{connected:false,generation:0,mode:'rule'},facts:{ownedCoreRunning:false,visibleMihomoCount:0,ownedAuxiliaryMihomoCount:0,visibleTunCount:0,managedTunCount:0}},
 nativeStatisticsComponent:{createObject:(_,p)=>({...p,running:false})}});
 for(const name of ['nativeCanAct','nativeStatisticsCurrent','nativeCanSaveStatistics'])vm.runInContext('Object.defineProperty(this,"'+name+'",{get:function(){return '+expression(name)+'}})',c);
 for(const name of ['statisticsContextCurrent','readNativeStatistics','requestNativeStatistics','finishNativeStatistics']){const a=service.indexOf('  function '+name+'('),b=service.indexOf('\n  }',a)+4;assert(a>=0);vm.runInContext(service.slice(a,b),c);}
 c.root=c;return c;
}
let count=0;function test(name,f){try{f();count++}catch(e){e.message=name+': '+e.message;throw e;}}
test('exact decimal u64 survives without floating-point rounding and UTC is bounded',()=>{
 const value=parser.parseStatistics(frame(),fence);assert.equal(value.received,result.receivedBytes);assert.equal(value.sent,'9007199254740993');
 assert.equal(parser.statisticsBytes(value.received,'en'),'18,446,744,073,709,551,615');
 assert.equal(parser.statisticsBytes('1000','ru'),'1\u00a0000');
 for(const n of ['01','-1','1.0','18446744073709551616','1e6',9007199254740993])assert.equal(parser.statisticsBytes(n,'en'),'--');
 assert.equal(parser.statisticsSince(0),'');assert.equal(parser.statisticsSince(1),'1970-01-01 00:00 UTC');assert.equal(parser.statisticsSince(253402300799),'9999-12-31 23:59 UTC');assert.equal(parser.statisticsSince(253402300800),'');
});
test('malformed private extra fields and stale epoch do not become zeros',()=>{
 for(const mutate of [p=>p.revision++,p=>p.result.instanceId='other',p=>p.result.receivedBytes=1,p=>p.result.sentBytes='18446744073709551616',p=>p.result.path='private',p=>p.result.partial='true',p=>p.result.periodStartedAtUtc=0,p=>p.result.controlRevision=-1]){const p=JSON.parse(frame());mutate(p);assert.equal(parser.parseStatistics(JSON.stringify(p),fence),null);}
 const unavailable=parser.parseStatistics(frame({schemaVersion:1,scope:result.scope,availability:'unavailable',instanceId:'runtime'}),fence);assert.equal(unavailable.available,false);assert.equal(unavailable.received,undefined);
});
test('Off-only request keeps lifecycle pending independent and duplicate activation bounded',()=>{
 const c=context();assert(c.nativeCanSaveStatistics);assert(c.requestNativeStatistics('on',0));const p=c.nativeStatisticsPending;
 assert.equal(p.command[2],'native-statistics-configure');assert.equal(p.command[7],'on');assert.equal(c.nativePending,null);
 assert(!c.requestNativeStatistics('on',0));assert.equal(c.nativeStatisticsFact.enabled,false,'no optimistic enabled state');
 c.nativeSnapshot.desired.connected=true;assert(!c.nativeCanSaveStatistics);
});
test('late pre-action read cannot overwrite the acknowledged new state',()=>{
 const c=context();assert(c.readNativeStatistics(false));const read=c._nativeStatisticsRead;
 assert(c.requestNativeStatistics('on',0));const p=c.nativeStatisticsPending;
 assert(c.finishNativeStatistics(p,0,frame({...result,operationId:p.context.operationId,receivedBytes:'0',sentBytes:'0',controlRevision:1})));
 assert.equal(c.nativeStatisticsFact.revision,1);assert(!c.finishNativeStatistics(read,0,frame({...result,enabled:false,controlRevision:0})));
 assert.equal(c.nativeStatisticsFact.revision,1);
});
test('refusal with already-matching cached value cannot claim success or retry',()=>{
 const c=context();c.nativeStatisticsFact.enabled=true;assert(c.requestNativeStatistics('off',0));const p=c.nativeStatisticsPending;
 const rejected=JSON.stringify({api:'omavless.control',version:1,id:'s',ok:false,revision:7,error:{code:'conflict'}});
 assert(!c.finishNativeStatistics(p,1,rejected));assert.equal(c.nativeStatisticsFact,null);assert.equal(c.nativeStatisticsError,'error.conflict');assert.equal(c.nativeStatisticsPending,null);assert.equal(c._nativeOperationSerial,1);
});
test('unknown is local, preserves Disconnect admission, and requires explicit current-value check',()=>{
 const c=context();assert(c.requestNativeStatistics('on',0));assert(!c.finishNativeStatistics(c.nativeStatisticsPending,1,''));
 assert.equal(c.nativeStatisticsError,'stats.unconfirmed');assert.equal(c.nativeOutcomeUnknown,false);assert.equal(c.nativePending,null);assert(!c.nativeCanSaveStatistics);
 assert(c.readNativeStatistics(false));c.finishNativeStatistics(c._nativeStatisticsRead,0,frame());assert.equal(c.nativeStatisticsError,'stats.unconfirmed');
 assert(c.readNativeStatistics(true));c.finishNativeStatistics(c._nativeStatisticsRead,0,frame());assert.equal(c.nativeStatisticsError,'');assert.equal(c._nativeOperationSerial,1);
});
test('wrong reset period or restarted runtime cannot apply an old completion',()=>{
 const c=context();assert(!c.requestNativeStatistics('reset',99));assert(c.requestNativeStatistics('reset',0));const p=c.nativeStatisticsPending;c.nativeSnapshot.instanceId='new-runtime';
 assert(!c.finishNativeStatistics(p,0,frame({...result,operationId:p.context.operationId})));
 assert.equal(c.nativeStatisticsFact,null);
 assert(panel.includes('vless.nativeSnapshot.instanceId === root.statisticsResetInstance'));assert(panel.includes('vless.nativeStatisticsFact.since === root.statisticsResetSince'));
});
console.log('native statistics: '+count+' passed');
