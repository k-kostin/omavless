// SPDX-License-Identifier: MIT
// Production function controls; no native/desktop/clipboard process is run.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const I18n = require('../plugin/I18n.js');
const source = fs.readFileSync(__dirname + '/../plugin/HelpPrompt.qml', 'utf8');
const panel = fs.readFileSync(__dirname + '/../plugin/Panel.qml', 'utf8');
let count = 0;
function test(name, run) { try { run(); count++; } catch (e) { e.message = name + ': ' + e.message; throw e; } }
function loadFunction(code, name, context) {
  const start = code.indexOf('  function ' + name + '(');
  assert(start >= 0, name);
  let end = code.indexOf('{', start) + 1, depth = 1, quote = null;
  for (; end < code.length && depth; end++) {
    const ch = code[end];
    if (quote) { if (ch === '\\') end++; else if (ch === quote) quote = null; }
    else if (ch === '"' || ch === "'") quote = ch;
    else if (ch === '{') depth++;
    else if (ch === '}') depth--;
  }
  assert.equal(depth, 0, 'complete function ' + name);
  vm.runInContext(code.slice(start, end), context);
}
function context(locale = 'en') {
  const c = vm.createContext({locale, I18n, visible: false, guideSection: 'help', copyState: '', pendingText: '', generation: 0, copyGeneration: 0,
    clipboardWriter: {running: false, stdinEnabled: false}, helpScroll: {contentY: 90},
    requestText: {deselect() {}}, copyButton: {forceActiveFocus() { c.focusedCopy = true; }},
    Qt: {callLater: f => f(), Key_Escape: 1, Key_Tab: 2, Key_Backtab: 3, ShiftModifier: 4},
    canceled() { c.closed = true; }, guideRequested(section) { c.guide = section; },
    issueRequested() { c.issue = true; }});
  c.help = c;
  for (const name of ['tr', 'openHelp', 'dismiss', 'copyRequest', 'finishCopy', 'openGuide', 'openIssue', 'navigate']) loadFunction(source, name, c);
  c.agentPrompt = I18n.translate('help.agent_prompt', locale);
  return c;
}
test('both prompts are short public text, with the problem last and no placeholder', () => {
  for (const locale of ['en', 'ru']) {
    const prompt = I18n.translate('help.agent_prompt', locale);
    assert(prompt.includes('~/.config/omarchy/plugins/kdk.omavless/docs/user/USER_AGENT_GUIDE.md'));
    assert(prompt.includes(locale === 'en' ? 'AGENTS.md in the plugin directory if present' : 'AGENTS.md в папке плагина, если он есть'));
    assert(prompt.endsWith(locale === 'en' ? 'Problem:\n\n' : 'Проблема:\n\n'));
    assert(prompt.length < I18n.MAX_TEXT_LENGTH);
    assert(!/\[|\]|https?:\/\/|vless:\/\/|\b(?:sudo|pacman|UPGRADE|INSTALL|diagnostic)\b/i.test(prompt));
    for (const [key, entry] of Object.entries(I18n.CATALOG).filter(([key]) => key.startsWith('help.'))) {
      assert(entry.en && entry.ru, key);
      assert(!I18n.translate(key, locale).startsWith('Missing translation:'), key);
    }
  }
});
test('opening help is local navigation without runtime, copy, browser or wizard mutation', () => {
  for (const locale of ['en', 'ru']) for (const section of ['install', 'updates', 'help', '../bad', '']) {
    const c = context(locale);
    c.openHelp(section);
    assert(c.visible && c.focusedCopy);
    assert.equal(c.guideSection, ['install', 'updates', 'help'].includes(section) ? section : 'help');
    assert.equal(c.helpScroll.contentY, 0);
    assert.equal(c.copyState, ''); assert(!c.clipboardWriter.running);
    assert.equal(c.guide, undefined); assert.equal(c.issue, undefined);
    c.dismiss(); assert(!c.visible);
  }
});
test('copy snapshots only the displayed prompt on explicit action and waits for its result', () => {
  for (const locale of ['en', 'ru']) {
    const c = context(locale);
    assert(!c.copyRequest()); assert(!c.clipboardWriter.running);
    c.openHelp('help'); assert(c.copyRequest());
    assert.equal(c.pendingText, c.agentPrompt);
    assert(c.clipboardWriter.running && c.clipboardWriter.stdinEnabled);
    assert.equal(c.copyState, 'copying');
    assert(!c.copyRequest(), 'no overlapping clipboard writer');
    c.finishCopy(1); assert.equal(c.copyState, 'copy_failed');
    c.finishCopy(124); assert.equal(c.copyState, 'copy_failed');
    c.finishCopy(0); assert.equal(c.copyState, 'copied');
  }
  assert(source.includes('command: ["/usr/bin/timeout", "5s", "wl-copy"]'));
  assert(source.includes('write(help.pendingText)'));
  assert(source.includes('readOnly: true') && source.includes('textFormat: TextEdit.PlainText'));
  assert(source.includes('selectByMouse: true') && source.includes('activeFocusOnTab: true'));
  assert(!/vless\.|backendPath|clipboard-read|wl-paste|execDetached|sudo|FileView/.test(source));
});
test('guide and issue are independent explicit navigation, not copy prerequisites', () => {
  const c = context(); c.openHelp('updates');
  c.openGuide(); assert.equal(c.guide, 'updates');
  c.openIssue(); assert(c.issue); assert(!c.clipboardWriter.running);
  assert.equal(c.copyState, '');
  for (const locale of ['en', 'ru']) {
    for (const key of ['help.guide', 'help.issue']) assert(I18n.translate(key, locale).endsWith(' ↗'));
    for (const key of ['help.copy_request', 'setup.guide_short', 'setup.update_help', 'native.onboarding.setup_guide']) assert(!I18n.translate(key, locale).includes('↗'));
  }
  assert(panel.includes('onIssueRequested: Qt.openUrlExternally("https://github.com/k-kostin/omavless/issues/new")'));
});
test('a late clipboard result cannot announce success in a reopened help surface', () => {
  const c = context(); c.openHelp('help'); c.copyRequest(); c.dismiss();
  c.finishCopy(0); assert.equal(c.copyState, 'copying');
  c.openHelp('install'); assert.equal(c.copyState, '');
  c.finishCopy(0); assert.equal(c.copyState, '');
  assert(!c.copyRequest(), 'the original bounded writer still owns its slot');
  c.clipboardWriter.running = false;
  assert(c.copyRequest()); c.finishCopy(0); assert.equal(c.copyState, 'copied');
});
test('keyboard wraps in both directions and Escape dismisses help only', () => {
  const c = context();
  c.focusTargets = Array.from({length: 5}, (_, index) => ({activeFocus: false, forceActiveFocus() {
    c.focusTargets.forEach(item => item.activeFocus = false); this.activeFocus = true; c.index = index;
  }}));
  c.focusTargets[1].forceActiveFocus();
  for (let i = 0; i < 10; i++) { const e = {key: 2, modifiers: 0, accepted: false}; c.navigate(e); assert(e.accepted); assert.equal(c.index, (i + 2) % 5); }
  c.navigate({key: 2, modifiers: 4}); assert.equal(c.index, 0);
  c.navigate({key: 3, modifiers: 0}); assert.equal(c.index, 4);
  const textKey = {key: 99, modifiers: 0, accepted: false}; c.navigate(textKey); assert(!textKey.accepted);
  const escape = {key: 1, modifiers: 0, accepted: false}; c.navigate(escape); assert(c.closed && escape.accepted);
});
test('panel restores the originating focus and does not mark onboarding or terminal complete', () => {
  const c = vm.createContext({helpReturnFocus: null, helpPrompt: {openHelp(section) { c.section = section; }, dismiss() { c.hidden = true; }},
    Qt: {callLater: f => f()}, onboardingWizard: {visible: true, forceActiveFocus() { c.wizardFocus = true; }},
    keyCatcher: {forceActiveFocus() { c.panelFocus = true; }}}); c.root = c;
  for (const name of ['openHelp', 'closeHelp']) loadFunction(panel, name, c);
  const target = {visible: true, forceActiveFocus() { c.originFocus = true; }};
  c.openHelp('help', target); assert.equal(c.section, 'help'); c.closeHelp();
  assert(c.hidden && c.originFocus); assert(c.onboardingWizard.visible); assert.equal(c.helpReturnFocus, null);
  c.openHelp('install', null); c.closeHelp(); assert(c.wizardFocus);
  c.onboardingWizard.visible = false; c.closeHelp(); assert(c.panelFocus);
  assert(panel.includes('readonly property bool modalInputActive: helpPrompt.visible || onboardingWizard.visible'));
  assert(panel.includes('blocked: helpPrompt.visible || root.quitConfirmation'));
  assert(panel.includes('helpPrompt.visible ? Style.space(600) : root.bootstrapRequired'));
  assert(panel.includes('onHelpRequested: function(section) { root.openHelp(section, setupPage) }'));
  assert(panel.includes('onSetupGuideRequested: root.openHelp("install", onboardingWizard)'));
  assert(panel.includes('nativeHelpSetting.focusTarget'));
});
test('public guide has the exact copied English template and both local guide files ship', () => {
  const guide = fs.readFileSync(__dirname + '/../docs/user/NATIVE_INSTALL.md', 'utf8');
  assert(guide.includes('```text\n' + I18n.translate('help.agent_prompt', 'en') + '```'));
  const agent = fs.readFileSync(__dirname + '/../packaging/release/distribution/AGENTS.md', 'utf8');
  assert(agent.includes('docs/user/USER_AGENT_GUIDE.md'));
  assert(fs.readFileSync(__dirname + '/../packaging/release/export-distribution.py', 'utf8').includes("'docs/user/USER_AGENT_GUIDE.md'"));
  for (const section of ['install', 'updates', 'help']) assert(guide.includes('id="' + section + '"') || new RegExp('^## ' + section + '$', 'mi').test(guide));
});
console.log('Agent-first help: ' + count + ' production-function/navigation cases PASS; no clipboard/host effects.');
