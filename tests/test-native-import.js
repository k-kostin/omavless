// SPDX-License-Identifier: MIT
// Execute the actual production parser and Service methods with synthetic data.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const parser = vm.createContext({});
vm.runInContext(fs.readFileSync(path.join(__dirname, '../plugin/NativeSnapshot.js'), 'utf8'), parser);
let count = 0;
function test(name, fn) { try { fn(); count++; } catch(e) { e.message = name + ': ' + e.message; throw e; } }
function profile() { return {version:1,protocol:'vless',server:'192.0.2.1',port:443,transport:'tcp',security:'tls',
  sni:'',flow:'',insecure:false,advancedXhttp:false,experimental:false,experimentalFeatures:[],
  compatibilityNote:'',credentialHint:'••••1234',suggestedName:'Synthetic'}; }
function frame(result={version:1,kind:'profile',profile:profile()}) {
  return JSON.stringify({api:'omavless.control',version:1,id:'preview',ok:true,revision:4,result});
}
function harness() {
  const source = fs.readFileSync(path.join(__dirname,'../plugin/Service.qml'),'utf8');
  const c = vm.createContext({NativeSnapshot:parser,nativeOwner:true,nativeSnapshotFailed:false,
    nativeSnapshot:{instanceId:'instance-one',revision:4,lastKnownActual:'disconnected',
      desired:{connected:false,mode:'rule',generation:3},profiles:[{id:'existing-id',name:'Existing',missing:false,subscriptionId:''}]},
    nativeObservation:{instanceId:'instance-one',revision:4,lastKnownActual:'disconnected',
      desired:{connected:false,mode:'rule',generation:3},availability:'observed',manualRecoveryRequired:false},
    nativePending:null,nativeOutcomeUnknown:false,nativeActionCode:'',nativeImportCode:'',
    _nativeOperationSerial:0,_nativeImportContext:null,_nativeSourceContext:null,_nativePreviewContext:null,
    importPreview:{},nativeImportSource:{running:false},nativeImportPreview:{running:false},
    nativeActionProcess:{running:false},backendPath:'/synthetic/backend.sh',ready:[],subscriptionReady:[]});
  c.importReady = (...args) => c.ready.push(args);
  c.startNativeSubscription = (...args) => { c.subscriptionReady.push(args); return true; };
  for (const name of ['nativeFactsCurrent','nativeCanAct','nativeActionRunning','nativeImportBusy']) {
    const match = source.match(new RegExp('readonly property bool '+name+': ([\\s\\S]*?)(?=\\n  (?:property|readonly|function|$))'));
    assert(match, name);
    vm.runInContext('Object.defineProperty(this,"'+name+'",{get:function(){return ('+match[1].trim()+');}});',c);
  }
  for (const name of ['cancelNativeImport','startNativeImport','nativeImportCurrent','finishNativeImportSource',
    'finishNativeImportPreview','confirmNativeImport','isValidName','reconcileNativeAction']) {
    const start=source.indexOf('  function '+name+'('), end=source.indexOf('\n  }',start)+4;
    assert(start>=0 && end>start,name);vm.runInContext(source.slice(start,end),c);
  }
  return c;
}
function sourced(c, input='synthetic-private-link', kind='clipboard') {
  assert.equal(c.startNativeImport(kind),true);c.nativeImportSource.running=false;
  c.finishNativeImportSource(0,input,'');
}
function previewed(c, input='synthetic-private-link', kind='clipboard') {
  sourced(c,input,kind);c.nativeImportPreview.running=false;c.finishNativeImportPreview(0,frame());
}
test('UTF8 input bounds never truncate and reject NUL or malformed surrogate',()=>{
  for(const input of ['x','x'.repeat(32768),'я'.repeat(16384)]) assert.equal(parser.importInput(input),true);
  for(const input of ['',null,1,'x'.repeat(32769),'я'.repeat(16385),'bad\0value','\ud800']) assert.equal(parser.importInput(input),false);
});
test('exact private preview shape and supported variants',()=>{
  for(const protocol of ['vless','trojan','hysteria2','tuic']) {
    const p=profile();p.protocol=protocol;assert.equal(parser.parseImportPreview(frame({version:1,kind:'profile',profile:p}),4).profile.protocol,protocol);
  }
  assert.equal(parser.parseImportPreview(frame(),3),null);
  for(const [key,value] of [['port',1.5],['port',0],['port',65536],['insecure','false'],['server',''],['server','x'.repeat(254)],
    ['security','unknown'],['transport','unknown'],['credentialHint','raw-secret'],['experimentalFeatures',['x'.repeat(65)]],['extra','secret']]) {
    const p=profile();p[key]=value;assert.equal(parser.parseImportPreview(frame({version:1,kind:'profile',profile:p}),4),null);
  }
  for(const raw of ['not-json','x'.repeat(262145),JSON.stringify({...JSON.parse(frame()),extra:true}),
    JSON.stringify({...JSON.parse(frame()),ok:false}),JSON.stringify({...JSON.parse(frame()),revision:4.5})]) {
    assert.equal(parser.parseImportPreview(raw,4),null);
  }
});
test('clipboard and file contents use same classifier without exposing input in argv',()=>{
  for(const kind of ['clipboard','file']) {
    const c=harness(), before=JSON.stringify(c.nativeSnapshot);previewed(c,'private\ninput',kind);
    assert.equal(c.nativeImportSource.command[2],'native-import-'+kind);
    assert.equal(c._nativeImportContext.input,'private\ninput');
    assert.equal(c.importPreview.server,'192.0.2.1');assert.equal(c.ready.length,1);
    assert.deepEqual(c.ready[0],['native-'+kind,'','Synthetic']);
    assert(!JSON.stringify(c.nativeImportSource.command).includes('private'));
    assert.equal(JSON.stringify(c.nativeSnapshot),before);
  }
});
test('cancellation invalidates late source and preview completions',()=>{
  const a=harness();a.startNativeImport('file');a.cancelNativeImport();a.nativeImportSource.running=false;
  a.finishNativeImportSource(0,'private-input','');assert.equal(a._nativeImportContext,null);assert.equal(a.nativeImportPreview.running,false);
  const b=harness();sourced(b);b.cancelNativeImport();b.nativeImportPreview.running=false;b.finishNativeImportPreview(0,frame());
  assert.equal(b.ready.length,0);assert.equal(b._nativeImportContext,null);
  const c=harness();c.startNativeImport('file');c.finishNativeImportSource(3,'','private-error');
  assert.equal(c.nativeImportCode,'');assert.equal(c._nativeImportContext,null);
});
test('stale revision instance and owner refuse source preview and confirmation',()=>{
  for(const change of [c=>c.nativeSnapshot.revision++,c=>c.nativeSnapshot.instanceId='new-instance',c=>c.nativeOwner=false]) {
    const a=harness();a.startNativeImport('clipboard');change(a);a.nativeImportSource.running=false;a.finishNativeImportSource(0,'private','');
    assert.equal(a.nativeImportCode,'stale');assert.equal(a.ready.length,0);
    const b=harness();sourced(b);change(b);b.nativeImportPreview.running=false;b.finishNativeImportPreview(0,frame());
    assert.equal(b.nativeImportCode,'stale');assert.equal(b.ready.length,0);
    const c=harness();previewed(c);change(c);assert.equal(c.confirmNativeImport('New'),false);assert.equal(c.nativePending,null);
  }
});
test('subscription classifications route clipboard/file to confirmation only, duplicates refuse',()=>{
  for(const kind of ['clipboard','file']) for(const duplicate of [false,true]) {
    const c=harness();sourced(c,' https://synthetic.invalid/token ',kind);c.nativeImportPreview.running=false;
    c.finishNativeImportPreview(0,frame({version:1,kind:'subscription',suggestedName:'Subscription',duplicate}));
    assert.equal(c.nativeImportCode,duplicate?'duplicateSubscription':'');
    assert.equal(c.subscriptionReady.length,duplicate?0:1);
    if(!duplicate)assert.deepEqual(c.subscriptionReady[0],['','Subscription','https://synthetic.invalid/token',kind]);
    assert.equal(c.ready.length,0);assert.equal(c._nativeImportContext,null);assert.equal(c.nativePending,null);
  }
});
test('helper and invalid input failures produce only fixed safe classifications',()=>{
  for(const [error,code] of [['File picker unavailable: install zenity, kdialog or yad','picker'],
    ['Clipboard unavailable: install wl-clipboard','clipboard'],['private://credential','source']]) {
    const c=harness();c.startNativeImport('clipboard');c.nativeImportSource.running=false;c.finishNativeImportSource(2,'private-output',error);
    assert.equal(c.nativeImportCode,code);assert.equal(c._nativeImportContext,null);assert.equal(c.ready.length,0);
  }
  for(const input of ['', 'я'.repeat(16385), '\ud800']) {
    const c=harness();sourced(c,input);assert.equal(c.nativeImportCode,'input');assert.equal(c.nativeImportPreview.running,false);
  }
});
test('Add rejects duplicates without replacement and confirmation preserves exact stdin retry',()=>{
  for(const name of ['Existing','','two\nlines','x'.repeat(81)]) {
    const c=harness();previewed(c);assert.equal(c.confirmNativeImport(name),false);assert.equal(c.nativePending,null);
  }
  const c=harness(), before=JSON.stringify(c.nativeSnapshot);previewed(c,'  private-input\n','file');
  assert.equal(c.confirmNativeImport('New <b>name</b>'),true);
  assert.equal(c.nativePending.action,'profile-import');assert.equal(c.nativePending.input,'New <b>name</b>\n  private-input\n');
  assert.equal(c.nativeActionProcess.command.length,6);assert(!JSON.stringify(c.nativeActionProcess.command).includes('private-input'));
  assert.equal(c._nativeImportContext,null);assert.equal(JSON.stringify(c.importPreview),'{}');
  const saved=JSON.stringify(c.nativePending);c.nativeActionProcess.running=false;c.nativeOutcomeUnknown=true;
  assert.equal(c.reconcileNativeAction(),true);assert.equal(JSON.stringify(c.nativePending),saved);assert.equal(c.nativeActionProcess.stdinEnabled,true);
  assert.equal(JSON.stringify(c.nativeSnapshot),before);
});
console.log(`${count} native import tests passed`);
