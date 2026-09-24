// SPDX-License-Identifier: MIT
// Execute production presentation/receipt functions, not a parallel probe model.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const i18n = require('../plugin/I18n.js');
const panel = fs.readFileSync(__dirname + '/../plugin/Panel.qml', 'utf8');
const service = fs.readFileSync(__dirname + '/../plugin/Service.qml', 'utf8');
const snapshot = vm.createContext({});
vm.runInContext(fs.readFileSync(__dirname + '/../plugin/NativeSnapshot.js', 'utf8'), snapshot);
function load(context, source, name) {
  const start = source.indexOf('  function ' + name + '(');
  assert(start >= 0);
  vm.runInContext(source.slice(start, source.indexOf('\n  }', start) + 4), context);
}
let checked = 0;
for (const locale of ['en', 'ru']) {
  const rows = {
    success: {resolved: true, reachable: true, latencyMs: 42},
    failed: {resolved: true, reachable: false, latencyMs: -1},
    unresolved: {resolved: false, reachable: false, latencyMs: -1}
  };
  const context = vm.createContext({
    vless: {probeResult: id => rows[id] || null},
    textFor: (key, args) => i18n.translate(key, locale, args)
  });
  load(context, panel, 'nativeProbeLabel');
  assert.equal(context.nativeProbeLabel('untested'), '');
  assert.equal(context.nativeProbeLabel('success'), i18n.translate('native.probe.https_delay', locale, {ms: 42}));
  assert.equal(context.nativeProbeLabel('failed'), i18n.translate('native.probe.https_failed', locale));
  assert.equal(context.nativeProbeLabel('unresolved'), i18n.translate('native.probe.dns_failed', locale));
  assert(!context.nativeProbeLabel('failed').includes('-1'));
  assert.notEqual(context.nativeProbeLabel('failed'), context.nativeProbeLabel('unresolved'));
  for (const key of ['native.probe.test', 'native.probe.sort', 'native.probe.scope', 'native.test.scope', 'native.ping.observed', 'native.ping.unavailable']) {
    const text = i18n.translate(key, locale);
    assert(!text.includes('Missing translation'));
    assert(text.length < 200);
    assert(!/[<>]/.test(text));
  }
  checked++;
}
// A negative HTTPS observation must not mutate the healthy tunnel snapshot.
const fence = {instanceId: 'instance', revision: 7, generation: 1};
const context = vm.createContext({NativeSnapshot: snapshot, panelVisible: true, nativeCanAct: true,
  nativeSnapshot: {instanceId: 'instance', revision: 7, desired: {connected: true}, lastKnownActual: 'connected'},
  nativeTestGeneration: 1, nativeTestResult: null, nativeTestStatus: ''});
load(context, service, 'finishNativeConnectionTest');
const before = JSON.stringify(context.nativeSnapshot);
const reply = {api: 'omavless.control', version: 1, id: 'test', ok: true, revision: 7,
  result: {schemaVersion: 1, scope: 'current_route_https', https: false, observedIp: null, elapsedMs: 3000, code: 'request_failed', instanceId: 'instance'}};
context.finishNativeConnectionTest(fence, 0, JSON.stringify(reply));
assert.equal(context.nativeTestStatus, 'failed');
assert.equal(JSON.stringify(context.nativeSnapshot), before);
// ICMP no-reply and HTTPS success coexist; neither manufactures tunnel proof.
const ping = {...reply, result: {schemaVersion: 1, scope: 'controller_attributed_tun_icmp', instanceId: 'instance',
  availability: 'observed', sample: {outcome: 'loss', latencyMs: null}, code: 'timeout'}};
assert.equal(snapshot.parsePing(JSON.stringify(ping), fence).value, -1);
Object.assign(reply.result, {https: true, code: 'ok', observedIp: '203.0.113.7', elapsedMs: 50});
context.finishNativeConnectionTest(fence, 0, JSON.stringify(reply));
assert.equal(context.nativeTestStatus, 'ok');
assert.equal(context.nativeTestResult.tunVerified, undefined);
assert.equal(JSON.stringify(context.nativeSnapshot), before);
assert(panel.includes('readonly property bool showMainConnectionTest: false'));
assert(panel.includes('readonly property bool showMainLatencySection: false'));
assert(panel.includes('root.textFor("native.probe.scope")'));
console.log(`probe semantics: ${checked} locale matrices + independent HTTPS/ICMP/lifecycle checks PASS`);
