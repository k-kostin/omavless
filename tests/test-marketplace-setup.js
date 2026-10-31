// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict');
const fs = require('node:fs');
const state = require('../plugin/SetupState.js');
const i18n = require('../plugin/I18n.js');
const page = fs.readFileSync(__dirname + '/../plugin/SetupPage.qml', 'utf8');
const panel = fs.readFileSync(__dirname + '/../plugin/Panel.qml', 'utf8');
const card = fs.readFileSync(__dirname + '/../plugin/RequiredComponents.qml', 'utf8');
assert(card.includes('card.facts.state === "needs_pair_install" ? "prepare_title"'),
  'A missing component needs setup, not an UPDATE AVAILABLE heading');
let count = 0;
function test(name, run) { try { run(); count++; } catch (e) { e.message = name + ': ' + e.message; throw e; } }
test('glossary action names match the catalog and terminal return controls', () => {
  const glossary=fs.readFileSync(__dirname+'/../GLOSSARY.md','utf8');
  const terminal=fs.readFileSync(__dirname+'/../plugin/setup-runtime.sh','utf8');
  const rows=[...glossary.matchAll(/^\| `([^`]+)` \| ([^|]+) \| ([^|]+) \|$/gm)]
    .filter(m=>m[1].includes('.'));
  assert(rows.length>=11,'The shared setup action vocabulary must remain discoverable');
  for(const row of rows)for(const [index,locale] of [[2,'en'],[3,'ru']]) {
    assert.equal(row[index].trim(),i18n.translate(row[1],locale,{}),row[1]+' '+locale);
  }
  for(const key of ['setup.prepare_update','setup.start_broker','setup.start_app',
                    'setup.resume_dns','setup.terminal_closed','setup.check','setup.guide_short']) {
    for(const locale of ['en','ru']) {
      const label=i18n.translate(key,locale,{}).replace(/ ↗$/,'');
      assert(terminal.includes(label),'Terminal names a real visible action: '+key+' '+locale);
    }
  }
  assert(terminal.includes('OMAVLESS_LOCALE="$setup_locale"'));
});
test('update flow entry and return instructions use the same user-facing label', () => {
  const terminal=fs.readFileSync(__dirname+'/../plugin/setup-runtime.sh','utf8');
  const guide=fs.readFileSync(__dirname+'/../docs/user/NATIVE_INSTALL.md','utf8');
  for(const locale of ['en','ru']) {
    const label=i18n.translate('setup.prepare_update',locale,{});
    assert(label.endsWith('…'));
    assert(terminal.includes(label),'Return guidance must name the actual visible control');
    if(locale==='en')assert(guide.includes('**'+label+'**'));
  }
  assert(card.includes('onClicked: card.installRequested("update-pair", pairedUpdateButton)'));
  assert(guide.includes('`PREPARE`')&&guide.includes('`UPGRADE`'));
  assert(terminal.includes('setup_step upgrade-operation upgrade_package "$operation"'));
});
test('operational steps precede optional updates across inventory states', () => {
  const vm=require('node:vm');
  function presentation(facts) {
    const c={SetupState:state,facts};c.card=c;
    for(const name of ['appMissing','pairUpdate','updateReview','operationalStep','showPreparation','optionalUpdate']) {
      const expression=card.match(new RegExp('readonly property bool '+name+': ([^\\n]+)'))[1];
      c[name]=vm.runInNewContext(expression,c);
    }
    return c;
  }
  for(const delivery of ['current','stale','stale_manual'])for(const coreInstalled of [true,false,null]) {
    for(const status of ['needs_activation','needs_selection','needs_dns_setup','needs_broker_stopped','needs_runtime_start']) {
      const c=presentation({state:status,coreInstalled,delivery});assert(c.operationalStep);assert(c.showPreparation);
    }
    const ready=presentation({state:'ready',coreInstalled,delivery});assert.equal(ready.showPreparation,coreInstalled===null);
    for(const status of ['needs_broker','needs_runtime_stop','needs_attention','checking'])assert(presentation({state:status,coreInstalled,delivery}).showPreparation);
    for(const status of ['needs_update','needs_pair_install','needs_pair_mismatch','needs_companion'])assert(!presentation({state:status,coreInstalled,delivery}).optionalUpdate);
    for(const status of ['needs_pair_update','needs_pair_update_manual'])assert(presentation({state:status,coreInstalled,delivery}).updateReview);
  }
  assert(card.indexOf('id: preparation') < card.indexOf('id: updates'));
  assert(card.indexOf('id: prepareButton') < card.indexOf('id: pairedUpdateButton'));
});
test('recovery disclosure is navigation, and restore retains all existing barriers', () => {
  const vm=require('node:vm');
  const toggle=card.slice(card.indexOf('id: recoveryToggle'),card.indexOf('\n      }',card.indexOf('id: recoveryToggle')));
  assert(toggle.includes('onClicked: card.recoveryExpanded = !card.recoveryExpanded'));
  assert(!toggle.includes('installRequested'));
  const restore=card.slice(card.indexOf('id: restoreButton'),card.indexOf('\n      }',card.indexOf('id: restoreButton')));
  const visible=restore.match(/visible: ([^\n]+)/)[1],enabled=restore.match(/enabled: ([^\n]+)/)[1];
  for(const shown of [false,true])for(const recoveryExpanded of [false,true])for(const coreInstalled of [true,false,null])for(const effectBusy of [false,true])for(const terminalOpened of [false,true]) {
    const c={card:{recoveryExpanded,facts:{coreInstalled},effectBusy,terminalOpened},recoveryToggle:{visible:shown}};
    c.visible=vm.runInNewContext(visible,c);
    assert.equal(c.visible,shown&&recoveryExpanded);
    assert.equal(vm.runInNewContext(enabled,c),shown&&recoveryExpanded&&coreInstalled===true&&!effectBusy&&!terminalOpened);
  }
});
test('offline inventory hides unsupported recovery but keeps ordinary continuations', () => {
  const vm=require('node:vm');
  const toggle=card.slice(card.indexOf('id: recoveryToggle'),card.indexOf('\n      }',card.indexOf('id: recoveryToggle')));
  const visible=toggle.match(/visible: ([^\n]+)/)[1];
  for(const channel of ['', '\toffline-candidate'])for(const present of ['present','missing']) {
    const facts=state.inventory('needs_broker_stopped\t'+present+channel+'\n',0);
    const restore=channel===''&&present==='present';
    assert.equal(vm.runInNewContext(visible,{SetupState:state,card:{facts}}),restore);
    assert.equal(state.canRunAction(facts,'restore-enrollment'),restore);
    assert.equal(state.canRunAction(facts,'start-broker'),present==='present');
    assert(!state.hasUpdate(facts));
  }
  for(const [status,action] of [['needs_package','install'],['needs_activation','install'],
      ['needs_dns_setup','resume-dns'],['needs_selection','finish-selection'],['needs_runtime_start','start-app']]) {
    const facts=state.inventory(status+'\tpresent\toffline-candidate\n',0);
    assert.equal(facts.channel,'offline-candidate');
    assert(state.canRunAction(facts,action));
    assert(!state.canRunAction(facts,'restore-enrollment'));
    assert(!state.canRunAction(facts,'update-pair'));
  }
  assert(card.includes('onFactsChanged: if (!SetupState.canRestoreHelperAccess(facts)) recoveryExpanded = false'));
});
test('every web-link label has the same trailing marker, local URL reveal does not', () => {
  for(const locale of ['en','ru'])for(const key of ['help.guide','help.issue','setup.guide','settings.source']) {
    const value=i18n.translate(key,locale,{source:'Synthetic source'});assert(value.endsWith(' ↗'));assert.equal(value.split('↗').length,2);
  }
  for(const locale of ['en','ru'])for(const key of ['common.show_url','common.hide_url','setup.install_all','setup.prepare_update','setup.guide_short','setup.update_help','native.onboarding.setup_guide','help.copy_request'])assert(!i18n.translate(key,locale,{}).includes('↗'));
});
test('local setup Help keeps a neutral button boundary and its existing local destination', () => {
  for (const [source, id, handler] of [[card, 'guideButton', 'card.guideRequested(guideButton)'],
      [page, 'reviewGuideButton', 'setup.guide(reviewGuideButton)']]) {
    const control = source.slice(source.indexOf('id: ' + id)).split('}')[0];
    assert(control.includes('bordered: true'), id + ' needs a visible button boundary');
    assert(control.includes('focusable: true'), id + ' remains keyboard reachable');
    assert(!/primary:\s*true/.test(control), id + ' must not compete with setup');
    assert(control.includes('onClicked: ' + handler), id + ' preserves local navigation');
    assert(!/installRequested|openUrlExternally/.test(control));
  }
  assert(page.includes('helpRequested(SetupState.hasUpdate(facts) ? "updates" : "install", returnFocus)'));
});
test('stopped app offers only explicit native start, never reinstall or terminal setup', () => {
  const facts={state:'needs_runtime_start',coreInstalled:true};
  assert.equal(state.parse('needs_runtime_start\n',0),'needs_runtime_start');
  assert.deepEqual(state.inventory('needs_runtime_start\tpresent\n',0),facts);
  assert.equal(state.missingAction(facts),'start-app');
  assert(!state.canInstall(facts.state));
  assert(state.canRunAction(facts,'start-app'));
  for(const action of ['install','start-broker','shell','restore-enrollment']) assert(!state.canRunAction(facts,action));
  for(const locale of ['en','ru']) for(const key of ['start_app','needs_runtime_start']) assert(!i18n.translate('setup.'+key,locale,{}).includes('Missing translation'));
  const vm=require('node:vm');
  const start=page.indexOf('  function install(action, returnFocus)'),end=page.indexOf('\n  }',start)+4;
  const c=vm.createContext({SetupState:state,facts,busy:false,launching:false,terminalOpened:false,nativeEffectBlocked:false,startApp:{running:false},launch:{running:false}});
  vm.runInContext(page.slice(start,end),c);
  c.install('start-app');assert(c.startApp.running);assert(!c.launch.running);assert(!c.launching);
  c.startApp.running=false;c.busy=true;c.install('start-app');assert(!c.startApp.running);
  assert(page.includes('command: ["/usr/bin/omavless", "app", "start"]'));
});
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
test('terminal handoff closes only the panel and keeps its barrier until explicit closure', () => {
  const vm=require('node:vm');
  function method(name) {
    const start=page.indexOf('  function '+name+'(');
    assert(start>=0,name+' exists');
    return page.slice(start,page.indexOf('\n  }',start)+4);
  }
  let handedOff=0,checked=0;
  const c=vm.createContext({SetupState:state,facts:{state:'needs_package',coreInstalled:false},
    busy:false,launching:false,terminalOpened:false,nativeEffectBlocked:false,
    launch:{running:false},terminalRequested(){handedOff++},check(){checked++}});
  vm.runInContext(method('install'),c);
  c.install('install');assert(c.launch.running);assert(c.launching);assert(c.terminalOpened);
  assert.equal(handedOff,0,'request is not observed process start');
  vm.runInContext(method('handOffTerminal'),c);
  c.handOffTerminal();assert.equal(handedOff,1);
  vm.runInContext(method('acknowledgeTerminalClosed'),c);
  c.acknowledgeTerminalClosed();assert(c.terminalOpened);assert.equal(checked,0);
  c.launch.running=false;c.acknowledgeTerminalClosed();assert(c.terminalOpened);
  c.launching=false;c.acknowledgeTerminalClosed();assert(!c.terminalOpened);assert.equal(checked,1);
  c.handOffTerminal();assert.equal(handedOff,1,'no stale handoff after completion');
  assert(page.includes('onStarted: setup.handOffTerminal()'));
  assert(panel.includes('onTerminalRequested: root.close()'));
  const setupFlick=panel.slice(panel.indexOf('id: setupFlick'),panel.indexOf('id: nativeFlick'));
  assert(setupFlick.includes('Keys.onPressed: function(event) { root.handlePanelControlKey(event) }'));
  const ack=card.slice(card.indexOf('id: retryButton')).split('\n')[0];
  assert(ack.includes('enabled: !card.busy'));
  assert(!page.includes('setup.terminalOpened = code === 0'),'failure cannot silently clear closure barrier');
});
test('setup has separate navigation and cannot expose legacy mutation shortcuts', () => {
  assert(panel.includes('readonly property bool bootstrapRequired: componentSetupReview || (runtimePanelState === "" && SetupState.bootstrapRequired('));
  assert(panel.includes('vless.nativeActionRunning, vless.nativePending !== null, vless.nativeOutcomeUnknown'));
  assert(page.includes('readonly property bool appMissing: SetupState.appMissing(facts)'));
  assert(panel.includes('if (root.bootstrapRequired) return setupPage.focusTargets'));
  assert(panel.includes('root.bootstrapRequired ? setupFlick'));
  assert(panel.includes('onReady: function(runtimeResumed)'));
  assert(panel.includes('vless.nativeRuntimeRefreshing = runtimeResumed'));
  assert(panel.includes('vless.refreshAfterChange()'));
  assert(panel.includes('if (root.bootstrapRequired) root.close()'));
  assert(panel.includes('onTextKey: function(t) {\n        if (root.bootstrapRequired) return'));
  assert(panel.includes('width: Math.max(0, setupFlick.width - root.scrollGutter)'));
});
test('component review retains help for unverified core, not completed readiness', () => {
  assert(page.includes('property bool componentReview: false'));
  assert(page.includes('visible: !setup.probing && !setup.componentReview'));
  for(const name of ['reviewTerminalButton','reviewCheckButton','reviewGuideButton','closeButton']) assert(page.includes(name));
  assert(page.includes('onClicked: { setup.acknowledgeTerminalClosed(); setup.factsRefreshRequested() }'));
  assert(page.includes('onClicked: { setup.check(); setup.factsRefreshRequested() }'));
  assert(page.includes('setup.componentReview ? "back_to_panel" : "later"'));
  assert(page.includes('&& !setup.needsAttention && !setup.reviewReady'));
  for(const locale of ['en','ru'])for(const key of ['review_components','review_core_attention','back_to_panel'])
    assert(!i18n.translate('setup.'+key,locale,{}).includes('Missing translation'));
});
test('missing app or unactivated account keeps setup shell after read-only launcher failure', () => {
  for (const nativeOwner of [true, false]) for (const nativeSnapshot of [null, {instanceId:'synthetic'}]) for (const value of ['checking', 'needs_package', 'needs_activation', 'needs_companion', 'needs_selection', 'needs_broker', 'needs_broker_stopped', 'needs_runtime_stop', 'needs_runtime_start', 'needs_attention', 'release_unavailable', 'ready']) {
    const appMissing = state.appMissing({state:value});
    const stopped=['needs_runtime_start','needs_broker_stopped'].includes(value);
    assert.equal(state.bootstrapRequired({state:value}, nativeOwner, nativeSnapshot!==null, false, false, false),
      value !== 'ready' && (appMissing || !nativeOwner || !nativeSnapshot || stopped));
  }
  assert(panel.includes('visible: !root.bootstrapRequired && vless.nativeOwner && root.page !== "diagnostics"'));
  assert(panel.includes('visible: !root.bootstrapRequired && vless.nativeOwner && (root.page === "main" || root.page === "subscription")'));
});
test('known stopped facts supersede a stale snapshot but never waive pending or unknown work', () => {
  for (const value of ['needs_runtime_start','needs_broker_stopped']) {
    assert(state.bootstrapRequired({state:value},true,true,false,false,false));
    for (const flags of [[true,false,false],[false,true,false],[false,false,true]])
      assert(!state.bootstrapRequired({state:value},true,true,...flags));
  }
  for (const value of ['ready','checking','needs_attention','needs_broker','needs_runtime_stop'])
    assert(!state.bootstrapRequired({state:value},true,true,false,false,false));
});
test('failed-poll handler refreshes only inventory even with the panel closed', () => {
  const vm=require('node:vm');
  const start=panel.indexOf('    function onStatusFailureCountChanged()');
  const end=panel.indexOf('\n    }',start)+6;
  assert(start>=0);
  const c=vm.createContext({root:{opened:true},vless:{statusFailureCount:1},setupPage:{checks:0,check(){this.checks++}}});
  vm.runInContext(panel.slice(start,end),c);
  c.onStatusFailureCountChanged(); assert.equal(c.setupPage.checks,1);
  c.root.opened=false;c.onStatusFailureCountChanged();assert.equal(c.setupPage.checks,2);
  c.root.opened=true;c.vless.statusFailureCount=0;c.onStatusFailureCountChanged();assert.equal(c.setupPage.checks,2);
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
    assert.equal(state.missingAction({state:'needs_companion',coreInstalled}),'update-help');
    assert.equal(state.missingAction({state:'needs_broker',coreInstalled}),'');
    assert.equal(state.missingAction({state:'needs_runtime_stop',coreInstalled}),'');
  }
  assert.equal(state.missingAction({state:'needs_attention',coreInstalled:false}),'');
});
test('same-version updates, partial pairs and manual-only deliveries select honest actions', () => {
  for (const coreInstalled of [true,false]) {
    for(const name of ['needs_pair_update','needs_pair_install']) {
      const facts={state:name,coreInstalled};
      assert.equal(state.parse(name+'\n',0),name);
      assert.deepEqual(state.inventory(name+'\t'+(coreInstalled?'present':'missing')+'\n',0),facts);
      assert.equal(state.missingAction(facts),'update-pair');
      assert(state.canRunAction(facts,'update-pair'));
      assert(!state.canRunAction(facts,'upgrade'));
      assert(!state.canRunAction(facts,'install'));
    }
    for(const name of ['needs_pair_update_manual','needs_pair_mismatch']) {
      const facts={state:name,coreInstalled};
      assert.equal(state.missingAction(facts),'update-help');
      assert(state.canRunAction(facts,'update-help'));
      assert(!state.canRunAction(facts,'update-pair'));
      assert(!state.canInstall(name));
    }
  }
  for(const locale of ['en','ru']) for(const key of ['needs_pair_update','needs_pair_install','needs_pair_update_manual','needs_pair_mismatch','complete_pair'])
    assert(!i18n.translate('setup.'+key,locale,{}).includes('Missing translation'));
});
test('freshness is advisory and preserves connection admission and stopped actions', () => {
  for (const delivery of ['stale', 'stale_manual']) {
    for (const [status, action] of [['ready',''], ['needs_runtime_start','start-app'], ['needs_broker_stopped','start-broker']]) {
      const facts = state.inventory(status+'\tpresent\t'+delivery+'\n',0);
      assert.equal(facts.state,status);
      assert.equal(state.missingAction(facts),action);
      assert(state.needsAttention(facts));
      assert.equal(state.canRunAction(facts,'update-pair'),delivery==='stale');
      assert(state.canRunAction(facts,'update-help'));
      if(action) assert(state.canRunAction(facts,action));
      // Panel admits Connect by operational state, never delivery freshness.
      assert.equal(facts.state !== 'ready', status !== 'ready');
      assert.equal(state.bootstrapRequired(facts,true,true,false,false,false), status === 'needs_broker_stopped');
      const core = {installed:true,version:'1.19.31',tunDevice:'present',fileNetworkCapabilities:'present'};
      if(status==='ready') assert.equal(state.coreReview(core,facts,false,false,false),'ready');
    }
  }
  for (const suffix of ['unknown','stale\textra','stale\nready'])
    assert.equal(state.inventory('ready\tpresent\t'+suffix+'\n',0).state,'needs_attention');
  for(const status of ['needs_attention','needs_pair_mismatch','needs_broker','checking'])
    assert(!state.canRunAction({state:status,coreInstalled:true,delivery:'stale'},'update-pair'));
});
test('installed programs disappear from the required-components card; readiness is separate', () => {
  assert(card.includes('visible: !card.checking && (card.appMissing || card.coreMissing)'));
  assert(card.includes('visible: card.appMissing; text: card.tr("app_missing")'));
  assert(card.includes('visible: card.coreMissing; text: card.tr("core_missing")'));
  assert(card.includes('visible: SetupState.needsAttention(facts)'));
  assert(card.includes('[installButton, prepareButton, recoveryToggle, restoreButton, updateButton, pairedUpdateButton, updateHelpButton, retryButton, checkButton, guideButton]'));
  assert(card.includes('visible: card.operationalStep'));
  // Longer helper labels need independent full-width targets, not two
  // competing half-width buttons on a narrow setup panel.
  for(const id of ['prepareButton','restoreButton']) {
    const control=card.slice(card.indexOf('id: '+id),card.indexOf('\n      }',card.indexOf('id: '+id)));
    assert(control.includes('Layout.fillWidth: true'));
  }
  assert(card.includes('onClicked: card.installRequested("restore-enrollment", restoreButton)'));
  assert(card.includes('card.facts.coreInstalled === true && !card.effectBusy'));
  assert(panel.includes('panelOpen: root.opened'));
  assert(panel.includes('id: nativeRequiredComponents'));
  assert(panel.indexOf('id: nativeRequiredComponents')>panel.indexOf('} // nativeProfilesFrame'));
});
test('real install handler rejects stale target, arbitrary action and repeated launch', () => {
  const vm=require('node:vm');
  const start=page.indexOf('  function install(action, returnFocus)'), end=page.indexOf('\n  }',start)+4;
  const c=vm.createContext({SetupState:state,facts:{state:'needs_companion',coreInstalled:false},busy:false,launching:false,terminalOpened:false,nativeEffectBlocked:false,launch:{running:false}});
  vm.runInContext(page.slice(start,end),c);
  c.install('install');assert(!c.launch.running);
  c.install('shell');assert(!c.launch.running);
  c.install('install-core');assert(!c.launch.running);
  c.facts={state:'needs_selection',coreInstalled:true};c.install('finish-selection');assert(c.launch.running);assert.equal(c.launchAction,'finish-selection');
  c.launch.running=false;c.install('finish-selection');assert(!c.launch.running);
  c.launching=false;c.terminalOpened=true;c.install('finish-selection');assert(!c.launch.running);
  c.terminalOpened=false;c.facts={state:'needs_activation',coreInstalled:true};c.install('install');assert(c.launch.running);
  // Each distinct terminal case starts only after its previous closure is acknowledged.
  c.launching=false;c.launch.running=false;c.terminalOpened=false;c.facts={state:'needs_broker_stopped',coreInstalled:true};c.install('start-broker');assert(c.launch.running);assert.equal(c.launchAction,'start-broker');
  c.launching=false;c.launch.running=false;c.terminalOpened=false;c.install('restore-enrollment');assert(c.launch.running);assert.equal(c.launchAction,'restore-enrollment');
});
test('setup effect handler blocks unresolved native work across both card locations', () => {
  const vm=require('node:vm');
  const start=page.indexOf('  function install(action, returnFocus)'),end=page.indexOf('\n  }',start)+4;
  for(const action of ['resume-dns','start-app']) {
    const c=vm.createContext({SetupState:state,facts:{state:action==='resume-dns'?'needs_dns_setup':'needs_runtime_start',coreInstalled:true},
      busy:false,launching:false,terminalOpened:false,nativeEffectBlocked:true,
      launch:{running:false},startApp:{running:false}});
    vm.runInContext(page.slice(start,end),c);
    c.install(action); assert(!c.launch.running); assert(!c.startApp.running);
    c.nativeEffectBlocked=false; c.install(action);
    assert.equal(action==='resume-dns'?c.launch.running:c.startApp.running,true);
  }
  assert(panel.includes('nativeEffectBlocked: vless.nativeActionRunning || vless.nativePending !== null'));
  assert(panel.includes('|| vless.nativeOutcomeUnknown || vless.nativeQuitting'));
  assert(panel.includes('busy: setupPage.busy; effectBlocked: setupPage.nativeEffectBlocked'));
  assert(page.includes('busy: setup.busy; effectBlocked: setup.nativeEffectBlocked'));
  for (const busy of [false,true]) for (const effectBlocked of [false,true]) {
    const context={card:{busy,effectBlocked}};
    context.card.effectBusy=busy||effectBlocked;
    const check=card.match(/id: checkButton;[^\n]*enabled: ([^;]+);/)[1];
    assert.equal(require('node:vm').runInNewContext(check,context),!busy);
    const install=card.slice(card.indexOf('id: installButton'));
    const enabled=install.match(/enabled: ([^\n]+)/)[1];
    context.visible=true;context.card.terminalOpened=false;
    assert.equal(require('node:vm').runInNewContext(enabled,context),!busy&&!effectBlocked);
  }
});
test('partial DNS entry is explicit and never substitutes for generic broker recovery', () => {
  for (const present of [false,true]) {
    const facts=state.inventory('needs_dns_setup\t'+(present?'present':'missing')+'\n',0);
    assert.equal(state.missingAction(facts),present?'resume-dns':'');
    assert.equal(state.canRunAction(facts,'resume-dns'),present);
    assert(!state.canRunAction(facts,'install'));
    assert(!state.canRunAction({state:'needs_broker',coreInstalled:present},'resume-dns'));
    assert(state.bootstrapRequired(facts,true,true,false,false,false));
  }
  assert(card.includes('"resume_dns"'));
  assert(card.includes('card.facts.state === "needs_dns_setup"'));
});
test('Later closes only the panel and never marks onboarding complete or hides reminders persistently', () => {
  assert(page.includes('onClicked: setup.closeRequested()'));
  assert(!/completeOnboarding|onboardingComplete|settings\.[\w]+\s*=/.test(page));
});
test('prior stable upgrade is explicit and help remains navigational', () => {
  const vm=require('node:vm');
  for(const present of [false,true]) {
    const facts=state.inventory('needs_update\t'+(present?'present':'missing')+'\n',0);
    assert.equal(facts.state,'needs_update');
    assert(!state.appMissing(facts));assert(state.canInstall(facts.state));
    assert.equal(state.missingAction(facts),'upgrade');
    assert(state.canRunAction(facts,'upgrade'));
    assert(state.canRunAction(facts,'update-help'));
    for(const action of ['install','shell','start-app','restore-enrollment']) assert(!state.canRunAction(facts,action));
    const start=page.indexOf('  function install(action, returnFocus)'),end=page.indexOf('\n  }',start)+4;
    let guides=0;
    const c=vm.createContext({SetupState:state,facts,busy:false,launching:false,terminalOpened:false,
      nativeEffectBlocked:false,launch:{running:false},startApp:{running:false},guide(){guides++}});
    vm.runInContext(page.slice(start,end),c);c.install('update-help');
    assert.equal(guides,1);assert(!c.launch.running);assert(!c.startApp.running);
    c.nativeEffectBlocked=true;c.install('upgrade');assert(!c.launch.running);
    c.install('update-help');assert.equal(guides,2);
    c.nativeEffectBlocked=false;c.install('upgrade');assert(c.launch.running);
    assert.equal(c.launchAction,'upgrade');assert(!c.startApp.running);
    c.launch.running=false;c.install('upgrade');assert(!c.launch.running);
  }
  for(const locale of ['en','ru'])for(const key of ['update_title','update_help','update_components','needs_update']) {
    const text=i18n.translate('setup.'+key,locale,{});
    assert(text.length>0&&text.length<=512&&!text.includes('Missing translation'));
  }
  assert(card.includes('id: updateHelpButton'));
  assert(card.includes('onClicked: card.installRequested("update-help", updateHelpButton)'));
  assert(card.includes('onClicked: card.installRequested("upgrade", updateButton)'));
  for(const busy of [false,true])for(const effectBlocked of [false,true])for(const terminalOpened of [false,true]) {
    const context={visible:true,card:{busy,effectBlocked,effectBusy:busy||effectBlocked,terminalOpened}};
    const update=card.slice(card.indexOf('id: updateButton')).match(/enabled: ([^\n]+)/)[1];
    const help=card.slice(card.indexOf('id: updateHelpButton')).match(/enabled: ([^\n]+)/)[1];
    assert.equal(vm.runInNewContext(update,context),!busy&&!effectBlocked&&!terminalOpened);
    assert.equal(vm.runInNewContext(help,context),!busy&&!terminalOpened);
  }
  for(const value of ['ready','needs_package','needs_activation','needs_dns_setup','needs_attention'])
    assert(!state.canRunAction({state:value,coreInstalled:true},'upgrade'));
});
console.log(`${count} marketplace setup contracts passed`);
