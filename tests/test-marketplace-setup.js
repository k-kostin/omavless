// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict');
const fs = require('node:fs');
const state = require('../plugin/SetupState.js');
const i18n = require('../plugin/I18n.js');
const page = fs.readFileSync(__dirname + '/../plugin/SetupPage.qml', 'utf8');
const panel = fs.readFileSync(__dirname + '/../plugin/Panel.qml', 'utf8');
let count = 0;
function test(name, run) { try { run(); count++; } catch (e) { e.message = name + ': ' + e.message; throw e; } }
test('only public bounded status enums cross the process boundary', () => {
  for (const value of ['ready', 'needs_package', 'needs_activation', 'needs_attention', 'release_unavailable']) {
    assert.equal(state.parse(value + '\n', 0), value);
    assert.equal(state.parse(value, 1), 'needs_attention');
  }
  for (const value of ['', null, {}, 'secret://synthetic', 'x'.repeat(100), 'ready\nready', ' ready', 'ready\n\n'])
    assert.equal(state.parse(value, 0), 'needs_attention');
});
test('only explicit missing-package or legacy states offer setup', () => {
  for (const value of ['ready', 'needs_attention', 'release_unavailable', 'checking']) assert(!state.canInstall(value));
  assert(state.canInstall('needs_package')); assert(state.canInstall('needs_activation'));
});
test('page never uses missing backend, never installs on load or check', () => {
  assert(page.includes('Component.onCompleted: check()'));
  assert(page.includes('command: ["/bin/bash", setup.script, "status"]'));
  assert(page.includes('["omarchy", "launch", "terminal", "/bin/bash", setup.script, "install"'));
  assert(!/backendPath|execDetached|sudo|connect\(/.test(page));
  assert(page.includes('launching || terminalOpened'));
  assert(page.includes('setup.terminalOpened = false; setup.check()'));
});
test('setup has separate navigation and cannot expose legacy mutation shortcuts', () => {
  assert(panel.includes('readonly property bool bootstrapRequired: setupPage.state !== "ready"'));
  assert(panel.includes('if (root.bootstrapRequired) return setupPage.focusTargets'));
  assert(panel.includes('root.bootstrapRequired ? setupFlick'));
  assert(panel.includes('onReady: { vless.enterNativeReadOnly(); vless.refresh() }'));
  assert(panel.includes('if (root.bootstrapRequired) root.close()'));
  assert(panel.includes('onTextKey: function(t) {\n        if (root.bootstrapRequired) return'));
  assert(panel.includes('width: Math.max(0, setupFlick.width - root.scrollGutter)'));
});
test('launcher exit 71 cannot dismiss setup or reveal a competing normal page', () => {
  const vm = require('node:vm');
  const expression = panel.match(/readonly property bool bootstrapRequired: ([^\n]+)/)[1];
  for (const nativeOwner of [true, false]) for (const value of ['checking', 'needs_package', 'needs_activation', 'needs_attention', 'release_unavailable', 'ready']) {
    assert.equal(vm.runInNewContext(expression, {vless:{nativeOwner}, setupPage:{state:value}}), value !== 'ready');
  }
  assert(panel.includes('visible: !root.bootstrapRequired && vless.nativeOwner && root.page !== "diagnostics"'));
  assert(panel.includes('visible: !root.bootstrapRequired && vless.nativeOwner && (root.page === "main" || root.page === "subscription")'));
});
test('all first-use and unavailable states have bounded EN/RU plain text', () => {
  for (const locale of ['en', 'ru']) for (const key of ['title', 'checking', 'ready', 'needs_package',
    'needs_activation', 'needs_attention', 'release_unavailable', 'explanation', 'terminal', 'terminal_closed', 'install', 'prepare', 'check', 'guide', 'later']) {
    const text = i18n.translate('setup.' + key, locale, {});
    assert(text.length > 0 && text.length <= 512);
    assert(!/Missing translation|[<>]/.test(text));
  }
  assert(!page.includes('Text.AutoText')); assert(page.includes('PlainText'));
});
console.log(`${count} marketplace setup contracts passed`);
