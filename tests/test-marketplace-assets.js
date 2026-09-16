// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const {spawnSync} = require('node:child_process');
const root = path.join(__dirname, '..');
(async () => {
  const {response} = await import('./marketplace-visual/backend.mjs');
  const parser = vm.createContext({});
  vm.runInContext(fs.readFileSync(path.join(root, 'plugin/NativeSnapshot.js'), 'utf8'), parser);
  const presentation = vm.createContext({});
  vm.runInContext(fs.readFileSync(path.join(root, 'plugin/NativePresentation.js'), 'utf8'), presentation);
  const snapshot = parser.parse(JSON.stringify(response('status')));
  const observation = parser.parseObservation(JSON.stringify(response('native-observation')));
  assert(snapshot && observation);
  assert.equal(parser.coherent(snapshot, observation), true);
  const projected = presentation.project(snapshot, observation, false);
  assert.equal(projected.state, 'disconnected');
  assert.equal(projected.connected, false);
  assert.equal(projected.activeId, '');
  assert.equal(projected.profiles.length, 5);
  assert.deepEqual(Array.from(snapshot.profiles, p => p.name), [
    'Netherlands · Amsterdam', 'Germany · Frankfurt', 'Finland · Helsinki',
    'Sweden · Stockholm', 'France · Paris']);
  assert(snapshot.profiles.every(p => p.id.startsWith('demo-') && !/demo/i.test(p.name)));
  assert(snapshot.subscriptions.every(s => s.name === 'My servers' && s.id.startsWith('demo-')));
  assert.equal(snapshot.startup.enabled, false);
  assert(parser.desktopCapabilities(JSON.stringify(response('native-desktop-capabilities'))));
  assert(parser.parseCoreSetupFacts(JSON.stringify(response('native-core-readiness'))));
  for (const command of ['native-connect', 'native-disconnect', 'native-quit', 'cleanup-runtime',
    'watch-plugin-removal', 'native-subscription-refresh', 'native-import-clipboard', 'native-profile-qr', 'anything']) {
    assert.equal(response(command), null);
    const result = spawnSync(process.execPath, [path.join(__dirname, 'marketplace-visual/backend.mjs'), command]);
    assert.equal(result.status, 64);
    assert.equal(result.stdout.length, 0);
    assert.equal(result.stderr.length, 0);
  }
  const manifest = JSON.parse(fs.readFileSync(path.join(root, 'manifest.json')));
  assert.equal(manifest.description.length, 364);
  assert(manifest.description.startsWith('OmaVLESS is a VPN app for Omarchy'));
  assert(!/autoconnect|WireGuard|Amnezia|kill switch/i.test(manifest.description));
  console.log('marketplace assets: fixture parsers, disconnected-only state, 9 refused commands and description PASS');
})().catch(error => { console.error(error); process.exitCode = 1; });
