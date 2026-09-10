// SPDX-License-Identifier: MIT
const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const panel=fs.readFileSync(__dirname+'/../plugin/Panel.qml','utf8');
const service=fs.readFileSync(__dirname+'/../plugin/Service.qml','utf8');
const wizard=fs.readFileSync(__dirname+'/../plugin/OnboardingWizard.qml','utf8');
const parser=vm.createContext({});vm.runInContext(fs.readFileSync(__dirname+'/../plugin/NativeSnapshot.js','utf8'),parser);
let count=0;function test(name,f){try{f();count++}catch(e){e.message=name+': '+e.message;throw e}}
function functions(source,c,names){for(const name of names){const start=source.indexOf('  function '+name+'('),end=source.indexOf('\n  }',start)+4;assert(start>=0);vm.runInContext(source.slice(start,end),c)}}
function context(){const c=vm.createContext({vless:{nativeOwner:true,nativeSnapshotFailed:false,nativePending:null,nativeImportBusy:false,nativeActionCode:'',nativeSnapshot:{onboardingComplete:false,routing:{storedPreset:''},profiles:[]},completeOnboarding:()=>true,useRoutingPreset:()=>true},opened:true,onboardingDismissed:false,nativeOnboardingPresetPending:'',onboardingWizard:{visible:false,step:1,openAt(step){this.visible=true;this.step=step},dismiss(){this.visible=false}},importDialog:{visible:false},subscriptionPrompt:{visible:false},keyCatcher:{forceActiveFocus(){}},Qt:{callLater:f=>f()}});c.root=c;functions(panel,c,['openOnboarding','syncNativeOnboarding','chooseOnboardingPreset','dismissOnboarding','finishOnboarding']);return c}
test('first use opens existing wizard, completed and dismissed states do not loop',()=>{
 const c=context();c.syncNativeOnboarding();assert(c.onboardingWizard.visible);assert.equal(c.onboardingWizard.step,1);
 c.dismissOnboarding();c.syncNativeOnboarding();assert(!c.onboardingWizard.visible);
 c.onboardingDismissed=false;c.vless.nativeSnapshot.onboardingComplete=true;c.syncNativeOnboarding();assert(!c.onboardingWizard.visible);
 assert(c.openOnboarding(3));assert.equal(c.onboardingWizard.step,3); // Explicit Settings re-entry.
});
test('missing/stale/closed/pending metadata never opens first use over another operation',()=>{
 for(const change of [c=>c.vless.nativeSnapshot=null,c=>c.vless.nativeSnapshotFailed=true,c=>c.opened=false,c=>c.vless.nativePending={},c=>c.vless.nativeImportBusy=true,c=>c.importDialog.visible=true,c=>c.subscriptionPrompt.visible=true]){const c=context();change(c);c.syncNativeOnboarding();assert(!c.onboardingWizard.visible)}
});
test('preset step advances only after confirmed snapshot, not initial click or rejection',()=>{
 const c=context();c.openOnboarding(2);c.vless.nativePending={};assert(c.chooseOnboardingPreset('china-cn-direct'));assert.equal(c.onboardingWizard.step,2);
 c.syncNativeOnboarding();assert.equal(c.onboardingWizard.step,2);c.vless.nativePending=null;c.syncNativeOnboarding();assert.equal(c.onboardingWizard.step,2);
 c.vless.nativeSnapshot.routing.storedPreset='china-cn-direct';c.syncNativeOnboarding();assert.equal(c.onboardingWizard.step,3);
 c.openOnboarding(2);c.chooseOnboardingPreset('iran-ir-direct');c.vless.nativeActionCode='permission_denied';c.syncNativeOnboarding();assert.equal(c.onboardingWizard.step,2);assert.equal(c.nativeOnboardingPresetPending,'');
});
test('finish submits acknowledgement then uses normal pending/recovery surface',()=>{
 const c=context();c.openOnboarding(3);c.vless.completeOnboarding=()=>false;c.finishOnboarding();assert(c.onboardingWizard.visible);
 c.vless.completeOnboarding=()=>true;c.finishOnboarding();assert(!c.onboardingWizard.visible);assert(c.onboardingDismissed);assert.equal(c.vless.nativeSnapshot.onboardingComplete,false);
});
test('native completion shares exact pending fence and zero private arguments',()=>{
 const c=vm.createContext({nativeOwner:true,nativeCanAct:true,nativeSnapshot:{instanceId:'instance',revision:7},_nativeOperationSerial:0,backendPath:'/synthetic/backend.sh',nativeActionProcess:{},nativeActionCode:'',nativeOutcomeUnknown:false,nativePending:null});
 functions(service,c,['requestNativeAction','completeOnboarding']);assert(c.completeOnboarding());assert.equal(c.nativePending.action,'onboarding-complete');assert.equal(c.nativePending.instanceId,'instance');assert.equal(c.nativePending.revision,7);assert.equal(c.nativePending.command.length,6);assert.equal(c.nativePending.input,undefined);
 assert.equal(c.nativePending.command[2],'native-onboarding-complete');c.nativeCanAct=false;assert(!c.completeOnboarding());
 assert.equal(parser.parseActionExit('',{action:'onboarding-complete'},74).code,'invalid_argument');
});
test('native wizard preserves layout and explicitly avoids legacy grants/readiness claims',()=>{
 assert(wizard.includes('property bool nativeContext: false'));
 assert(wizard.includes('visible: !wizard.nativeContext && wizard.coreSetup.installed && !wizard.coreSetup.tunReady'));
 assert(wizard.includes('enabled: (wizard.nativeContext || wizard.coreSetup.tunReady) && !wizard.busy'));
 assert(wizard.includes('wizard.textFor("native.core.scope")'));
 assert(wizard.includes('wizard.textFor("native.onboarding.scope")'));
 assert(wizard.includes('"native.onboarding.profiles"'));
 assert(panel.includes('vless.startNativeImport("clipboard")'));assert(panel.includes('vless.startNativeImport("file")'));
 assert(panel.includes('(root.page === "settings" || onboardingWizard.visible)'));
 assert(panel.includes('nativeOnboardingSetting.focusTarget'));
 assert(!wizard.includes('nativeCoreFacts.tunReady'));
});
test('EN/RU scope and imported count never borrow connected/ready wording',()=>{
 const i18n=require('../plugin/I18n.js');for(const locale of ['en','ru']) for(const key of ['native.onboarding.scope','native.onboarding.profiles']) {
  const value=i18n.translate(key,locale,{count:2});assert(value);assert(!value.includes('Missing translation'));assert(!value.includes('{count}'));
 }
});
console.log(`${count} native onboarding tests passed`);
