// SPDX-License-Identifier: MIT
const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const parser=vm.createContext({}); vm.runInContext(fs.readFileSync(__dirname+'/../plugin/NativeSnapshot.js','utf8'),parser);
const source=fs.readFileSync(__dirname+'/../plugin/Service.qml','utf8');
const facts=()=>({schemaVersion:1,scope:'desktop_setup_facts',installed:true,version:'1.19.30',tunDevice:'present',fileNetworkCapabilities:'present',servicePermissionReadiness:'not_verified',coverage:{serviceContextVerified:false,tunCreationVerified:false,controllerQueried:false},remediation:'verify_native_host_setup'});
let count=0; function test(name,fn){try{fn();count++}catch(e){e.message=name+': '+e.message;throw e}}
test('strict facts only, no service health projection',()=>{
 const result=parser.parseCoreSetupFacts(JSON.stringify(facts()));assert(result);assert.equal(result.version,'1.19.30');assert.equal(result.tunReady,undefined);
 const missing=facts();missing.installed=false;missing.version=null;missing.fileNetworkCapabilities='not_applicable';missing.remediation='install_core_using_host_setup';assert(parser.parseCoreSetupFacts(JSON.stringify(missing)));
});
test('reject unknown fields, unsafe versions, overclaims and malformed responses',()=>{
 for(const [key,value] of [['scope','private'],['installed','yes'],['version','<b>secret</b>'],['version','1.2.3-secret'],['tunDevice','ready'],['fileNetworkCapabilities','ready'],['servicePermissionReadiness','ready'],['remediation','sudo private'],['path','private']]){
  const f=facts();f[key]=value;assert.equal(parser.parseCoreSetupFacts(JSON.stringify(f)),null);
 }
 for(const key of ['serviceContextVerified','tunCreationVerified','controllerQueried','private']){const f=facts();f.coverage[key]=true;assert.equal(parser.parseCoreSetupFacts(JSON.stringify(f)),null)}
 for(const raw of ['private-token','x'.repeat(8193),'[]'])assert.equal(parser.parseCoreSetupFacts(raw),null);
});
function context(){const c=vm.createContext({NativeSnapshot:parser,nativeOwner:true,nativeCoreSetupVisible:true,nativeCoreSetupFacts:null,nativeCoreSetupStatus:'',_nativeCoreSetupRead:null,_nativeCoreSetupGeneration:3,backendPath:'/synthetic/backend.sh',nativeCoreSetupComponent:{createObject:(_,p)=>({...p,destroy(){this.destroyed=true}})}});c.root=c;
 for(const name of ['refreshNativeCoreSetup','finishNativeCoreSetup']){const start=source.indexOf('  function '+name+'('),end=source.indexOf('\n  }',start)+4;vm.runInContext(source.slice(start,end),c)}return c;
}
test('fixed read command and disposable result',()=>{const c=context();assert(c.refreshNativeCoreSetup());const p=c._nativeCoreSetupRead;assert.equal(p.command.join('|'),'bash|/synthetic/backend.sh|native-core-readiness');assert(!c.refreshNativeCoreSetup());c.finishNativeCoreSetup(p,0,JSON.stringify(facts()));assert.equal(c.nativeCoreSetupFacts.version,'1.19.30');assert(p.destroyed)});
test('close and ownership changes discard late replies',()=>{for(const key of ['nativeOwner','nativeCoreSetupVisible']){const c=context();c.refreshNativeCoreSetup();const p=c._nativeCoreSetupRead;c[key]=false;c.finishNativeCoreSetup(p,0,JSON.stringify(facts()));assert.equal(c.nativeCoreSetupFacts,null);assert(p.destroyed)}});
test('reopen generation obtains a fresh sample',()=>{const c=context();c.refreshNativeCoreSetup();const p=c._nativeCoreSetupRead;c._nativeCoreSetupGeneration++;c.finishNativeCoreSetup(p,0,JSON.stringify(facts()));assert.equal(c.nativeCoreSetupFacts,null);assert.equal(c._nativeCoreSetupRead.generation,4);assert(p.destroyed)});
test('public fixed failure and QML watchdog',()=>{const c=context();c.refreshNativeCoreSetup();c.finishNativeCoreSetup(c._nativeCoreSetupRead,1,'private-token');assert.equal(c.nativeCoreSetupStatus,'failed');assert.equal(c.nativeCoreSetupFacts,null);const component=source.slice(source.indexOf('id: nativeCoreSetupComponent'),source.indexOf('id: nativeQrRenderComponent'));assert(component.includes('property Timer watchdog: Timer'));assert(!component.includes('console.'));const panel=fs.readFileSync(__dirname+'/../plugin/Panel.qml','utf8');assert(panel.includes('nativeCoreSetupRow.focusTarget'));assert(panel.includes('root.textFor("native.core.scope")'));assert(!panel.includes('vless.nativeCoreSetupFacts.tunReady'))});
console.log(`${count} native core setup tests passed`);
