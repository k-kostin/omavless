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
  const c = vm.createContext({locale, I18n, visible: false, guideSection: 'help', copyState: '', requestExpanded: false, pendingText: '', generation: 0, copyGeneration: 0,
    clipboardWriter: {running: false, stdinEnabled: false}, helpScroll: {contentY: 90, contentHeight: 500, height: 300},
    requestText: {deselect() { c.deselected = true; }}, copyButton: {forceActiveFocus() { c.focusedCopy = true; }},
    requestToggle: {forceActiveFocus() { c.focusedToggle = true; }}, reveal(control) { c.revealed = control; },
    Qt: {callLater: f => f(), Key_Escape: 1, Key_Tab: 2, Key_Backtab: 3, ShiftModifier: 4},
    canceled() { c.closed = true; }, guideRequested(section) { c.guide = section; },
    issueRequested() { c.issue = true; }});
  c.help = c;
  c.guideButton = {activeFocus: false}; c.issueButton = {activeFocus: false}; c.closeButton = {activeFocus: false};
  c.focusTargets = [c.copyButton, c.requestToggle, c.requestText, c.guideButton, c.issueButton, c.closeButton];
  for (const name of ['tr', 'openHelp', 'dismiss', 'copyRequest', 'finishCopy', 'toggleRequest', 'clampScroll', 'openGuide', 'openIssue', 'navigate']) loadFunction(source, name, c);
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
    assert.equal(c.requestExpanded, false);
    assert.equal(c.guide, undefined); assert.equal(c.issue, undefined);
    c.dismiss(); assert(!c.visible);
  }
});
test('copy snapshots the unchanged public prompt without requiring expansion', () => {
  for (const locale of ['en', 'ru']) {
    const c = context(locale);
    assert(!c.copyRequest()); assert(!c.clipboardWriter.running);
    c.openHelp('help'); assert(c.copyRequest());
    assert.equal(c.requestExpanded, false);
    assert.equal(c.pendingText, c.agentPrompt);
    assert(c.clipboardWriter.running && c.clipboardWriter.stdinEnabled);
    assert.equal(c.copyState, 'copying');
    assert(!c.copyRequest(), 'no overlapping clipboard writer');
    c.finishCopy(1); assert.equal(c.copyState, 'copy_failed'); assert(c.requestExpanded);
    c.finishCopy(124); assert.equal(c.copyState, 'copy_failed');
    c.finishCopy(0); assert.equal(c.copyState, 'copied');
  }
  assert(source.includes('command: ["/usr/bin/timeout", "5s", "wl-copy"]'));
  assert(source.includes('write(help.pendingText)'));
  assert(source.includes('readOnly: true') && source.includes('textFormat: TextEdit.PlainText'));
  assert(source.includes('selectByMouse: true') && source.includes('activeFocusOnTab: true'));
  assert(!/vless\.|backendPath|clipboard-read|wl-paste|execDetached|sudo|FileView/.test(source));
});
test('request is hidden by default and disclosure never copies or changes its text', () => {
  const c = context();
  c.toggleRequest(); assert(!c.requestExpanded, 'hidden Help cannot expand');
  c.openHelp('help');
  c.toggleRequest(); assert(c.requestExpanded);
  assert.equal(c.revealed, c.requestToggle);
  assert(!c.clipboardWriter.running); assert.equal(c.pendingText, '');
  c.requestText.activeFocus = true;
  c.toggleRequest(); assert(!c.requestExpanded && c.focusedToggle && c.deselected);
  c.toggleRequest(); c.dismiss(); c.openHelp('help');
  assert(!c.requestExpanded, 'new visit starts compact');
  assert.equal(c.agentPrompt, I18n.translate('help.agent_prompt', 'en'));
  assert(source.includes('visible: help.requestExpanded'));
  assert(source.indexOf('id: copyButton') < source.indexOf('id: requestToggle'));
  assert(source.indexOf('id: requestToggle') < source.indexOf('id: requestText'));
});
test('collapsing clamps the scroll even when the toggle does not own focus', () => {
  const c = context(); c.openHelp('help'); c.toggleRequest();
  c.helpScroll.contentY = 400; c.helpScroll.contentHeight = 330;
  c.toggleRequest(); assert.equal(c.helpScroll.contentY, 30);
  c.helpScroll.contentHeight = 150; c.clampScroll(); assert.equal(c.helpScroll.contentY, 0);
});
test('copy failure reveals manual text without stealing another action focus', () => {
  const c = context(); c.openHelp('help'); c.copyRequest();
  c.copyButton.activeFocus = false;
  c.guideButton.activeFocus = true;
  c.finishCopy(1);
  assert(c.requestExpanded); assert.equal(c.copyState, 'copy_failed');
  assert.equal(c.revealed, c.guideButton); assert(!c.focusedToggle);
  c.guideButton.activeFocus = false;
  c.requestExpanded = false; c.copyButton.activeFocus = true;
  c.finishCopy(124);
  assert(c.requestExpanded); assert.equal(c.revealed, c.copyButton);
  c.requestExpanded = false; c.finishCopy(0);
  assert(!c.requestExpanded, 'success keeps compact view');
});
test('late failure reveals the same lower focused action using its new layout position', () => {
  for (const name of ['guideButton', 'issueButton', 'closeButton']) {
    const c = context(); c.openHelp('help'); c.copyRequest();
    c[name].activeFocus = true; c[name].height = 35;
    c[name].mapToItem = () => ({y: c.requestExpanded ? 650 : 220});
    c.helpScroll.height = 300; c.helpScroll.contentHeight = 800;
    loadFunction(source, 'reveal', c);
    c.finishCopy(1);
    assert(c[name].activeFocus, 'focus stays on ' + name);
    assert.equal(c.helpScroll.contentY, 385, name + ' remains visible after expansion');
    assert(!c.focusedToggle);
  }
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
test('a late clipboard result cannot announce success or expand a reopened help surface', () => {
  const c = context(); c.openHelp('help'); c.copyRequest(); c.dismiss();
  c.finishCopy(0); assert.equal(c.copyState, 'copying');
  c.openHelp('install'); assert.equal(c.copyState, '');
  c.finishCopy(0); assert.equal(c.copyState, '');
  c.finishCopy(1); assert.equal(c.copyState, ''); assert(!c.requestExpanded);
  assert(!c.copyRequest(), 'the original bounded writer still owns its slot');
  c.clipboardWriter.running = false;
  assert(c.copyRequest()); c.finishCopy(0); assert.equal(c.copyState, 'copied');
});
test('actual focus binding skips hidden or disabled controls in visual order', () => {
  const c = context();
  const names = ['copyButton', 'requestToggle', 'requestText', 'guideButton', 'issueButton', 'closeButton'];
  for (const name of names) {
    c[name] = {name, visible: true, enabled: true, activeFocus: false, deselect() {}, forceActiveFocus() {
      names.forEach(key => c[key].activeFocus = false); this.activeFocus = true;
    }};
  }
  Object.defineProperty(c.requestText, 'visible', {get: () => c.requestExpanded});
  const binding = source.match(/readonly property var focusTargets: (.+)/)[1];
  Object.defineProperty(c, 'focusTargets', {get: () => vm.runInContext(binding, c)});
  c.openHelp('help');
  assert.deepEqual(Array.from(c.focusTargets, item => item.name), names.filter(name => name !== 'requestText'));
  c.navigate({key: 2, modifiers: 0}); assert(c.requestToggle.activeFocus);
  c.navigate({key: 2, modifiers: 0}); assert(c.guideButton.activeFocus);
  c.toggleRequest(); c.requestToggle.forceActiveFocus();
  c.navigate({key: 2, modifiers: 0}); assert(c.requestText.activeFocus);
  c.navigate({key: 3, modifiers: 0}); assert(c.requestToggle.activeFocus);
  c.guideButton.enabled = false;
  assert(!Array.from(c.focusTargets).includes(c.guideButton));
  names.forEach(name => c[name].enabled = false);
  const e = {key: 2, modifiers: 0, accepted: false}; c.navigate(e); assert(!e.accepted);
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
  assert(panel.includes('readonly property bool modalInputActive: removeReview.visible || helpPrompt.visible || onboardingWizard.visible'));
  assert(panel.includes('blocked: removeReview.visible || helpPrompt.visible || subscriptionUsagePrompt.visible || root.quitConfirmation'));
  assert(panel.includes('helpPrompt.visible || subscriptionUsagePrompt.visible ? Style.space(600) : root.bootstrapRequired'));
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
function guideContext() {
  const c = vm.createContext({});
  vm.runInContext(fs.readFileSync(__dirname + '/../plugin/GuideLinks.js', 'utf8'), c);
  return c;
}
function syntheticMeaningReview(c) {
  // A test-only attestation exercises routing; it never approves actual prose.
  const m = c.guideManifest;
  m.russian.humanReview = {kind:'owner-human-meaning', version:m.version,
    englishSha256:m.english.sha256, russianSha256:m.russian.sha256, reviewedOn:'2026-10-30'};
}
test('guide source hashes bind the complete actual pair, before export rewrites', () => {
  const crypto = require('node:crypto');
  const c = guideContext();
  for (const language of ['english', 'russian']) {
    const entry = c.guideManifest[language];
    assert.equal(crypto.createHash('sha256').update(fs.readFileSync(__dirname + '/../' + entry.path)).digest('hex'), entry.sha256);
  }
});
test('unreviewed or unavailable translations use English for every section', () => {
  const c = guideContext();
  c.guideManifest.russian.humanReview = null;
  for (const section of ['install', 'updates', 'help']) {
    assert.equal(c.guideUrl('ru', '0.9.9', section), 'https://github.com/k-kostin/omavless/blob/main/docs/user/NATIVE_INSTALL.md#' + section);
  }
  c.guideManifest.russian = null;
  assert(c.guideUrl('ru', '0.9.9', 'install').endsWith('NATIVE_INSTALL.md#install'));
});
test('reviewed exact pair follows effective UI locale and closed section map', () => {
  const c = guideContext(); syntheticMeaningReview(c);
  for (const locale of ['en', 'ru']) for (const section of ['install', 'updates', 'help']) {
    assert(c.guideUrl(locale, '0.9.9', section).endsWith((locale === 'ru' ? 'NATIVE_INSTALL_RU.md#' : 'NATIVE_INSTALL.md#') + section));
  }
  for (const locale of ['system', '', 'xx', null]) assert(c.guideUrl(locale, '0.9.9', 'help').endsWith('NATIVE_INSTALL.md#help'));
  for (const section of ['', '../secret', 'https://example.invalid', null]) assert(c.guideUrl('ru', '0.9.9', section).endsWith('NATIVE_INSTALL_RU.md#help'));
  assert(panel.includes('GuideLinks.guideUrl(root.uiLocale, root.releaseManifest ? root.releaseManifest.version : "", section)'));
});
test('stale version, source identity or review cannot activate Russian', () => {
  for (const change of [c => {c.guideManifest.version = '0.9.8';},
    c => {c.guideManifest.english.sha256 = 'a'.repeat(64);},
    c => {c.guideManifest.russian.sha256 = 'b'.repeat(64);},
    c => {c.guideManifest.russian.humanReview.kind = 'model-review';},
    c => {c.guideManifest.russian.path = 'https://example.invalid';}]) {
    const c = guideContext(); syntheticMeaningReview(c); change(c);
    assert(c.guideUrl('ru', '0.9.9', 'updates').endsWith('NATIVE_INSTALL.md#updates'));
  }
  const c = guideContext(); syntheticMeaningReview(c);
  for (const version of ['', null, '0.9.8', '1.0.0']) assert(c.guideUrl('ru', version, 'help').endsWith('NATIVE_INSTALL.md#help'));
});
console.log('Agent-first help: ' + count + ' production-function/navigation cases PASS; no clipboard/host effects.');
