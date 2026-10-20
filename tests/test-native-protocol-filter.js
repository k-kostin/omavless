// SPDX-License-Identifier: MIT
// Presentation-only selection: synthetic inventory, no runtime or provider IO.
const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm'),path=require('node:path');
const p=vm.createContext({});
vm.runInContext(fs.readFileSync(path.join(__dirname,'../plugin/NativePresentation.js'),'utf8'),p);
const values=['vless','trojan','hysteria2','tuic','wireguard','amneziawg'];
const inventory=values.map((protocol,i)=>({id:'p'+i,name:i%2?'Other':'Тест',protocol,favorite:i===4,subscriptionId:i%2?'sub':''}));
let count=0;function test(name,f){try{f();count++;}catch(e){e.message=name+': '+e.message;throw e;}}
test('each canonical protocol is exact, All retains every protocol and source order',()=>{
  const before=JSON.stringify(inventory);
  for(const protocol of values) {
    assert.deepEqual(Array.from(p.filtered(inventory,'',protocol),x=>x.protocol),[protocol]);
  }
  assert.equal(p.filtered(inventory,'','').length,6);
  assert.equal(JSON.stringify(inventory),before);
});
test('name and protocol filters intersect without changing names or IDs',()=>{
  assert.equal(p.filtered(inventory,'тЕСТ','wireguard')[0].id,'p4');
  assert.equal(p.filtered(inventory,'Other','wireguard').length,0);
  assert.equal(p.filtered(inventory,'Тест','amneziawg').length,0);
  assert.equal(p.filtered(inventory,'Other','amneziawg')[0].name,'Other');
});
test('wireguard never matches amneziawg or unrecognized protocol tokens',()=>{
  const rows=inventory.concat([{id:'unknown',name:'WireGuard',protocol:'wireguard-extra',favorite:false}]);
  assert.deepEqual(Array.from(p.filtered(rows,'','wireguard'),x=>x.id),['p4']);
});
console.log('native protocol filter: '+count+' passed');
