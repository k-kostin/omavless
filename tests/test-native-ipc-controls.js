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
