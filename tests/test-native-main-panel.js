// SPDX-License-Identifier: MIT
// Actual presentation functions, synthetic metadata only.
const assert=require('node:assert/strict'), fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const source=fs.readFileSync(path.join(__dirname,'../plugin/Panel.qml'),'utf8');
const presentation=vm.createContext({});
vm.runInContext(fs.readFileSync(path.join(__dirname,'../plugin/NativePresentation.js'),'utf8'),presentation);
const standalone={id:'local',name:'Local',protocol:'vless',subscriptionId:'',favorite:false,missing:false};
const managed={id:'managed',name:'Match',protocol:'vless',subscriptionId:'sub',favorite:true,missing:false};
function context(){
  const calls=[];
  const c=vm.createContext({NativePresentation:presentation,profileFilter:'',nativeExpanded:{},nativeSelectedProfile:'local',
    nativeView:{state:'disconnected',connected:false,mode:'rule',lastProfileId:'local',profiles:[standalone,managed],subscriptions:[{id:'sub',name:'Synthetic'}]},
    vless:{nativeCanAct:true,nativeOwner:true,requestNativeAction:(...args)=>calls.push(args)},page:'main',nativeFlick:{contentY:90}});
  c.root=c;c.calls=calls;
  for(const name of ['nativeRecord','buildNativeRows','toggleNativeSubscription','nativeToggleConnection','openSettings']){
    const start=source.indexOf('  function '+name+'('),end=source.indexOf('\n  }',start)+4;
    assert(start>=0&&end>start);vm.runInContext(source.slice(start,end),c);
  }return c;
}
let count=0;function test(name,f){try{f();count++;}catch(e){e.message=name+': '+e.message;throw e;}}
test('standalone and collapsed subscriptions preserve grouping',()=>{
  const c=context();let rows=c.buildNativeRows();assert.equal(rows.length,2);assert.equal(rows[0].profile.id,'local');assert.equal(rows[1].subscription.id,'sub');
  c.toggleNativeSubscription('sub');rows=c.buildNativeRows();assert.equal(rows.length,3);assert.equal(rows[2].profile.id,'managed');
  c.toggleNativeSubscription('sub');assert.equal(c.buildNativeRows().length,2);
});
test('search expands matching subscription without mutating stored expansion',()=>{
  const c=context();c.profileFilter='MATCH';const rows=c.buildNativeRows();assert.equal(rows.length,2);assert.equal(rows[0].expanded,true);assert.equal(rows[1].profile.id,'managed');assert.equal(c.nativeExpanded.sub,undefined);
  c.profileFilter='absent';assert.equal(c.buildNativeRows().length,0);
});
test('power toggles only verified connected or disconnected state',()=>{
  const c=context();c.nativeToggleConnection();assert.deepEqual(c.calls.pop(),['connect','local','rule']);
  c.nativeView.connected=true;c.nativeView.state='connected';c.nativeToggleConnection();assert.deepEqual(c.calls.pop(),['disconnect','','']);
  c.nativeView.connected=false;c.nativeView.state='unavailable';c.nativeToggleConnection();assert.equal(c.calls.length,0);
  c.nativeView.state='disconnected';c.vless.nativeCanAct=false;c.nativeToggleConnection();assert.equal(c.calls.length,0);
});
test('settings changes only page and scroll, locale reuses existing persistence',()=>{
  const c=context();assert(c.openSettings());assert.equal(c.page,'settings');assert.equal(c.nativeFlick.contentY,0);assert.equal(c.calls.length,0);
  assert.match(source,/id: nativeLanguageRow[^\n]*onAction: root.cycleLanguageSetting\(\)/);
});
test('familiar header, truthful states, same-height focusable controls and no outer power box',()=>{
  const section=source.slice(source.indexOf('id: nativeHeader'),source.indexOf('id: nativeSettingsBack'));
  assert.match(section,/PanelHero/);assert.match(section,/title: "OmaVLESS"/);assert.match(section,/native.state./);
  assert.equal((section.match(/size: nativeHeader.controlHeight/g)||[]).length,2);
  assert.match(section,/height: nativeHeader.controlHeight/);assert.match(section,/cursorRing: false/);assert.match(section,/Keys.onReturnPressed/);
  assert(!source.includes('vless.nativeOwner ? "?"'));
});
test('private names stay plaintext and selected row exposes managed-safe actions',()=>{
  const section=source.slice(source.indexOf('id: nativeProfiles'),source.indexOf('AdvancedDiagnostics {'));
  assert.match(section,/subscription.name; textFormat: Text.PlainText/);
  assert.match(section,/profile.name : ""; textFormat: Text.PlainText/);
  for(const id of ['rowRename','rowPin','rowDelete','rowQr','rowEdit'])assert(section.includes('id: '+id));
  assert.equal(context().nativeRecord(managed).managed,true);assert.equal(context().nativeRecord(standalone).managed,false);
  assert.match(section,/visible: nativeRow.selected/);
});
console.log('native main panel: '+count+' passed');
