// SPDX-License-Identifier: MIT
// Run only an operator-prepared synthetic stage, never the installed shell.
import {spawn, spawnSync} from 'node:child_process';
import {realpathSync, statSync} from 'node:fs';
import path from 'node:path';
const stage = process.argv[2];
if (!stage || !path.isAbsolute(stage) || realpathSync(stage) !== stage
    || !statSync(path.join(stage, 'shell.qml')).isFile()) throw Error('Explicit fixture stage required');
const mode = process.argv[3] || 'gui';
if (!['gui', 'baseline', 'passive', 'slow-passive', 'late-passive'].includes(mode)) throw Error('Unknown fixture mode');
const env = {...process.env, QT_QPA_PLATFORM:'offscreen', QT_QPA_PLATFORMTHEME:'generic',
  QT_QUICK_CONTROLS_STYLE:'Basic', XDG_RUNTIME_DIR:path.join(stage, 'runtime')};
// Deliberately exclude the real wl-paste even if fixture discovery fails.
env.PATH = path.join(stage, 'mock-tools');
delete env.WAYLAND_DISPLAY; delete env.DISPLAY; delete env.DBUS_SESSION_BUS_ADDRESS;
delete env.LD_PRELOAD; delete env.OMAVLESS_PROBE_BASELINE; delete env.OMAVLESS_PROBE_KIND;
delete env.OMAVLESS_PASSIVE_PROBE;
delete env.OMAVLESS_PROBE_LATE;
if (mode !== 'gui') {
  env.OMAVLESS_PASSIVE_PROBE = '1';
  env.LD_PRELOAD = path.join(stage, 'clipboard-probe.so');
  if (mode === 'baseline') env.OMAVLESS_PROBE_BASELINE = '1';
  if (mode === 'slow-passive') env.OMAVLESS_PROBE_KIND = 'slow';
  if (mode === 'late-passive') env.OMAVLESS_PROBE_LATE = '1';
}
const child = spawn('/usr/bin/qs', ['-p', stage], {env, stdio:['ignore','pipe','pipe']});
let output = '', receipt = false, stopped = false;
// Two explicit slow-provider cases each exercise the six-second helper bound.
// Leave bounded startup/Qt-test time on the ARM VM without changing that bound.
const timer = setTimeout(() => child.kill('SIGTERM'), 45000);
function observe(bytes) {
  output += bytes.toString();
  if (output.length > 65536) { child.kill('SIGTERM'); return; }
  if (mode === 'gui') {
    const match = output.match(/SAFE_INPUT_RESULT (\{[^\n]+\})/);
    if (match) { const result = JSON.parse(match[1]); receipt = result.failed === 0 && result.passed >= 8; }
  } else if (/SAFE_CLIPBOARD_PROBE requests=\d+ expected=(?:positive|zero) result=PASS/.test(output)) {
    receipt = true;
    if (!stopped) {
      stopped = true;
      const stopEnv = {...env}; delete stopEnv.LD_PRELOAD;
      const stop = spawnSync('/usr/bin/qs', ['ipc','-p',stage,'call','safeInputReview','finish'],
        {env:stopEnv, encoding:'utf8', timeout:10000, maxBuffer:8192});
      if (stop.status !== 0) receipt = false;
    }
  }
}
child.stdout.on('data', observe); child.stderr.on('data', observe);
child.on('error', () => { receipt = false; });
child.on('close', code => {
  clearTimeout(timer);
  // Receipts, process exit and absence of QML errors are independent gates.
  const passed = receipt && code === 0 && !/ERROR:|TypeError:|ReferenceError:|has crashed/.test(output);
  console.log(`${mode}: ${passed ? 'PASS' : 'FAIL'}`);
  if (!passed) console.log(output);
  process.exitCode = passed ? 0 : 1;
});
