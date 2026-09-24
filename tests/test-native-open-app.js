// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync(__dirname + '/../plugin/Service.qml', 'utf8');
const panel = fs.readFileSync(__dirname + '/../plugin/Panel.qml', 'utf8');
let count = 0;
function test(name, run) { try { run(); count++; } catch (error) { error.message = name + ': ' + error.message; throw error; } }
function context() {
  const c = vm.createContext({panelVisible:true,nativeAppAvailable:false,nativeAppChecking:false,
    nativeAppOpening:false,nativeAppLaunchFailed:false,nativeAppCheck:{},nativeAppLauncher:{}});
  for (const name of ['refreshNativeAppAvailability','finishNativeAppAvailability','openNativeApp']) {
    const start = source.indexOf('  function ' + name + '(');
    const end = source.indexOf('\n  }', start) + 4;
    assert(start >= 0); vm.runInContext(source.slice(start, end), c);
  }
  return c;
}
test('feature probe admits exact bounded token, not version guesses or raw errors', () => {
  for (const output of ['omavless.tui.v1','omavless.tui.v1\n']) {
    const c = context(); assert(c.refreshNativeAppAvailability());
    assert(c.nativeAppChecking); assert(c.nativeAppCheck.running);
    assert(c.finishNativeAppAvailability(0, output)); assert(!c.nativeAppChecking);
  }
  for (const output of ['',null,'omavless.tui.v2','private-secret','omavless.tui.v1\nextra',' '.repeat(65)+'omavless.tui.v1']) {
    const c = context(); assert(!c.finishNativeAppAvailability(0, output)); assert(!c.openNativeApp());
  }
  const c = context(); assert(!c.finishNativeAppAvailability(1,'omavless.tui.v1'));
});
test('probe avoids duplicate reads and clears previous availability', () => {
  const c = context(); c.nativeAppAvailable = true; assert(c.refreshNativeAppAvailability());
  assert(!c.nativeAppAvailable); assert(!c.refreshNativeAppAvailability());
  c.nativeAppChecking = false; c.panelVisible = false; assert(!c.refreshNativeAppAvailability());
});
test('open is explicit single-flight and independent of runtime health', () => {
  const c = context(); assert(!c.openNativeApp()); c.nativeAppAvailable = true;
  c.nativeAppLaunchFailed = true; assert(c.openNativeApp()); assert(c.nativeAppOpening);
  assert(c.nativeAppLauncher.running); assert(!c.nativeAppLaunchFailed); assert(!c.openNativeApp());
  for (const [field,value] of [['panelVisible',false],['nativeAppChecking',true],['nativeAppOpening',true]]) {
    const blocked = context(); blocked.nativeAppAvailable = true; blocked[field] = value;
    assert(!blocked.openNativeApp()); assert(!blocked.nativeAppLauncher.running);
  }
});
test('literal Omarchy launch or focus uses stable app id and never lifecycle IPC', () => {
  const start = source.indexOf('  property bool nativeAppAvailable');
  const end = source.indexOf('  property var nativeDesktopCapabilities', start);
  const block = source.slice(start, end);
  assert(block.includes('command: ["omavless", "tui", "--available"]'));
  assert(block.includes('command: ["omarchy", "launch", "or", "focus", "tui", "--app-id=org.omarchy.omavless", "omavless", "tui"]'));
  assert(block.includes('interval: 3000')); assert(block.includes('nativeAppLaunchCooldown.restart()'));
  assert(block.includes('running: root.nativeAppChecking'));
  assert(block.includes('interval: 20000'));
  assert(block.includes('root.nativeAppLaunchFailed = timedOut || code !== 0'));
  for (const forbidden of ['systemctl','sudo','pkexec','backendPath','nativeSnapshot','requestNativeAction','console.']) assert(!block.includes(forbidden));
  assert(panel.includes('nativeOpenAppRow.focusTarget'));
  assert(panel.includes('vless.nativeAppAvailable ? "native.app.open" : "common.refresh"'));
});
test('English and Russian copy explain lifetime and package compatibility', () => {
  const i18n = require('../plugin/I18n.js');
  for (const key of ['title','open','scope','unavailable','failed']) for (const locale of ['en','ru']) {
    const text = i18n.translate('native.app.' + key, locale);
    assert(text && text.length <= 512 && !text.includes('Missing translation'));
  }
});
test('normal native package includes TUI and CI verifies feature without runtime', () => {
  const manifest = fs.readFileSync(__dirname + '/../crates/omavless-runtime/Cargo.toml','utf8');
  assert(manifest.includes('default = ["tui"]'));
  const build = fs.readFileSync(__dirname + '/../packaging/release/build-native-ci.sh','utf8');
  assert(build.includes('tui --available) == omavless.tui.v1'));
});
console.log(`${count} Open app launcher tests passed`);
