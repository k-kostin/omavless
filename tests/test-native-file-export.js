// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict'), fs = require('node:fs'), vm = require('node:vm');
const source = fs.readFileSync(__dirname + '/../plugin/Service.qml', 'utf8');
const panel = fs.readFileSync(__dirname + '/../plugin/Panel.qml', 'utf8');
const parser = vm.createContext({});
vm.runInContext(fs.readFileSync(__dirname + '/../plugin/NativeSnapshot.js', 'utf8'), parser);
let count = 0;
function test(name, fn) { try { fn(); count++; } catch (e) { e.message = name + ': ' + e.message; throw e; } }
function context() {
  const c = vm.createContext({NativeSnapshot:parser, nativeCanAct:true, nativeSnapshot:{instanceId:'instance',revision:7,profiles:[{id:'one'},{id:'two'}]}, nativeFileExportContext:null, nativeFileExportProcess:null, nativeFileExportStatus:'',backendPath:'/synthetic/backend.sh',nativeExportPickerFinished:()=>{}});
  c.root = c;
  // QML initial properties cross a QVariant map; object identity is not retained.
  c.nativeFileExportComponent = {createObject:(_, properties) => ({...JSON.parse(JSON.stringify(properties)), destroy(){this.destroyed=true}})};
  for (const name of ['startNativeExportPicker','validNativeExportPath','nativeFileExportCurrent','startNativeFileExport','startNativeReportFileExport','finishNativeFileExport']) {
    const start=source.indexOf('  function '+name+'('), end=source.indexOf('\n  }', start)+4;
    assert(start>=0); vm.runInContext(source.slice(start,end), c);
  }
  return c;
}
const frame = (revision=7) => JSON.stringify({api:'omavless.control',version:1,id:'export',ok:true,revision,result:{format:'uri',content:'synthetic-private-content'}});
test('typed native configuration file export is private multiline stdin, never QR or JSON record',()=>{
  const content='[Interface]\r\nPrivateKey = synthetic\r\n[Peer]\r\n';
  const response=JSON.stringify({...JSON.parse(frame()),result:{format:'wireguard-conf',content}});
  assert.equal(parser.parseQrExport(response,7),null);
  assert.equal(parser.parsePrivateProfileExport(response,7).content,content);
  const c=context();c.startNativeFileExport({uuid:'one'},'/tmp/native.conf');
  c.finishNativeFileExport(c.nativeFileExportProcess,0,response);
  assert.equal(c.nativeFileExportProcess.privateInput,'/tmp/native.conf\n'+content);
  assert(!c.nativeFileExportProcess.command.join('|').includes('PrivateKey = synthetic'));
  for(const result of [{format:'wireguard-record',content:'{}'}, {format:'wireguard-conf',content:'x'.repeat(32769)},
      {format:'wireguard-conf',content:'secret\0key'}])
    assert.equal(parser.parsePrivateProfileExport(JSON.stringify({...JSON.parse(frame()),result}),7),null);
});
test('fixed action record preserves family for an honest native QR unavailable state',()=>{
  const start=panel.indexOf('  function nativeRecord('),end=panel.indexOf('\n  }',start)+4;
  const c=vm.createContext({});vm.runInContext(panel.slice(start,end),c);
  for(const protocol of ['wireguard','amneziawg'])
    assert.equal(c.nativeRecord({id:'one',name:'Synthetic',protocol,subscriptionId:''}).protocol,protocol);
  const qr=panel.slice(panel.indexOf('id: rowQr;'),panel.indexOf('id: rowExport;'));
  assert(qr.includes('record.protocol'));assert(qr.includes('!nativeCredential'));
  assert(qr.includes('native.main.qr_native_unavailable'));
});
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
  assert(panel.includes('vless.startNativeExportPicker(null, root.uiLocale)'));
  assert(!panel.includes('id: exportWindow'));
  assert(!panel.includes('QtQuick.Dialogs'));
  assert(source.includes('process.running && !process.picking'));
  // Export lives in the single fixed dock, after QR in the visual/tab order.
  assert(panel.includes('rowQr, rowExport, rowDetails'));
  const component=source.slice(source.indexOf('id: nativeFileExportComponent'),source.indexOf('id: nativeQrRenderComponent'));
  assert(component.includes('property Timer watchdog: Timer'));
  assert(component.includes('writeAdmitted = root.nativeFileExportCurrent(context)'));
  assert(component.includes('if (writeAdmitted) write(privateInput)'));
  assert(component.includes('privateInput = ""'));
  assert(!component.includes('console.'));
});
test('report export uses canonical report projection, not profile export', () => {
  const c=context(); let parsed=0;
  c.NativeSnapshot={editorText:parser.editorText,configurationReport:(raw,revision)=>{parsed++;assert.equal(raw,'synthetic response');assert.equal(revision,7);return '{"safe":true}';},parseQrExport:()=>{throw Error('wrong exporter')}};
  assert(c.startNativeReportFileExport('/tmp/report.json'));
  const reader=c.nativeFileExportProcess;
  assert.equal(reader.command.join('|'),'bash|/synthetic/backend.sh|native-support-report');
  c.finishNativeFileExport(reader,0,'synthetic response');
  assert.equal(parsed,1);assert.equal(c.nativeFileExportProcess.privateInput,'/tmp/report.json\n{"safe":true}');
  assert(c.nativeFileExportCurrent(c.nativeFileExportContext));
  c.nativeSnapshot.revision++;assert(!c.nativeFileExportCurrent(c.nativeFileExportContext));
});
test('stale and malformed reports never reach writer', () => {
  for(const stale of [false,true]) {
    const c=context();c.startNativeReportFileExport('/tmp/report.json');
    if(stale)c.nativeSnapshot.instanceId='replacement';
    c.finishNativeFileExport(c.nativeFileExportProcess,0,'secret-invalid-report');
    assert.equal(c.nativeFileExportProcess,null);assert.equal(c.nativeFileExportStatus,'failed');
  }
  assert(source.includes('context.kind === "report") startNativeReportFileExport'));
  assert(panel.slice(panel.indexOf('function panelTabTargets()'),panel.indexOf('function availablePanelTabTargets()')).includes('nativeSupportSetting.exportFocusTarget'));
});

