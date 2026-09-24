// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const os = require('node:os');
const path = require('node:path');
const {spawn} = require('node:child_process');
const source = fs.readFileSync(__dirname + '/../plugin/Service.qml', 'utf8');
const panel = fs.readFileSync(__dirname + '/../plugin/Panel.qml', 'utf8');
let count = 0;
function test(name, run) { try { run(); count++; } catch (error) { error.message = name + ': ' + error.message; throw error; } }
function context() {
  const c = vm.createContext({panelVisible:true,nativeAppAvailable:false,nativeAppChecking:false,
    nativeAppOpening:false,nativeAppLaunchFailed:false,nativeAppCheck:{},nativeAppLauncher:{}});
  for (const name of ['refreshNativeAppAvailability','finishNativeAppAvailability','openNativeApp']) {
    const start = source.indexOf('  function ' + name + '(');
    const end = source.indexOf('\n  }', start) + 4;
    assert(start >= 0); vm.runInContext(source.slice(start, end), c);
  }
  return c;
}
test('feature probe admits exact bounded token, not version guesses or raw errors', () => {
  for (const output of ['omavless.tui.v1','omavless.tui.v1\n']) {
    const c = context(); assert(c.refreshNativeAppAvailability());
    assert(c.nativeAppChecking); assert(c.nativeAppCheck.running);
    assert(c.finishNativeAppAvailability(0, output)); assert(!c.nativeAppChecking);
  }
  for (const output of ['',null,'omavless.tui.v2','private-secret','omavless.tui.v1\nextra',' '.repeat(65)+'omavless.tui.v1']) {
    const c = context(); assert(!c.finishNativeAppAvailability(0, output)); assert(!c.openNativeApp());
  }
  const c = context(); assert(!c.finishNativeAppAvailability(1,'omavless.tui.v1'));
});
test('probe avoids duplicate reads and clears previous availability', () => {
  const c = context(); c.nativeAppAvailable = true; assert(c.refreshNativeAppAvailability());
  assert(!c.nativeAppAvailable); assert(!c.refreshNativeAppAvailability());
  c.nativeAppChecking = false; c.panelVisible = false; assert(!c.refreshNativeAppAvailability());
});
test('open is explicit single-flight and independent of runtime health', () => {
  const c = context(); assert(!c.openNativeApp()); c.nativeAppAvailable = true;
  c.nativeAppLaunchFailed = true; assert(c.openNativeApp()); assert(c.nativeAppOpening);
  assert(c.nativeAppLauncher.running); assert(!c.nativeAppLaunchFailed); assert(!c.openNativeApp());
  for (const [field,value] of [['panelVisible',false],['nativeAppChecking',true],['nativeAppOpening',true]]) {
    const blocked = context(); blocked.nativeAppAvailable = true; blocked[field] = value;
    assert(!blocked.openNativeApp()); assert(!blocked.nativeAppLauncher.running);
  }
});
test('literal Omarchy launch or focus uses stable app id and never lifecycle IPC', () => {
  const start = source.indexOf('  property bool nativeAppAvailable');
  const end = source.indexOf('  property var nativeDesktopCapabilities', start);
  const block = source.slice(start, end);
  assert(block.includes('command: ["omavless", "tui", "--available"]'));
  assert(block.includes('command: ["bash", "-c", "exec omarchy launch or focus tui --app-id=org.omarchy.omavless omavless tui </dev/null >/dev/null 2>&1"]'));
  assert(block.includes('interval: 3000')); assert(block.includes('nativeAppLaunchCooldown.restart()'));
  assert(block.includes('running: root.nativeAppChecking'));
  assert(block.includes('interval: 20000'));
  assert(block.includes('root.nativeAppLaunchFailed = timedOut || code !== 0'));
  for (const forbidden of ['systemctl','sudo','pkexec','backendPath','nativeSnapshot','requestNativeAction','console.']) assert(!block.includes(forbidden));
  assert(panel.includes('targets.concat([nativeOpenAppButton, nativeAppRefresh])'));
});
test('Open app is fixed below profile actions, not a Settings or profile mutation', () => {
  assert(!panel.includes('nativeOpenAppRow'));
  const dock = panel.slice(panel.indexOf('id: nativeProfileActions\n'), panel.indexOf('id: nativeAppFooter\n'));
  const footer = panel.slice(panel.indexOf('id: nativeAppFooter\n'), panel.indexOf('      AdvancedDiagnostics {'));
  assert(dock.includes('anchors.bottom: nativeAppFooter.visible ? nativeAppFooter.top : parent.bottom'));
  assert(footer.includes('visible: !root.bootstrapRequired && vless.nativeOwner && root.page === "main"'));
  assert(footer.includes('anchors.bottom: parent.bottom'));
  assert(footer.includes('text: root.textFor("native.app.open")'));
  assert(footer.includes('enabled: vless.nativeAppAvailable && !vless.nativeAppChecking && !vless.nativeAppOpening'));
  assert(footer.includes('onClicked: vless.refreshNativeAppAvailability()'));
  assert(!footer.includes('nativeActionProfile') && !footer.includes('nativeCanAct'));
  const handler = footer.match(/onClicked: \{ ([^\n]+) \}/)[1];
  for (const accepted of [false, true]) {
    let opened=0, closed=0;
    vm.runInNewContext(handler, {vless:{openNativeApp:()=>{opened++;return accepted}},root:{close:()=>closed++}});
    assert.equal(opened,1); assert.equal(closed,Number(accepted));
  }
});
test('English and Russian copy explain lifetime and package compatibility', () => {
  const i18n = require('../plugin/I18n.js');
  for (const key of ['title','open','scope','unavailable','failed']) for (const locale of ['en','ru']) {
    const text = i18n.translate('native.app.' + key, locale);
    assert(text && text.length <= 512 && !text.includes('Missing translation'));
  }
});
test('normal native package includes TUI and CI verifies feature without runtime', () => {
  const manifest = fs.readFileSync(__dirname + '/../crates/omavless-runtime/Cargo.toml','utf8');
  assert(manifest.includes('default = ["tui"]'));
  const build = fs.readFileSync(__dirname + '/../packaging/release/build-native-ci.sh','utf8');
  assert(build.includes('tui --available) == omavless.tui.v1'));
});
// Model the real Omarchy adapter: its detached terminal outlives the launcher.
// Quickshell closes its capture pipes on launcher exit; a later terminal write
// must still work. This fixture launches no desktop app and reads no store.
async function detachedTerminalTest() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'omavless-launch-test-'));
  const marker = path.join(dir, 'terminal-alive');
  const launcher = source.slice(source.indexOf('    id: nativeAppLauncher'), source.indexOf('  property var nativeDesktopCapabilities'));
  const argv = JSON.parse(launcher.match(/command: (\[[^\n]+\])/)[1]);
  const child = `setTimeout(() => { const f=require('node:fs'); f.writeSync(1,'terminal output'); f.writeSync(2,'terminal warning'); f.writeFileSync(${JSON.stringify(marker)},'alive'); },150)`;
  fs.writeFileSync(path.join(dir,'omarchy'), '#!' + process.execPath + '\n'
    + `require('node:assert/strict').deepEqual(process.argv.slice(2),['launch','or','focus','tui','--app-id=org.omarchy.omavless','omavless','tui']);\n`
    + `require('node:child_process').spawn(process.execPath,['-e',${JSON.stringify(child)}],{detached:true,stdio:'inherit'}).unref();\n`, {mode:0o700});
  try {
    await new Promise((resolve,reject) => {
      const p = spawn(argv[0],argv.slice(1),{env:{...process.env,PATH:dir+path.delimiter+process.env.PATH},stdio:['ignore','pipe','pipe']});
      p.on('error',reject);
      p.on('exit',code => { p.stdout.destroy(); p.stderr.destroy(); code===0 ? resolve() : reject(new Error('launcher failed')); });
    });
    for (let i=0;i<60 && !fs.existsSync(marker);i++) await new Promise(r=>setTimeout(r,50));
    assert.equal(fs.readFileSync(marker,'utf8'),'alive','detached terminal survives closed QML pipes');
    count++;
  } finally { fs.rmSync(dir,{recursive:true,force:true}); }
}
detachedTerminalTest().then(()=>console.log(`${count} Open app launcher tests passed`)).catch(error=>{console.error(error);process.exitCode=1;});
