// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict'), fs = require('node:fs'), vm = require('node:vm');
const source = fs.readFileSync(__dirname + '/../plugin/Service.qml', 'utf8');
const panel = fs.readFileSync(__dirname + '/../plugin/Panel.qml', 'utf8');
const parser = vm.createContext({});
vm.runInContext(fs.readFileSync(__dirname + '/../plugin/NativeSnapshot.js', 'utf8'), parser);
let count = 0;
function test(name, fn) { try { fn(); count++; } catch (e) { e.message = name + ': ' + e.message; throw e; } }
function context() {
  const c = vm.createContext({NativeSnapshot:parser, nativeCanAct:true, nativeSnapshot:{instanceId:'instance',revision:7,profiles:[{id:'one'},{id:'two'}]}, nativeFileExportContext:null, nativeFileExportProcess:null, nativeFileExportStatus:'',backendPath:'/synthetic/backend.sh'});
  c.root = c;
  c.nativeFileExportComponent = {createObject:(_, properties) => ({...properties, destroy(){this.destroyed=true}})};
  for (const name of ['validNativeExportPath','nativeFileExportCurrent','startNativeFileExport','finishNativeFileExport']) {
    const start=source.indexOf('  function '+name+'('), end=source.indexOf('\n  }', start)+4;
    assert(start>=0); vm.runInContext(source.slice(start,end), c);
  }
  return c;
}
const frame = (revision=7) => JSON.stringify({api:'omavless.control',version:1,id:'export',ok:true,revision,result:{format:'uri',content:'synthetic-private-content'}});
test('absolute bounded path without controls; shell syntax remains inert data', () => {
  const c=context();
  for(const path of ['/tmp/private file','/tmp/$(do-not-run)','/tmp/a;not-a-command']) assert(c.validNativeExportPath(path));
  for(const path of ['', 'relative', '/tmp/a\nb', '/tmp/a\r', '/tmp/a\0', '/'+'x'.repeat(4096), '/'+'界'.repeat(1400)]) assert(!c.validNativeExportPath(path));
});
test('export selected record only and no destination argv', () => {
  const c=context(); assert(c.startNativeFileExport({uuid:'two'}, '/tmp/private-destination'));
  const p=c.nativeFileExportProcess;
  assert.equal(p.command.join('|'),'bash|/synthetic/backend.sh|native-profile-file|two');
  assert(!p.command.join('|').includes('private-destination'));
  assert(!c.startNativeFileExport({uuid:'one'}, '/tmp/another'));
});
test('private content and destination go only to writer stdin', () => {
  const c=context(); c.startNativeFileExport({uuid:'one'}, '/tmp/private-destination');
  const p=c.nativeFileExportProcess; c.finishNativeFileExport(p,0,frame());
  assert(p.destroyed);
  const writer=c.nativeFileExportProcess;
  assert.equal(writer.command.join('|'),'bash|/synthetic/backend.sh|native-export-write');
  assert.equal(writer.privateInput,'/tmp/private-destination\nsynthetic-private-content');
  writer.writeAdmitted=true;
  c.finishNativeFileExport(writer,0,''); assert.equal(c.nativeFileExportStatus,'saved'); assert.equal(c.nativeFileExportContext,null); assert(writer.destroyed);
});
test('denied writer admission cannot report saved even for zero exit', () => {
  const c=context(); c.startNativeFileExport({uuid:'one'},'/tmp/destination');
  c.finishNativeFileExport(c.nativeFileExportProcess,0,frame());
  const writer=c.nativeFileExportProcess; writer.writeAdmitted=false;
  c.finishNativeFileExport(writer,0,''); assert.equal(c.nativeFileExportStatus,'failed');
});
test('no file write after stale revision, instance, admission or deleted record', () => {
  for(const change of [c=>c.nativeSnapshot.revision++,c=>c.nativeSnapshot.instanceId='replacement',c=>c.nativeCanAct=false,c=>c.nativeSnapshot.profiles=[]]) {
    const c=context(); c.startNativeFileExport({uuid:'one'},'/tmp/destination'); const p=c.nativeFileExportProcess;
    change(c); c.finishNativeFileExport(p,0,frame()); assert.equal(c.nativeFileExportProcess,null); assert.equal(c.nativeFileExportStatus,'failed'); assert(p.destroyed);
  }
});
test('failure malformed and mismatched reply do not release private data', () => {
  for(const [code,reply] of [[1,frame()],[0,'password=synthetic-private-content'],[0,frame(8)]]) {
    const c=context(); c.startNativeFileExport({uuid:'one'},'/tmp/destination'); c.finishNativeFileExport(c.nativeFileExportProcess,code,reply);
    assert.equal(c.nativeFileExportStatus,'failed'); assert.equal(c.nativeFileExportContext,null);
  }
});
test('explicit confirmation UI and Process cleanup guards', () => {
  assert(panel.includes('hint: root.textFor("native.fileExport.warning")'));
  assert(panel.includes('context.revision !== vless.nativeSnapshot.revision'));
  assert(panel.includes('context.instanceId !== vless.nativeSnapshot.instanceId'));
  assert(panel.includes('exportWindow.value = ""'));
  assert(panel.includes('rowQr, rowExport, rowEdit]'));
  const component=source.slice(source.indexOf('id: nativeFileExportComponent'),source.indexOf('id: nativeQrRenderComponent'));
  assert(component.includes('property Timer watchdog: Timer'));
  assert(component.includes('writeAdmitted = root.nativeFileExportCurrent(context)'));
  assert(component.includes('if (writeAdmitted) write(privateInput)'));
  assert(component.includes('privateInput = ""'));
  assert(!component.includes('console.'));
});
console.log(`${count} native file-export tests passed`);