test('isolated chooser is explicit, serialized, private and has no writer before acceptance', () => {
  const c=context();assert(c.startNativeExportPicker(null,'ru'));
  const picker=c.nativeFileExportProcess;
  assert.equal(picker.command.join('|'),'bash|/synthetic/backend.sh|native-pick-report-export');
  assert.equal(picker.privateInput,'ru');assert(picker.picking);
  assert(!c.startNativeExportPicker({uuid:'one'},'en'));
  assert(!picker.writing);
  c.finishNativeFileExport(picker,3,'private-discarded','');
  assert.equal(c.nativeFileExportProcess,null);assert.equal(c.nativeFileExportStatus,'');assert(picker.destroyed);
});
test('chooser accepted destination enters canonical report/profile reader, then secure writer', () => {
  for(const profile of [null,{uuid:'one'}]) {
    const c=context();assert(c.startNativeExportPicker(profile,'unsupported'));
    const picker=c.nativeFileExportProcess;assert.equal(picker.privateInput,'en');
    c.finishNativeFileExport(picker,0,'/tmp/selected $(false).conf','');
    assert.equal(c.nativeFileExportContext.path,'/tmp/selected $(false).conf');
    assert.equal(c.nativeFileExportStatus,'pending');
    assert.equal(c.nativeFileExportProcess.command[2],profile?'native-profile-file':'native-support-report');
    assert(picker.destroyed);
  }
});
test('chooser cancellation, stale result, invalid path and fixed public errors', () => {
  for(const change of [c=>c.nativeSnapshot.revision++,c=>c.nativeSnapshot.instanceId='new',c=>c.nativeCanAct=false,c=>c.nativeSnapshot.profiles=[]]) {
    const c=context();c.startNativeExportPicker({uuid:'one'},'en');const p=c.nativeFileExportProcess;
    change(c);c.finishNativeFileExport(p,0,'/tmp/file','secret-error');
    assert.equal(c.nativeFileExportProcess,null);assert.equal(c.nativeFileExportStatus,'failed');
  }
  for(const path of ['relative','/tmp/a\nb','/tmp/'+ 'x'.repeat(4097)]) {
    const c=context();c.startNativeExportPicker(null,'en');c.finishNativeFileExport(c.nativeFileExportProcess,0,path,'');
    assert.equal(c.nativeFileExportProcess,null);assert.equal(c.nativeFileExportStatus,'failed');
  }
  const c=context();c.startNativeExportPicker(null,'en');
  c.finishNativeFileExport(c.nativeFileExportProcess,2,'','File picker unavailable: install zenity, kdialog or yad\n');
  assert.equal(c.nativeFileExportStatus,'pickerUnavailable');
  c.startNativeExportPicker(null,'en');
  c.finishNativeFileExport(c.nativeFileExportProcess,2,'','Invalid desktop helper command\n');
  assert.equal(c.nativeFileExportStatus,'helperUnavailable');
});
console.log(`${count} native file-export tests passed`);
