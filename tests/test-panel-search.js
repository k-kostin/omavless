// SPDX-License-Identifier: MIT
// Exercise the production QML JavaScript model, without claiming rendered focus.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const source = fs.readFileSync(path.join(__dirname, '../plugin/Panel.qml'), 'utf8');
function extract(name) {
  const start = source.indexOf('  function ' + name + '(');
  const end = source.indexOf('\n  }', start) + 4;
  assert(start >= 0 && end > start, 'Production function missing: ' + name);
  return source.slice(start, end);
}
let supported = true;
const pending = [];
let searchFocus = 0, panelFocus = 0;
const context = {
  profileFilter: '', profileSearchRequested: false, expandedSubscriptions: {},
  vless: {profiles: [], subscriptions: [], supports: () => supported},
  Qt: {callLater: f => pending.push(f)},
  profileSearch: {text: '', forceActiveFocus: () => searchFocus++},
  keyCatcher: {forceActiveFocus: () => panelFocus++},
};
vm.createContext(context);
vm.runInContext(['profileSearchVisible', 'openProfileSearch', 'dismissProfileSearch',
  'profileMatches', 'buildProfileRows'].map(extract).join('\n'), context);
for (const count of [0, 1, 7, 8]) {
  context.vless.profiles = Array.from({length: count}, (_, i) => ({
    name: i === 0 ? 'match' : 'other' + i, managed: false,
    active: false, favorite: false,
  }));
  context.profileFilter = '';
  context.profileSearchRequested = false;
  assert.equal(context.profileSearchVisible(), count >= 8);
  context.openProfileSearch();
  assert.equal(context.profileSearchVisible(), true);
  assert.equal(pending.length, 1);
  pending.shift()();
  context.dismissProfileSearch();
  assert.equal(context.profileSearchVisible(), count >= 8);
}
assert.equal(searchFocus, 4);
assert.equal(panelFocus, 4);
context.profileFilter = 'match';
context.profileSearch.text = 'match';
assert.equal(context.buildProfileRows().length, 1);
context.vless.profiles.shift();
assert.equal(context.vless.profiles.length, 7);
assert.equal(context.buildProfileRows().length, 0);
assert.equal(context.profileSearchVisible(), true, 'Active filter remains editable');
// Close/reopen resets only explicit discovery, not the surviving query.
context.profileSearchRequested = false;
assert.equal(context.profileSearchVisible(), true);
context.dismissProfileSearch();
assert.equal(context.profileSearch.text, '');
assert.equal(context.buildProfileRows().length, 7);
assert.equal(context.profileSearchVisible(), false);
assert.equal(panelFocus, 5, 'Focus returns when clearing hides the field');
context.vless.profiles = [];
context.profileFilter = 'match';
assert.equal(context.profileSearchVisible(), true, 'Empty store must allow clearing');
supported = false;
context.profileSearchRequested = false;
assert.equal(context.profileSearchVisible(), false);
context.openProfileSearch();
assert.equal(context.profileSearchRequested, false);
assert.equal(pending.length, 0);
assert(source.includes('visible: root.profileSearchVisible()'));
assert(source.includes('else if (t === "/") root.openProfileSearch()'));
assert(source.includes('root.dismissProfileSearch()'));
console.log('Panel search behavior: PASS (production functions; rendering not tested)');
