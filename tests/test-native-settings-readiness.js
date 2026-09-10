// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict'), fs = require('node:fs'), vm = require('node:vm');
const parser = vm.createContext({});
vm.runInContext(fs.readFileSync(__dirname + '/../plugin/NativeSnapshot.js', 'utf8'), parser);
const source = fs.readFileSync(__dirname + '/../plugin/Service.qml', 'utf8');
const fixture = () => ({schemaVersion:1, clipboardReadAvailable:true, clipboardWriteAvailable:true, filePicker:'zenity', configEditorAvailable:true, qrEncoderAvailable:true, gtk4FallbackAvailable:false});
let count=0;
function test(name, fn) { try { fn(); count++; } catch(e) { e.message=name+': '+e.message; throw e; } }
test('all three supported pickers and missing helpers',()=>{
 for (const filePicker of ['zenity','kdialog','yad',null]) {
  const f=fixture(); f.filePicker=filePicker; f.configEditorAvailable=false; f.qrEncoderAvailable=false;
  const value=parser.desktopCapabilities(JSON.stringify(f)); assert(value); assert.equal(value.filePicker,filePicker); assert.equal(value.configEditorAvailable,false);
 }
});
test('strict schema, unknown provider and no fabricated fallback',()=>{
 for(const [key,value] of [['schemaVersion',2],['filePicker','gtk4'],['configEditorAvailable','yes'],['qrEncoderAvailable',1],['gtk4FallbackAvailable',true],['clipboardWriteAvailable',null],['path','private-secret']]) {
  const f=fixture(); f[key]=value; assert.equal(parser.desktopCapabilities(JSON.stringify(f)),null);
 }
 for(const key of Object.keys(fixture())) { const f=fixture(); delete f[key]; assert.equal(parser.desktopCapabilities(JSON.stringify(f)),null); }
});
test('malformed duplicate bounded nested and private input rejected',()=>{
 const raw=JSON.stringify(fixture());
 for(const value of ['',raw.replace('{','{"schemaVersion":1,'),raw+'{}','x'.repeat(2049),'private-secret',raw.replace('true','{"private":"secret"}'),raw.replace('schemaVersion','schema\\u0056ersion')]) assert.equal(parser.desktopCapabilities(value),null);
});
function context() {
 const c=vm.createContext({NativeSnapshot:parser,nativeOwner:true,panelVisible:true,nativeDesktopLoading:false,nativeDesktopCapabilities:null,_nativeDesktopRead:null,_nativeDesktopGeneration:0,backendPath:'/synthetic/backend.sh',nativeDesktopComponent:{createObject:(_,p)=>({...p})}}); c.root=c;
 for(const name of ['refreshNativeDesktopCapabilities','finishNativeDesktopCapabilities','completeNativeDesktopRead']) {
  const start=source.indexOf('  function '+name+'('), end=source.indexOf('\n  }',start)+4; assert(start>=0); vm.runInContext(source.slice(start,end),c);
 } return c;
}
test('fixed read-only launcher and refresh clears prior inventory',()=>{
 const c=context(); c.nativeDesktopCapabilities=fixture(); assert(c.refreshNativeDesktopCapabilities()); assert.equal(c.nativeDesktopCapabilities,null);
 assert.equal(c._nativeDesktopRead.command.join('|'),'bash|/synthetic/backend.sh|native-desktop-capabilities'); assert(c.nativeDesktopLoading);
 assert(c.finishNativeDesktopCapabilities(1,0,JSON.stringify(fixture()))); assert(!c.nativeDesktopLoading);
});
test('closed or changed ownership, stale generation and error fail closed',()=>{
 for(const change of [c=>c.panelVisible=false,c=>c.nativeOwner=false,c=>c._nativeDesktopGeneration++]) { const c=context(); change(c); assert(!c.finishNativeDesktopCapabilities(0,0,JSON.stringify(fixture()))); assert.equal(c.nativeDesktopCapabilities,null); }
 const c=context(); assert(!c.finishNativeDesktopCapabilities(0,1,JSON.stringify(fixture()))); assert(!c.finishNativeDesktopCapabilities(0,0,'private-secret'));
 for(const change of [c=>c.panelVisible=false,c=>c.nativeOwner=false,c=>c.nativeDesktopLoading=true]) { const c=context(); change(c); assert(!c.refreshNativeDesktopCapabilities()); }
});
test('close/reopen retires pending read, rejects stale data and allows fresh read',()=>{
 const c=context(); assert(c.refreshNativeDesktopCapabilities()); const old=c._nativeDesktopRead;
 // Closing invalidates generation and stops the process, but keeps admission
 // busy until that exact process delivers onExited.
 c.panelVisible=false; c._nativeDesktopGeneration++; c.nativeDesktopCapabilities=null; old.running=false;
 c.panelVisible=true; assert(!c.refreshNativeDesktopCapabilities());
 assert(!c.completeNativeDesktopRead(old,1,0,JSON.stringify(fixture())));
 assert.equal(c.nativeDesktopCapabilities,null); assert(!c.nativeDesktopLoading);
 assert(c.refreshNativeDesktopCapabilities()); const current=c._nativeDesktopRead;
 assert(!c.completeNativeDesktopRead(old,1,0,JSON.stringify(fixture())));
 assert.equal(c._nativeDesktopRead,current); assert(c.nativeDesktopLoading);
 assert(c.completeNativeDesktopRead(current,3,0,JSON.stringify(fixture())));
 assert(c.nativeDesktopCapabilities); assert(!c.nativeDesktopLoading);
});
test('old controls, explicit clipboard actions, focus and watchdog contract',()=>{
 const panel=fs.readFileSync(__dirname+'/../plugin/Panel.qml','utf8'), launcher=fs.readFileSync(__dirname+'/../backend.sh','utf8');
 for(const id of ['nativeFileImportRow','nativeProfileEditorRow','nativeQrExportRow']) assert(panel.includes(id+'.focusTarget'));
 assert(panel.includes('vless.refreshNativeDesktopCapabilities(); return true'));
 assert(panel.includes('onAction: vless.copyText(root.qrEncoderInstallCommand)'));
 assert(launcher.includes('exec omavless desktop capabilities'));
 const component=source.slice(source.indexOf('id: nativeDesktopComponent'),source.indexOf('function copyNativeConfigurationReport'));
 assert(component.includes('property Timer timeout: Timer')); assert(component.includes('timedOut ? -1 : code'));
 assert(!component.includes('coreSetup')); assert(!component.includes('console.'));
});
console.log(`${count} native Settings readiness tests passed`);
