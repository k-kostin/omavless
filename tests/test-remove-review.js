// SPDX-License-Identifier: MIT
// Existing production QML-handler seam; no native/terminal process is run.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const panel = fs.readFileSync(__dirname + '/../plugin/Panel.qml', 'utf8');
let count = 0;
function test(name, run) { try { run(); count++; } catch (e) { e.message = name + ': ' + e.message; throw e; } }
function loadFunction(source, name, context) {
  const start = source.indexOf('  function ' + name + '(');
  assert(start >= 0, 'production handler ' + name);
  let end = source.indexOf('{', start) + 1, depth = 1, quote = null;
  for (; end < source.length && depth; end++) {
    const ch = source[end];
    if (quote) { if (ch === '\\') end++; else if (ch === quote) quote = null; }
    else if (ch === '"' || ch === "'") quote = ch;
    else if (ch === '{') depth++;
    else if (ch === '}') depth--;
  }
  assert.equal(depth, 0);
  vm.runInContext(source.slice(start, end), context);
}
test('Remove entry opens local review even while native/setup facts are unavailable', () => {
  for (const state of ['checking', 'needs_package', 'needs_activation', 'needs_attention', 'ready', 'stopped']) {
    const origin = {visible: true};
    const c = vm.createContext({opened: true, removalReturnFocus: null,
      setupPage: {state, busy: true, terminalOpened: true},
      vless: new Proxy({}, {get() { throw Error('navigation must not use native effects'); }}),
      removeReview: {visible: false, terminalOpened: true, openReview() { this.visible = true; }}});
    c.root = c;
    loadFunction(panel, 'openRemoval', c);
    c.openRemoval(origin);
    assert(c.removeReview.visible);
    assert.equal(c.removalReturnFocus, origin);
    assert(c.removeReview.terminalOpened, 'navigation cannot acknowledge a terminal');
    c.openRemoval({visible: true});
    assert.equal(c.removalReturnFocus, origin, 'a duplicate does not replace the origin');
    c.removeReview.visible = false; c.opened = false; c.openRemoval(null);
    assert(!c.removeReview.visible, 'closed panel cannot open a hidden action');
  }
});
test('review and Escape return are navigation, never terminal acknowledgement', () => {
  const source = fs.readFileSync(__dirname + '/../plugin/RemoveReview.qml', 'utf8');
  for (const terminalOpened of [false, true]) {
    const c = vm.createContext({visible: false, terminalOpened,
      removeScroll: {contentY: 123}, closeButton: {forceActiveFocus() { c.focusedClose = true; }},
      launch: {running: false}, Qt: {callLater: f => f()}});
    c.removal = c;
    for (const name of ['openReview', 'dismiss']) loadFunction(source, name, c);
    c.openReview();
    assert(c.visible && c.focusedClose, 'safe return action owns initial focus');
    assert.equal(c.removeScroll.contentY, 0);
    assert.equal(c.terminalOpened, terminalOpened);
    assert(!c.launch.running);
    c.dismiss();
    assert(!c.visible);
    assert.equal(c.terminalOpened, terminalOpened, 'hiding is not a terminal receipt');
  }
});
test('only an explicit unblocked review activation can submit the fixed terminal once', () => {
  const source = fs.readFileSync(__dirname + '/../plugin/RemoveReview.qml', 'utf8');
  for (let flags = 0; flags < 32; flags++) {
    const c = vm.createContext({visible: true, nativeEffectBlocked: !!(flags & 1),
      setupBusy: !!(flags & 2), setupTerminalOpened: !!(flags & 4),
      terminalOpened: !!(flags & 8), launching: !!(flags & 16), launchFailed: false,
      launch: {running: false}});
    c.removal = c;
    for (const name of ['blockReason', 'startRemoval']) loadFunction(source, name, c);
    assert.equal(c.startRemoval(), flags === 0, 'state flags ' + flags);
    assert.equal(c.launch.running, flags === 0);
    if (flags === 0) {
      assert(c.terminalOpened && c.launching);
      assert(!c.startRemoval(), 'repeat activation does not submit again');
    }
  }
  const c = vm.createContext({visible: false, nativeEffectBlocked: false, setupBusy: false,
    setupTerminalOpened: false, terminalOpened: false, launching: false, launchFailed: false,
    launch: {running: false}});
  c.removal = c;
  for (const name of ['blockReason', 'startRemoval']) loadFunction(source, name, c);
  assert(!c.startRemoval()); assert(!c.launch.running);
});
test('Back while a removal terminal is outstanding closes only the panel', () => {
  for (const terminalOpened of [false, true]) {
    const origin = {visible: true, forceActiveFocus() { c.returned = true; }};
    const c = vm.createContext({removalReturnFocus: origin, closed: false,
      close() { c.closed = true; }, syncNativeOnboarding() {},
      onboardingWizard: {visible: false}, keyCatcher: {forceActiveFocus() { c.fallback = true; }},
      removeReview: {visible: true, terminalOpened, dismiss() { this.visible = false; }},
      Qt: {callLater: f => f()}});
    c.root = c;
    loadFunction(panel, 'closeRemoval', c);
    c.closeRemoval();
    assert.equal(c.closed, terminalOpened);
    assert.equal(c.removeReview.terminalOpened, terminalOpened);
    if (!terminalOpened) assert(c.returned && !c.removeReview.visible);
  }
});
test('the same review is reachable from setup, every native wizard step and Settings', () => {
  const setup = fs.readFileSync(__dirname + '/../plugin/SetupPage.qml', 'utf8');
  const wizard = fs.readFileSync(__dirname + '/../plugin/OnboardingWizard.qml', 'utf8');
  assert(setup.includes('signal removeRequested(var control)'));
  assert(setup.includes('id: removeButton') && setup.includes('setup.removeRequested(removeButton)'));
  assert(setup.includes('reviewGuideButton, removeButton, closeButton]'));
  const begin = wizard.indexOf('id: onboardingRemoveButton');
  assert(begin >= 0);
  const button = wizard.slice(begin, wizard.indexOf('\n          }', begin));
  const visible = button.match(/visible: ([^\n]+)/)[1];
  for (const step of [1, 2, 3]) for (const nativeCoreState of ['ready', 'checking', 'missing', 'attention', 'terminal']) {
    assert(vm.runInNewContext(visible, {wizard: {nativeContext: true, step, nativeCoreState}}));
  }
  assert(wizard.includes('wizard.removeRequested(onboardingRemoveButton)'));
  assert(panel.includes('id: nativeRemoveSetting'));
  assert.equal((panel.match(/onRemoveRequested: function\(control\) \{ root.openRemoval\(control\) \}/g) || []).length, 2);
  assert(panel.includes('onAction: root.openRemoval(nativeRemoveSetting.focusTarget)'));
  assert(panel.includes('if (page === "settings") targets.push(nativeRemoveSetting.focusTarget)'));
});
test('terminal acknowledgement requires an outstanding submission and a settled launcher', () => {
  const source = fs.readFileSync(__dirname + '/../plugin/RemoveReview.qml', 'utf8');
  for (const code of [0, 1, 124]) {
    const c = vm.createContext({visible: true, terminalOpened: true, launching: true,
      launchFailed: false, launch: {running: true}, recheckRequested() { c.rechecked = true; }});
    c.removal = c;
    for (const name of ['finishLaunch', 'acknowledgeTerminalClosed', 'dismiss', 'openReview']) loadFunction(source, name, c);
    c.finishLaunch(code);
    assert(c.terminalOpened, 'exit ' + code + ' is not removal or prompt closure');
    assert.equal(c.launchFailed, code !== 0);
    assert(!c.acknowledgeTerminalClosed(), 'actual Process still owns its slot');
    c.launch.running = false;
    assert(c.acknowledgeTerminalClosed()); assert(c.rechecked && !c.terminalOpened);
    c.rechecked = false;
    assert(!c.acknowledgeTerminalClosed(), 'absence is not another closure receipt');
    assert(!c.rechecked);
  }
});
test('modal keys, focus and scroll remain local and skip disabled actions', () => {
  const source = fs.readFileSync(__dirname + '/../plugin/RemoveReview.qml', 'utf8');
  const c = vm.createContext({visible: true, terminalOpened: true,
    Qt: {Key_Escape: 1, Key_Tab: 2, Key_Backtab: 3, ShiftModifier: 4, callLater: f => f()},
    canceled() { c.canceledCount = (c.canceledCount || 0) + 1; },
    removeScroll: {contentItem: {}, contentY: 0, contentHeight: 800, height: 200}});
  c.removal = c;
  c.focusTargets = Array.from({length: 2}, (_, n) => ({activeFocus: n === 0,
    forceActiveFocus() { c.focusTargets.forEach(x => x.activeFocus = false); this.activeFocus = true; c.focused = n; }}));
  for (const name of ['navigate', 'reveal']) loadFunction(source, name, c);
  c.navigate({key: 2, modifiers: 0}); assert.equal(c.focused, 1);
  c.navigate({key: 2, modifiers: 0}); assert.equal(c.focused, 0);
  c.navigate({key: 3, modifiers: 0}); assert.equal(c.focused, 1);
  c.navigate({key: 1, modifiers: 0}); assert.equal(c.canceledCount, 1); assert(c.terminalOpened);
  c.reveal({activeFocus: true, height: 40, mapToItem: () => ({y: 550})});
  assert.equal(c.removeScroll.contentY, 390);
  c.visible = false;
  c.reveal({activeFocus: true, mapToItem() { throw Error('stale target must not be read'); }});
  assert(source.includes('.filter(function(control) { return control.visible && control.enabled })'));
});
test('fixed local terminal entry and native/setup fences have no arbitrary argument or outcome reset', () => {
  const source = fs.readFileSync(__dirname + '/../plugin/RemoveReview.qml', 'utf8');
  assert(source.includes('command: ["omarchy", "launch", "terminal", "/bin/bash", removal.script]'));
  assert(!/sudo|curl|wget|vless\.|clipboard|stdinEnabled|execDetached/.test(source));
  const binding = panel.slice(panel.indexOf('      RemoveReview {')).match(/nativeEffectBlocked: ([\s\S]*?)\n        setupBusy:/)[1];
  const flags = ['nativeActionRunning', 'nativeOutcomeUnknown', 'nativeQuitting', 'nativeImportBusy',
    'nativeBatchBusy', 'nativeBatchUnknown', 'nativeBatchRequestRunning', 'nativeEditorRunning'];
  const blank = () => Object.assign(Object.fromEntries(flags.map(k => [k, false])),
    {nativePending: null, nativeFileExportProcess: null, nativeQrExportProcess: null});
  assert(!vm.runInNewContext(binding, {vless: blank()}));
  for (const flag of flags.concat(['nativePending', 'nativeFileExportProcess', 'nativeQrExportProcess'])) {
    const vless = blank(); vless[flag] = flags.includes(flag) ? true : {};
    assert(vm.runInNewContext(binding, {vless}), flag);
  }
  assert(panel.includes('if (removeReview.terminalOpened) { removeReview.openReview(); return }'));
  const setupBinding = panel.slice(panel.indexOf('        SetupPage {')).match(/nativeEffectBlocked: ([\s\S]*?)\n          onReady:/)[1];
  assert(vm.runInNewContext(setupBinding, {removeReview: {terminalOpened: true}, vless: blank()}));
  const I18n = require('../plugin/I18n.js');
  for (const [key, entry] of Object.entries(I18n.CATALOG).filter(([k]) => k.startsWith('remove.'))) {
    for (const locale of ['en', 'ru']) {
      assert(entry[locale] && I18n.translate(key, locale).length <= I18n.MAX_TEXT_LENGTH);
      assert(!I18n.translate(key, locale).includes('Missing translation'));
    }
  }
});
test('keyboard scrolling exposes the whole review without changing focus or submitting', () => {
  const source = fs.readFileSync(__dirname + '/../plugin/RemoveReview.qml', 'utf8');
  const c = vm.createContext({visible: true, Qt: {Key_PageUp: 10, Key_PageDown: 11, Key_Home: 12, Key_End: 13},
    removeScroll: {contentY: 500, height: 200, contentHeight: 800}, focusTargets: []});
  loadFunction(source, 'navigate', c);
  for (const [key, expected] of [[12, 0], [11, 200], [13, 600], [11, 600], [10, 400], [12, 0], [10, 0]]) {
    const event = {key, modifiers: 0, accepted: false}; c.navigate(event);
    assert.equal(c.removeScroll.contentY, expected); assert(event.accepted);
  }
  c.removeScroll.contentHeight = 50;
  c.navigate({key: 13, modifiers: 0}); assert.equal(c.removeScroll.contentY, 0);
});
console.log('remove review: ' + count + ' controls PASS');
