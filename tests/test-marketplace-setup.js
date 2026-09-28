// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict');
const fs = require('node:fs');
const state = require('../plugin/SetupState.js');
const i18n = require('../plugin/I18n.js');
const page = fs.readFileSync(__dirname + '/../plugin/SetupPage.qml', 'utf8');
const panel = fs.readFileSync(__dirname + '/../plugin/Panel.qml', 'utf8');
const card = fs.readFileSync(__dirname + '/../plugin/RequiredComponents.qml', 'utf8');
let count = 0;
function test(name, run) { try { run(); count++; } catch (e) { e.message = name + ': ' + e.message; throw e; } }
test('only public bounded status enums cross the process boundary', () => {
  for (const value of ['ready', 'needs_package', 'needs_activation', 'needs_companion', 'needs_selection', 'needs_broker', 'needs_broker_stopped', 'needs_runtime_stop', 'needs_attention', 'release_unavailable']) {
    assert.equal(state.parse(value + '\n', 0), value);
    assert.equal(state.parse(value, 1), 'needs_attention');
  }
  for (const value of ['', null, {}, 'secret://synthetic', 'x'.repeat(100), 'ready\nready', ' ready', 'ready\n\n'])
    assert.equal(state.parse(value, 0), 'needs_attention');
});
test('only explicit package, activation, selection or stopped-broker states offer setup', () => {
  for (const value of ['ready', 'needs_companion', 'needs_broker', 'needs_runtime_stop', 'needs_attention', 'release_unavailable', 'checking']) assert(!state.canInstall(value));
  assert(state.canInstall('needs_package')); assert(state.canInstall('needs_activation')); assert(state.canInstall('needs_selection')); assert(state.canInstall('needs_broker_stopped'));
});
test('page never uses missing backend, never installs on load or check', () => {
  assert(page.includes('Component.onCompleted: check()'));
  assert(page.includes('command: ["/bin/bash", setup.script, "components"]'));
  assert(page.includes('["omarchy", "launch", "terminal", "/bin/bash", setup.script, setup.launchAction'));
  assert(!/backendPath|execDetached|sudo|connect\(/.test(page));
  assert(page.includes('launching || terminalOpened'));
  assert(page.includes('terminalOpened = false; check()'));
});
test('setup has separate navigation and cannot expose legacy mutation shortcuts', () => {
  assert(panel.includes('readonly property bool bootstrapRequired: setupPage.state !== "ready" && !vless.nativeOwner'));
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
  for (const nativeOwner of [true, false]) for (const value of ['checking', 'needs_package', 'needs_activation', 'needs_companion', 'needs_selection', 'needs_broker', 'needs_broker_stopped', 'needs_runtime_stop', 'needs_attention', 'release_unavailable', 'ready']) {
    assert.equal(vm.runInNewContext(expression, {vless:{nativeOwner}, setupPage:{state:value}}), value !== 'ready' && !nativeOwner);
  }
  assert(panel.includes('visible: !root.bootstrapRequired && vless.nativeOwner && root.page !== "diagnostics"'));
  assert(panel.includes('visible: !root.bootstrapRequired && vless.nativeOwner && (root.page === "main" || root.page === "subscription")'));
});
test('all first-use and unavailable states have bounded EN/RU plain text', () => {
  for (const locale of ['en', 'ru']) for (const key of ['title', 'checking', 'ready', 'needs_package',
    'needs_activation', 'needs_companion', 'needs_selection', 'needs_broker', 'needs_broker_stopped', 'needs_runtime_stop', 'needs_attention', 'release_unavailable', 'explanation', 'terminal', 'terminal_closed', 'install', 'prepare', 'select_pair', 'start_broker', 'restore_enrollment', 'check', 'guide', 'later', 'components', 'app_missing', 'core_missing', 'install_all', 'install_app', 'install_core', 'prepare_title', 'guide_short', 'panel_unavailable', 'profiles_unavailable']) {
    const text = i18n.translate('setup.' + key, locale, {});
    assert(text.length > 0 && text.length <= 512);
    assert(!/Missing translation|[<>]/.test(text));
  }
  assert(!page.includes('Text.AutoText')); assert(page.includes('PlainText'));
});
test('inventory rejects fragments, extra fields and secrets without echo', () => {
  for (const value of ['ready\tpresent\n\n', 'ready\tpresent\textra', 'ready\tunknown', 'ready\tmissing\nsecret://synthetic', null, {}, 'x'.repeat(97)])
    assert.deepEqual(state.inventory(value,0),{state:'needs_attention',coreInstalled:null});
  assert.deepEqual(state.inventory('ready\tmissing\n',0),{state:'ready',coreInstalled:false});
  assert.deepEqual(state.inventory('ready\tpresent\n',0),{state:'ready',coreInstalled:true});
  assert.deepEqual(state.inventory('ready\tpresent\n',1),{state:'needs_attention',coreInstalled:null});
});
test('all component combinations select one correct install target or no action', () => {
  for (const coreInstalled of [true,false]) {
    const missing={state:'needs_package',coreInstalled};assert(state.appMissing(missing));assert.equal(state.missingAction(missing),'install');
    const unpublished={state:'release_unavailable',coreInstalled};assert.equal(state.missingAction(unpublished),'');
    const ready={state:'ready',coreInstalled};assert(!state.appMissing(ready));assert.equal(state.missingAction(ready),'');
    assert.equal(state.needsAttention(ready),!coreInstalled);
    const activation={state:'needs_activation',coreInstalled};assert(!state.appMissing(activation));assert(state.needsAttention(activation));
    assert.equal(state.missingAction(activation),coreInstalled?'install':'');
    const selection={state:'needs_selection',coreInstalled};assert.equal(state.missingAction(selection),coreInstalled?'finish-selection':'');
    const stopped={state:'needs_broker_stopped',coreInstalled};assert.equal(state.missingAction(stopped),coreInstalled?'start-broker':'');
    assert.equal(state.canRunAction(stopped,'restore-enrollment'),coreInstalled);
    assert.equal(state.canRunAction(stopped,'start-broker'),coreInstalled);
    assert(!state.canRunAction(stopped,'install'));
    assert.equal(state.missingAction({state:'needs_companion',coreInstalled}),'');
    assert.equal(state.missingAction({state:'needs_broker',coreInstalled}),'');
    assert.equal(state.missingAction({state:'needs_runtime_stop',coreInstalled}),'');
  }
  assert.equal(state.missingAction({state:'needs_attention',coreInstalled:false}),'');
});
test('installed programs disappear from the required-components card; readiness is separate', () => {
  assert(card.includes('visible: card.appMissing || card.coreMissing'));
  assert(card.includes('visible: card.appMissing; text: card.tr("app_missing")'));
  assert(card.includes('visible: card.coreMissing; text: card.tr("core_missing")'));
  assert(card.includes('visible: SetupState.needsAttention(facts)'));
  assert(card.includes('[installButton, prepareButton, restoreButton, retryButton, checkButton, guideButton]'));
  assert(card.includes('visible: card.facts.state === "needs_activation" || card.facts.state === "needs_selection" || card.facts.state === "needs_broker_stopped"'));
  assert(card.includes('RowLayout {\n        Layout.fillWidth: true\n        spacing: Style.space(8)\n        visible: card.facts.state'));
  assert(card.includes('onClicked: card.installRequested("restore-enrollment")'));
  assert(card.includes('card.facts.coreInstalled === true && !card.busy'));
  assert(panel.includes('panelOpen: root.opened'));
  assert(panel.includes('id: nativeRequiredComponents'));
  assert(panel.indexOf('id: nativeRequiredComponents')>panel.indexOf('} // nativeProfilesFrame'));
});
test('real install handler rejects stale target, arbitrary action and repeated launch', () => {
  const vm=require('node:vm');
  const start=page.indexOf('  function install(action)'), end=page.indexOf('\n  }',start)+4;
  const c=vm.createContext({SetupState:state,facts:{state:'needs_companion',coreInstalled:false},busy:false,launching:false,terminalOpened:false,launch:{running:false}});
  vm.runInContext(page.slice(start,end),c);
  c.install('install');assert(!c.launch.running);
  c.install('shell');assert(!c.launch.running);
  c.install('install-core');assert(!c.launch.running);
  c.facts={state:'needs_selection',coreInstalled:true};c.install('finish-selection');assert(c.launch.running);assert.equal(c.launchAction,'finish-selection');
  c.launch.running=false;c.install('finish-selection');assert(!c.launch.running);
  c.launching=false;c.terminalOpened=true;c.install('finish-selection');assert(!c.launch.running);
  c.terminalOpened=false;c.facts={state:'needs_activation',coreInstalled:true};c.install('install');assert(c.launch.running);
  c.launching=false;c.launch.running=false;c.facts={state:'needs_broker_stopped',coreInstalled:true};c.install('start-broker');assert(c.launch.running);assert.equal(c.launchAction,'start-broker');
  c.launching=false;c.launch.running=false;c.install('restore-enrollment');assert(c.launch.running);assert.equal(c.launchAction,'restore-enrollment');
});
test('Later closes only the panel and never marks onboarding complete or hides reminders persistently', () => {
  assert(page.includes('onClicked: setup.closeRequested()'));
  assert(!/completeOnboarding|onboardingComplete|settings\.[\w]+\s*=/.test(page));
});
console.log(`${count} marketplace setup contracts passed`);
