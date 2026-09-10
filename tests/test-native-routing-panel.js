// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync(__dirname + '/../plugin/Panel.qml', 'utf8');
const calls = [];
const c = vm.createContext({
  vless: {nativeOwner:true, loadCustomRules:()=>calls.push('load'), useRoutingPreset:(...args)=>calls.push(args)},
  routingToolsPrompt:{openTools:()=>calls.push('open'),dismiss:()=>calls.push('dismiss')},
  routingPresetPrompt:{dismiss:()=>calls.push('preset-dismiss')},
  Qt:{callLater:()=>{}},keyCatcher:{forceActiveFocus:()=>{}}
});
c.root=c;
for (const name of ['openRoutingTools','closeRoutingTools','applyFirstRoutingPreset']) {
  const start=source.indexOf('  function '+name+'('), end=source.indexOf('\n  }',start)+4;
  assert(start>=0&&end>start);
  vm.runInContext(source.slice(start,end),c);
}
c.openRoutingTools();
assert.deepEqual(calls.splice(0),['open','load']);
c.closeRoutingTools();
assert.deepEqual(calls.splice(0),['dismiss']);
c.applyFirstRoutingPreset('china-cn-direct');
assert.deepEqual(calls.splice(0),['preset-dismiss',['china-cn-direct',true]]);
c.vless.nativeOwner=false;
c.applyFirstRoutingPreset('roscomvpn-default');
assert.deepEqual(calls.splice(0),['preset-dismiss',['roscomvpn-default',false]]);
assert.match(source,/nativeRoutingToolsVisible: root.opened && routingToolsPrompt.visible/);
assert.match(source,/refreshAvailable: !vless.nativeOwner && vless.routing.ruleUpdateAvailable/);
assert.match(source,/nativeRoutingPresetSetting.focusTarget, nativeRoutingToolsSetting.focusTarget/);
assert.match(source,/errorText: vless.nativeOwner.*root.textFor\(vless.nativeRoutingErrorCode\)/);
assert.match(source,/accepted: selectedPreset !== "" && !vless.busy && \(!vless.nativeOwner \|\| vless.nativeCanAct\)/);
console.log('native routing panel: 9 checks passed');
