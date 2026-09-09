// SPDX-License-Identifier: MIT
// Execute production JS using synthetic credentials only; no desktop or daemon.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const parser = vm.createContext({});
const source = fs.readFileSync(path.join(__dirname, '../plugin/Service.qml'), 'utf8');
vm.runInContext(fs.readFileSync(path.join(__dirname, '../plugin/NativeSnapshot.js'), 'utf8'), parser);
const png = 'data:image/png;base64,' + Buffer.from([137,80,78,71,13,10,26,10]).toString('base64');
const uri = 'vless://synthetic-only';
const frame = content => JSON.stringify({api:'omavless.control',version:1,id:'qr',ok:true,revision:4,result:{format:'uri',content}});
function context() {
  const removed=[];
  const component={createObject:(_root, properties)=>({...properties,running:false})};
  const c=vm.createContext({NativeSnapshot:parser,nativeOwner:true,nativeCanAct:true,nativePending:null,
    nativeSnapshotFailed:false,nativeSnapshot:{instanceId:'instance',revision:4,profiles:[{id:'record'}]},
    nativeQrExportProcess:null,nativeQrRenderProcess:null,nativeQrExportComponent:component,nativeQrRenderComponent:component,
    _nativeQrContext:null,_nativeQrExportContext:null,_nativeQrRenderContext:null,_nativeQrInput:'',
    qrLoading:false,qrDataUri:'',qrPath:'',qrName:'',qrErrorCode:'',_qrWanted:false,backendPath:'/synthetic/backend.sh',
    removeQrFile:p=>{if(p)removed.push(p);}});
  c.root=c;c.removed=removed;
  for(const name of ['nativeQrCurrent','showNativeQr','finishNativeQrExport','finishNativeQrRender','disposeNativeQrProcess','closeQr']) {
    const start=source.indexOf('  function '+name+'(');
    const end=source.indexOf('\n  }',start)+4;
    assert(start>=0 && end>start);vm.runInContext(source.slice(start,end),c);
  }
  return c;
}
let count=0;
function test(name,fn){try{fn();count++;}catch(e){e.message=name+': '+e.message;throw e;}}
test('export envelope exact shape, revision, Unicode and byte bounds',()=>{
  assert.equal(parser.parseQrExport(frame(uri),4),uri);
  for(const raw of [frame(uri),frame('\u0000'),frame('\ud800'),frame('é'.repeat(16385)),frame('x'.repeat(32769))])
    assert.equal(parser.parseQrExport(raw,raw===frame(uri)?5:4),null);
  for(const change of [p=>p.result.extra=true,p=>p.ok=false,p=>p.api='other',p=>p.version=2,p=>p.result.format='yaml',p=>p.revision=-1]) {
    const p=JSON.parse(frame(uri));change(p);assert.equal(parser.parseQrExport(JSON.stringify(p),4),null);
  }
});
test('PNG data URI only, bounded decoded bytes, never file or SVG',()=>{
  assert.equal(parser.qrDataUri(png),png);
  for(const value of ['/tmp/code.png','file:///tmp/x','data:image/svg+xml;base64,AAAA',png+'\n',png+'A','data:image/png;base64,AAAA',png+'A'.repeat(5592430)])
    assert.equal(parser.qrDataUri(value),'');
  const bytes=Buffer.alloc(4194304);Buffer.from([137,80,78,71,13,10,26,10]).copy(bytes);
  const max='data:image/png;base64,'+bytes.toString('base64');assert.equal(parser.qrDataUri(max),max);
  assert.equal(parser.qrDataUri(max.replace(/==$/,'AA')),'');
});
test('managed selection uses exact read then private stdin, not argv or mutation',()=>{
  const c=context();c.showNativeQr({uuid:'record',name:'Synthetic',managed:true});
  assert.equal(c.qrLoading,true);assert.equal(c.nativeSnapshot.revision,4);
  assert.deepEqual(Array.from(c.nativeQrExportProcess.command),['bash','/synthetic/backend.sh','native-profile-qr','record']);
  c.finishNativeQrExport(0,frame(uri));
  assert.equal(c._nativeQrInput,uri);
  assert.deepEqual(Array.from(c.nativeQrRenderProcess.command),['bash','/synthetic/backend.sh','native-qr-render']);
  c.finishNativeQrRender(0,png,'');assert.equal(c.qrDataUri,png);assert.equal(c._nativeQrInput,'');
  c.closeQr();assert.equal(c.qrDataUri,'');assert.equal(c.qrName,'');assert.equal(c._nativeQrContext,null);assert.equal(c.removed.length,0);
});
test('cancel and stale identity/revision discard late export and image',()=>{
  for(const change of [c=>c.closeQr(),c=>c.nativeSnapshot.revision++,c=>c.nativeSnapshot.instanceId='new',c=>c.nativeOwner=false,c=>c.nativePending={}]) {
    let c=context();c.showNativeQr({uuid:'record',name:'Synthetic'});change(c);c.finishNativeQrExport(0,frame(uri));
    assert.equal(c._nativeQrInput,'');assert.equal(c.nativeQrRenderProcess,null);
    c=context();c.showNativeQr({uuid:'record',name:'Synthetic'});c.finishNativeQrExport(0,frame(uri));change(c);c.finishNativeQrRender(0,png,'');
    assert.equal(c.qrDataUri,'');assert.equal(c.qrLoading,false);
  }
});
test('exact dependency error only, all private errors discarded',()=>{
  for(const [error,expected] of [['QR encoder unavailable: install qrencode','dependency_missing'],['private-token','render_failed']]) {
    const c=context();c.showNativeQr({uuid:'record',name:'Synthetic'});c.finishNativeQrExport(0,frame(uri));c.finishNativeQrRender(2,'private-token',error);
    assert.equal(c.qrErrorCode,expected);assert.equal(c.qrDataUri,'');assert.equal(c._nativeQrInput,'');
  }
});
test('unknown selection and duplicate request do not start another export',()=>{
  const c=context();c.showNativeQr({uuid:'other',name:'Synthetic'});assert.equal(c.nativeQrExportProcess,null);
  c.showNativeQr({uuid:'record',name:'Synthetic'});const first=c._nativeQrContext;c.closeQr();c.showNativeQr({uuid:'record',name:'Synthetic'});
  assert.equal(c._nativeQrContext,null);assert.notEqual(first,null);
});
test('production sinks clear private output and keep legacy cleanup separate',()=>{
  const window=fs.readFileSync(path.join(__dirname,'../plugin/QrWindow.qml'),'utf8');
  assert.match(window,/cache: false/);assert.match(window,/!root.open \? ""/);assert.match(window,/NativeSnapshot.qrDataUri\(dataUri\)/);
  assert.match(source,/write\(root\._nativeQrInput\)/);assert.match(source,/process.destroy\(\)/);
  assert.doesNotMatch(source,/removeQrFile\(qrDataUri\)/);
});
test('per-request collectors are destroyed after success, errors and cancellation',()=>{
  for(const kind of ['export','render']) for(const code of [0,2]) for(const cancel of [true,false]) {
    const c=context();c.showNativeQr({uuid:'record',name:'Synthetic'});
    if(kind==='render')c.finishNativeQrExport(0,frame(uri));
    if(cancel)c.closeQr();let destroyed=0;
    c.disposeNativeQrProcess(kind,{destroy:()=>destroyed++},code,kind==='render'?png:frame(uri),'private-token');
    assert.equal(destroyed,1);
    assert.equal(c[kind==='export'?'nativeQrExportProcess':'nativeQrRenderProcess'],null);
  }
});
console.log('native QR: '+count+' passed');
