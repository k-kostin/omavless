// SPDX-License-Identifier: MIT
const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const source=fs.readFileSync(__dirname+'/../plugin/Panel.qml','utf8');
const calls=[];
const c=vm.createContext({nativeSelectedProfile:'selected',nativeView:{connected:false,state:'disconnected',lastProfileId:'last',mode:'rule'},vless:{nativeOwner:true,nativeCanAct:true,requestNativeAction:(...args)=>{calls.push(args);return true},toggle:()=>{throw Error('legacy toggle')},disconnectAll:()=>{throw Error('legacy disconnect')}}});
c.root=c;
function load(name,indent){const start=source.indexOf(indent+'function '+name+'('),end=source.indexOf('\n'+indent+'}',start)+indent.length+2;assert(start>=0);vm.runInContext(source.slice(start,end).replace(/\): string/g,')'),c);}
load('nativeToggleConnection','  ');load('toggle','    ');load('down','    ');
assert.equal(c.toggle(),'ok');assert.deepEqual(calls.pop(),['connect','selected','rule']);
c.nativeSelectedProfile='';assert.equal(c.toggle(),'ok');assert.deepEqual(calls.pop(),['connect','last','rule']);
c.nativeView.connected=true;assert.equal(c.toggle(),'ok');assert.deepEqual(calls.pop(),['disconnect','','']);
c.nativeView.connected=false;c.nativeView.state='unavailable';assert.equal(c.toggle(),'error: native action unavailable');assert.equal(calls.length,0);
assert.equal(c.down(),'ok');assert.deepEqual(calls.pop(),['disconnect','','']);
c.vless.nativeCanAct=false;assert.equal(c.toggle(),'error: native action unavailable');assert.equal(calls.length,0);
c.vless.requestNativeAction=()=>false;assert.equal(c.down(),'error: native action unavailable');
c.vless.nativeOwner=false;c.vless.toggle=()=>true;c.vless.disconnectAll=()=>true;
assert.equal(c.toggle(),'ok');assert.equal(c.down(),'ok');
console.log('native IPC controls: 8 checks passed');

const service=fs.readFileSync(__dirname+'/../plugin/Service.qml','utf8');
const s=vm.createContext({nativeOwner:true,nativeCanAct:true,nativeSnapshot:{profiles:[
  {id:'record-a',name:'Example',favorite:true,subscriptionId:''},
  {id:'record-b',name:'Duplicate',favorite:false,subscriptionId:'subscription-a'},
  {id:'record-c',name:'Duplicate',favorite:false,subscriptionId:'subscription-a'}]},
  profiles:[],findByUuid:()=>{throw Error('legacy lookup')},findByName:()=>{throw Error('legacy lookup')}});
for(const name of ['resolveTarget','countByName']) {
  const start=service.indexOf('  function '+name+'('),end=service.indexOf('\n  }',start)+4;
  assert(start>=0);vm.runInContext(service.slice(start,end),s);
}
assert.equal(s.resolveTarget('record-a').profile.name,'Example');
assert.equal(s.resolveTarget('Example').profile.uuid,'record-a');
assert.equal(s.resolveTarget('record-b').profile.managed,true);
assert.equal(s.resolveTarget('Duplicate').profile,null);
assert.equal(s.countByName('Duplicate'),2);
const privateTarget='https://private.invalid/password=not-a-real-secret';
assert.equal(s.resolveTarget(privateTarget).error,'no such profile');
assert(!JSON.stringify(s.resolveTarget(privateTarget)).includes(privateTarget));
assert.equal(s.resolveTarget('x'.repeat(161)).profile,null);
s.nativeCanAct=false;assert.equal(s.resolveTarget('record-a').profile,null);
s.nativeCanAct=true;s.nativeSnapshot=null;assert.equal(s.resolveTarget('record-a').profile,null);
s.nativeOwner=false;s.findByUuid=value=>value==='legacy'?{uuid:value}:null;
assert.equal(s.resolveTarget('legacy').profile.uuid,'legacy');
console.log('native IPC profile resolution: 10 checks passed');
