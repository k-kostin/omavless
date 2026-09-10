// SPDX-License-Identifier: MIT
// Execute the actual retained QML oracle against the corpus also consumed by
// Rust sysfs-reader tests. Synthetic totals only; no real interface inspection.
const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict'),path=require('node:path');
const source=fs.readFileSync(path.join(__dirname,'../plugin/Service.qml'),'utf8');
const cases=JSON.parse(fs.readFileSync(path.join(__dirname,'traffic-cases.json'),'utf8'));
const context=vm.createContext({traffic:{},primaryDevice:'synthetic',rxHistory:[],txHistory:[],historyMaxPoints:60,Date:{now:()=>0}});
const start=source.indexOf('  function applyTraffic('),end=source.indexOf('\n  }',start)+4;
assert(start>=0);vm.runInContext(source.slice(start,end),context);
for(const c of cases){context.Date.now=()=>c.at;context.applyTraffic(`synthetic ${c.rx} ${c.tx}\n`);
 for(const key of ['rx','tx','rated','rxRate','txRate'])assert.equal(context.traffic.synthetic[key],c[key],c.id+': '+key);
}
context.applyTraffic('');assert.equal(Object.keys(context.traffic).length,0);
console.log('traffic reference: '+cases.length+' corpus cases and missing-interface reset PASS');
