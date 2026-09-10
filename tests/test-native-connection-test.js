// SPDX-License-Identifier: MIT
const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const p=vm.createContext({});vm.runInContext(fs.readFileSync(__dirname+'/../plugin/NativeSnapshot.js','utf8'),p);
const context={instanceId:'instance',revision:7,generation:1};
const frame={api:'omavless.control',version:1,id:'test',ok:true,revision:7,result:{schemaVersion:1,scope:'current_route_https',https:true,observedIp:'203.0.113.7',elapsedMs:50,code:'ok',instanceId:'instance'}};
const parse=(f,c=context)=>p.connectionTest(JSON.stringify(f),c);
assert.equal(parse(frame).https,true);assert.equal(parse(frame).elapsedMs,50);
for(const change of [f=>f.result.observedIp='secret-token',f=>f.result.raw='secret-token',f=>f.result.scope='tun_verified',f=>f.result.elapsedMs=3001,f=>f.revision++,f=>f.result.instanceId='other',f=>f.result.https=false]){const f=structuredClone(frame);change(f);assert.equal(parse(f),null);}
const failed=structuredClone(frame);Object.assign(failed.result,{https:false,observedIp:null,code:'request_failed'});assert.equal(parse(failed).https,false);
assert.equal(p.connectionTest('secret-token',context),null);assert.equal(p.connectionTest(' '.repeat(2049),context),null);
const source=fs.readFileSync(__dirname+'/../plugin/Service.qml','utf8');
// Execute the retained production Python-era QML parser as the reference for
// well-formed provider responses; Rust deliberately rejects malformed IPv6.
const legacy=vm.createContext({});
const oldStart=source.indexOf('  function parsePublicIp('),oldEnd=source.indexOf('\n  }',oldStart)+4;
vm.runInContext(source.slice(oldStart,oldEnd),legacy);
for(const raw of ['203.0.113.7\n','2001:db8::7\n']) assert(legacy.parsePublicIp(raw));
for(const raw of ['secret-token','999.1.1.1','<b>1.2.3.4</b>']) assert.equal(legacy.parsePublicIp(raw),'');
assert(source.includes('https://checkip.amazonaws.com https://api.ipify.org'));
const c=vm.createContext({NativeSnapshot:p,panelVisible:true,nativeCanAct:true,nativeSnapshot:{instanceId:'instance',revision:7},nativeTestGeneration:1,nativeTestResult:null,nativeTestStatus:'loading'});
for(const name of ['clearNativeTest','finishNativeConnectionTest']){const start=source.indexOf('  function '+name+'('),end=source.indexOf('\n  }',start)+4;vm.runInContext(source.slice(start,end),c);}
c.finishNativeConnectionTest(context,0,JSON.stringify(frame));assert.equal(c.nativeTestStatus,'ok');
c.clearNativeTest();c.finishNativeConnectionTest(context,0,JSON.stringify(frame));assert.equal(c.nativeTestResult,null);assert.equal(c.nativeTestStatus,'');
c.nativeTestGeneration=1;c.panelVisible=false;c.finishNativeConnectionTest(context,0,JSON.stringify(frame));assert.equal(c.nativeTestResult,null);
assert(source.includes('property Timer watchdog: Timer'));assert(source.includes('nativeSnapshot.revision !== nativeTestFence.revision'));
const panel=fs.readFileSync(__dirname+'/../plugin/Panel.qml','utf8');assert(panel.includes('targets.push(nativeTestButton)'));assert(panel.includes('vless.showExitIp'));assert(panel.includes('root.textFor("native.test.scope")'));
assert(panel.includes('targets.push(nativeExitIpSetting.focusTarget)'));
const privacyRow=panel.slice(panel.indexOf('id: nativeExitIpSetting'),panel.indexOf('id: nativeExitIpSetting')+650);
assert(privacyRow.includes('root.setWidgetSetting("showExitIp", !vless.showExitIp, true)'));
assert(!privacyRow.includes('requestNativeAction'));
console.log('native connection test: parser/fences/privacy/UI PASS');
