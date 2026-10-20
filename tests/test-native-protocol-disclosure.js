// SPDX-License-Identifier: MIT
// Real component functions; inert focus/model witnesses. Actual Qt rendering
// and delegate lifetimes remain a separate affected-surface gate.
const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm'),path=require('node:path');
const source=fs.readFileSync(path.join(__dirname,'../plugin/ProtocolFilter.qml'),'utf8');
function context(){
  const c=vm.createContext({protocols:['wireguard','amneziawg'],_values:['','wireguard','amneziawg'],expanded:false,visible:true,
    disclosure:{visible:true,enabled:true,activeFocus:false},reset:{visible:false,activeFocus:false},
    Qt:{Key_Escape:27},I18n:{translate:()=> 'All'},locale:'en',events:[]});
  c.root=c;c.disclosure.forceActiveFocus=()=>c.events.push('header');
  c.searchFocusRequested=()=>c.events.push('search');c.controlKey=e=>c.events.push(e.key);
  c.items=[{activeFocus:false},{activeFocus:false},{activeFocus:false}];
  c.choices={count:3,itemAt:i=>c.items[i]};
  Object.defineProperty(c,'values',{get:()=>c._values});
  Object.defineProperty(c,'hasChoices',{get:()=>c.values.length>2});
  for(const name of ['updateChoices','label','collapse','tabTargets','handleKey']){
    const from=source.indexOf('  function '+name+'('),to=source.indexOf('\n  }',from)+4;
    assert(from>=0&&to>from);vm.runInContext(source.slice(from,to),c);
  }
  return c;
}
let count=0;function test(name,f){try{f();count++;}catch(e){e.message=name+': '+e.message;throw e;}}
test('collapsed and hidden children are excluded from traversal',()=>{
  const c=context();assert.deepEqual(Array.from(c.tabTargets()),[c.disclosure]);
  c.expanded=true;assert.equal(c.tabTargets().length,4);
  c.reset.visible=true;assert.equal(c.tabTargets().length,5);
  c.visible=false;assert.equal(c.tabTargets().length,0);
});
test('equal-set refresh preserves the model object, expansion and focus',()=>{
  const c=context();c.expanded=true;c.items[2].activeFocus=true;
  const model=c.values;c.protocols=['wireguard','amneziawg'];c.updateChoices();
  assert.equal(c.values,model);assert(c.expanded);assert.deepEqual(c.events,[]);
});
test('real option removal hands off focus before model replacement, without stealing unrelated focus',()=>{
  for(const origin of ['choice','header','reset','unrelated'])for(const next of [[],['wireguard'],['vless','trojan']]){
    const c=context();c.expanded=true;
    if(origin==='choice')c.items[2].activeFocus=true;
    if(origin==='header')c.disclosure.activeFocus=true;
    if(origin==='reset')c.reset.activeFocus=true;
    c.protocols=next;c.updateChoices();
    assert(!c.expanded);assert.equal(c.values.length,next.length>1?3:0);
    assert.deepEqual(c.events,origin==='unrelated'?[]:[next.length>1?'header':'search']);
  }
});
test('Escape collapses first; next Escape follows the parent route',()=>{
  const c=context();c.expanded=true;c.items[1].activeFocus=true;
  const first={key:27,accepted:false};c.handleKey(first);
  assert(first.accepted);assert(!c.expanded);assert.deepEqual(c.events,['header']);
  c.handleKey({key:27});assert.deepEqual(c.events,['header',27]);
});
test('component consumes only supplied protocols and owns no effect path',()=>{
  assert.match(source,/var next = protocols.length > 1 \? \[""\]\.concat\(protocols\) : \[\]/);
  assert(!/\bProcess\s*\{|\bFileView\s*\{|requestNative|setPreference/.test(source));
  assert.match(source,/Accessible.name:/);assert.match(source,/selectedLabel/);
});
test('the hosted isolated runner includes these component regressions exactly once',()=>{
  const runner=fs.readFileSync(path.join(__dirname,'run.sh'),'utf8');
  assert.equal((runner.match(/^\s*node "\$here\/test-native-protocol-disclosure\.js"$/gm)||[]).length,1);
});
console.log('native protocol disclosure: '+count+' passed');
