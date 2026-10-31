// SPDX-License-Identifier: MIT
const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const panel=fs.readFileSync(__dirname+'/../plugin/Panel.qml','utf8');
const service=fs.readFileSync(__dirname+'/../plugin/Service.qml','utf8');
const wizard=fs.readFileSync(__dirname+'/../plugin/OnboardingWizard.qml','utf8');
const parser=vm.createContext({});vm.runInContext(fs.readFileSync(__dirname+'/../plugin/NativeSnapshot.js','utf8'),parser);
let count=0;function test(name,f){try{f();count++}catch(e){e.message=name+': '+e.message;throw e}}
test('successful core step contains no recheck, installation help or repair navigation',()=>{
 const state=require('../plugin/SetupState.js');
 const controls=['onboardingCoreRefresh','onboardingSetupGuide','onboardingComponentSetup','onboardingDnsHelperIntro','onboardingOtherVpnAdvice'];
 for(const nativeCoreState of ['ready','missing','update','checking','attention','terminal'])
  for(const delivery of ['current','stale','stale_manual'])for(const firstRun of [false,true]) {
   const facts={state:'ready',coreInstalled:true,delivery};
   const c={SetupState:state,wizard:{nativeContext:true,nativeCoreState,nativeSetupFacts:facts,firstRun}};
   for(const name of controls){
    const block=wizard.slice(wizard.indexOf('objectName: "'+name+'"'));
    assert(block.startsWith('objectName:'),name+' must name the actual control');
    const binding=block.slice(0,block.indexOf('\n            }')).match(/visible: ([^\n]+)/);
    assert(binding,name+' requires an explicit visibility decision');
    const shown=vm.runInNewContext(binding[1],c);
    const expected=!['ready','checking'].includes(nativeCoreState)&&(name!=='onboardingOtherVpnAdvice'||firstRun);
    assert.equal(shown,expected,name+' in '+nativeCoreState+'/'+delivery);
   }
  }
});
test('footer separates back from forward and names completion without a profile',()=>{
 const footer=wizard.slice(wizard.indexOf('id: navigation'));
 assert(footer.indexOf('id: backButton') < footer.indexOf('id: forward'));
 assert(footer.includes('Qt.AlignLeft'));assert(footer.includes('Qt.AlignRight'));
 assert(footer.includes('columns: width >= backButton.implicitWidth + forward.implicitWidth'));
 assert(!footer.includes('text: wizard.textFor("common.check_again")'));
 const i18n=require('../plugin/I18n.js');
 assert.equal(i18n.translate('onboarding.finish_later','en'),'Finish without a profile');
 assert.equal(i18n.translate('onboarding.finish_later','ru'),'Завершить без профиля');
 for(const locale of ['en','ru'])for(const key of ['native.onboarding.empty_next','native.onboarding.ready_next','onboarding.saving_routing'])assert(!i18n.translate(key,locale).includes('Missing translation'));
 assert(!i18n.translate('native.onboarding.ready_next','en').includes('connected'));
});
test('routing progress blocks forward navigation until confirmed outcome',()=>{
 const control=wizard.slice(wizard.indexOf('objectName: "onboardingRoutingContinue"'));
 const enabled=control.match(/enabled: ([^\n]+)/)[1];
 for(const busy of [false,true])for(const routingSavePending of [false,true])
  assert.equal(vm.runInNewContext(enabled,{wizard:{busy,routingSavePending}}),!busy&&!routingSavePending);
 assert(panel.includes('routingSavePending: root.nativeOnboardingPresetPending !== ""'));
 const progress=wizard.slice(wizard.indexOf('objectName: "onboardingRoutingProgress"'));
 const visible=progress.match(/visible: ([^\n]+)/)[1];
 for(const step of [1,2,3])for(const routingSavePending of [false,true])for(const routingSaveUnknown of [false,true])
  assert.equal(vm.runInNewContext(visible,{wizard:{step,routingSavePending,routingSaveUnknown}}),step===2&&routingSavePending&&!routingSaveUnknown);
 assert(panel.includes('routingSaveUnknown: vless.nativeOutcomeUnknown'));
});
test('DNS helper is introduced before setup labels and VPN advice is first-run only',()=>{
 const card=fs.readFileSync(__dirname+'/../plugin/RequiredComponents.qml','utf8');
 const page=fs.readFileSync(__dirname+'/../plugin/SetupPage.qml','utf8');
 assert(card.indexOf('objectName: "setupDnsHelperIntro"') < card.indexOf('text: card.tr("core_missing")'));
 const properties=card.match(/readonly property (?:bool|string) (?:appMissing|pairUpdate|updateReview|operationalStep|showPreparation|optionalUpdate): [^\n]+/g);
 const state=require('../plugin/SetupState.js');
 for(const delivery of ['current','stale','stale_manual']) for(const operational of ['ready','needs_broker_stopped','needs_dns_setup','needs_runtime_start','needs_selection']) {
  const c={SetupState:state,facts:{delivery,state:operational,coreInstalled:true}};c.card=c;
  for(const property of properties){const [,name,expression]=property.match(/property (?:bool|string) ([^:]+): (.*)/);c[name]=vm.runInNewContext(expression,c);}
  assert.equal(c.showPreparation,operational!=='ready');
 }
 assert(card.indexOf('id: prepareButton') < card.indexOf('id: pairedUpdateButton'));
 assert(wizard.indexOf('objectName: "onboardingDnsHelperIntro"') < wizard.indexOf('objectName: "onboardingCoreState"'));
 const warning=wizard.slice(wizard.indexOf('objectName: "onboardingOtherVpnAdvice"'));
 const visible=warning.match(/visible: ([^\n]+)/)[1];
 for(const firstRun of [true,false]) for(const nativeContext of [true,false])for(const nativeCoreState of ['ready','missing','attention'])
  assert.equal(vm.runInNewContext(visible,{wizard:{firstRun,nativeContext,nativeCoreState}}),firstRun && nativeContext && nativeCoreState!=='ready');
 assert(panel.includes('firstRun: vless.nativeOwner && vless.nativeSnapshot !== null && !vless.nativeSnapshot.onboardingComplete'));
 assert(page.includes('(setup.state === "needs_package" || setup.state === "needs_activation")'));
 const i18n=require('../plugin/I18n.js');
 assert.equal(i18n.translate('onboarding.step.connection','en'),'VPN profile');
 assert.equal(i18n.translate('onboarding.step.connection','ru'),'VPN-профиль');
 for(const locale of ['en','ru']) for(const key of ['dns_helper_intro','other_vpn_advice']) {
  const text=i18n.translate('setup.'+key,locale);assert(text.length < 180);
  assert(!/Missing translation|broker|enrollment|брокер|регистраци/.test(text));
 }
});
function functions(source,c,names){for(const name of names){const start=source.indexOf('  function '+name+'('),end=source.indexOf('\n  }',start)+4;assert(start>=0);vm.runInContext(source.slice(start,end),c)}}
test('component review never calls an in-flight core check a failure',()=>{
 const page=fs.readFileSync(__dirname+'/../plugin/SetupPage.qml','utf8');
 const start=page.lastIndexOf('    PlainText {',page.indexOf('"review_core_attention"'));
 const block=page.slice(start,page.indexOf('\n    }',start));
 const visible=block.match(/visible: ([\s\S]*?)\n      text:/)[1].trim();
 const text=block.match(/text: ([^\n]+)/)[1];
 for(const componentReview of [false,true])for(const needsAttention of [false,true])
  for(const terminalOpened of [false,true])for(const reviewCoreState of ['checking','ready','attention','terminal','missing','update']) {
   const setup={componentReview,needsAttention,terminalOpened,reviewCoreState,tr:key=>key};
   const shown=vm.runInNewContext(visible,{setup});
   const key=vm.runInNewContext(text,{setup});
   assert.equal(shown,componentReview&&!needsAttention&&(terminalOpened||!['ready','checking'].includes(reviewCoreState)));
   if(shown&&terminalOpened)assert.equal(key,'terminal');
   if(shown&&key==='review_core_attention')assert.notEqual(reviewCoreState,'checking');
  }
 const sequence=['terminal','checking','ready','checking','attention'];
 const seen=sequence.map(reviewCoreState=>{
  const setup={componentReview:true,needsAttention:false,terminalOpened:reviewCoreState==='terminal',reviewCoreState,tr:key=>key};
  return vm.runInNewContext(visible,{setup})?vm.runInNewContext(text,{setup}):null;
 });
 assert.deepEqual(seen,['terminal',null,null,null,'review_core_attention']);
});
test('successful component review removes recheck/help without hiding unsettled recovery controls',()=>{
 const page=fs.readFileSync(__dirname+'/../plugin/SetupPage.qml','utf8');
 const ready=page.match(/readonly property bool reviewReady: ([\s\S]*?)\n  readonly property string script:/)[1].trim();
 const expressions=['reviewCheckButton','reviewGuideButton'].map(id=>{
  const block=page.slice(page.indexOf('id: '+id),page.indexOf('\n    }',page.indexOf('id: '+id)));
  return block.match(/visible: ([^\n]+)/)[1];
 });
 for(const state of ['ready','checking','needs_attention','needs_dns_setup','needs_pair_update'])
  for(const reviewCoreState of ['ready','checking','attention','terminal','missing','update'])
   for(const busy of [false,true])for(const terminalOpened of [false,true])
    for(const nativeEffectBlocked of [false,true])for(const componentReview of [false,true])
     for(const needsAttention of [false,true]) {
      const setup={state,reviewCoreState,busy,terminalOpened,nativeEffectBlocked,componentReview,needsAttention};
      setup.reviewReady=vm.runInNewContext(ready,{setup});
      const complete=state==='ready'&&reviewCoreState==='ready'&&!busy&&!terminalOpened&&!nativeEffectBlocked;
      assert.equal(setup.reviewReady,complete);
      for(const expression of expressions)
       assert.equal(vm.runInNewContext(expression,{setup}),componentReview&&!needsAttention&&!complete);
     }
 const acknowledgement=page.slice(page.indexOf('id: reviewTerminalButton'));
 const shown=acknowledgement.match(/visible: ([^\n]+)/)[1];
 assert(vm.runInNewContext(shown,{setup:{componentReview:true,needsAttention:false,terminalOpened:true,reviewReady:false}}));
});
function context(){
 const c=vm.createContext({vless:{nativeOwner:true,nativeSnapshotFailed:false,nativePending:null,
  nativeImportBusy:false,nativeActionCode:'',nativeMetadataErrorAction:'',nativeMetadataErrorCode:'',
  nativeSnapshot:{instanceId:'instance',revision:7,onboardingComplete:false,routing:{storedPreset:''},profiles:[]},
  completeOnboarding:()=>true,useRoutingPreset:()=>true,
  refreshNativeDesktopCapabilities(){this.helperReads=(this.helperReads||0)+1}},
  setupPage:{checks:0,check(){this.checks++}},setupFlick:{contentY:90},componentSetupReview:false,
  page:'settings',opened:true,onboardingDismissed:false,nativeOnboardingPresetPending:'',nativeOnboardingCompletionPending:null,nativeOnboardingCompletionUnconfirmed:false,
  onboardingCompletionTimeout:{stop(){},restart(){}},
  onboardingWizard:{visible:false,step:1,openAt(step){this.visible=true;this.step=step},dismiss(){this.visible=false}},
  removeReview:{visible:false,terminalOpened:false},
  importDialog:{visible:false},importEntryPrompt:{visible:false},bundleImportPrompt:{visible:false},subscriptionPrompt:{visible:false},keyCatcher:{forceActiveFocus(){}},Qt:{callLater:f=>f()}});
 c.root=c;functions(panel,c,['openOnboarding','syncNativeOnboarding','chooseOnboardingPreset',
  'dismissOnboarding','finishOnboarding','markOnboardingCompletionUnconfirmed','reviewComponentSetup']);return c;
}
test('first use opens existing wizard, completed and dismissed states do not loop',()=>{
 const c=context();c.syncNativeOnboarding();assert(c.onboardingWizard.visible);assert.equal(c.onboardingWizard.step,1);
 assert.equal(c.vless.helperReads,1);c.syncNativeOnboarding();assert.equal(c.vless.helperReads,1);
 c.dismissOnboarding();c.syncNativeOnboarding();assert(!c.onboardingWizard.visible);
 c.onboardingDismissed=false;c.vless.nativeSnapshot.onboardingComplete=true;c.syncNativeOnboarding();assert(!c.onboardingWizard.visible);
 assert(c.openOnboarding(3));assert.equal(c.onboardingWizard.step,3); // Explicit Settings re-entry.
});
test('component review is navigation with fresh inventory, no install or connection effect',()=>{
 const c=context();c.openOnboarding(1);const before=c.setupPage.checks;
 c.setupPage.install=()=>{throw new Error('navigation must not install')};
 c.vless.completeOnboarding=()=>{throw new Error('navigation must not complete onboarding')};
 c.reviewComponentSetup();assert(c.componentSetupReview);assert.equal(c.page,'main');
 assert(!c.onboardingWizard.visible);assert(c.onboardingDismissed);assert.equal(c.setupFlick.contentY,0);
 assert.equal(c.setupPage.checks,before+1);c.syncNativeOnboarding();assert(!c.onboardingWizard.visible);
 assert(panel.includes('onComponentSetupRequested: root.reviewComponentSetup()'));
 assert(panel.includes('nativeSetupFacts: setupPage.facts'));
});
test('a removal review or unsettled terminal cannot lose focus to late onboarding facts',()=>{
 for(const [visible,terminalOpened] of [[true,false],[false,true],[true,true]]) {
  const c=context();c.removeReview={visible,terminalOpened};
  c.syncNativeOnboarding();assert(!c.onboardingWizard.visible);
  assert.equal(c.vless.helperReads,undefined);assert.equal(c.setupPage.checks,0);
  assert.equal(c.openOnboarding(3),false);assert(!c.onboardingWizard.visible);
  assert.equal(c.nativeOnboardingCompletionPending,null);
 }
});
test('render fixtures refuse stale captures after an unadmitted scenario',()=>{
 for(const file of ['onboarding-visual/shell.qml','setup-visual/shell.qml']) {
  const source=fs.readFileSync(__dirname+'/'+file,'utf8');
  const capture=source.slice(source.indexOf('function capture('),source.indexOf('function result()'));
  assert(capture.includes('["ready", "captured"].indexOf(review.result) < 0'));
  assert(/result = "refused"; return "refused"/.test(source));
 }
});
test('ready, missing, update and unknown core states do not borrow each other actions',()=>{
 const state=require('../plugin/SetupState.js');
 const ready={installed:true,version:'1.19.31',tunDevice:'present',fileNetworkCapabilities:'present'};
 const packageFacts={state:'ready',coreInstalled:true};
 assert.equal(state.coreReview(ready,packageFacts,false,false),'ready');
 for(const core of [null,{...ready,installed:false},{...ready,version:null},{...ready,tunDevice:'missing'},{...ready,fileNetworkCapabilities:'missing'}]) {
  assert.notEqual(state.coreReview(core,packageFacts,false,false),'ready');
 }
 assert.equal(state.coreReview(ready,packageFacts,true,false),'checking');
 assert.equal(state.coreReview(ready,packageFacts,false,true),'attention');
 assert.equal(state.coreReview(ready,packageFacts,false,false,true),'terminal');
 assert.equal(state.coreReview(ready,{state:'needs_update',coreInstalled:false},false,false,true),'terminal');
 assert(panel.includes('nativeSetupTerminalOpened: setupPage.terminalOpened'));
 assert.equal(state.coreReview(ready,{state:'checking',coreInstalled:null},false,false),'checking');
 for(const facts of [{state:'needs_attention',coreInstalled:true},{state:'needs_dns_setup',coreInstalled:true}])
  assert.equal(state.coreReview(ready,facts,false,false),'attention');
 assert.equal(state.coreReview(ready,{state:'needs_update',coreInstalled:false},false,false),'update');
 assert.equal(state.coreReview(null,{state:'needs_package',coreInstalled:false},false,false),'missing');
 assert.equal(state.coreReview(null,{state:'needs_companion',coreInstalled:false},false,false),'missing');
 // A missing companion beside an existing app still has no admitted automatic install.
 assert.equal(state.missingAction({state:'needs_companion',coreInstalled:false}),'update-help');
 assert(!state.canRunAction({state:'needs_companion',coreInstalled:false},'update-pair'));
 for(const name of ['needs_pair_update','needs_pair_update_manual'])
  assert.equal(state.coreReview(ready,{state:name,coreInstalled:true},false,false),'update');
 const button=wizard.slice(wizard.indexOf('objectName: "onboardingCoreContinue"'));
 const enabled=button.match(/enabled: ([^\n]+)/)[1];
 for(const nativeCoreState of ['ready','missing','update','checking','attention','terminal']) {
  const c=vm.createContext({wizard:{nativeContext:true,nativeCoreState,nativeCanContinue:true,busy:false}});
  assert.equal(vm.runInContext(enabled,c),nativeCoreState==='ready');
  c.wizard.busy=true;assert(!vm.runInContext(enabled,c));
  c.wizard.busy=false;c.wizard.nativeCanContinue=false;assert(!vm.runInContext(enabled,c));
 }
 const i18n=require('../plugin/I18n.js');for(const locale of ['en','ru'])for(const key of ['core_ready','core_ready_scope','core_checking','core_missing','core_update','core_attention','core_terminal','terminal_help','review_install','review_update','review_setup'])
  assert(!i18n.translate('native.onboarding.'+key,locale,{}).includes('Missing translation'));
});
test('compatible stale delivery keeps ready Continue without update distractions',()=>{
 const state=require('../plugin/SetupState.js');
 const core={installed:true,version:'1.19.31',tunDevice:'present',fileNetworkCapabilities:'present'};
 const continueBlock=wizard.slice(wizard.indexOf('objectName: "onboardingCoreContinue"'));
 const enabled=continueBlock.match(/enabled: ([^\n]+)/)[1];
 const reviewBlock=wizard.slice(wizard.indexOf('objectName: "onboardingComponentSetup"'));
 const visible=reviewBlock.match(/visible: ([^\n]+)/)[1];
 for(const delivery of ['stale','stale_manual']) {
  const nativeSetupFacts={state:'ready',coreInstalled:true,delivery};
  const nativeCoreState=state.coreReview(core,nativeSetupFacts,false,false,false);
  const c=vm.createContext({SetupState:state,wizard:{nativeContext:true,nativeSetupFacts,nativeCoreState,nativeCanContinue:true,busy:false}});
  assert(vm.runInContext(enabled,c));
  assert(!vm.runInContext(visible,c));
  c.wizard.nativeCoreState=state.coreReview(core,nativeSetupFacts,false,false,true);
  assert(!vm.runInContext(enabled,c));
 }
});
test('native helper bindings keep read, write and picker capabilities independent',()=>{
 const c=vm.createContext({nativeContext:true,nativeDesktopFacts:null,filePicker:{available:true},nativeCanContinue:true,busy:false});c.wizard=c;
 for(const name of ['clipboardReady','pickerReady','commandCopyReady']) {
  const expression=wizard.match(new RegExp('readonly property bool '+name+': ([^\\n]+)'))[1];
  Object.defineProperty(c,name,{get:()=>vm.runInContext(expression,c)});
 }
 function enabled(name){const block=wizard.slice(wizard.indexOf('objectName: "'+name+'"'));const expression=block.match(/enabled: ([^\n]+)/)[1];return vm.runInContext(expression,c)}
 for(const read of [false,true])for(const write of [false,true])for(const picker of [null,'zenity','kdialog','yad']) {
  c.nativeDesktopFacts={clipboardReadAvailable:read,clipboardWriteAvailable:write,filePicker:picker};
  assert.equal(enabled('onboardingPaste'),read);assert.equal(enabled('onboardingFile'),!!picker);assert.equal(enabled('onboardingCopyCommand'),write);
 }
 c.nativeDesktopFacts=null;
 for(const name of ['onboardingPaste','onboardingFile','onboardingCopyCommand'])assert.equal(enabled(name),false);
 c.nativeDesktopFacts={clipboardReadAvailable:true,clipboardWriteAvailable:true,filePicker:'zenity'};
 c.busy=true;for(const name of ['onboardingPaste','onboardingFile','onboardingCopyCommand'])assert.equal(enabled(name),false);
 c.busy=false;c.nativeCanContinue=false;assert.equal(enabled('onboardingPaste'),false);assert.equal(enabled('onboardingFile'),false);
 c.nativeContext=false;assert.equal(enabled('onboardingPaste'),true);assert.equal(enabled('onboardingFile'),true);
});
test('native onboarding wires fresh inventory and local help without installing anything',()=>{
 assert(panel.includes('nativeDesktopFacts: vless.nativeDesktopCapabilities'));
 assert(panel.includes('nativeDesktopLoading: vless.nativeDesktopLoading'));
 const refresh=panel.slice(panel.indexOf('onRefreshRequested: {',panel.indexOf('OnboardingWizard {')),panel.indexOf('onPresetChosen:',panel.indexOf('OnboardingWizard {')));
 assert(refresh.includes('vless.refreshNativeDesktopCapabilities()'));
 assert(refresh.includes('onSetupGuideRequested: function(returnFocus) { root.openHelp("install", returnFocus) }'));
 assert(!refresh.includes('execDetached'));assert(!refresh.includes('sudo'));assert(!refresh.includes('connect('));
 assert(wizard.includes('command: "omarchy pkg add wl-clipboard"'));
 assert(wizard.includes('command: "omarchy pkg add zenity"'));
 assert(wizard.includes('wizard.nativeDesktopFacts !== null && !wizard.pickerReady'));
 assert(wizard.includes('wizard.textFor("native.onboarding.copy_unavailable")'));
});
test('new onboarding guidance has complete safe EN/RU text',()=>{
 const i18n=require('../plugin/I18n.js');
 for(const locale of ['en','ru'])for(const key of ['setup_help','setup_guide','import_help','picker_missing','clipboard_missing','install_clipboard','copy_unavailable','helpers_unknown']) {
  const value=i18n.translate('native.onboarding.'+key,locale,{});assert(value);assert(!value.includes('Missing translation'));assert(!/[<>]/.test(value));
 }
});
test('missing/stale/closed/pending metadata never opens first use over another operation',()=>{
 for(const change of [c=>c.vless.nativeSnapshot=null,c=>c.vless.nativeSnapshotFailed=true,c=>c.vless.nativeStatusRefreshing=true,c=>c.opened=false,c=>c.vless.nativePending={},c=>c.vless.nativeImportBusy=true,c=>c.importDialog.visible=true,c=>c.importEntryPrompt.visible=true,c=>c.bundleImportPrompt.visible=true,c=>c.subscriptionPrompt.visible=true]){const c=context();change(c);c.syncNativeOnboarding();assert(!c.onboardingWizard.visible)}
});
test('quick reopen waits for fresh completion rather than cached unfinished onboarding',()=>{
 const c=context();c.vless.nativeStatusRefreshing=true;
 c.syncNativeOnboarding();assert(!c.onboardingWizard.visible);
 c.vless.nativeSnapshot.onboardingComplete=true;
 c.syncNativeOnboarding();assert(!c.onboardingWizard.visible);
 c.vless.nativeStatusRefreshing=false;c.syncNativeOnboarding();assert(!c.onboardingWizard.visible);
 assert.equal(c.vless.helperReads,undefined);
 // A genuinely fresh unfinished store still opens once the read settles.
 c.vless.nativeSnapshot.onboardingComplete=false;c.vless.nativeStatusRefreshing=true;
 c.syncNativeOnboarding();assert(!c.onboardingWizard.visible);
 c.vless.nativeStatusRefreshing=false;c.syncNativeOnboarding();assert(c.onboardingWizard.visible);
 c.dismissOnboarding();c.vless.nativeStatusRefreshing=true;c.vless.nativeSnapshotFailed=true;
 c.onboardingDismissed=false;c.vless.nativeStatusRefreshing=false;c.syncNativeOnboarding();assert(!c.onboardingWizard.visible);
 // Explicit Settings review remains available even after completion.
 c.vless.nativeSnapshotFailed=false;c.vless.nativeSnapshot.onboardingComplete=true;
 assert(c.openOnboarding(1));assert(c.onboardingWizard.visible);
 assert(service.includes('readonly property bool nativeStatusRefreshing: statusProcess.running || !_nativeStatusSettled'));
 assert(panel.includes('function onNativeStatusRefreshingChanged() { Qt.callLater(root.syncNativeOnboarding) }'));
});
test('preset step advances only after confirmed snapshot, not initial click or rejection',()=>{
 const c=context();c.openOnboarding(2);c.vless.nativePending={};assert(c.chooseOnboardingPreset('china-cn-direct'));assert.equal(c.onboardingWizard.step,2);
 c.syncNativeOnboarding();assert.equal(c.onboardingWizard.step,2);c.vless.nativePending=null;c.syncNativeOnboarding();assert.equal(c.onboardingWizard.step,2);
 c.vless.nativeSnapshot.routing.storedPreset='china-cn-direct';c.syncNativeOnboarding();assert.equal(c.onboardingWizard.step,3);
 c.openOnboarding(2);c.chooseOnboardingPreset('iran-ir-direct');c.vless.nativeActionCode='permission_denied';c.syncNativeOnboarding();assert.equal(c.onboardingWizard.step,2);assert.equal(c.nativeOnboardingPresetPending,'');
 c.vless.nativeActionCode='';c.chooseOnboardingPreset('iran-ir-direct');c.vless.nativeMetadataErrorAction='routing-preset';c.vless.nativeMetadataErrorCode='permission_denied';c.syncNativeOnboarding();assert.equal(c.onboardingWizard.step,2);assert.equal(c.nativeOnboardingPresetPending,'');
 c.vless.nativeMetadataErrorCode='';c.chooseOnboardingPreset('iran-ir-direct');c.vless.nativeOutcomeUnknown=true;c.vless.nativePending={action:'routing-preset'};c.syncNativeOnboarding();assert.equal(c.nativeOnboardingPresetPending,'iran-ir-direct');assert.equal(c.onboardingWizard.step,2);
 assert(panel.includes('function onNativeMetadataErrorCodeChanged() { root.syncNativeOnboarding() }'));
});
test('finish keeps the wizard until persisted completion is confirmed',()=>{
 const c=context();c.openOnboarding(3);c.vless.completeOnboarding=()=>false;c.finishOnboarding();assert(c.onboardingWizard.visible);
 c.vless.completeOnboarding=()=>{c.vless.nativePending={action:'onboarding-complete'};return true};
 c.finishOnboarding();assert(c.onboardingWizard.visible);assert.equal(c.onboardingWizard.step,3);
 assert.equal(c.vless.nativeSnapshot.onboardingComplete,false);
 c.syncNativeOnboarding();assert(c.onboardingWizard.visible);
 // Acknowledgement precedes its persisted snapshot; it is not completion.
 c.vless.nativePending=null;c.syncNativeOnboarding();assert(c.onboardingWizard.visible);
 c.vless.nativeSnapshot.onboardingComplete=true;c.syncNativeOnboarding();
 assert(!c.onboardingWizard.visible);assert(c.onboardingDismissed);
 // Ordinary close/reopen clears session dismissal, not the native completion.
 c.opened=false;c.syncNativeOnboarding();c.opened=true;c.onboardingDismissed=false;
 c.syncNativeOnboarding();assert(!c.onboardingWizard.visible);
});
test('native completion shares exact pending fence and zero private arguments',()=>{
 const c=vm.createContext({nativeOwner:true,nativeCanAct:true,nativeSnapshot:{instanceId:'instance',revision:7},_nativeOperationSerial:0,backendPath:'/synthetic/backend.sh',nativeActionProcess:{},nativeActionCode:'',nativeSubscriptionDraft:null,nativeSubscriptionCode:'',nativeOutcomeUnknown:false,nativePending:null,nativeRefusalVerification:null,nativeRefusalVerificationTimeout:{stop(){}}});
 functions(service,c,['requestNativeAction','completeOnboarding']);assert(c.completeOnboarding());assert.equal(c.nativePending.action,'onboarding-complete');assert.equal(c.nativePending.instanceId,'instance');assert.equal(c.nativePending.revision,7);assert.equal(c.nativePending.command.length,6);assert.equal(c.nativePending.input,undefined);
 assert.equal(c.nativePending.command[2],'native-onboarding-complete');c.nativeCanAct=false;assert(!c.completeOnboarding());
 assert.equal(parser.parseActionExit('',{action:'onboarding-complete'},74).code,'invalid_argument');
});
function completionResponse() {
 return {api:'omavless.control',version:1,id:'request',ok:true,revision:8,
  result:{schemaVersion:1,instanceId:'instance',operationId:'operation',action:'onboarding-complete',applied:true}};
}
function completionPending() {return {instanceId:'instance',operationId:'operation',action:'onboarding-complete',revision:7}}
test('completion success and already-complete acknowledgement are recognized with exact fences',()=>{
 for(const revision of [7,8]) {
  const frame=completionResponse();frame.revision=revision;
  assert.equal(parser.parseActionExit(JSON.stringify(frame),completionPending(),0)?.ok,true);
 }
 for(const change of [p=>p.result.instanceId='other',p=>p.result.operationId='other',p=>p.result.action='disconnect',
  p=>p.result.applied=false,p=>p.revision=6,p=>p.result.extra='private']) {
  const frame=completionResponse();change(frame);assert.equal(parser.parseActionExit(JSON.stringify(frame),completionPending(),0),null);
 }
 const pending=completionPending();pending.action='unrecognized';
 assert.equal(parser.parseActionExit(JSON.stringify(completionResponse()),pending,0),null);
});
test('completion rejection keeps only the safe code and malformed replies remain unknown',()=>{
 const frame={api:'omavless.control',version:1,id:'request',ok:false,revision:7,
  error:{code:'conflict',message:'https://private.invalid/password?key=secret',retryable:false}};
 const result=parser.parseActionExit(JSON.stringify(frame),completionPending(),2);
 assert.equal(result?.ok,false);assert.equal(result.code,'conflict');assert(!JSON.stringify(result).includes('private'));
 assert.equal(parser.parseActionExit('',completionPending(),73),null);
});
test('real Service exit handler clears completion pending and refreshes confirmed state',()=>{
 const process=service.slice(service.indexOf('    id: nativeActionProcess'));
 const start=process.indexOf('    onExited: function(exitCode) {');
 const end=process.indexOf('\n    }',start)+6;
 const handler=process.slice(start,end).replace('    onExited: function(exitCode)', 'function exited(exitCode)');
 function run(raw,code) {
  const c=vm.createContext({NativeSnapshot:parser,nativeActionStdout:{text:raw},nativePending:completionPending(),
   nativeOutcomeUnknown:false,nativeActionCode:'',nativeObservation:{},refreshes:0,
   nativeMetadataTransitionTimeout:{restart(){},stop(){}},
   finishNativeEditorAction(){},finishNativeSubscriptionAction(){},finishNativeRoutingAction(){},
   refreshAfterChange(){this.refreshes++}});c.root=c;vm.runInContext(handler,c);c.exited(code);return c;
 }
 const c=run(JSON.stringify(completionResponse()),0);
 assert.equal(c.nativePending,null);assert.equal(c.nativeOutcomeUnknown,false);assert.equal(c.nativeActionCode,'');
 assert.equal(c.nativeObservation,null);assert.equal(c.refreshes,1);
 const unknown=run('',73);assert(unknown.nativePending);assert.equal(unknown.nativeOutcomeUnknown,true);
});
test('real routing rejection clears saving but never advances on the old matching preset',()=>{
 const process=service.slice(service.indexOf('    id: nativeActionProcess'));
 const start=process.indexOf('    onExited: function(exitCode) {'),end=process.indexOf('\n    }',start)+6;
 const handler=process.slice(start,end).replace('    onExited: function(exitCode)', 'function exited(exitCode)');
 const frame={api:'omavless.control',version:1,id:'request',ok:false,revision:7,error:{code:'permission_denied',message:'Not permitted',retryable:false}};
 const c=context();c.openOnboarding(2);c.nativeOnboardingPresetPending='iran-ir-direct';
 const v=c.vless;v.nativeSnapshot.routing.storedPreset='iran-ir-direct';
 v.nativePending={instanceId:'instance',operationId:'operation',action:'routing-preset',revision:7};
 const notifications=[];
 const notifyBody=panel.match(/function onNativePendingChanged\(\) \{ ([^\n]+) \}/)[1];
 let pendingValue=v.nativePending;
 Object.defineProperty(v,'nativePending',{configurable:true,get(){return pendingValue},set(value){pendingValue=value;vm.runInNewContext(notifyBody,{root:c,Qt:{callLater:f=>notifications.push(f)}});}});
 Object.assign(v,{NativeSnapshot:parser,nativeActionStdout:{text:JSON.stringify(frame)},nativeOutcomeUnknown:false,
  nativeMetadataTransitionTimeout:{restart(){},stop(){}},finishNativeEditorAction(){},finishNativeSubscriptionAction(){},finishNativeRoutingAction(){},refreshAfterChange(){}});
 const action=vm.createContext(v);action.root=action;vm.runInContext(handler,action);action.exited(2);
 assert.equal(v.nativePending,null);assert.equal(v.nativeActionCode,'');assert.equal(v.nativeMetadataErrorAction,'routing-preset');assert.equal(v.nativeMetadataErrorCode,'permission_denied');
 while(notifications.length)notifications.shift()();
 c.syncNativeOnboarding();assert.equal(c.nativeOnboardingPresetPending,'');assert.equal(c.onboardingWizard.step,2);
 // Unknown retains the original request and must not show a still-running save.
 v.nativeMetadataErrorCode='';v.nativePending={instanceId:'instance',operationId:'operation',action:'routing-preset',revision:7};v.nativeActionStdout.text='';c.nativeOnboardingPresetPending='iran-ir-direct';action.exited(73);
 assert(v.nativeOutcomeUnknown);assert(v.nativePending);while(notifications.length)notifications.shift()();c.syncNativeOnboarding();assert.equal(c.nativeOnboardingPresetPending,'iran-ir-direct');assert.equal(c.onboardingWizard.step,2);
});
test('native wizard preserves layout and explicitly avoids legacy grants/readiness claims',()=>{
 assert(wizard.includes('property bool nativeContext: false'));
 assert(wizard.includes('visible: !wizard.nativeContext && wizard.coreSetup.installed && !wizard.coreSetup.tunReady'));
 assert(wizard.includes('enabled: (wizard.nativeContext ? wizard.nativeCoreState === "ready" && wizard.nativeCanContinue : wizard.coreSetup.tunReady) && !wizard.busy'));
 assert(!wizard.includes('wizard.textFor("native.core.scope")'));
 assert(!wizard.includes('wizard.textFor("native.core.tun."'));
 assert(wizard.includes('wizard.textFor("native.core.version", {version:'));
 assert(wizard.includes('wizard.profiles.length > 0 ? "native.onboarding.ready_next" : "native.onboarding.empty_next"'));
 assert(wizard.includes('"native.onboarding.profiles"'));
 assert(panel.includes('vless.startNativeImport("clipboard")'));assert(panel.includes('vless.startNativeImport("file")'));
 assert(panel.includes('(root.page === "settings" || onboardingWizard.visible || root.componentSetupReview)'));
 assert(panel.includes('nativeOnboardingSetting.focusTarget'));
 assert(!wizard.includes('nativeCoreFacts.tunReady'));
});
test('EN/RU scope and imported count never borrow connected/ready wording',()=>{
 const i18n=require('../plugin/I18n.js');for(const locale of ['en','ru']) for(const key of ['native.onboarding.scope','native.onboarding.profiles']) {
  const value=i18n.translate(key,locale,{count:2});assert(value);assert(!value.includes('Missing translation'));assert(!value.includes('{count}'));
 }
});
test('first-use wizard has its own bounded height rather than the empty inventory height',()=>{
 const start=panel.indexOf('    contentHeight: panel.fittedContentHeight(');
 const end=panel.indexOf('\n    )',start);
 const expression=panel.slice(start,end+6).replace('    contentHeight: ','');
 for(const visible of [false,true]) {
  const c=vm.createContext({panel:{fittedContentHeight:(h,max)=>Math.min(h,max)},
   Style:{space:n=>n},onboardingWizard:{visible},removeReview:{visible:false},helpPrompt:{visible:false},subscriptionUsagePrompt:{visible:false},root:{page:'main'},vless:{nativeOwner:true},
   nativeColumn:{implicitHeight:280},nativeProfileActions:{visible:false},nativeAppFooter:{visible:false}});
  assert.equal(vm.runInContext(expression,c),visible?600:280);
  c.nativeProfileActions={visible:true,height:70};c.nativeAppFooter={visible:true,height:36};
  assert.equal(vm.runInContext(expression,c),visible?600:410);
  c.helpPrompt.visible=true;assert.equal(vm.runInContext(expression,c),600);
  c.helpPrompt.visible=false;c.subscriptionUsagePrompt.visible=true;
  assert.equal(vm.runInContext(expression,c),600,'subscription usage owns the same bounded modal height');
  c.subscriptionUsagePrompt.visible=false;c.helpPrompt.visible=true;
  c.root.bootstrapRequired=true;c.setupPage={implicitHeight:250};
  assert.equal(vm.runInContext(expression,c),600,'help owns its bounded height before native setup too');
 }
});
test('wizard buttons share native keyboard activation without changing callbacks',()=>{
 assert(wizard.includes('component WizardButton: Button {'));
 assert(wizard.includes('    focusable: true'));
 assert.equal((wizard.match(/\bButton \{/g)||[]).length,1);
 assert.equal((wizard.match(/\bWizardButton \{/g)||[]).length,13);
 assert(wizard.includes('objectName: "onboardingRemove"'), 'the additional shared button is explicit removal review');
 assert(wizard.includes('onActiveFocusChanged: if (activeFocus) wizard.revealControl(this)'));
 assert(wizard.includes('Keys.onEscapePressed: canceled()'));
});
test('focused wizard controls scroll into view, including constrained height',()=>{
 for(const [top,height,initial,expected] of [[20,30,100,20],[350,40,0,190],[60,30,0,0],[480,30,0,300]]) {
  const c=vm.createContext({visible:true,Qt:{callLater:f=>f()},
   onboardingScroll:{contentItem:{},contentY:initial,height:200,contentHeight:500}});
  c.wizard=c;functions(wizard,c,['revealControl']);
  c.revealControl({activeFocus:true,height,mapToItem:()=>({y:top})});
  assert.equal(c.onboardingScroll.contentY,expected);
 }
});
test('dismissed wizard and stale unfocused controls never scroll',()=>{
 for(const [visible,activeFocus] of [[false,true],[true,false]]) {
  const c=vm.createContext({visible,Qt:{callLater:f=>f()},onboardingScroll:{contentY:17}});
  c.wizard=c;functions(wizard,c,['revealControl']);
  c.revealControl({activeFocus,mapToItem:()=>{throw new Error('must not inspect stale target')}});
  assert.equal(c.onboardingScroll.contentY,17);
 }
});
test('wizard Tab remains local, skips disabled or hidden steps and wraps in both directions',()=>{
 const a={visible:true,enabled:true,onboardingControl:true}, b={...a}, disabled={...a,enabled:false}, hidden={...a};
 const c=vm.createContext({visible:true,enabled:true,children:[a,disabled,{visible:false,enabled:true,children:[hidden]},b]});c.wizard=c;
 let selected;for(const item of [a,b,disabled,hidden])item.forceActiveFocus=()=>{for(const other of [a,b,disabled,hidden])other.activeFocus=false;item.activeFocus=true;selected=item};
 functions(wizard,c,['focusControl']);c.focusControl(1);assert.equal(selected,a);c.focusControl(1);assert.equal(selected,b);
 c.focusControl(1);assert.equal(selected,a);c.focusControl(-1);assert.equal(selected,b);
 c.visible=false;selected=null;c.focusControl(1);assert.equal(selected,null);
});
test('wizard navigation consumes only Tab/Backtab and preserves other keys',()=>{
 const calls=[],c=vm.createContext({Qt:{Key_Tab:1,Key_Backtab:2,ShiftModifier:4},focusControl:d=>calls.push(d)});
 functions(wizard,c,['handleNavigation']);
 for(const [key,modifiers,expected] of [[1,0,1],[1,4,-1],[2,0,-1]]) {const e={key,modifiers,accepted:false};c.handleNavigation(e);assert(e.accepted);assert.equal(calls.at(-1),expected)}
 const e={key:3,modifiers:0,accepted:false};c.handleNavigation(e);assert(!e.accepted);assert.equal(calls.length,3);
});
test('completion refusal, unknown outcome and close/reopen never reset to step one',()=>{
 for(const outcome of ['refused','unknown','waiting','complete']) {
  const c=context();c.openOnboarding(3);
  c.vless.completeOnboarding=()=>{c.vless.nativePending={action:'onboarding-complete'};return true};
  c.finishOnboarding();c.opened=false;c.onboardingWizard.dismiss();c.onboardingDismissed=false;
  if(outcome==='refused'){c.vless.nativePending=null;c.vless.nativeActionCode='conflict'}
  if(outcome==='unknown')c.vless.nativeOutcomeUnknown=true;
  if(outcome==='complete'){c.vless.nativePending=null;c.vless.nativeSnapshot.onboardingComplete=true}
  c.opened=true;c.syncNativeOnboarding();
  if(c.onboardingWizard.visible)assert.equal(c.onboardingWizard.step,3);
  if(outcome==='refused'){
   assert(c.onboardingWizard.visible);
   c.onboardingWizard.dismiss();c.onboardingDismissed=false;c.syncNativeOnboarding();
   assert(c.onboardingWizard.visible);assert.equal(c.onboardingWizard.step,3);
  }
  if(outcome==='complete')assert.equal(c.nativeOnboardingCompletionPending,null);
 }
});
test('status read is settled only after publishing result, including a queued post-action read',()=>{
 const process=service.slice(service.indexOf('    id: statusProcess'));
 const start=process.indexOf('    onExited: function(exitCode) {'),end=process.indexOf('\n    }',start)+6;
 const handler=process.slice(start,end).replace('    onExited: function(exitCode)','function exited(exitCode)');
 const expression=service.match(/readonly property bool nativeStatusRefreshing: ([^\n]+)/)[1];
 for(const queued of [false,true]) {
  const c=context(),v=c.vless,callbacks=[];
  const s=vm.createContext({statusProcess:{running:true},_nativeStatusSettled:false,_refreshAfterStatus:queued,
   statusStdout:{text:'synthetic'},statusFailureCount:0,nativeOwner:true,
   Qt:{callLater:f=>callbacks.push(f)},refreshAfterChange(){this.statusProcess.running=true},
   applyStatus(){v.nativeSnapshot.onboardingComplete=!this._refreshAfterStatus;c.syncNativeOnboarding();return true}});
  s.root=s;Object.defineProperty(v,'nativeStatusRefreshing',{get:()=>vm.runInContext(expression,s)});
  vm.runInContext(handler,s);
  // Quickshell can publish running=false before calling its exit handler.
  s.statusProcess.running=false;c.syncNativeOnboarding();assert(!c.onboardingWizard.visible);
  s.exited(0);c.syncNativeOnboarding();assert(!c.onboardingWizard.visible);
  assert.equal(v.nativeStatusRefreshing,queued);
  if(queued){assert.equal(callbacks.length,1);callbacks.shift().call(s);assert(s.statusProcess.running);
   s.statusProcess.running=false;s.exited(0);assert.equal(v.nativeStatusRefreshing,false)}
 }
});
test('restart and acknowledged unknown release completion wait without claiming success',()=>{
 for(const restart of [false,true])for(const complete of [false,true]) {
  const c=context();c.openOnboarding(3);c.finishOnboarding();
  c.vless.nativeOutcomeUnknown=true;c.vless.nativePending={action:'onboarding-complete'};
  c.syncNativeOnboarding();assert(c.nativeOnboardingCompletionPending);
  // The existing recovery action explicitly accepts a freshly joined state.
  c.vless.nativePending=null;c.vless.nativeOutcomeUnknown=false;
  c.vless.nativeFactsCurrent=true;c.vless.nativeMetadataBusy=false;
  if(restart)c.vless.nativeSnapshot.instanceId='new-instance';
  c.vless.nativeSnapshot.onboardingComplete=complete;c.syncNativeOnboarding();
  assert.equal(c.nativeOnboardingCompletionPending,null);
  assert.equal(c.onboardingWizard.visible,restart||!complete);
  if(restart||!complete){assert(c.nativeOnboardingCompletionUnconfirmed);assert.equal(c.onboardingWizard.step,3)}
  if(!complete){c.opened=false;c.onboardingWizard.dismiss();c.onboardingDismissed=false;
   c.opened=true;c.syncNativeOnboarding();assert(c.onboardingWizard.visible);assert.equal(c.onboardingWizard.step,3)}
 }
});
test('failed read cannot spin forever; late completion still needs a confirmed native snapshot',()=>{
 const c=context();c.openOnboarding(3);c.finishOnboarding();
 c.vless.nativeSnapshotFailed=true;c.syncNativeOnboarding();assert(c.nativeOnboardingCompletionPending);
 c.markOnboardingCompletionUnconfirmed();assert(c.nativeOnboardingCompletionUnconfirmed);
 assert(c.onboardingWizard.visible);assert.equal(c.vless.nativeSnapshot.onboardingComplete,false);
 c.vless.nativeSnapshotFailed=false;c.vless.nativeSnapshot.onboardingComplete=true;
 c.syncNativeOnboarding();assert(!c.onboardingWizard.visible);assert(!c.nativeOnboardingCompletionUnconfirmed);
 const unknown=context();unknown.openOnboarding(3);unknown.finishOnboarding();
 unknown.vless.nativePending={action:'onboarding-complete'};unknown.vless.nativeOutcomeUnknown=true;
 unknown.markOnboardingCompletionUnconfirmed();unknown.syncNativeOnboarding();
 assert(unknown.nativeOnboardingCompletionPending);assert(unknown.vless.nativePending);
 assert(unknown.onboardingWizard.visible);assert(unknown.vless.nativeOutcomeUnknown);
});
test('known bounded metadata refresh is neutral, never healthy or a hidden real failure',()=>{
 const expression=panel.match(/readonly property bool nativeMetadataVerifying: ([\s\S]*?)\n  readonly property bool nativeStatusWaiting:/)[1].trim();
 function fixture(){return {nativeMetadataTransition:{instanceId:'instance',failed:false},nativeSnapshot:{instanceId:'instance',lastKnownActual:'disconnected'},nativeObservation:null,nativeOutcomeUnknown:false,nativeActionCode:'',nativeSnapshotFailed:false}}
 function verify(v){return vm.runInNewContext(expression,{vless:v,NativeSnapshot:parser})}
 assert(verify(fixture()));
 for(const mutate of [v=>v.nativeMetadataTransition=null,v=>v.nativeMetadataTransition.failed=true,
  v=>v.nativeSnapshot.instanceId='new',v=>v.nativeSnapshot.lastKnownActual='failed',
  v=>v.nativeSnapshot.lastKnownActual='manualRecoveryRequired',v=>v.nativeOutcomeUnknown=true,
  v=>v.nativeActionCode='conflict',v=>v.nativeSnapshotFailed=true]) {
  const v=fixture();mutate(v);assert(!verify(v));
 }
 assert(panel.includes('nativeTransitionKind !== "" || nativeMetadataVerifying'));
});
test('first component read shows checking, not an install/preparation card',()=>{
 const state=require('../plugin/SetupState.js'),card=fs.readFileSync(__dirname+'/../plugin/RequiredComponents.qml','utf8');
 const checking=card.match(/readonly property bool checking: ([^\n]+)/)[1];
 const surfaces=[...card.matchAll(/    visible: (!card.checking[^\n]+)/g)].map(m=>m[1]);
 for(const facts of [{state:'checking',coreInstalled:null},{state:'needs_package',coreInstalled:false},{state:'ready',coreInstalled:true}]) {
  const c={facts,busy:true,busyMessage:'checking',appMissing:state.appMissing(facts),coreMissing:facts.coreInstalled===false,showPreparation:true,updateReview:true,terminalOpened:false};
  c.card=c;c.checking=vm.runInNewContext(checking,c);assert(c.checking);
  for(const expression of surfaces)assert(!vm.runInNewContext(expression,c));
 }
 assert.equal(state.coreReview(null,{state:'ready',coreInstalled:true},false,false),'checking');
 assert.equal(state.coreReview(null,{state:'ready',coreInstalled:true},false,true),'attention');
 assert.equal(state.coreReview(null,{state:'needs_package',coreInstalled:false},false,false),'missing');
});
console.log(`${count} native onboarding tests passed`);
